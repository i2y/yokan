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

!!! note "Platforms"
    Today: **macOS on Apple silicon**, Python **3.14+**. Linux
    support lands shortly.

## Ship: clone the repository, have Rust

Only the native build (`yokan build` / `yokan gate`) needs more:
the Rust crates it compiles against live in the repository, so
clone it and have a Rust toolchain.

```console
$ git clone https://github.com/i2y/yokan && cd yokan
$ yokan gate path/to/app.py --script "click:+1"
$ yokan build path/to/app.py --release --onefile
```

- Install Rust via [rustup](https://rustup.rs); the compiler
  version is pinned by the repository and fetched automatically on
  the first build.
- On macOS you also need Xcode's Metal toolchain (the GPU engine
  builds shaders).
- Run `yokan` from inside the checkout and it finds the repository
  by itself; from anywhere else, point `PIXIE_REPO` at the
  checkout.
- The first build compiles the engine and takes a few minutes;
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
