# Installation

Wakakusa lives in its repository today: there is no gem to install and
nothing to add to a Gemfile. You clone the checkout, build two things
once, and then `./bin/wakakusa` is the whole command line.

## What you need

- **macOS on Apple silicon, or Linux.** The engine draws through the
  platform's own GPU stack: Metal there, Vulkan here, on Wayland or
  X11.
- **Ruby 4** (CRuby) — the interpreter that runs your app while you are
  writing it.
- **[spinel](https://github.com/matz/spinel)** — the ahead-of-time Ruby
  compiler the shipped binary is built with. You do not install this
  one: `just wakakusa-spinel` fetches the pinned revision and builds it
  into `~/.cache/spinel/<sha>`, a few minutes, once.
- **Rust**, via [rustup](https://rustup.rs). The exact compiler is
  pinned by the repository and fetched on the first build.
- **Xcode's Metal toolchain** on macOS, because the engine compiles
  its shaders at build time. On Linux, a C compiler and the libraries
  the engine links instead: alsa, fontconfig, freetype, xkbcommon with
  its x11 half, xcb, and the Vulkan loader with a driver.

## Once per machine

```console
$ export CARGO_TARGET_DIR=$HOME/.cache/pixie/target
$ just wakakusa-spinel     # fetch and build the pinned Ruby compiler
$ just wakakusa-capi       # build the engine's C face
```

`CARGO_TARGET_DIR` is shared by every crate and every generated app,
which is what keeps later builds fast. Set it in your shell profile and
forget it.

Nothing else is fetched behind your back: the engine is a crate in the
same checkout, built by `cargo` the first time you gate or build.

## The commands

Run these from `wakakusa/`.

```console
$ ./bin/wakakusa run   demo/counter.rb                    # a window, under CRuby
$ ./bin/wakakusa check demo/counter.rb                    # what it cannot take
$ ./bin/wakakusa gate  demo/counter.rb --script "click:+1,dump"
$ ./bin/wakakusa translate demo/counter.rb                # the C, to read
$ ./bin/wakakusa build demo/counter.rb --release --app    # the binary, and a .app
```

`run` opens a window and watches the file, so a save takes effect in
place. `check` starts no compiler at all. `gate` is the one that
matters: both runs, one script, compared byte for byte.

Two flags belong to `build` — `--release` drops the symbol table, and
`--app` wraps the binary in a macOS application bundle — and one
belongs to `gate`: `--fresh <path>` deletes a path before each run, so
an app that keeps a file or a database starts both runs from the same
nothing.

## Your first file

```ruby
require "wakakusa"

class Counter
  def initialize
    @count = 0
  end

  def view
    column(spacing: 12.0, padding: 16.0) {
      text "count: #{@count}", size: 34.0
      button("+1") { @count += 1 }
    }
  end
end

run(Counter.new, title: "counter")
```

```console
$ ./bin/wakakusa run app.rb
```

Edit and save while that window is open: the file is read again, and
the object the window is holding answers with the new `view` and keeps
every value it had.

## What a build produces

Measured on macOS/arm64, with the shared build directory warm.

| What | Value |
|---|---|
| the compiler's C output | under 10 ms, about 120 KB |
| `cc` link of the compiled run | 0.29 s |
| the compiled binary | 16.2 MB |
| the shipped binary (`--release`) | 12.3 MB |
| the application bundle (`--app`) | 12.5 MB |
| launch to a window on screen | under 0.3 s |
| one gate round, engine already built | 2.1 s |

The binary carries the engine and the compiled Ruby, and links nothing
but the system's own libraries. The person receiving it needs neither
Ruby nor the compiler.

## Checking the whole thing works

```console
$ just wakakusa-sweep
```

Every demo through both runs, plus every complete example in both
language tours and on this site. It is what a change to the vocabulary,
the door or the engine has to pass, and it is the fastest way to find
out that your setup is right.

## Where next

- [First app](tour.md) — the language, in the order you meet it.
- [Demos](demos.md) — forty-three apps, each with its screenshot and
  its source.
