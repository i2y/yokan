# yokan — the package

This directory builds the `yokan` Python module and the `yokan`
command. The module is real CPython driving the pixie substrate's
kernel and GPU engine: a native window for ordinary Python code,
with a state-preserving live reload. The command is the compiler
(`translate` / `gate` / `build`).

For what Yokan is and how apps are written, start at the
[root README](https://github.com/i2y/yokan/blob/main/README.md) and the [language tour](https://github.com/i2y/yokan/blob/main/crates/yokan/TOUR.md)
([日本語](https://github.com/i2y/yokan/blob/main/crates/yokan/TOUR.ja.md)).

## Build and run (in-tree)

```console
$ cargo build -p yokan --release --features extension-module
$ rm -f crates/yokan/yokan.so     # macOS: same-inode overwrite trips the
$ cp <target>/release/libyokan.dylib crates/yokan/yokan.so
$ codesign --force -s - crates/yokan/yokan.so   # …signature cache (SIGKILL on import)
$ uv run crates/yokan/demo/app.py     # edit demo/app.py while it runs
```

Or build a wheel: `uvx maturin build --release` in this directory
(it picks up `yokan.pyi` and the license files); `uv pip install`
the result in any venv.

`--features extension-module` is for the importable `.so` (CPython
symbols stay undefined and resolve from the host process). Plain
builds and `cargo test` link libpython instead, so the workspace
builds this crate like any other — `tests/headless.rs` embeds
CPython and drives a scripted app with no window.

## Headless

`PIXIE_SCRIPT="click:+1,input:Momo" uv run app.py` runs any yokan
app without a window: the steps are replayed, the element tree is
dumped to stdout, and `run()` returns. The step vocabulary is
`click:` `input[@n]:` `submit[@n]` `slide[@n]:` `select[@n]:`
`advance:` `theme:` `a11y` `mem`. In tests,
`ui._headless(view, state, script)` returns the dumps as a string.
Timers are skipped headless.

## Gate and ship

```console
$ yokan gate demo/counter.py --script "click:+1,input:Momo" --release
GATE OK — 2 dump lines identical across tiers
  binary: …/release/pycounter (13.4 MB)      # 10.4 MB stripped
```

The `yokan` command comes with the wheel (`uv tool install` this
directory, or any built wheel); in-tree, `python3 yokan_gate.py …`
is the same program. `translate` works anywhere; compiling needs
the repository checkout — found automatically from the tree or the
cwd, else set `PIXIE_REPO`.

The gate replays one interaction script against the interpreted app
and the compiled app and byte-diffs the dumped screens. The claim
is always per app and always checked — never "Python compiles".
The translator emits `.pix` in the hand-written demos' idiom (read
`demo/.gate/pycounter.pix` — it looks authored). What is outside
the compiled range is listed, with reasons, at the end of the tour.

`build` makes the artifact without the comparison (translate →
compile → package; dependencies go into the bundle, not the build
machine):

```console
$ yokan build app.py --release --onefile
built: …/onefile/app (20.7 MB)
  not gate-checked — `gate` with a script proves the app behaves the same
```

## Native extensions (Rust)

`yokan add <app.py> <crate> [VERSION | --path DIR] [--features …]`
declares a crate (into the PEP 723 tool table, or pyproject.toml's
`[tool.yokan.crates]` for project-style apps — whichever the app
has) and builds its doors immediately; the app then calls it
through `yokan.crates` by the crate's own snake_case names — see
the tour's [Calling a Rust crate](https://github.com/i2y/yokan/blob/main/crates/yokan/TOUR.md#calling-a-rust-crate),
`demo/rustcrate.py` and `demo/proj/`. Under the hood both doors are
generated from one source of truth, rustdoc's JSON output for the
crate (version crates are documented through a scratch manifest,
the substrate's own recipe):

- the compiled run's binding (`.rpi`) is derived by rpi-gen
  (rustdoc JSON format 61) and cached in `<app>/.yokan/rpi/`;
- the interpreted run's door is an auto-generated pyo3 shim crate,
  built into `<app>/.yokan/ext/<name>.so` and loaded lazily by
  `yokan.crates` — its argument adapters mirror the compiled side's
  (`&str` for String, owned `Vec` for lists), so one crate
  implementation serves both runs and the gate stays meaningful.

`yokan gate` / `yokan build` keep both doors current;
`yokan sync app.py` builds them without gating. The crossing set:
Int/Float/Bool/String, Lists of those, Optionals (None included),
str-keyed dicts (returned dicts arrive ordered by key, both runs),
structs (nested and width-annotated fields included) and enums — declare
same-named twins in the app, plainly; the runtime sweeps the app's
module at startup and the loader rebuilds returns as YOUR types —
and Result-returning functions (received with try/except, the
message identical in both runs; compound returns included).
Payload enums, twin methods, and enum- or list-typed struct fields
stay compiled-only, each refused with its reason.

The standard library itself is the same shape by hand: one Rust
implementation (`crates/yokan-stdlib`), a `#[pyfunction]` door on
the module (release the GIL around blocking work — `py.detach`),
and an `@rust(..)` line in the binding the gate writes. To extend
the stdlib, add the function once and expose it through both doors.
The substrate side of the machinery is described in
[docs/PIXIE.md](https://github.com/i2y/yokan/blob/main/docs/PIXIE.md).

Apps that keep real Python inside (`@py` escapes) ship
self-contained too:

- `--bundle` — an app FOLDER carrying its own Python runtime; the
  app's declared dependencies (numpy, say) — a PEP 723 inline
  block, or the nearest `pyproject.toml` — are installed into the
  bundled runtime. Nothing on the target machine is used: strip the
  env, move the folder, it still runs. ~13 MB app + 52 MB runtime
  with numpy.
- `--app` — a macOS application bundle in `dist/<Title>.app`
  (Info.plist, Dock identity, optional icon from `<stem>.png`);
  combine with `--bundle` and the runtime rides inside
  `Contents/MacOS`, relative lookups untouched.
- `--onefile` — one distributable FILE (launcher + compressed
  runtime; first run unpacks into the user cache, later runs start
  in ~40 ms). **20.7 MB with numpy, 17.2 MB stdlib-only**, and the
  gate can replay its script through the single file itself.
