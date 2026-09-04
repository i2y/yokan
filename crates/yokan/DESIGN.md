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

`skills/yokan/SKILL.md` is the guide an agent reads before writing
an app.
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

## The list vocabulary, and where each piece comes from

`in`, slices, `sorted` / `reversed` / `min` / `max` / `sum`,
comprehensions, `enumerate` / `zip`, a stepped `range` and joining
two lists were all refused, and each was refused for a different
missing piece rather than for a rule. They arrive from three places,
which is worth writing down because the shape repeats:

- What CPython answers and the engine has no opinion about — sorting,
  the aggregates, membership, slicing, joining — is a static in the
  standard library, per element type, written against Python's
  semantics (an empty `min` raises there and traps here).
- What is a loop wearing another name — a comprehension, `enumerate`,
  `zip`, a stepped range — is lowered to the loop, with the
  repeater's own index carrying `enumerate`.
- What only needed a type — a local list — asks for the annotation
  that says its element type, exactly as a state does.

`out = out + [x]` is still the append the compiled app performs in
place, whether `out` is a state or a local.

## `print` writes where the screen does not

A compiled `print` would write to stdout, and stdout is where a
headless run's screen dump goes — the one channel the gate compares.
So `print` keeps a refusal that names `log("…")`, which writes the
same line to stderr from both runs: one implementation, no channel
to share with the dump. `assert` and `raise` end the statement the
way Python's exception does, contained by the runtime, and both are
refused inside a `try` where the two runs would disagree about what
the `except` catches.

## A store field is written like any other field

`Cart.total = 5` from outside the store was refused with "write it
through a method", which is a style rule the language was enforcing:
Python allows the write and so does the compiled side. It is allowed
now, and the guidance stays guidance — a method is where an update
with an invariant belongs. Models already worked this way, so the
two agree.

Names the compiled side reads as syntax (`flags`, `state`, `case`
and the rest) are refused with a reason instead of reaching the
emitter, where they became a parse error inside generated code —
the compiler's bug to prevent, by D10.

## One reader for types, one for defaults

Every declaration had its own narrow table: a state's annotation
knew `list[str]` but not `list[Point]`, a store field knew
`dict[str, int]` but not `dict[str, list[str]]`, a model field knew
scalars only. The substrate takes all of them — nested lists,
dicts keyed by str or int, optionals of any of it, value classes
inside containers — so the tables became one reader for annotations
and one for defaults, and the combinations follow from the two.

`tuple` and `set` stay out, and not for want of a table: a tuple has
no compiled shape yet, and a Python set iterates in an order the
compiled side would not reproduce — the same reason a dict iterates
by key here.

## Values ride beside the statement

sqlite had no parameter binding, and the ledger demo showed what that
costs: it spelled the user's text into the SQL with an f-string, so an
apostrophe in an item name was a syntax error and a semicolon would
have been a second statement. Every sqlite call now takes a trailing
list of values to bind — `sqlite.exec(db, "INSERT INTO t VALUES (?,
?)", [name, str(n)])` — and the same spelling with and without it, so
the reader sees one function. Values bind as text and the column's
affinity converts, which is what Python's own driver does with a str
parameter; one implementation serves both runs, so there is nothing
for the two to disagree about.

The read side grew the shape that binding made worth having: a row as
a `list[str]`, a result as a `list[list[str]]`. The line a list shows
is written in Python now instead of assembled in SQL with `||`.

## The rest of the standard library's desk

Four gaps closed beside it, each one a thing an app of this kind
reaches for on its first afternoon: http POST, request headers, a
deadline in milliseconds and a status code on its own; fs directory
listing, append, remove, make, and `app_dir(name)` for the directory
an app may keep its own files in; `json.dumps` for writing a value
back out; and `time.format_local_ms`, the machine's own zone beside
the UTC one that verification scripts want.

Two of them needed a rule rather than a function. `json.dumps` has
one name and one meaning, but the writer differs by the value's type,
so the translator picks the static from the annotation while the
interpreted door reads the type at run time — the two land on the
same Rust function, which is what keeps the printed string single. A
dict is written in key order for the same reason dict iteration is:
a Rust HashMap has no order and a Python dict has insertion order,
and key order is the one both can agree on.


## A chord is a declaration

Keyboard shortcuts had no place to live: the substrate had no key
event at all, and a headless script had no way to press one. Both
gaps closed with the shape timers already had — `fn save @key("cmd-s")`
on a store, bound the moment the store exists, and `key:<chord>` as a
script step that reaches the same handler a keystroke would. In the
dialect that is `shortcut("cmd+s", save)` at module level, beside
`every(1.0, tick)`, and `on_key(typed)` for a handler that sees every
key as the chord it was.

The chord is spelled the way the platform spells it (`cmd-s`,
`shift-tab`), with `+` accepted for the same thing and one
normalization on both sides, so what a script presses and what a
window presses cannot drift apart. While a text field has the caret,
plain keys keep going into it and only chords carrying cmd or ctrl
reach the app — the platform's own rule, and the reason an app can
bind `cmd-s` without stealing the letter s.


## One clipboard, both runs

Copy and paste needed a place that a script can see. The clipboard is
one value in the kernel: an app sets and reads it, a window exchanges
it with the platform once per frame, and a headless run does neither
— so the two tiers of a gate agree because neither reaches a
machine-wide buffer, while a real window still trades text with every
other application. `clipboard.set_text` and `get_text` are ordinary
standard-library calls over that value.


## The menu bar is declared too

An item in the application's menu is the same kind of declaration a
shortcut is: `fn save @menu("File", "Save")` on a store, or
`menu_item("File", "Save", save)` in the dialect. The window hands
the list to the platform when it opens, and a headless script picks
one by the name it shows (`menu:Save`), reaching the same handler —
so a menu is something the gate checks rather than something only a
mouse can reach.

gpui menu items carry an ACTION, and an action is a type, so a bar
whose items are only known at run time needs one action type that
carries which item it was. That is the whole of the engine side: one
`MenuCommand { index }`, dispatched on the root.


## A decorator is folded in, not ignored

