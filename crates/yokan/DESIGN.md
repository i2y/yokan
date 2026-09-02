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

## A refusal names its file, line and column

A multi-module app is flattened into one program before
translation, and a refusal reported only a line number — so a
mistake in `widgets.py` came back as "line 9" of `app.py`, with no
column and no quote of the offending line. Every parsed module now
stamps its nodes with their file, and a refusal reads
`widgets.py:5:40: not in the dialect — text() does not take
`weight=``, followed by the source line and a caret under the
construct. The shape is the one editors and terminals already parse
(`file:line:col:`), so a build error is a click away from the code
it names; the excerpt is there because the message talks about a
construct, and the reader should not have to open the file to see
which one. A refusal that has no node to point at — a missing
`run(...)` under the guard — names the file alone. Lambda bodies
used to report "line ?" because the synthesized statement carried
no position; they now carry the lambda's own. Rejected: a Python
traceback (it points into the translator, not the app) and the
entry file's name for every refusal (wrong for imported modules,
which is where the report mattered most).

## Refusals speak the user's language

A refusal is the one piece of prose most users read, and the
translator's had been written from the inside: "cells" for what
the tour calls State, "tiers" and "native" for the two runs,
`List<String>` and `Map<String, Int>` for `list[str]` and
`dict[str, int]`, `ui.State` and `@ui.store` when the documented
spelling is bare, and a catch-all that printed an AST dump. Every
message now uses the tour's words, quotes the construct it refused
in the user's own source, and says what to write instead when
there is something to write: `count.set(5)` at module level points
at `run(view, on_start=setup)`, a str method at `strings.to_int`,
an in-place `append` at `items.set(items() + [x])`, a store field
written from outside at a method. The catch-alls for expressions,
statements and conditions inspect the shape they were handed —
a comprehension, a conditional expression, a walrus, `print`, a
tuple assignment, `while True`, a chained comparison, an Enum's
`.value` — and name it, so "not in the dialect" is never the whole
message.

The second rule is about time. "For now" and "yet" had been
attached to most refusals, including ones the ledger had decided
would never change, so a reader could not tell a constraint from a
gap. Now a refusal that records a decision states the reason and
stops — a bare `d[k]` raises in Python where a default would be
answered, truthiness is not a comparison, an index that turns
negative counts from the back — and only a refusal for something
planned or undecided says "yet". The word is a promise, and the
tour's closing list is where the promises are kept.

## The closing list is the whole boundary

The tour's closing list — what does not work yet, with a reason for
each — carried a dozen items while the translator refused a few
hundred shapes, so a reader who wrote inside the documented dialect
still met refusals the documentation had not mentioned: a module
constant read in a handler, a str method, a slice, a store method
returning a value, a style value taken from state, the row index of
a `list_view` used in a handler. An audit of the boundary (one
probe app per construct, run through `translate`) produced the
missing items, and the list now carries all of them, grouped by
what a reader was trying to do, each with the form to write
instead. The rule going forward: a refusal that is not a decision
appears in the list until the construct lands, and leaves the list
in the same change that lands it; the guide's list is the same
list. The design refusals stay at the top, worded as decisions.

## The table element says what it draws

`data_table` sat in the element catalog with nothing behind it: no
demo used it, no sentence said what it does, and the landing page's
"tables" pointed at it. It is not a bare container — the engine
draws the frame, shades the first `row` child as a header and
alternates the shading of the data rows below it — so the fix was
to write that contract into the tour, the guide and the stub, and
to add `demo/table.py`, which lines its columns up by giving the
cells of one column the same `grow` share and setting the numeric
column with `align="right"`. An element the catalog names but
nothing demonstrates is a claim without a witness; every element in
the catalog should have one.

The same change made the gallery's own claim exact. "Every one of
them passes the gate" was written when the sweep gated every demo,
and it stayed after four dict-state demos became development-only
by design. The sentence now names the exception and points at the
gallery, where each of the four already says so.

## The checker is the translator

Everything the compiled run refuses is decided by the translator, on
the ast alone, before any compiler is started — but the only way to
ask it was `translate` (which prints a `.pix` nobody wanted to read)
or a build. `yokan check app.py` asks that question directly: it
translates every module the app imports, prints the first refusal in
the `file:line:col` shape with the line and a caret, and says
nothing at all when the app is inside the dialect. Silence is the
answer a checker should give, and a checker that needs no compiler
is one an editor can run on save.

It reports the first refusal, not all of them. Enumerating one per
def would mean treating a refused def as opaque at its call sites
and continuing the pass, which is a translator change, not a CLI
one; the cheap version is honest about stopping where it stops.

