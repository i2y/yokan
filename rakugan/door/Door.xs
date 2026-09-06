/* Door.xs — pixie's C face (crates/pixie-capi) for Perl 5, the way
 * Wakakusa's CRuby door opens it with Fiddle. Only what the counter
 * needs so far: the builder protocol, handler registration, the event
 * text and the run. The two Perl callbacks live in static SVs; two
 * static C functions with the ABI's shape call them.
 *
 * The C symbols stay pixie_*, because the face is pixie's; the Perl
 * side is Rakugan::Door. */
#define PERL_NO_GET_CONTEXT
#include "EXTERN.h"
#include "perl.h"
#include "XSUB.h"
#include <stdint.h>
#include <stddef.h>

typedef int64_t (*PixieBuildFn)(void);
typedef void (*PixieEventFn)(int64_t, int64_t);

extern int64_t pixie_el(int32_t kind);
extern void    pixie_str(int64_t el, int32_t key, const char *v);
extern void    pixie_num(int64_t el, int32_t key, double v);
extern void    pixie_int(int64_t el, int32_t key, int64_t v);
extern void    pixie_bool(int64_t el, int32_t key, int32_t v);
extern void    pixie_on(int64_t el, int32_t key, int64_t handler);
extern void    pixie_children(int64_t el, const int64_t *ids, size_t n);
extern int64_t pixie_end(int64_t el);
extern void    pixie_set_event_handler(PixieEventFn f);
extern int64_t pixie_event_text_length(void);
extern int64_t pixie_event_text_char(int64_t i);
extern int32_t pixie_run(const char *title, double width, double height,
                         double padding, PixieBuildFn build);

static SV *build_cb = NULL;
static SV *event_cb = NULL;

/* A `die` must not unwind through the engine's Rust frames (Perl's
 * die is a longjmp), so both callbacks run under G_EVAL and report. */
static void report(pTHX) {
    if (SvTRUE(ERRSV))
        PerlIO_printf(PerlIO_stderr(), "rakugan: %" SVf, SVfARG(ERRSV));
}

static int64_t call_build(void) {
    dTHX; dSP;
    IV root = 0; int count;
    ENTER; SAVETMPS;
    PUSHMARK(SP);
    PUTBACK;
    count = call_sv(build_cb, G_SCALAR | G_EVAL);
    SPAGAIN;
    if (count == 1) root = POPi;
    PUTBACK;
    report(aTHX);
    FREETMPS; LEAVE;
    return (int64_t)root;
}

static void call_event(int64_t id, int64_t kind) {
    dTHX; dSP;
    ENTER; SAVETMPS;
    PUSHMARK(SP);
    EXTEND(SP, 2);
    PUSHs(sv_2mortal(newSViv((IV)id)));
    PUSHs(sv_2mortal(newSViv((IV)kind)));
    PUTBACK;
    call_sv(event_cb, G_DISCARD | G_EVAL);
    report(aTHX);
    FREETMPS; LEAVE;
}

MODULE = Rakugan::Door  PACKAGE = Rakugan::Door

PROTOTYPES: DISABLE

IV
el(IV kind)
  CODE:
    RETVAL = (IV)pixie_el((int32_t)kind);
  OUTPUT:
    RETVAL

void
str(IV el, IV key, SV *s)
  PREINIT:
    SV *c;
  CODE:
    /* The engine wants UTF-8; a mortal copy keeps the upgrade off the
     * caller's scalar. */
    c = sv_mortalcopy(s);
    pixie_str((int64_t)el, (int32_t)key, SvPVutf8_nolen(c));

void
num(IV el, IV key, NV v)
  CODE:
    pixie_num((int64_t)el, (int32_t)key, (double)v);

void
int(IV el, IV key, IV v)
  CODE:
    pixie_int((int64_t)el, (int32_t)key, (int64_t)v);

void
bool(IV el, IV key, IV b)
  CODE:
    pixie_bool((int64_t)el, (int32_t)key, b ? 1 : 0);

void
on(IV el, IV key, IV handler)
  CODE:
    pixie_on((int64_t)el, (int32_t)key, (int64_t)handler);

void
children(IV el, SV *ids)
  PREINIT:
    AV *av; SSize_t n, i; int64_t *buf; SV **e;
  CODE:
    if (!SvROK(ids) || SvTYPE(SvRV(ids)) != SVt_PVAV)
        croak("Rakugan::Door::children: expected an array ref of handles");
    av = (AV *)SvRV(ids);
    n = av_len(av) + 1;
    if (n > 0) {
        /* The handles cross as a packed buffer the engine reads once. */
        Newx(buf, n, int64_t);
        for (i = 0; i < n; i++) {
            e = av_fetch(av, i, 0);
            buf[i] = e ? (int64_t)SvIV(*e) : 0;
        }
        pixie_children((int64_t)el, buf, (size_t)n);
        Safefree(buf);
    }

IV
end(IV el)
  CODE:
    RETVAL = (IV)pixie_end((int64_t)el);
  OUTPUT:
    RETVAL

SV *
event_text()
  PREINIT:
    int64_t n, i; U8 buf[UTF8_MAXBYTES + 1]; U8 *e;
  CODE:
    /* Code points, counted out one at a time, become one UTF-8 string
     * with the flag on. */
    n = pixie_event_text_length();
    RETVAL = newSVpvn("", 0);
    SvUTF8_on(RETVAL);
    for (i = 0; i < n; i++) {
        e = uvchr_to_utf8(buf, (UV)pixie_event_text_char(i));
        sv_catpvn_flags(RETVAL, (const char *)buf, e - buf, SV_CATUTF8);
    }
  OUTPUT:
    RETVAL

IV
run(SV *title, NV width, NV height, NV padding, SV *build, SV *on_event)
  PREINIT:
    SV *t;
  CODE:
    if (build_cb) SvREFCNT_dec(build_cb);
    build_cb = newSVsv(build);
    if (event_cb) SvREFCNT_dec(event_cb);
    event_cb = newSVsv(on_event);
    pixie_set_event_handler(call_event);
    t = sv_mortalcopy(title);
    RETVAL = (IV)pixie_run(SvPVutf8_nolen(t), (double)width, (double)height,
                           (double)padding, call_build);
  OUTPUT:
    RETVAL