A user decorator used to pass through the translator unread: the
compiled app ran the bare function while the development app ran the
wrapper, and only the gate would have caught it. Refusing it by name
was the small fix; compiling it is the right one. Decoration happens
at import, and the compiled app never executes the module, so the
wrapper is inlined around the body it decorates and both runs do the
same thing.

The shape that folds is a def of one argument that returns that
argument, or defines a wrapper calling it once and returns the
wrapper. A decorator taking arguments of its own, a wrapper calling
the function twice (the compiled side cannot declare the same local
twice) or using its value are each refused with the reason.

## A field that holds paragraphs

`multiline=True` turns a text field into one: it shapes wrapped
instead of on one line, `enter` writes a newline rather than
submitting, `home` / `end` walk the line the caret is on, the arrows
move by visual line, and `rows=` says how many lines are visible.
The flag is in the dump, so a script sees which kind of field it is
looking at.

## Tooltips, dialogs and dropped files

Three more riders on the same idea that carried the shortcuts. A
`tooltip:` is a universal rider like `role:` — the sixth table both
lowerers share — and it is dumped whether or not a pointer is there
to reveal it. A file dialog is a wait for a person, so it runs inside
a `task`: the call blocks on the worker while the window opens the
panel on its own thread, and a headless run answers from the queue a
script filled with `file:<path>`. A dropped file is a declaration
like a shortcut (`on_file_drop`), delivered by the platform's drag or
by a script's `drop:<path>`.

Async lowering grew one thing to make the dialog usable: an `await`
inside an `if` inside an `async fn`. The branch used to go through
the sync lowering, where an await cannot appear at all — and "what
happens next depends on the answer" is the first thing anyone writes
after opening a dialog.


## The gate got its afternoon back

A gate ran in 110 seconds and used 19% of the machine, so the wait
was not compute. Two causes, both structural.

The generated app crate shared one target directory with the dev
tree while resolving the same dependencies under a different
configuration, so every switch between "build the compiler" and
"build an app" rebuilt gpui. Giving generated apps their own dev
profile (`debug = 0` — a gate never debugs the binary it compares)
made the two sets of artifacts distinct, and they stopped evicting
each other.

The second is what the compiled tier links: a gate's binary is never
hot-reloaded, so `pixie build --no-interp` leaves the interpreter and
the reload wiring out of the crate graph. The binary went from 62 MB
to 51 MB.

Measured on the demo sweep: 110 s per app build became 8 s when
nothing changed and 25 s when everything had to be rebuilt, and the
44-demo sweep went from over an hour to 19 minutes. Nothing about
what the gate CHECKS changed — the same two runs, the same
byte comparison.

## A table is a `list_view` with a header

`table(columns, count, row)` calls `row(i)` for the visible rows
only, exactly as `list_view` does, and the builder answers a `row`
of one cell per column laid on tracks whose shares are `widths=`.
The widget holds no order of its own: `on_select` hands back the
clicked row's index and `on_sort` the clicked column's, and the app
re-sorts its own lists; `sort=` / `descending=` only say which arrow
the header shows. Headless, `select:<first cell>` picks a row (a
table counts with the choosers) and `click:<column>` sorts, so both
wirings are gated. Rejected: a table that sorts its rows itself (a
second copy of the data, and two runs could disagree on ties); a
`cell(i, j)` builder (the row is the unit of virtualization and of
the dialect's row index); retiring `data_table`, which stays as the
static, styled container for a handful of rows.

## Text carries typography and a box

`text` took a size, a color and an alignment, so a status pill was
a colored bullet and a long line had nowhere to stop. It now takes
`bold` / `italic` / `mono` / `underline`, `wrap="nowrap"` or
`"ellipsis"` with a `width` to clip against, `max_lines`, and the
containers' box decoration (`background`, `padding`,
`border_radius` and the border props) painted on the text's own div
— a padded, rounded text in a row hugs its content, and that is the
pill. Every prop joins the dump only when set, so existing dumps
are byte-identical, and the style bag learned the same keys.
Rejected: selectable text (engine state no dump can see), mixed
styles inside one string (a different element), arbitrary font
families (an unknown name fails silently; `mono` is a promise the
engine can keep). The shared bool lowering now names its property,
so a bad flag no longer reports Modal's `open:`.

## A link opens a URL and changes nothing

`link(label, url)` is accent-colored, underlined text that opens the
URL in the browser. It carries no listener: opening a URL is not
application state, so a headless `click:` on a link is accepted and
does nothing, the contract `notify.send` already has. It derives the
`link` role with the label as its name.

## Charts plot over a range that contains zero

The charts normalized by the positive maximum alone, so a loss
clamped to nothing and an all-negative series drew nothing. The
range is now `min(0, smallest) .. max(0, largest)` unless `min=` /
`max=` pin it, and the zero line is the baseline a bar hangs from
and a polyline crosses. `axis=True` adds tick labels and gridlines
as ordinary text beside the painted plot; `series=` takes one
`list[list[float]]` field for several lines or bar groups, `colors=`
one color per series (a four-hue palette otherwise), `color=` a
single series' color. Rejected: a nested list literal for `series`
(a literal cannot reflect state, and a nested literal has no
lowering in a view); a legend, hover readouts and the data-swap
tween, each waiting on a verb or the kernel.

## Spacer and divider, the purely-layout leaves

A `spacer` takes the space its row or column has left, `grow=`
sharing it between several (0 = one share, since a spacer that grew
by nothing would never do anything). A `divider` is a rule whose
orientation is read off its slot rather than authored — vertical
inside a row, horizontal elsewhere, the way a text field reads its
width — in the theme's border color unless told otherwise. Neither
paints from state, carries a handler, or reaches the accessibility
tree.

## Typed numeric inputs commit; they do not report keystrokes

`number_field` (float) and `int_field` (int) show a bound value, as
`text_field` and `slider` do, and differ in when an edit counts:
typing reports nothing, and `enter`, an arrow key or leaving the
field commits — the text is parsed with Python's own `float()` /
`int()` rules, clamped into `min` / `max` (both zero = no range),
snapped onto `step` with the slider's rounding, and the handler runs
only when the result differs from the bound value. Text that is not
a number commits nothing and the field shows the app's value again,
so a half-typed "1" on the way to "12" is never a value of 1. The
shown text is `str(value)`, the function an f-string goes through,
so `3.0` reads the same everywhere. Headless, `input:` commits in
one step and `submit` is accepted and inert. Rejected: reporting
every keystroke, rounding a bad parse to zero, a grouped-thousands
display (it would break the equality with `str()`).

