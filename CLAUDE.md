# Yokan — notes for contributors and their agents

Yokan is a compiler for a statically typed subset of Python that
builds native desktop apps. This repository holds both halves:

- **The product** — `crates/yokan/`: the `yokan` Python module
  (`src/lib.rs`, a pyo3 runtime over the substrate), the
  translator/gate/CLI (`yokan_gate.py`), the typed stub
  (`yokan.pyi`), the demos (`demo/`), and the product ledger
  (`DESIGN.md`). `crates/yokan-stdlib/` is the standard library's
  single Rust implementation — both the interpreted and the
  compiled run call it.
- **The substrate** — the `crates/pixie-*` workspace: the pixie
  language (`.pix` is the checked intermediate source Yokan emits),
  its compiler, kernel, and the gpui-based engine. Substrate work
  happens in this tree; pixie is not maintained anywhere else.

User-facing docs: `README.md` / `README.ja.md` (landing),
`crates/yokan/TOUR*.md` (the language tour, one file per language
on purpose), `website/` (the zensical site — the tour is split into
six pages there), `docs/PIXIE.md` (the substrate, user-facing).

## Where design truth lives

`crates/yokan/DESIGN.md` is the ledger: every design decision, with
reasons, in the order it was made. **Read it before planning
anything, and append an entry when you land a design.** Comments in
the `pixie-*` crates cite sections like `§8.44` — that numbering
belongs to the pixie project's pre-fork ledger, which is not in
this tree; treat those citations as historical anchors, and do not
invent new ones (cite this repo's ledger in prose instead).

The tour closes with **What does not work yet**: what the dialect
refuses, each with its reason. When a limitation falls or appears,
update that section (both languages, repo and website copies) in
the same change.

## Setup

- macOS on Apple silicon (the only supported platform today),
  Python ≥ 3.14, [uv](https://docs.astral.sh/uv/), Rust via
  [rustup](https://rustup.rs) (the exact rustc is pinned by
  `rust-toolchain.toml` and fetched automatically), and Xcode's
  Metal toolchain (gpui compiles shaders at build time).
- `export CARGO_TARGET_DIR=~/.cache/pixie/target` before any cargo
  or gate work — every crate and generated app shares one target
  dir, which is what keeps builds fast.
- Regenerating `.rpi` bindings (rpi-gen work, `yokan add`) needs a
  nightly toolchain with the `rust-docs-json` component; ordinary
  builds and the gates do not.

## Commands — the product

`just` is the task runner: `just` alone lists the recipes, and the
common ones are `just gate <app> "<script>"`, `just sweep`,
`just dev-so`, `just test` / `just tier-gate`, `just site`,
`just publish <version>`. It exports the shared `CARGO_TARGET_DIR`
and encodes the invariants below, so prefer it over typing the raw
commands; the raw forms stay documented here because they are what
the recipes run.

Run these from `crates/yokan/` (they also work via
`uv run yokan_gate.py …`):

- `python3 yokan_gate.py gate demo/counter.py --script "click:+1"`
  — **the gate**: runs the app interpreted and compiled with the
  same interaction script and byte-compares the dumps. The gate is
  the product's core promise; a change is not done while it is red.
- `check` (the refusals alone — no compiler started), `translate`
  (emit the `.pix` only), `build` (native binary;
  `--release`, `--bundle`, `--onefile`, `--app`), `sync` (build the
  crate doors without gating), `add <app> <crate>` (declare a Rust
  crate dependency).
- `./tools/gate_all.sh` — the full demo sweep. Run it before
  merging any translator or runtime change. Do not edit demos while
  a sweep is running, and do not run other cargo jobs beside it.
- Rebuilding the dev extension after touching `src/lib.rs`:
  `cargo build --release -p yokan --features extension-module`,
  then copy `…/release/libyokan.dylib` to `crates/yokan/yokan.so`
  and `codesign -f -s -` it. A build **without**
  `--features extension-module` links a system libpython and
  crashes under uv's CPython at import.
- `uvx maturin build --release` builds the wheel.
- Type stub: `yokan.pyi` mirrors the runtime — after touching
  pyfunction signatures or decorators, update it and run
  `uv run --with pyright --with numpy pyright demo demo/opsboard`;
  it must stay at 0 errors. (mypy cannot follow type-changing class
  decorators; the docs recommend pyright.)
- Standard library: one manifest, `Translator.STDLIB` in
  `yokan_gate.py`. A row is the Python spelling, the `.pix` static,
  the signature in pixie's types and the Rust function; the `.rpi`
  door, the arity and type checks, the type a call reads as and the
  fallible twin are derived from it, so a new function is one line.
  A group's layer column says whether it is reached with
  `import math` or `from yokan import fs`; the row columns are `try`,
  `pure`, `cpython`, `const` (a value in Python) and `pick` (the twin
  follows the list's element type).
  `uv run tools/gen_expected.py` prints CPython's answers into
  `crates/yokan-stdlib/tests/expected/` (`--check` fails when a
  table is stale); `uv run tools/stdlib_coverage.py` reports how far
  each module reaches into Python's.
- Website: `website/build.sh` builds both languages, always in that
  order — building only one silently loses the other.

## Commands — the substrate

- `cargo test --workspace` — the compiler suites.
- `cargo test -p pixie-cli -- --ignored` — **the tier gate**: every
  pixie example through both execution tiers, failing on any dump
  difference. Run it before merging any change to the `pixie-*`
  crates.
- `cargo run -q -p pixie-cli -- build examples/counter/counter.pix
  --run` opens a window; the `PIXIE_SCRIPT` env var replays it
  headless (`click[@n]:`, `input[@n]:`, `submit`, `slide`, `select`,
  `key:<chord>`, `menu:<item>`, `file:<path>`, `drop:<path>`,
  `advance:<ms>`, `a11y`, `theme:dark|light`, `mem`, `dump`). Steps
  that produce output are collected into the run's returned
  transcript, so an embedder that captures the return value sees
  them; a comma inside a step's text is written `\,`.
- `pixie watch <file>` hot-reloads view-body edits in ~1 ms.
- First build on a machine: `pixie install-runtime` (prebuilds
  gpui, ~3 min, into the shared target).
- `cargo run -q -p pixie-rpi-gen -- <rustdoc.json> --bind
  mod=Class` derives a `.rpi` binding from rustdoc JSON.

## What to verify for which change

- Translator / runtime / stdlib / demo change → the touched demo's
  gate, then `gate_all.sh`, then pyright. A standard-library change
  also needs its module's ground-truth table (`cargo test -p
  yokan-stdlib`), regenerated first if the case set grew.
- Any `pixie-*` crate change → `cargo test --workspace` and the
  pixie tier gate, plus the yokan sweep if the change is reachable
  from the dialect.
- Anything visual → look at it: build, launch, screenshot, read the
  screenshot. A green gate proves the two runs agree, not that the
  window looks right. Kill stale binaries first
  (`pkill -f '/debug/<stem>'`) — most demos build to the same
  `main` stem in the shared target and overwrite each other, so
  build immediately before running.
- Gallery screenshots (`demo/screenshots/`, mirrored under
  `website/*/images/demos/`) show the state right after launch —
  refresh them when a demo's initial screen changes.

## Conventions and constraints

- Commit messages follow Conventional Commits
  (`type(scope): summary` — `feat`, `fix`, `docs`, `chore`,
  `refactor`, `test`, `perf`, `ci`), short English, present tense,
  the why over the what in the body. AI-assisted commits in this
  history carry `Co-Authored-By: Claude <noreply@anthropic.com>`.
- `git add` explicit file lists only — no `-A`, no directory adds
  (generated trees like `.gate/` live next to sources).
- Do **not** run `cargo fmt` on the forked front-end crates
  (`pixie-syntax`, `-hir`, `-types`, `-binding`, `-lsp`): they
  deliberately keep their ancestor's formatting so diffs against it
  stay readable.
- gpui is pinned to a specific Zed revision plus a vendored macOS
  platform crate (`vendor/gpui_macos`) carrying a panic-containment
  patch; the pin includes `features = ["font-kit"]`, without which
  no text renders. Upgrading gpui means bumping the rev everywhere,
  re-applying the vendored patch, and running the tier gate.
- Generated apps build with `debug = 0` and, under the gate,
  `--no-interp`: a gate's binary is never debugged or hot-reloaded,
  and the two together took a demo's build from 110 s to 8 s (the
  sweep from over an hour to 19 minutes). Changing either one costs
  a full dependency rebuild (~20 min) the first time.
- Rapid scripted edits to a demo `.py` can leave a stale
  `__pycache__` when size and mtime both match — remove it if a
  reload looks ignored.

## Design invariants, briefly

- **One implementation, both runs.** Standard-library functions
  live once, in Rust; the interpreted run calls the same code the
  compiled run links. This is what makes the gate meaningful.
  Blocking stdlib pyfunctions must `py.detach` around the wait.
- **Two layers, told apart by the name.** Python's own modules
  (`import math`, `random`, `statistics`) are a twin arrangement: the
  interpreted run is CPython's module, the compiled run a twin
  written against it. Yokan's own (`fs`, `sqlite`, `http`, `json`,
  `time`, `strings`, `clipboard`, `notify`) keep "one implementation,
  both runs", and never reuse a Python module's name.
- **Where the name is Python's, CPython is the specification.** The
  gate proves the two runs agree, never that they agree with Python
  — a function wrong the same way in both passes it. A function
  carrying a Python name is held to a table CPython printed
  (`crates/yokan-stdlib/tests/expected/`, read by `tests/expected.rs`,
  written by `tools/gen_expected.py`), and the manifest's `cpython`
  column says which rows make that claim. A row marked `~>` allows an
  ulp because the platform's libm decides it; anything CPython
  computes for itself is compared to the bit. Regenerate the tables
  when CPython moves, and read the diff.
- **A view calls what cannot change.** Purity is the manifest's
  `pure` column, not a blanket rule about the library: `math` and
  `statistics` are legal in a hole the way `.upper()` is, and
  anything reading a clock, a file or a generator is refused there by
  name. A view that fails collapses to one shared element in both
  runs, so it is a difference the gate compares.
- **Generated code is the compiler's responsibility** (the ledger
  calls this D10): the emitter produces only closed, borrow-clean
  stereotypes. A rustc error inside generated code is a compiler
  bug, never the user's; fix the emitter or refuse the input with a
  named reason.
- **Refusals teach.** When the dialect cannot take a shape, the
  error names what to write instead. Substrate messages may carry
  internal markers — `(M0)` unimplemented, `(M1)` a decided
  constraint, `(M2)` deferred; a constraint nobody can source is an
  `(M0)` in disguise. These labels never appear in user-facing
  prose.
- **Crate crossing is by twins**: a struct or enum crossing the
  Rust-crate boundary has a same-shaped class in the app;
  correspondence is checked, and the interpreted run's pyo3 door
  mirrors the compiled adapters exactly (down to error text).
- Data modeling guidance the memory model rewards: values
  (`@value`, lists, dicts) on store fields for data; `@model`
  classes where something must be observed or shared; `Weak[...]`
  for back pointers, since ownership cycles are not collected.
- A store method calls a sibling through the class name
  (`Calc.apply(o)`), not bare.

## Writing the docs

- User-facing vocabulary: "standard library", never "official";
  "interpreted and compiled" for the two runs, never internal tier
  or lane names; no roadmap/stage labels in README, tour, stub, or
  docstrings.
- Identity: Yokan introduces itself as a compiler for a statically
  typed subset of Python — the subset behaves exactly as Python and
  each build verifies it. Never "a Python-lookalike", never a
  library or toolkit framing, never a bare "Python compiler".
- Landing pages are written in the reader's order: what is this →
  what can I build → how does it feel → why trust it → how to
  start. Mechanism after value. Plain sentences; no coined terms;
  no underselling — honesty lives in the closing list, not in a
  timid pitch.
- Explain behavior without naming other languages as mechanisms
  (one approved comparison exists: the Flutter-and-Dart-shaped
  develop/ship split, stated once with what Yokan adds).
- Japanese documents: one sentence per line; avoid dashes and
  interpunct-as-list in prose (use colons, parentheses, 読点);
  keep terminology identical to the existing pages.
- English and Japanese are peers: every user-facing edit lands in
  both, and the website copies of the tour/demo pages are separate
  files that need the same edit.
