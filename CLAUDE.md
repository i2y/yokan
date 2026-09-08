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
- **The second language** — `wakakusa/`: Ruby on the same engine,
  reached through `crates/pixie-capi`, the substrate's C face. It has
  its own gate, its own vocabulary table and its own demos, and it
  shares the engine with Yokan and nothing else. `pixie-capi` is
  pixie's, not Wakakusa's: a third language would use the same face.
- **The third language** — `rakugan/`: Perl 5 (5.40 and newer, written
  with the `class` feature) on the same engine. Its compiled run is a
  translation to `.pix`, as Yokan's is; its interpreted run opens
  `crates/pixie-capi` through an XS door, as Wakakusa's does. It has
  its own gate, demos, tour and site, and shares the substrate and
  nothing else.

User-facing docs: `README.md` / `README.ja.md` (landing),
`crates/yokan/TOUR*.md` (the language tour, one file per language
on purpose), `website/` (the zensical site — the tour is split into
six pages there), `docs/PIXIE.md` (the substrate, user-facing).
Wakakusa has the same pair of its own (`wakakusa/TOUR*.md` and
`wakakusa/website/`), and so does Rakugan (`rakugan/TOUR*.md` and
`rakugan/website/`).
`skills/yokan/SKILL.md` is the agent guide, at the repository root
because that is where skill installers look for it; it is written
for an agent about to write an app, and follows the tour.

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

- macOS on Apple silicon, or Linux. Python ≥ 3.14,
  [uv](https://docs.astral.sh/uv/), Rust via
  [rustup](https://rustup.rs) (the exact rustc is pinned by
  `rust-toolchain.toml` and fetched automatically).
- macOS also needs Xcode's Metal toolchain (gpui compiles shaders
  at build time). Linux draws through Vulkan and opens its window
  on Wayland or X11, so it needs a C compiler and the development
  packages the engine links: alsa, fontconfig, freetype, xkbcommon
  and its x11 half, xcb, and the Vulkan loader with a driver. Add
  sqlite for `cargo test --workspace`, which builds `pixie-capi`
  against the system one (yokan-stdlib bundles its own). The wheel
  leaves those libraries to the machine rather than carrying them, so
  a container that only RUNS an app still needs the runtime halves:
  alsa-lib, fontconfig, libxcb and libxkbcommon with its x11 half. Packaging
  follows the platform: `--app` writes a `.app` or an AppDir,
  `--appimage` packs the AppDir, and `--bundle` / `--onefile` stay
  macOS's — see the ledger entries for why. What a Linux package
  leaves to the host is `HOST_LIBS` in `yokan_gate.py`, read both by
  the AppDir packer and by the release workflow that repairs the
  wheel, so the two agree; `--carry-libs` carries them anyway, for a
  target machine that may not have them.
- `export CARGO_TARGET_DIR=~/.cache/pixie/target` before any cargo
  or gate work — every crate and generated app shares one target
  dir, which is what keeps builds fast.
- Regenerating `.rpi` bindings (rpi-gen work, `yokan add`) needs a
  nightly toolchain with the `rust-docs-json` component; ordinary
  builds and the gates do not.

## Commands — the product

`just` is the task runner: `just` alone lists the recipes, and the
common ones are `just gate <app> "<script>"`, `just sweep`,
`just dev-so`, `just test` / `just tier-gate`, `just site` /
`just site-serve`, `just publish <version>`. It exports the shared `CARGO_TARGET_DIR`
and encodes the invariants below, so prefer it over typing the raw
commands; the raw forms stay documented here because they are what
the recipes run.

A release has two halves, because the module carries the engine and a
Mac cannot build Linux's: `just publish <version>` uploads macOS's
wheel and pushes the tag, the release workflow then builds the Linux
wheels and attaches them to that tag's release, and
`just publish-linux <version>` puts those on PyPI.

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
  table is stale). A table carries the CPython that printed it and
  that machine's libm, and `--check` compares bytes: under another
  Python every table reads stale on the version header alone, and on
  another platform the libm rows read stale too (measured: 44 in
  `math`, 2 in `random`). So regenerate on macOS and read the diff;
  what makes ONE table true on both platforms is the `~>` arrow,
  which the TEST honours by allowing an ulp.
  `uv run tools/stdlib_coverage.py` reports how far
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

## Commands — Wakakusa

Run these from `wakakusa/`. `just wakakusa-spinel` once per machine
fetches and builds the pinned Ruby compiler into `~/.cache/spinel/<sha>`.

- `./bin/wakakusa gate demo/counter.rb --script "click:+1,dump"` —
  **the gate**, the same promise Yokan's makes: the app run under CRuby
  and the app compiled to a native binary, driven by one interaction
  script, byte-compared.
- `check` (the refusals alone), `run` (a window, watching the file),
  `translate` (emit the C), `build` (`--release` strips, `--app` wraps
  it in a macOS bundle).
