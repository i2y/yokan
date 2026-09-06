/* Door.xs — pixie's C face (crates/pixie-capi) for Perl 5, the way
 * Wakakusa's CRuby door opens it with Fiddle: the builder protocol
 * (an element is opened, written by key, given its children, closed),
 * handler and row-builder registration, what an event carried, and
 * the run. The Perl callbacks live in static SVs; static C functions
 * with the ABI's shape call them.
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
typedef int64_t (*PixieRowFn)(int64_t, int64_t);
typedef void (*PixieTimerFn)(int64_t);
typedef void (*PixieTaskFn)(int64_t);
typedef int32_t (*PixieReloadFn)(void);

extern int64_t pixie_el(int32_t kind);
extern void    pixie_str(int64_t el, int32_t key, const char *v);
extern void    pixie_num(int64_t el, int32_t key, double v);
extern void    pixie_int(int64_t el, int32_t key, int64_t v);
extern void    pixie_bool(int64_t el, int32_t key, int32_t v);
extern void    pixie_push_str(int64_t el, int32_t key, const char *v);
extern void    pixie_push_num(int64_t el, int32_t key, double v);
extern void    pixie_list_break(int64_t el, int32_t key);
extern void    pixie_on(int64_t el, int32_t key, int64_t handler);
extern void    pixie_rows(int64_t el, int32_t key, int64_t handler);
extern void    pixie_children(int64_t el, const int64_t *ids, size_t n);
extern int64_t pixie_end(int64_t el);
extern void    pixie_set_event_handler(PixieEventFn f);
extern void    pixie_set_row_builder(PixieRowFn f);
extern void    pixie_set_timer_handler(PixieTimerFn f);
extern void    pixie_set_task_handler(PixieTaskFn f);
extern void    pixie_every(double seconds, int64_t handler);
extern int64_t pixie_task(void);
extern void    pixie_task_done(int64_t id);
extern void    pixie_watch(const char *path, PixieReloadFn f);
extern int64_t pixie_event_int(void);
extern double  pixie_event_num(void);
extern int64_t pixie_event_text_length(void);
extern int64_t pixie_event_text_char(int64_t i);
extern int32_t pixie_run(const char *title, double width, double height,
                         double padding, PixieBuildFn build);

static SV *build_cb = NULL;
static SV *event_cb = NULL;
static SV *row_cb = NULL;
static SV *timer_cb = NULL;
static SV *task_cb = NULL;
static SV *reload_cb = NULL;

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

/* One row of a list that builds its rows on demand: the handler's
 * number and the row's index go up, the row's handle comes back. */
static int64_t call_row(int64_t handler, int64_t index) {
    dTHX; dSP;
    IV h = 0; int count;
    ENTER; SAVETMPS;
    PUSHMARK(SP);
    EXTEND(SP, 2);
    PUSHs(sv_2mortal(newSViv((IV)handler)));
    PUSHs(sv_2mortal(newSViv((IV)index)));
    PUTBACK;
    count = call_sv(row_cb, G_SCALAR | G_EVAL);
    SPAGAIN;
    if (count == 1) h = POPi;
    PUTBACK;
    report(aTHX);
    FREETMPS; LEAVE;
    return (int64_t)h;
}

/* A tick that came due, and a piece of work that finished. Both reach
 * the app on the window's thread, like an event. */
static void call_with_id(SV *cb, int64_t id) {
    dTHX; dSP;
    if (!cb) return;
    ENTER; SAVETMPS;
    PUSHMARK(SP);
    EXTEND(SP, 1);
    PUSHs(sv_2mortal(newSViv((IV)id)));
    PUTBACK;
    call_sv(cb, G_DISCARD | G_EVAL);
    report(aTHX);
    FREETMPS; LEAVE;
}

static void call_timer(int64_t id) { call_with_id(timer_cb, id); }
static void call_task(int64_t id)  { call_with_id(task_cb, id); }

/* The app's file changed: read it again. Non-zero means it took. */
static int32_t call_reload(void) {
    dTHX; dSP;
    IV ok = 0; int count;
    if (!reload_cb) return 0;
    ENTER; SAVETMPS;
    PUSHMARK(SP);
    PUTBACK;
    count = call_sv(reload_cb, G_SCALAR | G_EVAL);
    SPAGAIN;
    if (count == 1) ok = POPi;
    PUTBACK;
    report(aTHX);
    FREETMPS; LEAVE;
    return (int32_t)ok;
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
push_str(IV el, IV key, SV *s)
  PREINIT:
    SV *c;
  CODE:
    c = sv_mortalcopy(s);
    pixie_push_str((int64_t)el, (int32_t)key, SvPVutf8_nolen(c));

void
push_num(IV el, IV key, NV v)
  CODE:
    pixie_push_num((int64_t)el, (int32_t)key, (double)v);

void
list_break(IV el, IV key)
  CODE:
    pixie_list_break((int64_t)el, (int32_t)key);

void
on(IV el, IV key, IV handler)
  CODE:
    pixie_on((int64_t)el, (int32_t)key, (int64_t)handler);

void
rows(IV el, IV key, IV handler)
  CODE:
    pixie_rows((int64_t)el, (int32_t)key, (int64_t)handler);

void
every(NV seconds, IV handler)
  CODE:
    pixie_every((double)seconds, (int64_t)handler);

IV
task()
  CODE:
    RETVAL = (IV)pixie_task();
  OUTPUT:
    RETVAL

void
task_done(IV id)
  CODE:
    pixie_task_done((int64_t)id);

void
watch(SV *path, SV *on_reload)
  PREINIT:
    SV *p;
  CODE:
    if (reload_cb) SvREFCNT_dec(reload_cb);
    reload_cb = newSVsv(on_reload);
    p = sv_mortalcopy(path);
    pixie_watch(SvPVutf8_nolen(p), call_reload);

IV
event_int()
  CODE:
    RETVAL = (IV)pixie_event_int();
  OUTPUT:
    RETVAL

NV
event_num()
  CODE:
    RETVAL = (NV)pixie_event_num();
  OUTPUT:
    RETVAL

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
run(SV *title, NV width, NV height, NV padding, SV *build, SV *on_event, SV *row_build, SV *on_tick, SV *on_task)
  PREINIT:
    SV *t;
  CODE:
    if (build_cb) SvREFCNT_dec(build_cb);
    build_cb = newSVsv(build);
    if (event_cb) SvREFCNT_dec(event_cb);
    event_cb = newSVsv(on_event);
    if (row_cb) SvREFCNT_dec(row_cb);
    row_cb = newSVsv(row_build);
    if (timer_cb) SvREFCNT_dec(timer_cb);
    timer_cb = newSVsv(on_tick);
    if (task_cb) SvREFCNT_dec(task_cb);
    task_cb = newSVsv(on_task);
    pixie_set_event_handler(call_event);
    pixie_set_row_builder(call_row);
    pixie_set_timer_handler(call_timer);
    pixie_set_task_handler(call_task);
    t = sv_mortalcopy(title);
    RETVAL = (IV)pixie_run(SvPVutf8_nolen(t), (double)width, (double)height,
                           (double)padding, call_build);
  OUTPUT:
    RETVAL
