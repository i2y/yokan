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
whenever it is asked to (`yokan gate`). Anything the dialect cannot honor is refused with a
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
  functions under `@component`, with `slot()` for children and
  `local()` for per-call-site state.
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

## Bare imports are the documented spelling

Elements import bare — `from yokan import button, column, run, …` —
and the docs and demos write them that way. `import yokan as ui`
remains fully supported (`ui.button`, `ui.run`); the two spellings
compile identically, and renamed imports work
(`from yokan import button as btn`). The namespace-by-default
alternative was rejected: the samples read better without the
prefix, and the shadowing risk it guarded against is already
caught — a local name that hides an element is reported by the
type checker at the use site, and the translator refuses what it
cannot resolve.

## OS notifications ride the engine

`notify.send(title, body)` is a standard-library call with the
stdlib's usual shape — one implementation, both runs — but its
delivery is the engine's job, because only the engine holds a
platform handle. Handlers push onto a small capped queue in the
kernel; the window's render pass drains it into the platform's
notification API (Notification Center on macOS). The consequences
fall out of the design rather than being special-cased: a headless
run never drains, so verification scripts stay deterministic; a
bare development binary logs-and-drops at the platform layer
(the notification framework requires an app bundle); an `.app`
build (`--app`) delivers for real. Sending is best-effort by
declaration — a notification is advice, not state — so the call
returns nothing and cannot fail. Rejected: shelling out to
osascript (a second delivery mechanism with its own identity and
quoting rules, when the platform layer already does it properly).

## The script harness checks the middle of a run

A headless script drove the app and the run printed the screen at
its start and its end, so a divergence that appeared mid-script and
resolved before the end was invisible to the gate. Three changes
close that and the gaps around it:

- **`dump`** prints the screen at that point in the script, making
  an intermediate state a checked output — the mechanism `a11y` and
  `mem` already used.
- **Steps that produce output no longer print**: they are collected
  into the transcript `run` returns, ahead of the final dump and in
  the order they ran. The bytes a caller prints are unchanged, and
  an embedder that CAPTURES the return value rather than the process
  stdout — the CPython tier — now sees them. Without this, any
  output-producing step compared a transcript against a stdout with
  more lines in it, so `a11y` and `mem` were quietly one-sided in
  that gate too.
- **`click@n:`** counts matches of the same label in tree order, so
  a row of identical buttons is reachable; the other steps had taken
  an index since they were written, and click had not.
- **`\,`** escapes a comma inside a step's text. The separator used
  to eat it, and the tail failed as an unknown step, so no script
  could carry prose.

Deliberately not done here: scrolling, key events and window
resizing. Each needs engine-side state that headless runs do not
have, so each is a design, not an addition.

## The release is a recipe, not a memory

Three releases in one day surfaced a set of invariants that lived
only in a person's head: the importable module must be built with
`--features extension-module` (a plain build links a system
libpython and aborts at import), every build wants the shared
`CARGO_TARGET_DIR`, the version lives in one place, and — the one
that actually caught bugs — a wheel must be installed into a
throwaway venv and driven headless BEFORE it is uploaded, because
what breaks is the artifact, not the checkout. A `justfile` now
encodes them. `just publish <version>` bumps, builds, smokes, asks
once, then uploads, commits, tags and writes the release; the smoke
step is a hard gate, and the upload is the only irreversible move,
so it is the only one that asks.

## Module level is declarations

The translator used to accept a module-level statement — an
`every(1.0, tick)`, a `count.set(5)`, a `fs.write_text(...)`, a
store call — and emit an app without it. That was the one hole in
"refused by name, never silently changed": the gate could see the
difference only when a script happened to exercise it, and a timer
never fires headless, so the gate could not see that one at all.
Module level now takes declarations only — imports, `State`,
classes, defs, `style()`, type aliases, model instances, literal
constants and the `__main__` guard, which holds `run(...)` alone —
and every other statement is refused with what to write instead: a
def passed as `run(view, on_start=setup)`, which both runs call
once after mount. `every` gets its own message, because a compiled
timer is a design still to be made rather than a constraint.

The reason is timing, not the emitter's convenience. The three ways
an app runs execute module level differently: `python app.py` runs
it once at import and again on every live reload; the gate's
interpreted run imports the module without `__main__`, so the
guard's body does not run there at all; the compiled app never
runs it. A statement whose effect depends on which of those
happened cannot be verified, so it is not a dialect statement.
Binding a literal is not a statement in this sense — nothing runs —
which is why `DB = "x.db"` stays a declaration; `sys.path.insert`
is import plumbing with no effect on the app and passes for the
same reason.

Rejected: compiling module-level statements into the store's
initializer. It would give startup work a second mechanism with a
different moment (before mount rather than after) and would still
not match a live reload, which re-executes the module with the app
running. Rejected: keeping the drop and printing a warning — a
warning is not a refusal, and the timer case shows the gate cannot
back it up. Compiling `every` (a kernel clock that headless scripts
advance) is future work; the refusal is what makes it safe to add
later, because nothing now depends on the drop.

## `task` is refused by name until it compiles

`task(work, on_done, on_error)` has always been a development-run
feature: the translator has no lowering for it, and it refused the
call with the generic handler-statement message, so a reader learned
that something was wrong, not that worker threads were the thing.
The refusal now names `task` and says what the compiled app does
instead — a compiled handler runs to the end, so an `http` fetch
there holds the window until the reply — and the tour's closing
list carries the same sentence. Compiling `task` onto the
substrate's async functions, with the continuation on the UI thread
and the headless run waiting for completion as the interpreted run
already does, is future work; naming the gap is what keeps the
closing list honest until then.

## The agent guide follows the tour

`skill/SKILL.md` is the guide an agent reads before writing an app.
It had grown by appending a paragraph per feature, so it opened with
dict state (which does not compile), said both that a bare float
renders and that it needs `.Nf`, and both that an enum must be
matched to a string and that it renders as Python prints it. A
guide that contradicts itself teaches trial and error, which is the
thing the refusals exist to prevent. The guide is now a condensed
tour: the tour's order, the tour's vocabulary ("interpreted and
compiled", "State", "store", "model"), the same code shapes, and the
same closing list. When the two disagree, the tour is right, and the
guide says so at its top. The tour stays the single specification;
the guide is derived from it and is updated in the same change as
the tour, like the website copies.