- `./tools/gate_all.sh` — the sweep: every demo, then every complete
  example in both tours. Run it before merging anything it touches.
- `crates/pixie-capi/elements.toml` is THE table: every element, its
  keywords, their types and defaults (it is the substrate's, read by
  both other languages). `tools/gen.rb` writes the Ruby methods, the key
  numbers both sides count with, and `crates/pixie-capi/src/vocab.rs`
  from it, and `--check` fails when they are stale. Adding an element
  is a row there and an arm in `materialize`.
- `PIXIE_FRAMES=<dir>` writes a PNG of a canvas after every script
  step, drawn by the same rasterizer the window uses — the way to look
  at drawn output with no window at all. It is the C face's, so it
  answers for every language on it; `WAKAKUSA_FRAMES` still works,
  because it was the first name.
- `website/` is Wakakusa's own zensical site (`just wakakusa-site`,
  `just wakakusa-site-serve` on :8002), deployed under Yokan's Pages at
  `i2y.github.io/yokan/wakakusa/` and written so it can move to its own
  repository by changing `site_url` and the two switcher links.
  Two of its pages are generated — `elements.md` from `elements.toml`,
  `demos.md` from `demo/` — by `just wakakusa-site-gen`; the sweep runs
  `website/tools/site_check.rb`, which fails when a page quotes a
  refusal the fixtures no longer print, and gates the site's tour
  examples along with the repository's.

## Commands — Rakugan

Run these from `rakugan/`. `just rakugan-perl` once per machine fetches
and builds the pinned perl (5.44.0) into `~/.cache/perl/<version>`; any
perl of 5.40 or newer runs an app when `RAKUGAN_PERL` points at it. The
command itself runs under whichever perl is first on the path and needs
PPI from CPAN (the macOS system perl ships it; `perl-PPI` packages it
on Fedora). Any PPI version reads: the translator takes either way PPI
has split an attribute list, which is not the same across its own
releases. The ground-truth tables are a claim about the C locale —
`strftime`'s `%A`/`%a`/`%B`/`%b` follow LC_TIME in both runs — so the
generator and the twin test pin it and the sweep needs no help.

- `./bin/rakugan gate demo/counter.pl --script "click:+1,dump"` — **the
  gate**: the app under perl through the XS door over pixie's C face,
  and the binary pixie built from the translated `.pix`, driven by one
  interaction script and byte-compared.
- `check` (perl's own verdict first, then the translator's refusals;
  nothing is printed when the app is inside the dialect), `run` (a
  window), `translate` (emit the `.pix` project under `demo/.gate/`),
  `build` (the native binary; `--release`, `--app` for a macOS bundle
  under `demo/dist/`, ad-hoc signed, icon from `<stem>.png`/`.icns`).
- `./tools/gate_all.sh` — the sweep: the generated tables, the
  refusals, the twins' ground truth, every demo, both tours, and the
  site's checks. Run it before merging anything under `rakugan/`.
- `tools/gen.pl` writes `lib/Rakugan/Vocab.pm` (the table as data,
  read by the runtime and the translator) and `lib/Rakugan/Elements.pm`
  (one sub per element) from `crates/pixie-capi/elements.toml`;
  `--check` fails when they are stale, and the sweep runs it. A new
  element is a row in the table, an arm in `materialize`, and nothing
  in Perl; a `.pix` spelling that breaks the camelCase rule is a
  `pix = "..."` on the row.
- `tools/refuse_test.sh` — every refusal that stands for a decision has
  a file in `test/refuse/` that triggers it and a `.txt` beside it
  holding the message word for word. The sweep runs this before it
  gates anything.
- `crates/rakugan-stdlib` holds the twins: what perl answers for itself
  and the compiled run would answer differently (`length`, `uc`,
  `sprintf`, `sum`, `strftime`, the regular expressions, `%` and `/`
  between whole numbers, a number or a bool in a string, `0 + $s`).
  `tools/gen_expected.pl` runs the case set through perl and writes
  `crates/rakugan-stdlib/tests/expected/`; `cargo test -p
  rakugan-stdlib` holds each twin to those rows, and the sweep runs
  both with `--check`.
- The framework's own standard library (files, sqlite, http, jsondoc,
  the clipboard, the clock, sound) is one manifest,
  `crates/yokan-stdlib/stdlib.toml`, which no language owns:
  `yokan_gate.py` reads it, and `rakugan/tools/gen_capi.pl` writes
  three files from it — the C face's arms
  (`crates/pixie-capi/src/stdlib.rs`), the Perl that calls them
  (`lib/Rakugan/Stdlib.pm`, `lib/Rakugan/Manifest.pm`) and the binding
  door the compiled run reads (`lib/Rakugan/yokan-stdlib.rpi`). A
  change there runs BOTH other sweeps.