## Segmented is the fourth chooser

`segmented(options, selected, on_change)` shares the contract of
`select`, `radio_group` and `tab_bar` — a list of options, the
current index, an index-carrying handler — painted as one joined
pill whose current segment is filled in the accent. It exists
because an app that wanted a pressed look wrote an `if` / `else`
per button with two style bags; the widget carries the state now.
It derives the `radioGroup` role and answers `select:`.

## Accessibility riders reach the dialect

pixie's universal `role:` / `label:` riders had no Python spelling.
Every element takes `role=` and `a11y_label=` beside `tooltip=`: a
literal role is checked against the kernel's vocabulary at translate
time, a bound one is resolved by the shared kernel in both runs
(an unknown name falls back to the derived role), and `a11y_label`
is absent from `checkbox` and `switch`, whose own label already is
their name. Landing it surfaced a nesting bug in the tooltip rider —
applied outermost, one layer too far when an element also animates,
is themed or spans a cell — fixed so the runtime nests the riders
the way both lowerers do.

## Progress takes a size, a caption and an indeterminate sweep

`progress` took only a value. `width` / `height` size the track,
`label` draws a caption above it and doubles as the accessible name,
and `indeterminate=True` ignores the value and sweeps a segment left
to right off the animation clock the spinner uses, parked under
reduced motion. Existing dumps are unchanged; new props join only
when set.

## The interpreted run eases the way the compiled run does

`animate=` without `easing=` tweened linearly in the interpreted run
and eased out in the compiled one. The dump carries the duration but
not the curve, so the gate could not see the disagreement; the
runtime now defaults to `out` like both lowerers.

## A cross-cutting property is a rider

`disabled` and the four sizing props joined the substrate as wrappers
— `Element::Disabled` and `Element::Sized` — rather than as fields on
thirty variants, the shape `tooltip:`, `role:`, `theme:`, `animate:`
and `colSpan:` already had. Both lowerers strip them in one fixed
order, element → Semantics → Tooltip → Disabled → Sized → Themed →
Anim → GridCell, pinned by a test that dumps a text wearing every
rider. A false `disabled:` produces no wrapper and an element with
native sides (button, image, svg, text, the charts, progress; the
list, scroll view and table for height) keeps its own props, so no
existing dump changed. A disabled control is dimmed under a mouse
shield in the window, inert but still counted for `@n` in a headless
script, and marked in both accessibility trees — a person cannot
press it and neither can a script, and the dump says so. The one
surprise was mechanical: a tree nine wrappers deep overran the render
walk's megabyte stack frame, now guarded with a heap-allocated
segment. Rejected: per-variant fields (the merge cost of wave 1, in
which every branch collided on the same signatures), and `occlude`
for the shield (a disabled scroller should still scroll to be read).

## One rider table per layer

On the Yokan side each rider had been hand-wired into some
constructors, some translator arms and some stub signatures, which is
why `theme=` reached two containers, the animation riders three
elements and the grid spans one. There is now one table in each of
the three places and one mechanism reading it: a `Riders` struct
applied in the lowerers' nesting order, a macro that writes the same
rider tail onto every element function so no signature can drift,
one translator function that strips the rider kwargs before any
element emitter runs, and a `Riders` TypedDict every element unpacks
in the stub. Every element takes every rider, and the next one is a
line per table. Two exceptions are spelled out rather than special-
cased: an element with its own `width` / `height` keeps them as its
props, and checkbox, switch and progress refuse `a11y_label=` because
their own label already is their accessible name — a refusal that
also closed a disagreement the two runs had, since the compiled run
dropped that label on a progress bar while the interpreted run built
a node for it.

## A dict keeps the order its keys went in

The substrate's `Map` was a `BTreeMap`, so a compiled dict answered
its keys sorted while the same dict in Python answered them in
insertion order. Rather than teach the dialect to live with two
orders, the container changed: `Map` is an `indexmap` now, and
insertion order is what iteration, `keys()`, `values()` and every
dump report. It is still deterministic, which is all the sorted map
was ever chosen for. The one place with no order to inherit is a
Rust crate's `HashMap` crossing the boundary; that goes through
`Map::from_unordered`, which sorts, so the crossing stays fixed.

With the orders agreeing, walking a dict entered the dialect:
`for k in d()`, `d().keys()` and `d().values()` all compile, and
`sorted(d())` now emits an actual sort instead of leaning on the
container's own order. `.items()` stays out, with a reason that is
now about pairs rather than about ordering — a two-name binding has
no compiled shape yet, so the form to write is a walk over the keys
with `d().get(k, default)` inside.

## The standard library, in one table

A standard-library function used to be spelled three times: a line in
the generated `.rpi` door, an entry in the translator's call table
saying which static and how many arguments, and a third entry saying
what type a call reads as. Three copies drift, and they had — half
the functions were missing from the third table, so `math.sqrt(x)`
had no type where `strings.to_int(s, 0)` did. There is one table now.
A row names the Python spelling, the `.pix` static, the parameters
and return in pixie's types, and the Rust function behind both runs;
the door, the arity and container checks, the type a call reads as,
the fallible twin a `try` routes to, the module list two scanners had
hard-coded, and the coverage report are all derived from it. Adding a
function is one line. Two columns say what a row is rather than what
it does: `pure`, for the day a view may call one, and `cpython`, for
a row that answers what Python's function of that name answers.

## What CPython printed, in a table

The gate proves the two runs agree. It cannot prove they agree with
Python: a twin that is wrong the same way in both runs passes it.
Where a name is Python's, that gap is now closed from the other side
— `tools/gen_expected.py` runs the case set through CPython and
writes the answers to `crates/yokan-stdlib/tests/expected/`, doubles
in hex, exceptions with their class and message, and a Rust test
holds each twin to the table. A row marked `~>` allows one ulp,
because its answer comes from the platform's libm rather than from
IEEE-754, and that distinction belongs in the table rather than in a
reader's head. `tools/stdlib_coverage.py` reads the same manifest and
reports, module by module, how far the dialect reaches into Python's
— and refuses to count a name it merely borrows.