A refusal also stopped saying "not in the dialect" twice. The
rendered head names the location and the category, and most messages
name the category themselves ("`print(...)` is not in the dialect
yet — …"), so the head now adds its prefix only to the messages that
do not.

## A module constant is a declaration, and the read is its value

`LIMIT = 10` at module level was already a declaration — the compiled
app reads the module rather than running it — but reading the name
inside a handler was refused, so apps wrote the literal in six
places. A constant is now resolved before the scan and written where
it is read: the name and the value are interchangeable, because
CPython evaluates the binding once at import and the compiled app has
no import step at all. Locals, parameters and loop variables shadow
it exactly as they do in Python.

The collector is deliberately wider than the scan's own test for a
constant: anything it takes that the scan does not count as a
declaration is refused at the binding, as a module-level statement,
so guessing wide cannot change what an app does.

## A method may answer; a view may not ask

A store or model method can now be annotated with a return type and
end with `return <expression>`, and handlers read what comes back.
Views cannot call it, and that is not a gap: building a view reads
the world and a method may write to it, so the substrate refuses the
call outright. The read-only form is `@property` — a single
`return <expression>` over the store's fields, which the translator
writes where it is read. A property is a name for a formula, so a
view can use it wherever a field goes, and nothing about the view's
purity changes.

`@staticmethod` completes the set: a plain function that happens to
live in the class, emitted as the same static a module-level helper
becomes, and callable from views for the same reason.

## One index, Python's meaning

`items()[0]` worked only through a local, `self.xs[i]` not at all,
`len(self.xs)` was refused where `len(Cart.xs)` was accepted, and a
variable index was refused everywhere because it might turn negative.
The four spellings are one story now: a list read, a store or model
field written either way, or a local, indexed by a literal or an
expression. A negative index counts from the back — the index is
bound to a local and folded when it is negative — and an index past
the end stops that statement in both runs, which is the containment
the tour already promised for the literal case.

The loop variable of `for i in range(n)` needed a fix underneath:
the substrate bound it to the error type so the body would soft-pass,
and the list-index rule then rejected it by name. A range now binds
Int and a list binds its element type, which is what the syntax says.

## The call site is Python's

Keyword arguments, default arguments, model constructor arguments
(`Acc(v=3)`) and the `Optional[T]` spelling were each refused for
their own reason, and each refusal was arbitrary: the information
needed to accept them was already in the signature. Arguments are
now put back into the signature's order at the call site, defaults
are filled from the def (literals only, so the value written is the
value Python would have bound), a constructor with arguments builds
the model and writes the fields the way a synthesized `__init__`
does, and `Optional[T]` is rewritten to `T | None` before anything
reads it.

## Conditions, chains and the conditional expression

A bool is a condition: `if on:` over a bool state, field, local or
parameter asks nothing about truthiness, so it needed no comparison —
and `while True:` is the same rule. `0 < n < 10` chains, with the
middle read once (a plain read is written twice, anything else is
bound to a local first), and `:=` binds outside a None test.
`a if c else b` lowers in a handler to a local and an if/else, each
branch keeping its own preparatory lines so an expression is still
evaluated only on the path that uses it; a view has nowhere to put
those lines, so there it stays refused with the shape to write
instead.

The mechanism under all of this is a statement prelude: an
expression may ask for lines before the statement it appears in.
Binding a model's receiver, wrapping a negative index and lowering a
conditional expression all use it.

## A helper is an ordinary function

Helpers took the four scalars, no defaults, and a single trailing
`return`. They now take and return what a method parameter does —
lists, value classes, enums — return early from a branch, call
themselves, and fill in default arguments at the call site. What is
still refused is refused for its own reason: an Optional return has
no lowering yet, and joining two lists is a list operation the
dialect does not have, not a fact about helpers.

## A literal reads best where a literal was written

`text(Cart.label)` asked for an f-string wrapper, and
`select(options=["a", "b"])` asked for a state cell to hold two
constants. Both now take what the reader would write: a str-typed
expression becomes the text (through the same interpolation the
hole would have produced, so a literal inside a concatenation never
has to survive as a quote inside a quote), and a list of string
literals is lowered for `options:` / `labels:` directly by the
engine.

## `match` over values, and an Enum that answers questions

pixie's `case` matches enum members and sum-type variants, so the
dialect's `match` took only those. A `match` over int, float, str or
bool is the if/elif chain it always was: the subject is read once
into a local, each arm compares against it, `|` alternatives become
`||`, and a guard that fails falls through to the next arm — which
is exactly Python's rule.

An Enum's `.name` and `.value` are answered where they are known: a
member written out (`Mood.SAD.value`) is a constant, and a value that
only arrives at runtime goes through a static synthesized for that
enum, beside the one that renders `Mood.SAD` in text. `auto()` counts
from 1 and an explicit value is taken as written, so the number the
compiled app reads is the number CPython read. `for m in Mood:` walks
the members in declaration order, which is the order Python walks
them.

## A style value is a value

`size=`, `color=`, `padding=` and the rest took a literal, so an app
that wanted a size to follow state had to duplicate the element under
an `if`. They now take what a view can read: a state, a field, or
arithmetic over them, re-read on every rebuild like anything else the
view shows. The interpreting side always did this — it is Python
calling a function — so this was a tier disagreement rather than a
rule, and the fix landed in both halves: the translator stopped
demanding a literal, and the engine's numeric property lowering
learned the arithmetic its own documentation already promised
(`fontSize: unit * qty`).

## The row index is part of the row

A `list_view` row builder takes an index, and the index was usable
only to look the row up (`items()[i]`). Everything a list actually
does with its position — numbering, marking the selected row, a
delete button for that row — was outside the dialect. The repeater
underneath binds the row's value; it now binds the row's index too
(`for row, i in xs` in the substrate), so the index reads like any
other local: in the text, in a condition, and in the row's handlers.
A handler that reads it becomes a store function that TAKES it, with
the repeater passing it at the call site — the element itself comes
back from the index (`items[i]`), so one parameter carries the row.

## A dict key is a str, not an identifier

Keys had to be string literals shaped like identifiers, which is a
rule about the emitter's quoting, not about dictionaries. A key is
now any str the app can name — a literal with spaces, a state read, a
loop variable — for writing, for `.get`, and for `in`. What stays
refused is `.values()` / `.items()`, and for a reason that will not
change: Python walks them in insertion order and the compiled dict is
ordered by key, so the honest form is `sorted(d())` with
`.get(k, default)`.

## Python's str, as a twin rather than a port

`.upper()`, `len(s)`, `s[i]`, `s[a:b]`, `in`, `int()`, `round()` —
the everyday half of Python's strings and conversions — were refused
because the compiled side had no implementation. It has one now: a
set of statics in the standard library, each written to answer what
CPython answers, including the failures (`int("x")` stops that
statement, as the raise does).

This is the arithmetic pattern, not the standard library's. `fs` and
`sqlite` are one implementation both runs call; a str method cannot
be, because the development run is Python calling Python's own
method. So the compiled side gets a twin written against CPython's
semantics and the gate holds the two together — `round` rounding
half to even is exactly the kind of detail that arrangement exists
to catch.

The same reasoning covers format specs. `f"{x:.2f}"` was the only
one the dialect took, because the engine implements that one; fill,
align, sign, width, `,`, precision and `d` / `f` / `e` / `%` / `s`
now go through a formatter written against CPython's mini-language,
in views and handlers alike.

## `task` is an async handler

A `task` was refused because the compiled app had no worker to give
it. The substrate had one all along — an `async fn` whose awaited
binding calls run on the engine's pool — so a handler containing a
task becomes async, and the standard-library calls inside the work
are awaited. That is what moves the fetch off the UI thread; pure
computation inside a task stays where it is written, and the docs
say so rather than implying a thread that is not there.

A task is the last statement of its handler. In Python the
statements after it run BEFORE the work finishes, and a lowering
that ran them after would be a silent reordering — so the dialect
asks for them above the task instead. `on_error=` waits for the
error union; the failing call is caught with `try` / `except` today.

## A timer is a declaration

`every(1.0, tick)` at module level now compiles, and the way it
compiles is the point: a timer is not a call an app makes, it is a
fact about the app, so it lives with the other declarations and both
runs start it when the app starts. Underneath, the kernel grew a
timer store on the animation clock — the same clock a headless
script steps — so a window fires ticks on frames and `advance:<ms>`
fires exactly the ticks that span would have. Timers became
gate-checkable in the same change that made them compile, which is
the only reason to have them at all.

A tick that is late by more than one period does not repeat. The
clock jumped (a slow frame, a script advancing a minute at once) and
running a minute of ticks is nobody's intent.

## A component takes what a component is given

Component parameters carried values, so a component could not take a
callback or a `State` cell — the two things a reusable fragment needs
to talk back to its caller. Both now work, and neither crosses as a
parameter: a component that takes one becomes a view per call site,
with the handler and the cell written into the body where the
parameter was named. Two call sites that pass the same thing share
one view. A component's own parameter is readable in its handlers
too, by passing it to the synthesized store function — the same
mechanism a `list_view` row uses for its index.

## The escape hatch takes the app's own shapes

`@py` signatures took scalars and lists. They take str-keyed dicts
(as std's `HashMap`, the marker the crate boundary already had), a
value class (the generated crate declares the struct, the escape
module gets a dataclass of the same shape, and the app's `@value`
twin serves the interpreted run), and `T | None` — with the
narrowing that goes with it: an optional local reads as its own
name inside `if v is not None:`, which is what Python means there.