- The door (`door/Door.xs`) is built for the app's perl into
  `~/.cache/rakugan/door/<version>/` by the command itself and rebuilt
  when its sources change, so nothing under `rakugan/` is generated in
  place except `demo/.gate/`.
- `TOUR.md` / `TOUR.ja.md` are the tour, peers; `tools/tour_check.pl`
  pulls every complete example out of them and puts it through the same
  command a demo goes through, and the sweep runs it.
- `website/` is Rakugan's own zensical site (`just rakugan-site`,
  `just rakugan-site-serve` on :8003), deployed under Yokan's Pages at
  `i2y.github.io/yokan/rakugan/` and written so it can move to its own
  repository by changing `site_url` and the two switcher links.
  Four of its pages are generated by `just rakugan-site-gen` — the six
  tour pages cut from `TOUR*.md`, `elements.md` from the table,
  `demos.md` from `demo/`, `refusals.md` from `test/refuse/` — and the
  sweep runs `website/tools/site_check.pl`, which fails when any of
  them is behind its source. The gallery under `demo/screenshots/` is
  mirrored into `website/docs*/images/demos/`.
- A field's type is read from its initializer; a container that starts
  empty says its type with `empty(Str)`. A method says what it is
  called with and what it answers in a core attribute
  (`method add :Sig(Int => Str) ($n)`). A second class in the file,
  with fields and no `view`, is a value the app holds
  (`field $x :param :reader = 0`). Types are spelled the way
  Types::Standard spells them (`Int`, `Str`, `ArrayRef[Int]`).

## What to verify for which change

- Translator / runtime / stdlib / demo change → the touched demo's
  gate, then `gate_all.sh`, then pyright. A standard-library change
  also needs its module's ground-truth table (`cargo test -p
  yokan-stdlib`), regenerated first if the case set grew.
- Anything under `wakakusa/`, or `crates/pixie-capi` → the touched
  demo's gate, then `wakakusa/tools/gate_all.sh`. A change to
  `elements.toml` or `tools/gen.rb` regenerates first; the sweep fails
  on a stale table.
- Anything under `rakugan/` → the touched demo's gate, then
  `rakugan/tools/gate_all.sh`. The translator is the checker there: a
  new shape gets a gate line in the sweep and a new refusal gets a
  file in `rakugan/test/refuse/`. A change to `crates/rakugan-stdlib`
  also needs `cargo test -p rakugan-stdlib`; a change to
  `crates/yokan-stdlib/stdlib.toml` regenerates first and runs all
  three sweeps, since Yokan reads the same file and the C face carries
  the library Wakakusa's dylib now holds too. A change to the tour, the
  table, a demo or a refusal moves a site page too: run
  `just rakugan-site-gen`, which the sweep only checks.
- Any `pixie-*` crate change → `cargo test --workspace` and the
  pixie tier gate, plus the yokan sweep if the change is reachable
  from the dialect, and the wakakusa sweep if it is reachable from
  the C face. A change to `pixie-kernel` is reachable from both.
- Anything visual → look at it: build, launch, screenshot, read the
  screenshot. A green gate proves the two runs agree, not that the
  window looks right. Kill stale binaries first
  (`pkill -f '/debug/<stem>'`) — most demos build to the same
  `main` stem in the shared target and overwrite each other, so
  build immediately before running.
- Gallery screenshots (`demo/screenshots/`, mirrored under
  `website/*/images/demos/`) show the state right after launch —
  refresh them when a demo's initial screen changes. The two games
  carry a GIF of play instead, recorded from the window with
  `screencapture -v -V<secs> -l<window id>` and cut to the canvas with
  ffmpeg; nothing can type into a window here, so the shooter's was
  recorded from a scratch copy whose player sweeps and fires on a
  timer.

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
  (`import math`, `random`, `statistics`, `json`, `datetime`, `time`,
  `re`, `string`, `textwrap`, `bisect`, `heapq`) are a twin arrangement: the interpreted run is CPython's module,
  the compiled run a twin written against it. Yokan's own (`fs`,
  `sqlite`, `http`, `jsondoc`, `clock`, `strings`, `clipboard`,
  `notify`) keep "one implementation, both runs", and never reuse a
  Python module's name — which is why the JSON path reads are
  `jsondoc` and the local-zone calls are `clock`. `datetime` values
  are carried as integers (a date is its ordinal, a datetime and a
  timedelta are microseconds), so comparison is integer comparison
  and `DT_ATTRS` / `DT_METHODS` in the translator map what the app
  writes onto statics over that integer. A regular expression is
  compiled by CPython at translate time (`re._parser` +
  `re._compiler`) and the array crosses as a `List<Int>`, so the
  engine on the other side runs CPython's own bytes — check
  `re._constants.MAGIC` against the `rustpython-sre_engine` crate's
  `SRE_MAGIC` when Python moves.
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