The first table put the eight `math` functions the library already
shipped in front of CPython, and two disagreed: `sqrt` of a negative
answered a quiet NaN where Python raises, and `pow` answered infinity
where Python overflows. Both now answer what Python answers. This is
the standard's own division: `sqrt` is exactly rounded and `sin` is
whatever the C library decided, so only one of them can be held to a
bit pattern.

## A view that fails draws the failure

Panics inside a handler and inside a task were contained; the view
build was left loud on the grounds that a panic there means a
compiler bug. It does not always: `xs[0]` on an empty list is a state
an app can reach, and the run that interprets the view already caught
it, printed the traceback and drew a red line in place of the screen.
The compiled run took the process down. Neither run had refused the
program, so the gate could only report that one side died. The view
build is contained now, and both runs collapse a failed view to the
same element from the same constructor — the failure is a thing the
gate compares, and the detail goes to the terminal in both.

## A generated app builds against the versions this tree tested

A generated crate is its own workspace, so without a lock file it
resolved its whole dependency graph afresh — and picked up whatever
the registry had published that morning. That is how a release of a
transitive dependency that does not compile broke every new app while
every already-resolved demo stayed green. Generated code is the
compiler's responsibility, and so is what it builds against: the
emitter now seeds the new crate with this tree's own `Cargo.lock`,
next to the toolchain pin it already copied. Seeded, not overwritten
— cargo adjusts it from there, and an app that has resolved is left
alone.

## The standard library, in two layers

`import math`, `import random` and `import statistics` are in the
dialect now, written as Python writes them. During development the
app imports CPython's module and CPython runs it; the shipped binary
calls a twin written against CPython's semantics; the gate holds the
two runs together, and a table of answers CPython printed holds the
twin to CPython — every function, and every error, including the
message. This is the arrangement `str` and the arithmetic already
had, extended from builtins to modules.

A module enters this layer when CPython's behavior is a specification
rather than an accident of the machine: `math` over IEEE doubles, the
Mersenne Twister and the algorithms `random` builds on it, the exact
rational sums `statistics` is defined by. What a platform's C library
decides is marked as such in the table and compared within an ulp;
what CPython computes for itself is compared to the bit, which is why
`hypot` here is CPython's vector norm rather than a call to the
platform's. Three things the first tables caught: `sqrt` of a
negative answered a quiet NaN, `pow` answered infinity where Python
overflows, and a square root of a fraction was an ulp low without the
round-to-odd step CPython uses to make its one rounding land right.

What Yokan adds of its own — `fs`, `sqlite`, `http`, `json`, `time`,
`strings`, `clipboard`, `notify` — keeps the other shape: one
implementation both runs call, deliberately flat for a typed subset.
The two layers are told apart by their names, so Yokan's own modules
never reuse a Python module's, and `from yokan import math` and
`random` went. What could not follow Python is refused by name with
its reason: `frexp` and `modf` answer tuples; `prod` and `statistics`
over a list of ints answer an int or a float depending on the values,
which a static type cannot say; `random.shuffle` reorders a list in
place, and a list lives in a `State` here.

Rejected: growing the bespoke API (names that look like Python and
mean something else — the old `random.int` was a different generator
under a familiar name), leaving it all to `@py` (ships a CPython, and
the gate sees nothing), and routing the interpreted run through doors
under Python's names (an import taken over for the app is taken over
for every package in the process).

## A view may call what cannot change

A view stays pure, which used to mean it could call no
standard-library function at all. Purity is a property of the
function, though, not of the library: `math.sqrt` has no more effect
on the world than `.upper()` does, and `.upper()` has been legal in a
view since the day it landed. The manifest carries the column, so the
rule reads off the row — `math` and `statistics` in a view, `random`
and the clock and the filesystem in a handler, each refused by name
with what the module does offer there.

Two things had to be true first. A view that fails had to fail the
same way in both runs, which it now does. And the interpreted run had
to survive one: a twin that stops its statement arrives there as
pyo3's `PanicException`, and printing one resumes the panic — which
is exactly how a failing handler reaches its containment, and exactly
what took the process down when a view had none of its own.

## `json.dumps` is Python's; the path reads are `jsondoc`

The module split rather than moved. Writing a value as JSON is
`json.dumps`, so it went to Python's side and now writes what CPython
writes: `", "` and `": "` between the parts, non-ASCII as `\uXXXX`
with a surrogate pair past the basic plane, floats through Python's
repr, `NaN` and `Infinity` where JSON has no syntax, and keys in the
order they went into the dict — which the dict only started
remembering when its map became insertion-ordered. serde_json is
close and none of those, so the writer is written out.

Reading a value out of a document by a dotted path is not something
Python's `json` does at all, and Yokan's modules do not carry a
Python module's name, so that half is `jsondoc`. `json.loads` is
refused with `jsondoc` in the message: what it answers has no shape
until it runs, and a typed subset cannot name that.

The twelve writers this replaces were one per value shape, and they
could not nest — the shape of `dict[str, list[int]]` had no writer
and never would, because the shapes multiply. The pieces compose
instead: a value renders to its text, and a container joins texts. A
literal nests as deep as it is written out, and a value the app is
holding renders through one call per container, which reaches a
level. The memo that planned this proposed a `Json` enum crossing
the binding boundary; payload-carrying enums do not cross yet, and
composition needs no new crossing.

## A pure static over lists is view-safe too

A view could call a binding static whose parameters and return were
scalars, and not one that took a list. Nothing about a `List<T>`
makes it less safe there — it carries no World handle, and a view
already reads list props for its repeaters and its charts — so the
rule now asks whether the shape is a value rather than whether it is
scalar. The same conversions the method side uses carry the list
across, so the two sides speak one vocabulary. That is what lets a
composing writer run in a text hole rather than only in a handler.

## `datetime` as integers, and `time` split from `clock`

