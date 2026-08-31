# Verify and ship

The [tour](tour.md) concludes: type checkers, headless runs, the gate, shipping — and what does not work yet.

## Working with type checkers

Yokan bundles type stubs, so checking with pyright (Pylance in VS Code) works out of the box.
The stubs declare the runtime's actual shapes.
`@store` binds the singleton instance to the class name, so `Settings.set_dark(True)` counts as a bound method call, and handler references like `on_change=Settings.set_dark` pass as the expected `(bool) -> None`.
`@model` and `@value` declare that constructors are synthesized from the fields, and `Weak[Node]` appears to a checker as `Node | None` (which is exactly what a read means).

mypy has a known limitation: it does not apply type transformations made by class decorators.
Under mypy, `@store` method calls are therefore misreported as "self is not passed".
We recommend pyright for checking.

## Headless runs and the gate

Running without a window is where verification starts.

```console
$ PIXIE_SCRIPT="click:+1,input:Momo" uv run app.py
```

The step vocabulary is `click:<label>`, `input[@n]:<text>`, `submit[@n]`, `slide[@n]:<value>`, `select[@n]:<label>`, `advance:<ms>`, `theme:light|dark`, `a11y`, `mem`.
The screen tree is dumped to stdout before and after the steps, and from tests `yokan._headless(view, state, script)` returns the same string.

The **gate** replays the same script against the development build and the shipped build, and diffs the dumps.

```console
$ yokan gate app.py --script "click:+1,input:Momo" --release
GATE OK — 2 dump lines identical across tiers
```

An app that writes to files or a DB takes `--fresh path/to/file.db`, so the first run's writes never leak into the second run's initial read.
An app with PEP 723 dependencies runs the gate itself under `uv run --with <dep>`.

## Shipping

```console
$ yokan build app.py --release              # native binary (no verification)
$ yokan build app.py --release --app        # a macOS .app bundle
$ yokan build app.py --release --bundle     # with @py: folder bundling the runtime
$ yokan build app.py --release --bundle --app   # runtime and all, inside the .app
$ yokan build app.py --release --onefile    # single-file distribution
```

The native build has one prerequisite.
The Rust crates it compiles against live in the repository, so clone the repository and run `yokan` inside it (or point `PIXIE_REPO` at the checkout).
The steps are collected on the [Installation](installation.md) page.

The release binary of an app that uses no escapes is self-contained on its own (zero links to Python).
`--bundle` produces a folder carrying the Python runtime and the declared dependencies; `--onefile` produces one file (about 17 MB with the stdlib only, about 21 MB with numpy. The first launch unpacks to a cache; later launches start in about 40 ms).
The gate can replay scripts against the single file itself.

`--app` produces `dist/<Title>.app`: a Dock name, double-click
launch, drag-to-Applications. With `--bundle --app` the CPython
runtime rides inside the bundle, so nothing lives outside it. Put
`<stem>.png` (or `.icns`) next to the app file and it becomes the
icon. `--onefile` is the single-file shape, so it and `--app` are
mutually exclusive.

## A real app

`demo/opsboard/` is a dashboard in three modules.
A sum-typed health model branched with `match` in the view, two stores, two line charts, a labeled bar chart, slotted KPI cards, a virtualized alert feed that grows to fill its area, severity filters, report export through `fs`, theme flipping, and seeded mock telemetry — all in this one app.
The release build is 13.7 MB (10.6 MB stripped), with zero links to Python.

```console
$ uv run demo/opsboard/app.py
$ yokan build demo/opsboard/app.py --release
```

The small examples live as a set under `demo/` (counter, todo, ledger, moods, geometry, cards, styled, tryfetch, pyops and more).
Every one of them passes the gate.

## What does not work yet

What lies outside this range is refused by name — it does not silently change behavior.
What Yokan cannot do as of today, with the reason for each refusal:

- **Iterating a dict in insertion order.** A Python dict iterates in insertion order; the compiled dict is ordered by key. The provided form is `sorted()` iteration (key order, the same in both).
- **Bare `d[k]` reads.** The read form is `.get(key, default)`, where the caller decides what a missing key means.
- **Indexing from the back through a variable** (an `xs[i]` whose i turns negative at runtime). The literal `xs[-1]` works.
- **Reading a local assigned in only one branch.** Had that branch not run, Python would raise NameError. Assign in both if and else and it reads fine.
- **Negative exponents on `int ** int`.** The result's type would change at runtime; make either side a float and it can be written.
- **Compiling dict state (`run(state={...})`).** It runs during development, but the compiled truth is typed `State`.
- **Calling Protocol-bound helpers from views** (handlers can call them).
- **Calling value-class methods from views** (handlers can; views read fields).
- **Iterating a list of models directly in a view.** Today, assemble the display strings on the store side and hand them to `list_view`.
- **A `Weak` field on a store.** A store is an owner; the non-owning reference belongs on the model side (the back pointer).
- **Type names the native side already uses, such as `Vec`.** Refused by name; pick another (`V2`, say).
- **Compiling `every`.** Timers are a development-run feature and do not run headless either.
- A component's `local` is **identified by call site**. Reordering the calls reassigns the states.
- Placing the same element object **twice**. Constructors consume their children.
- **At the Rust-crate boundary, payload-carrying enums and methods on a twin do not cross yet.** Scalars, String, Lists, Optionals, str-keyed dicts, structs (nested and width-annotated fields included), enums, and Result (compound returns too) all do. The two that remain each wait on something specific: payload enums on rpi-gen itself, methods on impl-splicing onto an rpi-declared struct. Enum- or list-typed fields inside a struct stay out too; every call outside the set is refused with a named reason.
- All measurements are macOS/arm64. Other platforms are not measured yet.

This list is updated every time a design lands.
The design principles behind it are collected in [DESIGN.md](https://github.com/i2y/yokan/blob/main/crates/yokan/DESIGN.md).
