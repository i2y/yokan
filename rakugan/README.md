# Rakugan

**Rakugan turns a Perl desktop app into one native binary, and
`rakugan gate` is how you check that what it ships behaves like what
you ran.**

An app is a Perl 5 class, written with the `class` feature of perl 5.40
and newer. `rakugan build` translates it into pixie's `.pix` (the
checked intermediate source Yokan emits from Python) and has pixie
compile that, with the drawing engine — **gpui**, the engine behind the
Zed editor — into one native binary with no interpreter in it. While
you are working, the same file runs under perl instead, reaching the
same engine through an XS door, so it is the real interpreter
answering. The gate drives both with one interaction script and
compares what they drew, byte for byte — so "it worked while I was
writing it" and "it works as shipped" are one claim, not two.

Rakugan is the third language on this engine, after Yokan (Python) and
Wakakusa (Ruby). It shares the substrate with them and nothing else.

## What an app looks like

The app is a class. Its state is its fields, `view` is a method that
answers one element, and a handler is an anonymous sub that closes over
the fields. `use Rakugan;` is written twice: at the top it turns on what
the dialect assumes (what `use v5.40` would, plus utf8 and the class
feature) and brings `run`; inside the class it brings the elements, the
type names and `empty`, because a Perl import is per package.

```perl
use Rakugan;

class Counter {
    use Rakugan;
    field $count = 0;
    field $name  = "";

    method view {
        column(
            text("count: $count", size => 34),
            row(
                button("+1",    on_click => sub { $count += 1 }),
                button("+10",   on_click => sub { $count += 10 }),
                button("reset", on_click => sub { $count = 0 }),
                spacing => 8,
            ),
            text_field($name, placeholder => "your name",
                       on_change => sub ($s) { $name = $s }),
            text("hello, $name"),
            spacing => 12,
            padding => 16,
        );
    }
}

run(Counter->new, title => "counter");
```

Nothing in it says a type, and the compiled run is typed all the same:
a field's type is read from its initializer (`0` is an Int, `""` a
Str), a handler's parameter type from the element it is written on
(`on_change` hands over a Str). A container that starts empty says what
it will hold, `field @items = empty(Str);`, and that is the whole
annotation syntax.

```console
$ ./bin/rakugan gate demo/counter.pl --script "click:+1,dump,input:Momo\, again"
GATE OK — 3 dump lines identical in both runs
  script:   click:+1,dump,input:Momo\, again
  emitted:  demo/.gate/counter/src/main.pix
  binary:   /Users/you/.cache/pixie/target/debug/main (53.8 MB)
```

What the dialect cannot take is refused before anything is built, with
the file, line and column, the line itself, and what to write instead:

```console
$ ./bin/rakugan check demo/counter.pl
demo/counter.pl:16:35: Rakugan cannot take this — `text` has no `weight =>`; it takes `size`
                text("count: $count", weight => 2),
                                      ^
```

## Setup

- A perl of 5.40 or newer for the app. `just rakugan-perl` fetches and
  builds the pinned 5.44.0 into `~/.cache/perl/5.44.0` (a few minutes,
  once); `RAKUGAN_PERL=/path/to/perl` points the command at another.
- PPI, for the command itself, under whichever perl is first on your
  path (the macOS system perl ships it; elsewhere `cpanm PPI`).
- The Rust toolchain and the shared target dir, as for everything else
  in this repository (`export CARGO_TARGET_DIR=~/.cache/pixie/target`).

Then, from `rakugan/`: `./bin/rakugan run demo/counter.pl` opens the
window; `gate` with a script proves the two runs agree;
`tools/gate_all.sh` runs every demo.

## The five things you can do to an app

| command | what |
|---|---|
| `check` | what the app writes that Rakugan cannot take; silent when it can |
| `run` | the app under perl, in a window |
| `translate` | the `.pix` project the compiled run is built from, under `demo/.gate/` |
| `build` | the native binary (`--release`) |
| `gate` | both runs headless, one script, byte-compared |

## The vocabulary

Every element an app can write — 33 of them — and the keywords each
one takes come from one table, `crates/pixie-capi/elements.toml`, the
engine's own. `tools/gen.pl` writes two files from it: the subs an app
calls (`lib/Rakugan/Elements.pm`) and the table as Perl data
(`lib/Rakugan/Vocab.pm`), which the interpreted run writes elements
from and the translator reads to write the `.pix`. The sweep fails
when either is behind the table. Wakakusa's Ruby and the engine's own
constants are written from the same table by its own generator.

## What is in today

Four demos gate green: the counter, `layout` (spacer and divider),
`badges` (text as pills, bags of keywords, a method called from a
handler) and `labels` (roles, accessible names, tooltips, a tween, a
progress bar, the window's size). What the translator takes: one
class; scalar fields with literal initializers and list fields
(`("a", "b")` or `empty(Str)`); methods without parameters; handlers
of every kind the elements have; `if` / `elsif` / `else` / `unless`
and the conditional expression on the right of `=`; arithmetic,
comparisons, `!`, `&&`, `||`; strings with holes for Int and Str
fields; hashes of keywords at the top of the file; and the keywords
every element takes, with `role` and `easing` checked against their
vocabularies. Methods with parameters, lists that build their rows on
demand, hash fields, `my` inside a method, loops, floats and bools in
text, live reload, timers and work off the window's thread follow, in
the order Wakakusa took.

## The name

落雁 (rakugan) is a pressed dry confection, in the line of castella,
yōkan and wakakusa; it also sounds like rakuda, the camel.
