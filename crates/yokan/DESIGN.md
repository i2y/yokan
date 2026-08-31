# Yokan — the design ledger

Decisions with their reasons, append-only. Entries made before
0.1.0 lived in a private notebook; the decisions they produced are
summarized here, and new entries are appended below as designs
land.

## Identity

Yokan is a compiler for a statically typed subset of Python — not a
lookalike language. The claim "inside the subset your code behaves
exactly as Python" is enforced, not promised: the interpreted run
IS CPython, and the gate byte-compares it against the compiled run
on every build. Anything the dialect cannot honor is refused with a
named reason instead of silently diverging; the refusals are
collected in the tour's closing section, What does not work yet.

## The two runs

- **Develop**: the whole app runs on real CPython, with a
  state-preserving live reload (~1 ms). Types come from the type
  annotations, which are required.
- **Ship**: the same source is translated to `.pix` (a readable
  intermediate), compiled through the pixie substrate and rustc to
  a native binary. `--release` links no libpython.
- **`@py` escapes**: a function marked `@py` stays real Python; an
  app that uses one ships with CPython embedded (`--bundle` /
  `--onefile`), and `--app` wraps either shape as a macOS bundle.

## Invariants

- **One implementation, both runs.** Standard-library modules
  (fs, sqlite, http, json, math, time, strings, random) are single
  Rust implementations; the interpreted run calls the same code the
  compiled run links. Without this the gate would compare two
  implementations instead of two runs.
- **Generated code is the compiler's responsibility** (D10): the
  emitter produces only closed, borrow-clean stereotypes. A rustc
  error inside generated code is a compiler bug, never the user's.
- **Refusals teach.** An error names what to write instead.
- **Determinism at every boundary.** Wherever the two runs could
  disagree on incidental order or formatting, the design picks one
  answer for both: dicts iterate in key order, float text renders
  as CPython's `str()`, seeded random draws the same sequence.

## The dialect's shape

- State lives in three forms: typed `State` cells, `@store`
  singletons (the class name is the instance), and `@model` classes
  for observed, shared objects. Data itself prefers values —
  `@value` classes (frozen dataclasses), lists, dicts — on store
  fields; models reference models, with `Weak[...]` for back
  pointers because ownership cycles are not collected.
- Views are plain functions using `with` blocks; components are
  functions under `@component`, with `ui.slot()` for children and
  `ui.local()` for per-call-site state.
- Errors follow three lanes: `*_or` total functions for the common
  case, full `try`/`except` where failure is data, and containment
  (a trapped statement stops that statement, not the app) for
  programming errors — with `f"{e}"` rendering the same message in
  both runs.

## Crossing into Rust crates

A crate declared in `[tool.yokan.crates]` (PEP 723 block or
pyproject) is callable as `crates.<name>.<fn>()`. Both doors are
derived from the crate's rustdoc JSON: a binding for the compiled
run and a pyo3 shim for the interpreted run, mirroring the same
argument adapters so the gate stays meaningful. Structs and enums
cross by **twins** — same-shaped classes declared in the app,
checked for correspondence, nested structs included. Dicts cross
as std `HashMap` and come back sorted, so both runs agree on
order. Everything outside the crossing set is refused with the
reason, and the current set is listed in the tour.

## Documentation doctrine

- User-facing pages never use internal vocabulary: "standard
  library" (never "official"), "interpreted and compiled" (never
  tier names), no roadmap labels.
- Landing pages are written in the reader's order — what is this,
  what can I build, how does it feel, why trust it, how to start —
  with mechanism after value, no coined terms, and no underselling:
  honesty lives in the tour's closing list, not in a timid pitch.
- English and Japanese are peers; every user-facing edit lands in
  both, plus the website copies.

---

New entries are appended below, newest last: state the decision,
the reason, and what was rejected, in plain prose — no session
narration.
