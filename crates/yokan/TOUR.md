# Yokan language tour

Yokan is a compiler for desktop apps: it takes a statically typed
subset of Python to native code.
This tour is one pass over how the apps are written.
Every piece of code in this tour runs as-is on today's tree.
What Yokan cannot do yet is collected, with reasons, at the end
([What does not work yet](#what-does-not-work-yet)).

A Yokan app is an ordinary Python file.
During development it runs on real CPython; when you ship, the same source compiles to a native binary.
And the **gate** replays the same interaction script against both the development build and the shipped build and byte-diffs the results, verifying per app that the two behave the same.
Everything this tour calls "compiled" has passed that check.
The sections below will not repeat this.

## Table of contents

1. [The smallest app](#the-smallest-app)
2. [Holding state](#holding-state)
3. [Writing views](#writing-views)
4. [Form controls](#form-controls)
5. [Handlers and control flow](#handlers-and-control-flow)
6. [Arithmetic](#arithmetic)
7. [Lists, charts, virtualized lists](#lists-charts-virtualized-lists)
8. [Dicts](#dicts)
9. [Value classes and interfaces](#value-classes-and-interfaces)
10. [Memory](#memory)
11. [Sum types and match](#sum-types-and-match)
12. [Optional and Enum](#optional-and-enum)
13. [Components](#components)
14. [Styles and themes](#styles-and-themes)
15. [Animation](#animation)
16. [The window](#the-window)
17. [Error handling](#error-handling)
18. [The standard library](#the-standard-library)
19. [Calling a Rust crate](#calling-a-rust-crate)
20. [CPython escapes](#cpython-escapes)
21. [Heavy work and timers](#heavy-work-and-timers)
22. [Working with type checkers](#working-with-type-checkers)
23. [Headless runs and the gate](#headless-runs-and-the-gate)
24. [Shipping](#shipping)
25. [A real app](#a-real-app)
26. [What does not work yet](#what-does-not-work-yet)

## The smallest app

```python
# /// script
# dependencies = ["yokan"]
# ///
from yokan import State, button, column, run, text

count: State[int] = State(0)

def view():
    with column(spacing=12, padding=16):
        text(f"count: {count()}", size=34)
        button("+1", on_click=lambda: count.set(count() + 1))

if __name__ == "__main__":
    run(view, title="counter")
```

There are three ways to run it.

```console
$ uv run app.py                                    # develop: CPython + live reload
$ yokan gate app.py --script "click:+1,click:+1"  # verify: dev build vs shipped build
$ yokan build app.py --release                    # ship: produce only the native binary
```

The three comment lines at the top declare the dependency to uv, so `uv run app.py` fetches what it needs and just runs.
Edit and save the file while `uv run` is up: the window stays open, the app swaps to the new code — views and handlers alike — and state survives.
The `if __name__ == "__main__":` guard is required — without it the reload machinery would attempt a second launch.

## Holding state

There are three tools for holding state, chosen like this:

- A **single value**: `State[T]`.
- A **coherent area** (a cart, settings, one screen's state): a **store** (`@store`). Fields alone are fine; operations become methods.
- **Objects you create many of and want the screen to react to**: a **model** (`@model`).

### State

App state is declared at module level with `State` (`from yokan import State`).
The type annotation is where the compile-time type comes from, so always write it.
Read with `count()`, write with `count.set(v)`.

```python
count: State[int] = State(0)
name: State[str] = State("")
show: State[bool] = State(False)
items: State[list[str]] = State([])
prices: State[dict[str, int]] = State({"apple": 120})
```

The available types are int, str, float, bool, lists and dicts of those, Optional (`int | None`), Enum, and the value classes and sum types introduced later.
An int state checks the 64-bit integer range on every write, so a number that passed during development is the same number after you ship.

### Stores

A **store** (`@store`) is a singleton with fields and methods.
The decorator returns the instance, so the class name itself is the store.

```python
@store
class Cart:
    items: list[str] = []
    total: int = 0

    def add(self, name: str, price: int) -> None:
        self.items = self.items + [name]
        self.total += price

    def take_all(self, xs: list[str]) -> None:
        for x in xs:
            self.items = self.items + [x]

button("add", on_click=lambda: Cart.add("apple", 120))
button("clear", on_click=Cart.clear)
text(f"n={len(Cart.items)} total={Cart.total}")
```

Fields take the same types as State.
Method bodies are written the same way as handlers, and stores can call each other's methods.
Method parameters can be int, float, str, bool, `list[...]` of those, value classes, and Enums.

### Models

A **model** (`@model`) is an observed object you can create any number of.
Fields need defaults, and methods are written the same way as handlers.
When a view reads a field, it re-renders in response to changes.

```python
@model
class Circle:
    r: float = 1.0
    def grow(self, by: float) -> None:
        self.r += by

left = Circle()
right = Circle()
button("grow", on_click=lambda: left.grow(0.5))
```

Models can reference models.
An owning reference is written `Node | None` (or a list of models, `list[Node]`); the non-owning back reference is `Weak[Node]`.
Reference fields start at None ([] for lists) and get wired inside handlers.

```python
from yokan import Weak, model, store

@model
class Node:
    label: str = "n"
    kid: Node | None = None
    parent: Weak[Node] = None      # the back reference does not own

@store
class Tree:
    root: Node | None = None

    def build(self) -> None:
        a = Node()
        a.label = "alpha"
        b = Node()
        b.label = "beta"
        a.kid = b
        b.parent = a               # no cycle: parent is Weak
        self.root = a
```

When parent and child point at each other and both references own, neither can ever be released.
Make the back reference `Weak` and the moment the owning chain is cut (`self.root = None`) the whole chain is freed — and reading the `Weak` from the survivor answers None.

Reads use walrus narrowing (the same shape as in the Optional section; it works on model references as-is).

```python
if (r := Tree.root) is not None:
    text(f"root: {r.label}")
```

Data itself belongs in the **value classes** introduced later, placed on store fields — that is the base pattern.
Use models only for things that are shared, mutated, and followed by the screen.

## Writing views

A view opens a container with `with` and calls element constructors inside it.
Elements add themselves to the open container.
A view function is a pure function building the screen from state; mutation is the handlers' job.

```python
def view():
    with column(spacing=10, padding=16):
        text(f"hello {name()}", size=20, color="accent")
        with row(spacing=6):
            text_field(name(), placeholder="name", on_change=name.set)
            button("clear", on_click=lambda: name.set(""))
```

The element catalog: `text`, `button`, `text_field`, `checkbox`, `switch`, `slider`, `select`, `radio_group`, `tab_bar`, `column`, `row`, `grid`, `stack`, `list_view`, `scroll_view`, `h_scroll_view`, `data_table`, `modal`, `image`, `svg`, `bar_chart`, `line_chart`, `progress`, `spinner`.
`grid(columns=, rows=)` lays equal tracks, and a button inside spans cells with `col_span=` / `row_span=` (`demo/calcgrid.py` is the keypad on one grid).
`data_table` draws the table itself: its first `row` child is the header, later `row` children are data rows shaded in alternation, and the columns line up when the cells of one column carry the same `grow` share (`demo/table.py`, where `align="right"` sets the numbers on their column's edge).
The samples import elements bare — `from yokan import button, column, run, …`.
If you prefer a namespace, `import yokan as ui` works identically (`button`, `run`); the two spellings compile the same.

Text holes are opened with f-strings.
int, str, float, bool and Enum values render directly, and the text matches Python's `str()` (`2.0` renders as `2.0`, `True` as `True`, `Mood.HAPPY` as `Mood.HAPPY`).
To pin the number of decimals, format specs like `f"{x:.1f}"` work too.
Holes can also compute with `+`, `-`, `*` (`f"{n * 2 + 1}"`).

Conditionals are ordinary `if` / `elif` / `else` inside the view.
A modal is open by existing, so wrap it in an `if`.

```python
if show():
    with modal():
        text("confirm?")
        button("yes", on_click=lambda: (done.set(True), show.set(False)))
```

## Form controls

Value input always has the same shape.
Pass the displayed value in from state; the handler receives **the one new value** and writes it back.

```python
from yokan import store

@store
class Settings:
    dark: bool = False
    wifi: bool = True
    volume: float = 5.0
    fruits: list[str] = ["apple", "banana", "cherry"]
    fruit: int = 0
    tabs: list[str] = ["General", "Details"]
    tab: int = 0

    def set_dark(self, on: bool) -> None:
        self.dark = on

    def set_wifi(self, on: bool) -> None:
        self.wifi = on

    def set_volume(self, v: float) -> None:
        self.volume = v

    def pick_fruit(self, i: int) -> None:
        self.fruit = i

    def pick_tab(self, i: int) -> None:
        self.tab = i


checkbox("Dark mode", checked=Settings.dark, on_change=Settings.set_dark)
switch("Wi-Fi", checked=Settings.wifi, on_change=Settings.set_wifi)
slider(value=Settings.volume, min=0.0, max=10.0, step=1.0, on_change=Settings.set_volume)
select(options=Settings.fruits, selected=Settings.fruit, on_change=Settings.pick_fruit)
radio_group(options=Settings.fruits, selected=Settings.fruit, on_change=Settings.pick_fruit)
tab_bar(labels=Settings.tabs, active=Settings.tab, on_change=Settings.pick_tab)
```

- **checkbox / switch**: a label and `checked=`. The handler receives the new bool. In verification scripts, `click:<label>` toggles.
- **slider**: `value=` plus `min=` / `max=` / `step=`. The handler receives the new float. The script verb is `slide:<value>` (clamped to the range, snapped to the step).
- **select / radio_group / tab_bar**: the list of options and the current position. The handler receives the chosen **index**. The script verb is `select:<label>`.

Switching tab content is a plain `if` / `elif` under the `tab_bar`.

## Handlers and control flow

Handlers can be passed in three forms:
a lambda (a tuple for multiple operations, `lambda: (a.set(x), b.set(y))`), a module-level def, and a store's bound method (`on_click=Cart.clear`).

The body of a def handler compiles with its real control flow.

```python
def double(v: int) -> int:          # a pure helper becomes a native function
    return v * 2

def tally():
    total.set(0)
    for i in range(1, 6):
        if i == 3:
            continue
        total.set(total() + double(i))
```

Available: `if` / `elif` / `else`, `while`, `for` (over `range()`, list states, list fields, list-typed parameters), `break` / `continue`, and locals (reassignable, as in Python).
A pure helper (parameters and return annotated, body ending in `return expression`) is callable from handlers and from view text.

A local assigned in **both** the if and the else reads fine after the branch, as in Python.

```python
def judge():
    n = score()
    if n > 20:
        verdict = "high"
    else:
        verdict = "low"
    grade.set(verdict)      # readable after the branch
```

Reading a local assigned in only one branch is refused (had the branch not run, Python would raise NameError there).

Optional narrowing is written with the walrus.

```python
if (v := sel()) is not None:
    text(f"picked {v}")      # v is bound only inside this branch
else:
    text("(none)")
```

## Arithmetic

Python's arithmetic operators work as-is.
Besides `+`, `-`, `*`: `/` (the result is always float), `//` (floor toward negative infinity), `%` (the result takes the divisor's sign) and `**` all compile to **exactly Python's results**.

```python
q.set(1 / 3)          # 0.3333333333333333
d.set(-7 // 2)        # -4
r.set(7 % -2)         # -1
p.set(2 ** 10)        # 1024
```

Division by zero and overflow are exceptions in Python, and in the shipped app they surface the same way — the statement aborts, the app does not crash.
Write `int ** int` exponents as non-negative literals (a negative exponent would change the result's type at runtime; with either side a float, negative exponents are fine).
Do fallible `/` `//` `%` `**` inside handlers and hand the view the result.

`and`, `or`, `not` work in conditions as-is.
They also work as bool values (`both.set(hot() and not cold())`).
Using `and` / `or` as a value on non-bools is refused (Python returns **one of the operands themselves** there, which is a different thing from a truth value).

## Lists, charts, virtualized lists

Append to a list by concatenating and putting it back.
The shipped app compiles this to a one-element append, so there is no copy cost.

```python
items.set(items() + [x])     # append
items.set([])                # clear
len(items())                 # count
```

Indexing from the back takes a literal.

```python
r = names()
tail.set(r[-1])              # last element (too short: the statement aborts)
```

Charts draw lists of float or int.

```python
values: State[list[float]] = State([])
line_chart(values(), height=120.0)
bar_chart(Metrics.svc_reqs, labels=Metrics.svc_names, height=100.0)
```

Long lists go to `list_view`.
It is **virtualized**: the row builder `row(i)` is called only for the visible range (a dozen or so calls even at 100k rows).

```python
def row(i):
    return text(items()[i])

list_view(len(items()), row, item_height=22.0, height=200.0)
list_view(len(items()), row, item_height=22.0, grow=1.0)   # fill the parent's remaining height
```

## Dicts

Read with `.get`, write per key, count with `len`, iterate with `sorted()`.

```python
prices["cherry"] = 200                 # per-key write
picked.set(prices().get("apple", -1))  # read: default when missing
if "cherry" in prices(): ...           # membership
len(prices())                          # count

def scan():
    for k in sorted(prices()):         # iterates in key order
        last.set(k)
```

Bare `d[k]` reads and bare `for k in d` are refused.
Missing keys and iteration order are the places where Python is particular, and `get` / `sorted()` are the forms that state the intent.

## Value classes and interfaces

Data itself lives in **value classes**.
A class marked `@value` compiles to a native struct (it is the same thing as `@dataclass(frozen=True)`, and that spelling works too).
Immutability is what makes "a Python reference" and "a native value" mean the same thing, so updates are functional, via `replace`.
A field may hold another value class declared earlier (nested values).

```python
from dataclasses import replace

@value
class Point:
    x: int
    y: int = 0

sel: State[Point] = State(Point(3, 4))
sel.set(replace(sel(), x=10))
text(f"x={sel().x}")
```

Value classes can have methods.
Define the operator dunders (`__add__`, `__sub__`, `__mul__`) and `+` `-` `*` take those meanings (during development Python itself runs them; the shipped app calls the same computation, compiled).
A body is a single `return expression` (an immutable value has nothing to assign to).

```python
@value
class V2:
    x: int
    y: int

    def __add__(self, o: "V2") -> "V2":
        return V2(self.x + o.x, self.y + o.y)

    def __mul__(self, k: int) -> "V2":
        return V2(self.x * k, self.y * k)

    def dot(self, o: "V2") -> int:
        return self.x * o.x + self.y * o.y

c.set(a() + b() * 2)      # operators go to the dunders
d.set(a().dot(b()))       # plain methods from handlers
```

Interfaces are `typing.Protocol`.
A model that lists a Protocol with method stubs as a base implements it, and a helper taking a Protocol-typed parameter compiles to a statically dispatched generic function.

```python
class Shape(Protocol):
    def area(self) -> float: ...

@model
class Circle(Shape):
    r: float = 1.0
    def area(self) -> float:
        return self.r * self.r * 3.0

def area_of(s: Shape) -> float:
    return s.area()
```

## Memory

There is nothing to free by hand.
Two shapes cover everything.

- **Values** (Value classes, lists, dicts, strings) mean copies.
  If the place you passed one to changes it, your side does not change.
  The release build shares the storage until the moment of the write (copy-on-write), so passing a large list costs no duplication.
- **Models** (and the stores that hold them) are references.
  The release build reference-counts them and frees at the very assignment that drops the last owner.
  No collector scans the heap; nothing pauses.

The daily habits fall out of those two.

- Keep data in Value classes and lists, on store fields. Reserve models for what is shared, mutated, and watched by the screen.
- A model created inside a handler and never handed out is freed when the handler ends — the temporaries a loop makes included.
- Cut an ownership chain (`self.root = None`) and everything below it is freed together; a surviving `Weak` reads back None.

Cycles are the one exception.
Objects that own each other never let go, and the release build does not free them (a leak, never a crash).
The habit is to not build cycles: make the back reference `Weak`.
One honest note: the CPython you develop on does collect cycles, so a cycle you build by mistake is the one place the two runs' memory behavior differs.
The gate compares screens; memory is outside what it checks.

The number of live objects is countable at any time with the headless `mem` step.

## Sum types and match

Bundle value classes with a `type` alias and you have a set of alternatives — a sum type with payloads.
`match` works in handlers and in views, with destructuring.

```python
@value
class Healthy: pass
@value
class Degraded: services: int
@value
class Outage: service: str

type Health = Healthy | Degraded | Outage

health: State[Health] = State(Healthy())

# in the view:
match health():
    case Healthy():
        text("ALL SYSTEMS NOMINAL")
    case Degraded(services):
        text(f"DEGRADED — {services} service(s)")
    case Outage(service):
        text(f"OUTAGE — {service} is down")
```

Missing arms are reported at compile time.
Variant fields cannot take defaults, and a variant belongs to exactly one sum type.

## Optional and Enum

Optional works in state and in fields (`last: int | None = None`).
Narrowing is as shown in the walrus sections.

Enums are ordinary `class Mood(Enum)` and compile as-is.
`match` arms are `Mood.MEMBER` or `_`, and missing arms are reported.
In text they render exactly as Python does: `Mood.HAPPY`.

## Components

A view fragment you want to reuse becomes a **component** (`@component`).
Per-instance state lives in `local` (independent per call site, and it survives re-renders).

```python
@component
def counter(label: str, step: int):
    n: State[int] = local(0)
    with row(spacing=6):
        text(f"{label}: {n()}")
        button(f"+{step}", on_click=lambda: n.set(n() + step))
```

A component that takes children declares `slots=True`, and the children land at `slot()`.
The caller passes them with `with`.

```python
@component(slots=True)
def card(title: str):
    with column(border_width=1.0, border_color="accent", padding=8):
        text(title, size=18)
        slot()

with card("counters"):
    counter("a", 1)
    counter("b", 10)
```

`local` identity is call-site based.
Reorder the calls and the states swap along with them.

## Styles and themes

A style is a named dict, splatted onto an element with `**` (one per element).
Compose them with `|`.

```python
chip = style(size=18, color="accent")
key = style(background="surface", hover_background="surfaceHover")
hot = key | style(background="#fab387")

text(f"n={n()}", **chip)
```

Colors take hex literals or **theme tokens**.
`windowBg`, `panel`, `surface`, `surfaceHover`, `border`, `text`, `textDim`, `accent` and the rest resolve to the color the theme in effect dictates.

A theme is applied to a subtree with `theme=`.
The value can be a literal or a state read, so an app can own its palette as state.

```python
mode: State[str] = State("dark")

def flip():
    if mode() == "dark":
        mode.set("light")
    else:
        mode.set("dark")

with column(background="windowBg", grow=1.0, theme=mode()):
    ...
    button("theme", on_click=flip)
```

An app that themes the root of its tree follows that palette down to the window's ground color.

## Animation

Give an element `animate=` (milliseconds) and changes to that element interpolate.
`easing=` picks from `"linear"`, `"in"`, `"out"`, `"inOut"`, and `enter=True` / `exit=True` extend the animation to appearing and disappearing.

```python
text("OUTAGE — api is down", animate=140, easing="out", **pill_crit)
```

## The window

The app declares its title and size in `run`.

```python
run(view, title="OpsBoard", width=1100, height=820, on_start=boot)
```

`width` / `height` are logical pixels, given as a pair (omitted, the engine default applies).
The declaration is baked into the compiled binary as well.
`on_start` is a handler that runs once right after mount, and a failure prints and continues (use it for loading startup data or seeding the RNG).
It is also the only place for startup work: module level holds declarations, and a statement there (`count.set(5)`, say, or `fs.write_text(...)`) is refused by name, because the compiled app reads the module and never executes it.

## Error handling

The order to reach for things is fixed.

1. **Use `*_or`**. A read that folds failure into a default; when the reason for the failure does not matter, this is all you need.
   `fs.read_text_or(p, "")`, `http.get_text_or(url, "")`, `sqlite.query_int_or(p, sql, 0)`.
2. **Use try/except**. The form for when the reason matters, written exactly as in Python:
   multiple statements in the body, per-exception except clauses, tuples (`except (ValueError, KeyError) as e:`), `else`, `finally`.
   Exceptions raised by `@py` escape functions are caught here too, and `e`'s message is exactly what Python produces.
3. **Do nothing**. An uncaught failure aborts its statement and the app lives on.
   It does not crash.

```python
try:
    body.set(http.get_text(url))
except Exception as e:
    status.set(f"offline: {e}")
```

## The standard library

Use it with `from yokan import fs, sqlite, http, math, json, time, strings, random, notify`.
Each one calls the same function, implemented in Rust, during development and after shipping alike.
The shipped binary needs no Python.
Call them from handlers (views stay pure).

- **fs**: `read_text` / `write_text` / `exists` / `read_text_or`
- **sqlite**: `exec` / `query_text` / `query_int` / `query_int_or` / `query_text_or` (SQLite bundled. Wrap aggregates in COALESCE and pin the order with ORDER BY)
- **http**: `get_text` / `get_text_or` (synchronous)
- **math**: `sqrt` / `sin` / `cos` / `pow` / `fabs` / `floor` / `ceil` / `pi`
- **json**: `get_text` / `get_int` / `get_float` / `get_bool` / `length` / `has` (looked up by dotted paths like `"items.0.title"`)
- **time**: `now_ms`, `format_ms(ms, "%Y-%m-%d")` (UTC. In verification scripts, pass a fixed ms)
- **strings**: `to_int(s, default)` / `to_float(s, default)` (numeric parsing where broken input becomes the default)
- **random**: `seed(n)` / `int(lo, hi)` (inclusive on both ends) / `float()` (seed it and the sequence repeats)
- **notify**: `send(title, body)` — an OS notification, delivered through Notification Center when the app runs as an `.app` bundle (`--app`); a bare dev run and headless runs drop it quietly

The discipline underneath all of these is determinism.
Pass fixed times, seed the RNG — and verification scripts replay the same result every time.

You can also add a Rust crate of your own.
That is the next section.

## Calling a Rust crate

Declare a Rust crate and call it from the app — a crates.io
version or a local path, either way.
Adding one is a single command.

```console
$ yokan add app.py deunicode 1                    # from crates.io
$ yokan add app.py hexfmt --path native/hexfmt    # a local crate
```

The declaration has two homes, matching how the app is shaped:
the PEP 723 block's `[tool.yokan.crates]` for script-style apps,
and the same table in pyproject.toml for project-style apps
(`yokan add` finds and writes whichever applies).

```python
# /// script
# requires-python = ">=3.14"
#
# [tool.yokan.crates]
# hexfmt = { path = "native/hexfmt" }
# ///
from yokan import crates

# in a handler
self.encoded = crates.hexfmt.encode("yokan")
self.total = crates.hexfmt.add(40, 2)
self.mean = crates.hexfmt.avg(self.samples)
```

The crate side is ordinary Rust — no pyo3, no yokan types.

```rust
pub fn encode(s: &str) -> String { … }
pub fn add(a: i64, b: i64) -> i64 { … }
pub fn avg(xs: Vec<f64>) -> f64 { … }
```

The machinery is the standard library's own shape: one
implementation, two doors.
For the CPython run a pyo3 door is generated and built
automatically; for the release build the binding is derived from
rustdoc's JSON output.
`yokan gate` / `yokan build` take care of both.
To run with plain `uv run` before ever gating, run
`yokan sync app.py` once.

The feature has the native build's prerequisites (the repository
checkout and Rust).
Functions are called by their documented snake_case names.
What crosses: Int, Float, Bool, String, Lists of those, Optionals
(None included), str-keyed dicts (`HashMap<String, …>`), structs (nested
ones included) and enums, and Result-returning functions — compound
returns like `Result<Vec<…>>` included.
A dict returned from a crate arrives ordered by key, the same
order in both runs.
A Result is received with try/except, and `f"{e}"` renders the same
message in both runs.
Structs and enums cross when the app declares their twins — the
same shapes under the same names, nothing more.
For a nested struct, declare the inner twin first and name it in
the outer one's field:

```python
@value
class Span:          # twin of the crate's struct Span
    lo: int
    hi: int

class Grade(Enum):   # twin of the crate's enum Grade
    Fine = 1
    Odd = 2

moved = crates.hexfmt.shift(Span(3, 8), 10)
self.verdict = crates.hexfmt.describe(crates.hexfmt.judge(7))
```

Structs whose Rust fields carry exact widths (`u32` and friends)
cross too — reads widen, writes cast back to the width — and the
same rules apply to nested fields.
Anything that cannot cross is refused with a named reason.
The demos: `demo/rustcrate.py` (a path crate and a crates.io crate;
Optionals, Result, a struct, an enum and a dict) and `demo/proj/`
(the pyproject spelling).

## CPython escapes

When you need Python beyond everything above, mark the function with `@py` (`from yokan import py`).
That function stays **real Python**.
During development it runs as-is; after shipping it runs on the bundled or ambient CPython (to be self-contained, use `--bundle` / `--onefile`, below).

```python
@py
def slug(t: str) -> str:
    import re                  # write imports inside the escape
    return re.sub(r"[^a-z0-9]+", "-", t.lower()).strip("-")
```

Annotate every parameter and the return (int / float / str / bool / list[...]).
Compiled extensions like numpy work inside escapes.

## Heavy work and timers

Never block a handler (the window freezes).
`task` does the work on a worker thread and runs the continuation on the UI thread when it finishes.

```python
def start():
    busy.set(True)
    task(fetch_data,
            on_done=lambda v: (busy.set(False), data.set(v)),
            on_error=lambda e: (busy.set(False), err.set(str(e))))
```

`work` must not build UI elements; it just returns a value.
Headless runs wait for task completion before taking the next step, so flows containing tasks are testable.
Today `task` runs in the development run only, and the compiler refuses the call by name until the compiled run has a worker thread to give it (see [What does not work yet](#what-does-not-work-yet)).
Until then, a compiled handler that fetches over `http` waits for the reply, and the window waits with it.

`every(seconds, cb)` is a timer with a seconds interval.
Call it before `run`.
Timers are a development-run feature: rather than shipping an app that starts without the timer, the compiler refuses the `every` call by name (see [What does not work yet](#what-does-not-work-yet)).

## Working with type checkers

Yokan bundles type stubs, so checking with pyright (Pylance in VS Code) works out of the box.
The stubs declare the runtime's actual shapes.
`@store` binds the singleton instance to the class name, so `Settings.set_dark(True)` counts as a bound method call, and handler references like `on_change=Settings.set_dark` pass as the expected `(bool) -> None`.
`@model` and `@value` declare that constructors are synthesized from the fields, and `Weak[Node]` appears to a checker as `Node | None` (which is exactly what a read means).

mypy has a known limitation: it does not apply type transformations made by class decorators.
Under mypy, `@store` method calls are therefore misreported as "self is not passed".
We recommend pyright for checking.

A type checker knows Python's types, not the dialect's boundary.
`yokan check app.py` answers that half: it runs the translator over every module the app imports, prints the first refusal in the `file:line:col` form, and says nothing when the app is inside the dialect.
No compiler is started, so it is the check to run while editing.

## Headless runs and the gate

Running without a window is where verification starts.

```console
$ PIXIE_SCRIPT="click:+1,input:Momo" uv run app.py
```

The step vocabulary is `click[@n]:<label>`, `input[@n]:<text>`, `submit[@n]`, `slide[@n]:<value>`, `select[@n]:<label>`, `advance:<ms>`, `theme:light|dark`, `a11y`, `mem`, `dump`.
`@n` picks the n-th match in tree order, so a row of identical buttons is reachable (`click@2:delete`).
`dump` prints the screen at that point in the script, which is what makes an intermediate state checked and not just the first and last.
A comma inside text is written `\,` (`input:hello\, world`).
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
The steps are collected in the README's Platforms section.

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
Every one of them passes the gate, except the four that hold state in a dict (`run(state={...})`) — those are development-only by design, and the gallery says so on each.

## What does not work yet

What lies outside this range is refused by name — it does not silently change behavior.
A refusal names the file, line and column and quotes the line:

```console
$ yokan build app.py --release
widgets.py:5:40: not in the dialect — text() does not take `weight=`
        return text(label, size=12, weight=2)
                                           ^
```

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
- **Statements at module level.** The compiled app reads the module's declarations (imports, `State`, classes, defs, `style()`, type aliases, literal constants, the `__main__` guard) and never executes it, so a `count.set(5)` or a `fs.write_text(...)` outside a function is refused by name. Startup work goes in a def passed as `run(view, on_start=setup)`.
- **Compiling `task`.** Worker threads are a development-run feature today, so the call is refused by name. A compiled handler runs to the end: one that fetches over `http` waits for the reply, and so does the window.
- **Compiling `every`.** Timers are a development-run feature and do not run headless either; the call is refused by name rather than compiled away.
- A component's `local` is **identified by call site**. Reordering the calls reassigns the states.
- Placing the same element object **twice**. Constructors consume their children.
- **Reading a module constant in a handler or a view.** `LIMIT = 10` at module level is a declaration, but reading it inside a handler is not in the dialect yet; write the literal, or hold the value in a State.
- **Store and model methods that return a value**, `@property` and `@staticmethod`. Keep derived values in a field the view reads.
- **Most list operations beyond append.** Indexing a list read directly (`items()[0]`, `self.xs[i]`), a variable index, slices, `in` over a list, `sorted` / `reversed` / `min` / `max` / `sum`, comprehensions, `enumerate` / `zip`, `range` with a step, and local lists and dicts. Append with `items.set(items() + [x])`; index through a local with a literal index.
- **str methods, `len(s)` and conversions.** `.upper()`, `.split()`, `.strip()` and the rest, `str()` / `int()` / `float()`, indexing a str. Parse numbers with `strings.to_int` / `strings.to_float`; render values in f-strings.
- **Format specs other than `.Nf`** in views (width, `,`, `%`, `e`, `d`), and any format spec in a handler f-string.
- **Dynamic dict keys** (`d[name()] = v`, `"two words"`), `.values()` / `.items()`, and dict literals in handlers.
- **Some control flow**: `while True`, chained comparisons, conditional expressions (`a if c else b`), a bool local as a bare condition, an early `return` in a helper, helper default and keyword arguments, tuple assignment, nested defs, `print`, `raise`, `assert`.
- **Keyword arguments to store and model methods**, model constructor arguments (`Node(v=3)`), and the `Optional[T]` spelling (write `T | None`).
- **`match` on int or str literals, guards and `|` patterns**; `.name` / `.value` on an Enum member; iterating an Enum.
- **Style values from state** (`size=count()`, `color=name()`), text from a str expression (`text(Store.label)`), and literal option lists in `select` / `tab_bar`. Branch with `if`; put the text in an f-string hole; hold the options in a State.
- **Component parameters other than str and int**, callback and State parameters, an `if` at the top of a component body, and a `local` holding a list.
- **The row index in `list_view` beyond indexing** (`lambda: Store.pick(i)`, `if i == sel`, `f"{i + 1}"`).
- **Types beyond one level**: `list[bool]`, `list[Point]`, `list[list[int]]`, `dict[str, list[str]]`, int-keyed dicts, tuple, set, `Point | None`, value-class fields that are lists or Optionals, model fields holding dicts or value classes.
- **`@py` signatures beyond scalars and lists** (dict, value class, Optional).
- **Writing a store field from outside the store** (`Cart.total = 5`). Write it through a method.
- **In the standard library**: sqlite parameter binding and multi-column rows, http POST / headers / timeouts, fs directory listing, json writing, local time.
- **At the Rust-crate boundary, payload-carrying enums and methods on a twin do not cross yet.** Scalars, String, Lists, Optionals, str-keyed dicts, structs (nested and width-annotated fields included), enums, and Result (compound returns too) all do. The two that remain each wait on something specific: payload enums on rpi-gen itself, methods on impl-splicing onto an rpi-declared struct. Enum- or list-typed fields inside a struct stay out too; every call outside the set is refused with a named reason.
- All measurements are macOS/arm64. Other platforms are not measured yet.

This list is updated every time a design lands.
The design principles behind it are collected in [DESIGN.md](DESIGN.md).
