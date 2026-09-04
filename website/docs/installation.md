# Installation

## Develop: uv is everything you need

A Yokan app is an ordinary Python file. Declare the dependency in
the PEP 723 header and uv does the rest:

```python
# /// script
# dependencies = ["yokan"]
# ///
from yokan import State, button, column, run, text
```

```console
$ uv run app.py
```

That covers the whole develop experience — the GPU window, the
state-preserving live reload, headless script runs. No Rust
involved.

In a project instead of a script: `uv add yokan`. For the `yokan`
command: `uv tool install yokan`. Plain pip works too.

## From the first file to a release

```console
$ uv tool install yokan                     # the yokan command
$ yokan init app.py                         # the first file
$ uv run app.py                             # develop: window, live reload
$ yokan check app.py                        # is it inside the dialect?
$ yokan gate app.py --script "click:+1"     # both runs, compared
$ yokan build app.py --release --onefile    # ship one file
```

The first four need nothing but uv. The last two compile, so they
need Rust — and they fetch the crates they compile against by
themselves.

`yokan translate app.py` prints the `.pix` the release build
compiles, at any point along the way.

!!! note "Platforms"
    Today: **macOS on Apple silicon**, Python **3.14+**. Linux
    support lands shortly.

## Ship: a Rust toolchain

- Install Rust via [rustup](https://rustup.rs); the compiler
  version is pinned by the repository and fetched automatically on
  the first build.
- On macOS you also need Xcode's Metal toolchain (the GPU engine
  builds shaders).
- The crates the build compiles against live in the repository, and
  the first `gate` or `build` fetches the checkout matching your
  version into `~/.cache/yokan/` (about 11 MB). There is nothing to
  clone by hand. Run `yokan` inside a checkout and it uses that one;
  `PIXIE_REPO` points it anywhere else.
- That first build compiles the engine and takes a few minutes;
  later builds are incremental.

## What a build produces

If the app uses no `@py` escapes, the executable contains no
CPython at all — zero links to Python, **13.4 MB** (10.4 MB
stripped), millisecond startup.

Apps that do use `@py` ship CPython embedded:

```console
$ yokan build app.py --release --bundle    # app folder + runtime
$ yokan build app.py --release --onefile   # one distributable file
```

`--onefile` is about **17 MB** stdlib-only, about **21 MB** with
numpy; the first launch unpacks to a cache and later launches start
in about 40 ms. Add `--app` (alone or with `--bundle`) for a macOS
`.app` bundle in `dist/` — Dock identity, double-click launch, an
icon from `<stem>.png` if present. Either way, the receiving
machine needs no Python and no pip.

Measured (macOS/arm64, release): 4.7 ms start, ~1 ms live reload.