The last two names Yokan had borrowed from Python are given back.
Reading the clock is `import time` — `time`, `time_ns`, `monotonic`,
`perf_counter`, `sleep` — and what stays on Yokan's side is the
machine's own zone, under `clock`, because Python reaches that only
through `localtime` and a struct. A clock is the one thing a
ground-truth table cannot pin down: the answer is the machine's, and
the two runs read it at different moments, so what a twin owes here
is the unit and the reference point CPython documents.

`datetime` is in, naive, as `date`, `datetime` and `timedelta`. Each
is carried as an integer — a date is its ordinal, a datetime and a
timedelta are microseconds — so comparison is integer comparison,
ordering included, and nothing new has to cross the binding boundary.
What the app writes as a method or an attribute is a static over that
integer, and a default is folded to the integer at translate time,
since the translator runs on the same CPython it could simply ask.
The plan had suggested a struct or an integer; the integer is what
made ordering free.

`strftime` is written out rather than handed to a C library. CPython
passes most directives to the platform, so the ones with a meaning of
its own are what the dialect takes — `%Y %m %d %H %M %S %f %j %a %A
%b %B %p %I %w %U %W %%`, with the month and day names of the C
locale — and `%c`, `%x`, `%X` and `%-d` are refused, because what
they answer is the machine's business and the two runs would not
agree on it.

What is refused, each by name: an aware value (`timezone`, `tzinfo`),
`datetime.time`, `replace`, `strptime`, a date inside a list or a
dict, and a date as a helper's parameter. The container one is the
representation showing through — a list of dates would read as a list
of numbers wherever the translator did not follow — and it is refused
rather than half-carried.

## `re` runs CPython's own compiled pattern

