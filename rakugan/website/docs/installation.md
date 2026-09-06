# Installation

Rakugan lives in its repository today: there is no distribution to
install and nothing to add to a cpanfile of yours. You clone the
checkout, build one thing once, and then `./bin/rakugan` is the whole
command line.

## What you need

- **macOS on Apple silicon.** The engine draws through the platform's
  own GPU stack, and that is the only port today.
- **A perl of 5.40 or newer** to run your app, because that is where
  `class` is a feature you can rely on. `just rakugan-perl` fetches and
  builds the pinned 5.44.0 into `~/.cache/perl/5.44.0`; set
  `RAKUGAN_PERL=/path/to/perl` to use one you already have.
- **PPI**, for the command itself, under whichever perl is first on your
  path. The macOS system perl ships it; elsewhere, `cpanm PPI` or
  `cpanm --installdeps .` from `rakugan/`.
- **Rust**, via [rustup](https://rustup.rs). The exact compiler is
  pinned by the repository and fetched on the first build.
- **Xcode's Metal toolchain**, because the engine compiles its shaders
  at build time.

The command and the app are deliberately two different perls. The
translator is ordinary Perl with no `class` in it, so it runs under a
system perl of 5.34; the app is the one that needs 5.40.

## Once per machine

```console
$ export CARGO_TARGET_DIR=$HOME/.cache/pixie/target
$ just rakugan-perl        # fetch and build the pinned perl
```

`CARGO_TARGET_DIR` is shared by every crate and every generated app,
which is what keeps later builds fast. Set it in your shell profile and
forget it.

Nothing else is fetched behind your back. The engine is a crate in the
same checkout, built by `cargo` the first time you gate or build, and
the XS door is built for your app's perl by the command itself, into
`~/.cache/rakugan/door/<version>/`, and rebuilt when its sources change.

## The commands

Run these from `rakugan/`.

```console
$ ./bin/rakugan run   demo/counter.pl                    # a window, under perl
$ ./bin/rakugan check demo/counter.pl                    # what it cannot take
$ ./bin/rakugan gate  demo/counter.pl --script "click:+1,dump"
$ ./bin/rakugan translate demo/counter.pl                # the .pix, to read
$ ./bin/rakugan build demo/counter.pl --release --app    # the binary, and a .app
```

`run` opens a window and watches the file, so a save takes effect in
place. `check` starts no compiler at all. `gate` is the one that
matters: both runs, one script, compared byte for byte.

Two flags belong to `build` — `--release` drops the symbol table, and
`--app` wraps the binary in a macOS application bundle — and one belongs
to `gate`: `--fresh <path>` deletes a path before each run, so an app
that keeps a file or a database starts both runs from the same nothing.

## Your first file

```perl
use Rakugan;

class Counter {
    use Rakugan;
    field $count = 0;

    method view {
        return column(
            text("count: $count", size => 34),
            button("+1", on_click => sub { $count += 1 }),
            spacing => 12,
            padding => 16,
        );
    }
}

run(Counter->new, title => "counter");
```

```console
$ ./bin/rakugan run app.pl
```

Edit and save while that window is open: the file is read again, and
the object the window is holding answers with the new `view` and keeps
every value it had.

## What a build produces

Measured on macOS/arm64, with the shared build directory warm.

| What | Value |
|---|---|
| the translator's `.pix` output | 0.08 s, about 600 bytes for the counter |
| the compiled binary | 53.8 MB |
| the shipped binary (`--release`) | 11.9 MB |
| the application bundle (`--app`) | 11.9 MB |
| launch to a window on screen | 0.2 s |
| one gate round, engine already built | 2.8 s |

The binary carries the engine and the translated app, and links nothing
but the system's own libraries. The person receiving it needs neither
perl 5.40 nor the toolchain.

## Checking the whole thing works

```console
$ just rakugan-sweep
```

Every demo through both runs, the refusals against their fixtures, the
tables against what perl prints, and every complete example in both
language tours. It is what a change to the vocabulary, the door or the
engine has to pass, and it is the fastest way to find out that your
setup is right.

## Where next

- [The first app](tour.md) — the language, in the order you meet it.
- [Demos](demos.md) — forty-one apps, each with its screenshot and its
  source.
