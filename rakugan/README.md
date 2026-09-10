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

The language, in the order you meet it: **[TOUR.md](TOUR.md)** /
**[TOUR.ja.md](TOUR.ja.md)**, and the same thing as a site under
`website/` (`just rakugan-site-serve`).

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

macOS on Apple silicon, or Linux. The engine draws through the
platform's own GPU stack: Metal on one, Vulkan on the other, and what a
Linux machine has to provide is listed on the site's installation page.

- **A perl of 5.40 or newer for the app**, because that is where `class`
  is a feature you can rely on. `just rakugan-perl` fetches and builds
  the pinned 5.44.0 into `~/.cache/perl/5.44.0` (a few minutes, once);
  `RAKUGAN_PERL=/path/to/perl` points the command at another.
- **PPI, for the command itself**, under whichever perl is first on your
  path. The macOS system perl ships it; elsewhere `cpanm PPI`, or
  `cpanm --installdeps .` from this directory — the `cpanfile` lists it
  and nothing else. The command and the app are deliberately two
  different perls: the translator has no `class` in it, so it runs under
  a system perl of 5.34.
- **The Rust toolchain** ([rustup](https://rustup.rs); the exact
  compiler is pinned by the repository and fetched on the first build)
  and **Xcode's Metal toolchain**, because the engine compiles its
  shaders at build time.
- **The shared target dir**, as for everything else in this repository:
  `export CARGO_TARGET_DIR=~/.cache/pixie/target`.

Nothing else is fetched behind your back. The engine is a crate in the
same checkout, built by `cargo` the first time you gate or build, and
the XS door is built for your app's perl by the command itself, into
`~/.cache/rakugan/door/<version>/`.

Then, from `rakugan/`: `./bin/rakugan run demo/counter.pl` opens the
window; `gate` with a script proves the two runs agree;
`tools/gate_all.sh` runs every demo, both tours and the site's checks.

## The five things you can do to an app

| command | what |
|---|---|
| `check` | what the app writes that Rakugan cannot take; silent when it can |
| `run` | the app under perl, in a window |
| `translate` | the `.pix` project the compiled run is built from, under `demo/.gate/` |
| `build` | the native binary (`--release`, `--app` for a macOS bundle) |
| `gate` | both runs headless, one script, byte-compared |

## The vocabulary

Every element an app can write — 33 of them — the keywords each one
takes, and the canvas's ten drawing commands come from one table,
`crates/pixie-capi/elements.toml`, the engine's own. `tools/gen.pl` writes two files from it: the subs an app
calls (`lib/Rakugan/Elements.pm`) and the table as Perl data
(`lib/Rakugan/Vocab.pm`), which the interpreted run writes elements
from and the translator reads to write the `.pix`. The sweep fails
when either is behind the table. Wakakusa's Ruby and the engine's own
constants are written from the same table by its own generator.

## What is in today

Forty-one demos gate green — the counter, the todo list, a calculator
on two layouts, a roster that sorts, charts, a dashboard driven by a
timer, work done off the window's thread, a pixel canvas and two of
Pyxel's games — and every one of them is a line-by-line port of the
same app in the two sibling languages, so the screens can be compared
side by side. Twenty-eight of the thirty-nine still pictures are
pixel-identical to Wakakusa's; of the eleven that are not, two are
alive when the picture is taken (a timer is running) and the rest are
differences this port meant (one of them is perl printing `0` where
Ruby prints `0.0`, which is perl being right about perl). The two games
carry a recording of play instead of a still.

What an app can write:

- **The class.** Scalar, list and hash fields, their types read from
  the initializers (`empty(Str)` for a container that starts empty);
  `ADJUST` for what has to be worked out before the first screen;
  methods with `:Sig(Int => Str)` for what they are called with and
  what they answer. A second class in the file, with fields and no
  `view`, is a value the app holds (`field $x :param :reader = 0`).
- **The screen.** Every element in the table, the keywords each takes
  and the ones every element takes; `if` / `unless` / `for` around the
  parts of a view, which an app collects into a list and hands to a
  container; a method that answers an element, which becomes a piece
  of the compiled screen with a name of its own; `list_view` and
  `table`, whose rows are built on demand.
- **The rest of Perl the dialect takes.** `my` names, `for` and
  `while` with `last` and `next`, `push` and the other things a list
  takes, a hash read with a fallback (`$prices{$k} // 0`), string
  interpolation including `$items[$i]` and `@{[ ... ]}`, arithmetic,
  comparisons, `?:`, `sprintf`, and `0 + $s` for reading a string as
  a number.
- **What happens on its own.** `every(1.0, sub { ... })` before `run`,
  `task(sub { ... }, on_done => ...)` on a thread of perl's own, and
  live reload: edit the file while the window is open and the class is
  redefined under the object it already has.

- **Perl's own functions.** `length` `substr` `index` `uc` `lc`
  `ucfirst` `reverse` `join` `split` `abs` `sqrt` `sprintf`, POSIX's
  `floor` `ceil` `fmod` `strftime`, List::Util's `sum` `max` `min`
  `uniq` `first`, and regular expressions — `=~`, `s///`, `split /…/`,
  `$1` and `$+{name}`. Where the name is Perl's, perl's output is the
  specification: every one of them is held to a table of 1,019 rows
  that perl itself printed, and the sweep fails when a table is not
  what perl says now.
- **The framework's own.** Files, a database with bound values, JSON
  read by path, the clipboard, the keyboard and the menu bar, dialogs,
  and sound. One implementation answers both runs: the compiled one
  links it through pixie's binding door and the interpreted one
  reaches the same Rust through the engine's C face, so what the gate
  compares is one library answering twice.
- **The canvas.** A grid of virtual pixels painted by the commands in a
  `paint` sub, colors by palette index, keys read as a device from the
  tick, and a PNG of any frame without a window (`PIXIE_FRAMES=<dir>`).
  `demo/jump.pl` and `demo/shooter.pl` are two of Pyxel's own examples
  (Takashi Kitao, MIT), ported and gated frame by frame.
- **Shipping.** `--release` drops the symbol table and `--app` wraps the
  binary in a macOS application bundle, ad-hoc signed, with `<stem>.png`
  or `<stem>.icns` beside the app as its icon. The bundle opens on a
  machine with neither perl 5.40 nor the toolchain.

What it refuses, it refuses by name. Thirty-one of those refusals have
a file in `test/refuse/` that triggers them and the message they must
print word for word, and the sweep checks them before it gates
anything: an unsorted walk of a hash, `say` and `print`, a string
`eval`, `local`, `wantarray`, `each`, `tie`, `"az"++`, a string where
a number is wanted, a view that calls a method, and the rest.

## Numbers

Measured here, on macOS/arm64, with the shared build directory warm.

| what | value |
|---|---|
| `check`, no compiler started | 0.07 s |
| a headless run, the screen as text | 0.07 s |
| the translator's `.pix` output | 0.08 s, 631 bytes for the counter |
| the compiled binary | 53.8 MB |
| the shipped binary (`--release`) | 11.9 MB |
| the application bundle (`--app`) | 11.9 MB |
| launch to a window on screen | 0.2 s |
| one gate round, engine already built | 2.8 s |
| the whole sweep: 41 demos, both tours, the site | 3 min 13 s |

## The tour and the site

`TOUR.md` and `TOUR.ja.md` are the language in the order you meet it,
and `tools/tour_check.pl` puts every complete example on them through
the same command a demo goes through, so the page cannot drift from the
vocabulary. `website/` is Rakugan's own zensical site, served under
`i2y.github.io/yokan/rakugan/` and written so it can move to its own
repository whole:
`just rakugan-site`, `just rakugan-site-serve` on :8003. Four of its
pages are generated — the tour from `TOUR.md`, the elements from the
table, the gallery from `demo/`, the refusals from the fixtures — and
`website/tools/site_check.pl`, which the sweep runs, fails when any of
them is behind its source.

## The name

落雁 (rakugan) is a pressed dry confection, in the line of castella,
yōkan and wakakusa; it also sounds like rakuda, the camel.