A pattern is a literal, and the translator runs on the same CPython
the app develops on — so the translator compiles it, with
`re._parser` and `re._compiler`, into the array CPython itself would
execute, and the shipped binary hands that array to an engine
(rustpython-sre_engine, the one RustPython uses, whose `SRE_MAGIC`
matches CPython 3.14's). The backtracking, the groups and the flags
are therefore CPython's rather than a second dialect of them, and no
regular-expression compiler is written on this side at all. A pattern
built at run time is refused by name, pointing at `@py`.

A `Match` has no shape a typed subset can hold — its groups are
`str | None` and its spans are its own — so what the dialect takes
are the calls whose answer is already one of its types: `findall`
(one group or none; two would answer tuples), `sub` (its replacement
template read by CPython too), `split` (over a pattern without
groups, since a group that does not participate is `None` between the
pieces), `escape`, and `re.search(p, s) is not None` as the test that
`if m:` would have been. Each refusal says which of those to reach
for instead.

## The small modules, and what stays out of them

`string`'s constants, `textwrap.dedent` and `indent`, `bisect_left`
and `bisect_right`, `heapq.nsmallest` and `nlargest`, and the rest of
`str` — `title`, `capitalize`, `swapcase`, `zfill`, `ljust`, `rjust`,
`center`, `expandtabs`, `splitlines`, `removeprefix`, `removesuffix`,
`rfind`, `index`, `rindex`, the `is…` family, and `strip` with a set
of characters. All of it pure, so a view may call any of it.

Two lines drawn, both from the same principle. What rearranges a list
in place — `heapq.heappush`, `bisect.insort` — is refused, because a
list lives in a `State` here and a call cannot reach into one; the
functions that answer a new list are what the dialect has.
`textwrap.wrap`, `fill` and `shorten` are refused because CPython
splits words there with a regular expression of its own, so a twin
for them is a port of that expression rather than a call.

`title` and `capitalize` needed Unicode's TITLECASE mapping, which is
not the uppercase one: `ß` titlecases to `Ss` and uppercases to `SS`,
and about a hundred characters make that distinction. The table is
taken from a crate rather than written. `casefold` is refused for the
other half of the same fact — its mapping expands `ß` to `ss` and
needs the full case-folding table, which `lower()` does not.

## A tuple is a value with a part per position

Python's iteration idioms are tuple-shaped — `dict.items()`,
`enumerate`, `zip`, a function answering two things, `divmod`,
`str.partition`, `math.frexp` — so a subset without tuples breaks at
the place people write most. A tuple is now a value: the translator
declares a struct per SHAPE and everything the struct machinery
already carried (a state, a field, a list's element, a parameter, a
return) carries it unchanged. The shapes are bounded by what the app
writes out, so no combination is invented that nobody asked for.

`t[0]` takes a literal position, because the parts have types of
their own and a computed index would have no one type to be. Two
parts or more. `a, b = expr` binds the value once and then its parts,
which is the order Python reads it in.

A stdlib call that answers a tuple does not answer one: each part is
a static of its own and the translator puts the pair together. That
is exactly what `divmod` is in Python (`(a // b, a % b)`), and it
means nothing new has to cross the crate boundary for `frexp`,
`modf`, `partition` or `rpartition`. What a Rust crate would have to
ANSWER as a tuple still does not cross, which is why `re.findall`
keeps refusing a pattern with two groups or more.

## A local cannot take a state's name

Writing `q = 5` in a handler where `q` is a `State` shadows the State
object for the rest of the function in Python, while the compiled
side reads the state — so a later `f"{q}"` printed two different
things. It was silent until a tuple unpacking put a common name like
`q` on the left. It is refused now, in both the plain assignment and
the unpacking, and the message names the rename.

## The container operations belong to the container

Sixteen functions — `in`, a slice, `+` and a reversal, once per
element type — said nothing about the element. They were four ideas
written four times, and a fifth element type meant writing them a
fifth. They are one implementation each now, generic on the kernel's
own list, so they take a list of anything the app can hold: a value
class, a tuple, an enum. Nothing was added to the boundary to get
there; what moved is where the operation lives.

Comparing is the opposite: it needs to know what to compare, and a
value class has no order of its own — which is what Python says too,
where `sorted` on a list of dataclasses raises. So the ordering
operations take Python's own answer to that, `key=`, and the refusal
without one names it.

## A key function is a loop, not a callback

`sorted(xs, key=f)` is decorate-sort-undecorate: the keys are
computed first, then the elements move to where their keys went.
Written that way the key function never becomes a value — the
translator emits the loop that computes the keys, in the app's own
dialect, and asks the container only for the order. So the key can be
anything the dialect can write, a lambda or a named helper, and the
crate boundary keeps carrying values and nothing else.

The sort is stable and `reverse=True` reverses the COMPARISON rather
than the result, which is what Python does and what
`sorted(...)[::-1]` does not: ties keep the order they came in.
`min`/`max` with a key take the first of equal keys, the way Python
does, and an empty list raises.

## `reversed` is an iterator, so `xs[::-1]` is the list

`reversed(xs)` answered a list here and an iterator in Python. The
difference is invisible in a `for` — where the name is nearly always
written — and shows up the moment the result is indexed or measured.
It is now what Python says it is: a `for` iterable. Where a list is
wanted the spelling is `xs[::-1]`, which is a list in Python too, and
the refusal for the old shape names it.

## Two of Python's modules are written out, not called

`collections` and `itertools` answer shape rather than arithmetic: a
count, a pairing, a product. There is nothing for a Rust twin to
compute — what they need is a loop, and the dialect can write loops.
So the translator writes the loop each call stands for, and the
interpreted run, which is CPython's own module, is what the gate
compares it against. That is a stronger check than a table of
answers, not a weaker one: the real module is running on the other
side of every comparison.

This is where the tuples and the generic containers were leading. A
`Counter` is a dict of counts; `most_common` is a stable sort by the
count, which the container now does for any element; `pairwise` and
`combinations` answer lists of tuples, which are values here. Nothing
new crosses the crate boundary for any of it.

## An iterator belongs in a `for`

Every `itertools` combinator answers a lazy iterator in Python. A
`for` cannot tell laziness from a list — the same elements in the
same order, and `break` stops at the same place because the list is
built before the loop runs — so that is the one place these are
taken. As a value they are refused, and the refusal names the `for`.
It is the rule `reversed` already follows.

The combinators that never end (`count`, `cycle`, `repeat`) are
refused for the reason that makes them useful in Python and useless
here. The ones that yield an iterator of their own (`groupby`, `tee`)
have nothing to become. `batched`'s last tuple is a different shape
from the rest, and a tuple's shape is written out.

## A Counter is a dict, and only a Counter has `most_common`

Python's `Counter` is a dict subclass, so it is a dict here: walked,
measured, tested with `in`, read with `.get`. What a plain dict does
not have is `.most_common` and `.total`, and neither does one here —
the translator remembers which locals came from a `Counter(...)`, and
a plain dict asking for `.most_common` is refused the way Python
refuses it.

`defaultdict` is the one member deliberately left out rather than
deferred. Its whole purpose is to answer for a missing key at the
dict; this dialect asks that question at the read instead, which is
the same reason bare `d[k]` is refused. Counting is `Counter`, and
grouping writes the list back — which now works, because a dict of
lists reads with `.get(k, [])`.

## `check` warns where the gate cannot look

The gate proves the two runs agree on what the screen shows. A
mistake both runs make the same way passes it, and so does a
difference in something a dump never carries. Memory is the second
kind: models are reference-counted after compilation and a cycle is
never freed, while the CPython the app is developed on collects it.
The tour has said exactly that from the start, with nothing behind
it that could point at the mistake in an app.

`check` now carries advisories beside its refusals. A warning means
the app is inside the dialect and something in it is still worth
changing: it prints on every command, the run does not fail, and
`--strict` turns it into one. Refusals keep their contract — silence
still means the app translates — because a warning is a different
answer, not a softer refusal.

The first one is the reference cycle. The translator already knows
every model field, which of them point at another model, and which
carry `Weak`, so the graph of strong references is in hand by the
end of the scan; the warning names the loop field by field
(`Kid.owner → Parent.kids → Kid`) and asks for `Weak[...]` on the
reference that points back. Only loops through two or more classes
are reported. A self-reference is how a list and a tree are written,
and at the type level those are indistinguishable from a ring; the
certain form of that catch reads the handler that wires two objects
to each other, and is a later question.

The rule this sets for the ones after it: an advisory earns its
place by naming what to write instead, the way a refusal does, and
by covering something no other check can see. What a Python linter
already reports stays with the Python linter, and there is no
configuration file, no severity, and no suppression comment — a
warning nobody can state in one sentence is not one.

## `init` writes the file the tour opens with

A formatter would be a second answer to a question Python already
answers: the source is Python, and Python's formatters format it. A
scaffold is not. Nothing outside this project knows the shape an app
has to have — exactly one `run(view, ...)` under the `__main__`
guard, a view that is one `column` block, state that is a `State` or
a store, the dependency declared where uv reads it. Each of those is
learned today by being refused.

`yokan init [app.py]` writes the smallest app that is already inside
the dialect, which is the same file the tour opens with, its title
taken from the file name. A name that is taken and a name that is not
a Python file are both refused, naming what to type instead.

What the command is really for is the three lines printed underneath
it: the run, the gate, the build. The gate is the promise this
product is built on, and a newcomer has no way to guess that it
exists. Putting it in front of them at minute one is worth more than
the code above it.

The scaffold is gated as the user receives it: the sweep runs `init`
into a scratch directory and gates the file that comes out. A
template is the one piece of code every new app starts from, so it
cannot be left to drift out of the dialect quietly. It lives as a
string in the CLI rather than a file in the wheel, so there is one
copy of it and nothing extra to install.

## The handler that writes the round trip

The type graph says a cycle is possible; the handler says one was
built. `a.kid = b` followed by `b.parent = a` puts two objects in
each other's hands whatever the field types would have allowed, and
that is a fact about the code rather than a shape it might take.

It reaches what the type graph cannot. A model that references its
own class is the ordinary way to write a list and a tree, and a ring
has the same type; only the wiring tells them apart, which is why the
first rule stays silent there and this one does not.

The analysis is small because it only has to be right. Objects are
locals, numbered afresh at every binding, so rebinding a name does
not carry the old object's edges forward; copying a local (`c = a`)
carries the identity, because it is the same object. A field
assignment adds an edge and replaces whatever the field held, and
anything the pass cannot follow removes it. Only a straight run of
simple statements is one analysis: two assignments in different
branches of an `if` do not both run, so a compound statement takes
its own bodies aside and clears what the run had learned. Nothing is
inferred across a call.

The two rules never say the same thing twice. The classes the type
graph reported are remembered, and a wired loop confined to them is
left to the first message.

## The checkout fetches itself

`gate` and `build` compile against the Rust crates in this
repository, so the instructions began with `git clone`. That is a
step the tool can take. `repo()` already resolved `PIXIE_REPO`, then
the tree this file sits in, then upward from the working directory,
and finding nothing was an error; finding nothing now means the
checkout is not installed yet, and the first native build fetches it.

The clone is pinned to the tag of the installed version and lives in
its own directory per version (`~/.cache/yokan/repo-<version>`). The
compiler and the standard library an app links come from the
checkout, so one that does not match the wheel is exactly the
mismatch the release gate exists to catch. A version with no tag yet
— a local build, a pre-release — falls back to the default branch. It
is a shallow clone, about 11 MB.

The order in front of it does not change. A checkout the user
already has still wins, so a contributor's build is untouched and
nothing is fetched behind their back. If `git` is missing or the
fetch fails, the message names what to clone and which variable to
set, which is where the instructions used to start.

The installation page changed shape with it. It used to be two
halves, develop and ship, with a clone between them; it now opens
the whole path — install the command, `init`, `uv run`, `check`,
`gate`, `build` — as one console block, and says which of those need
Rust. `init` prints the same four steps for the file it just wrote,
so the tool and the page agree.

## Two refusals that were not teaching

A file that does not parse got a Python traceback: the gate's own
frames under a `SyntaxError`. The first typo a newcomer writes is the
worst place to show them the compiler's insides. It now prints what
every refusal prints — `file:line:col`, the message, the source line,
the caret — because a file that does not parse is not a dialect
question and never a fault of the gate's.

`from os import getcwd` refused with "expressions are state reads,
fields, locals, literals…", while `import os` named the module and
what to write instead. The difference was only that the from-form
binds a bare name and nothing remembered where it came from. It is
remembered now, the way a plain import already was, so both spellings
of the same mistake teach the same thing.

## The coverage page is generated, and the builtins are probed

How far the dialect reaches had three answers in three places: the
manifest for the standard library, the translator's own tables for
`datetime`, `collections` and `itertools`, and for the builtins
nothing at all — what a reader could learn came from hitting a
refusal. The site now carries one page that answers it, and nothing
on that page is typed by hand.

The modules are read where they are declared. The manifest gives the
rows and the `cpython` flag that says which of them are held to what
CPython prints; the translator's tables give the three it carries
itself. Against those stands CPython's own surface, taken by
inspection when the page is generated — including a module's C
accelerator, because `bisect_left` says `_bisect`, and dropping it
had the page reading "bisect: 0 of Python's 0".

The builtins have no declaration to read, so they are probed. Each
one is written into a handler the way an app would write it, run past
the translator, and what comes back is either nothing or the refusal;
sixteen of forty-five are in. The refusals are printed, grouped by
what they say, so the answer to "why not" is the sentence that
already knows what to write instead. Probing is also the only honest
form: `range`, `zip` and `reversed` are refused as values and taken
in a `for`, and only running them says so.

Both languages come from the same generator, so the two pages cannot
drift apart, and neither can drift from the compiler.

## The build tree outlives the version, and `version` says what is in play

A user of the wheel has three things in play: the package, the
checkout it compiles against, and the tree those builds land in. Only
the first is theirs to manage — `uv tool upgrade yokan` — and the
second already follows it, since the checkout is fetched at the tag of
the installed version. The third was wrong.

Cargo's default target sits inside the manifest's directory, which for
a fetched checkout meant `~/.cache/yokan/repo-<version>/target`. That
ties the build tree to the version: an upgrade fetched a new checkout,
found no build tree in it, and compiled the engine from nothing — the
first-build wait again, at every release — while the tree it replaced
stayed on disk for good. The tree now sits beside the checkouts, one
of it, shared across versions, so an upgrade compiles what changed. A
checkout the user brought keeps cargo's own default and a
CARGO_TARGET_DIR they set wins over both, so a contributor's build is
untouched.

A fetch also drops the checkouts it supersedes. Each is one clone
away, and nothing reads them once the version has moved.

Upgrading stays where it belongs: the package manager moves the
wheel, and the checkout follows it by its tag. What that leaves is a
question of visibility, and `yokan version` answers it — the package,
the checkout with its tag and whether it was fetched or brought, and
the build tree with its size — while fetching nothing on the way,
because the question tends to be asked when something is already
wrong. `yokan clean` drops the
cache, which is safe to offer precisely because everything in it is
one fetch and one build away.

## The drawing surface, and the keyboard as a device (2026-09-04)

Two things the catalog could not do: paint a grid of pixels, and
answer "is that key held right now". Both are what a game needs, and
neither is only for games — the first is every chart nobody wants to
write as a chart, the second is every app with a direct manipulation
in it.

**A canvas, and commands that are not elements.** `Canvas` carries a
virtual size, an integer `scale` (how many logical pixels each virtual
one takes), a palette and a list of drawing commands: pixel, line,
rect, its outline, circle, its outline, triangle, its outline, sprite
and text. A command is a separate kind of thing from an element, and
that was the decision with the most consequences. As elements they
would each take the universal riders — a rectangle that can be
disabled, a sprite with a tooltip — and every walker in the runtime
would grow an arm for something it can never mean. As their own
values they are pure data: nothing to click, nothing to key, nothing
for the accessibility walk to describe, and the whole frame fits in
the dump.

**Inside a canvas a color is a number.** It is an index into the
palette the app declares, and the palette is required. The
alternative was to accept hex strings and theme tokens as well, and it
was refused for the shape it would give the app rather than for the
work: a frame writes a hundred colors, and two spellings for every one
of them is a worse interface than a table with sixteen rows. An index
past the end paints the last color rather than nothing, so an
off-by-one is visible; a canvas with an empty palette paints magenta,
which is what a missing color looks like everywhere else in graphics.
Coordinates are integers, because a pixel grid has no half pixels.

**The dump is one command per line.** Everything else in the element
tree dumps on a single line, and stays that way. A frame is hundreds
of commands, and a failing gate reports a difference by printing the
line it is on — so a frame on one line would print two twenty-kilobyte
lines and say nothing. One command per line makes that report a diff.
The colors are printed resolved, so a palette change is visible in the
comparison.

**The engine paints it itself.** Every other element hands gpui
elements and lets it lay them out; a canvas is rasterized here,
command by command, into a buffer and handed over as one image. Its
paths are antialiased and its image sampler interpolates, and dot art
must be neither, so the buffer is built at the display's own
resolution — virtual size times the scale times the device factor,
each virtual pixel a square of identical device pixels — where there
is nothing left to interpolate. Each canvas keeps its last image,
keyed by where it sits in the tree, and rebuilds only when the
commands, the palette or the display change; a replaced image is
handed back to the window, because the texture atlas keys on the image
and would otherwise hold every frame ever painted. Sprite sheets are
decoded by the rasterizer rather than through the asset cache, which
answers a frame later than a rasterizer can wait; a sheet that is not
there paints nothing, since a placeholder box in the middle of a frame
is worse than a hole. The text is drawn in a 4x6 font this repository
owns, so a drawing surface does not depend on an asset to write a
score.

**A chord is a message; a key is a device.** `shortcut` and `on_key`
deliver a chord to a handler and are over. A game asks a different
question, and the answer is not app state: it belongs to the keyboard,
which is one device per process — exactly the shape the clipboard
already has here. So the key state lives beside it, held, pressed and
released, and the three reads are ordinary functions with no World in
reach, which is what lets one implementation serve the interpreted and
the compiled run.

The sets hold bare keys, not chords: `left` is held whether or not
shift is down with it, which is what the question means, and the
modifiers answer under their own names. What is pressed and what is
released are spent by the tick that saw them — not by the frame,
because a window pumps frames at the display's rate while a game ticks
at thirty, and clearing per frame would take a press away before the
tick meant to read it. So a tick sees every press since the previous
tick and never sees one twice, in a window and under a script's
`advance:` alike, because both run the same timer pass. The platform's
auto-repeat holds a key down without pressing it again, and a key held
while the app is switched away is released rather than left down
forever.

A script presses one with `keydown:` and lets go with `keyup:`;
`key:<chord>` keeps delivering its chord and now also taps the key,
so no scripted step means less than the hardware does. None of this
is in the dump — what an app DID with the keys is, which is the thing
worth comparing.

**Measured, because the shape of the design rested on it.** A frame
of four hundred commands over two hundred moving things, rebuilt by
the interpreted run — CPython building the whole element tree and
crossing into Rust once per command — costs 0.11 ms; thirty of them a
second is a third of one percent of the time available. Rasterizing
that frame into a 1280x960 image costs 0.18 ms in a release build and
2.5 ms in a debug one. So the drawing surface can be what it looks
like — the app writes the frame it wants, every frame — instead of
something that holds a command list in state and sends differences.

What is not here yet: sound, closing a window from the app, the
mouse, tilemaps, a camera offset, and a canvas that sizes itself to
its box (the painted size would then depend on the window, which the
dump cannot see, so the scale is a number the app declares). What a
canvas paints is not readable by assistive technology either — the
canvas reports as an image, and a label is the honest way to say what
is on it.

## What a game found (2026-09-04)

Two of Pyxel's example games — a shoot-'em-up and a jump game, both
MIT — were ported to the dialect as the first apps written against the
canvas. A port is a good test because nobody chose it to fit: it is
three hundred lines of someone else's Python, and every line either
compiles or names what it needs. Three things it found, in order of
seriousness.

**Parentheses were dropped.** `score + (kind + 1) * 100` was emitted as
`score + kind + 1 * 100`, which is a different number. The translator
walked Python's own syntax tree — where the grouping is already
decided — and wrote the operands back without it, so the result was
re-read with plain left-to-right precedence. This is the one kind of
bug the gate cannot catch on its own: the interpreted run IS the
Python, the compiled run is the translation, and a translation that
silently means something else is exactly what a gate compares — but
only if the script reaches the line. The jump game's script scored a
fruit and printed 102 where CPython printed 300. An operand that is
itself an operation now keeps its parentheses.

**A local could quietly become a field.** In a store method the
compiled side reads a field by its bare name, so a local called
`score` beside a field called `score` is two different names in the
two runs: Python's `self.score = score` writes the field, and the
translation wrote a name to itself. Both ports had one, written
without a thought, because it is how Python is written. It is refused
now — a local, a parameter or a loop variable may not take a field's
name, with the message naming the rename — which is the same rule a
local that shadows a `State` already followed. Renaming behind the
app's back was the alternative, and it was rejected: the fix has to
show up in the source, where the reader can see which name means
which thing.

**A store method could not call a sibling defined after it.** Method
names were registered as their bodies were read, so `tick` calling a
`move` written below it was "not a method of store Game". Python does
not care about the order, and now neither does this: the names are all
known before any body is walked.

Two more reaches, both the same gap in different places: a view could
not read a repeater row's field in a condition (`if fruit.alive`) or
in a boolean property (`flip_x=enemy.flip`). Both now read the row
they are standing on, the way the text and the numbers already did.

The ports themselves are `demo/shooter.py` and `demo/jump.py`. What
had to change from the originals is small and it is written at the top
of each file: fractional speeds are carried in tenths of a pixel
because a canvas has whole pixels; `for i in range(2)` around a
parallax layer is written out twice because a view's `for` walks a
list; and there is no sound, because there is no audio here yet.
Everything else is the game, including the numbers — a color is an
index into Pyxel's own palette, declared in the file.

## The window's own ring (2026-09-05)

Every app's tree was drawn inside a 16 px inset the engine added and
nothing could remove. It is a good default — a column of controls
should not touch the window's edge, and no app should have to say so —
but it was a default with no way out, which is a different thing from
a decision. The two ported games showed what that costs: a canvas
sized to its window landed 16 px right and down and lost the same
strip off the other two sides.

The inset is a number now: `run(view, …, padding=0.0)`, and
`[window] padding` in a project's manifest for the compiled build. It
keeps its 16 when nothing says otherwise, so every app that existed
looks exactly as it did. It is window chrome rather than an element,
so no dump moves with it — which also means the gate cannot check it,
and only a window can.
