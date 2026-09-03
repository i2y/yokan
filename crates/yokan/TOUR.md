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
7. [Strings](#strings)
8. [Lists, charts, virtualized lists](#lists-charts-virtualized-lists)
9. [Dicts](#dicts)
10. [Value classes and interfaces](#value-classes-and-interfaces)
11. [Memory](#memory)
12. [Sum types and match](#sum-types-and-match)
13. [Optional and Enum](#optional-and-enum)
14. [Components](#components)
15. [Styles and themes](#styles-and-themes)
16. [Animation](#animation)
17. [The window](#the-window)
18. [Error handling](#error-handling)
19. [The standard library](#the-standard-library)
20. [Calling a Rust crate](#calling-a-rust-crate)
21. [CPython escapes](#cpython-escapes)
22. [Heavy work, timers and keys](#heavy-work-timers-and-keys)
23. [Working with type checkers](#working-with-type-checkers)
24. [Headless runs and the gate](#headless-runs-and-the-gate)
25. [Shipping](#shipping)
26. [A real app](#a-real-app)
27. [What does not work yet](#what-does-not-work-yet)

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

A value that never changes is not state at all: a module-level literal (`LIMIT = 10`, `NAMES = ["a", "b"]`) is a declaration, and handlers and views read it by name.

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
Method parameters can be int, float, str, bool, `list[...]` of those, value classes, and Enums, and they take keyword arguments and defaults as Python does.
A method annotated with a return type ends with `return <expression>`, and a handler reads what comes back (`Cart.count()`).
A view reads state rather than calling methods, so the read-only form is a `@property`: a name for a formula over the fields, usable wherever a field is.

```python
    @property
    def label(self) -> str:
        return f"{len(Cart.items)} items"

    @staticmethod
    def yen(n: int) -> str:
        return f"¥{n}"
```

A `@staticmethod` is a plain function that happens to live in the class; views may call it, like any pure helper.

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

The element catalog: `text`, `link`, `button`, `text_field`, `number_field`, `int_field`, `checkbox`, `switch`, `slider`, `select`, `radio_group`, `tab_bar`, `segmented`, `column`, `row`, `grid`, `stack`, `spacer`, `divider`, `list_view`, `table`, `scroll_view`, `h_scroll_view`, `data_table`, `modal`, `image`, `svg`, `bar_chart`, `line_chart`, `progress`, `spinner`.
`grid(columns=, rows=)` lays equal tracks, and a button inside spans cells with `col_span=` / `row_span=` (`demo/calcgrid.py` is the keypad on one grid).
`data_table` draws the table itself: its first `row` child is the header, later `row` children are data rows shaded in alternation, and the columns line up when the cells of one column carry the same `grow` share (`demo/table.py`, where `align="right"` sets the numbers on their column's edge).
`spacer()` takes the space its row or column has left (`grow=` shares it between several), and `divider()` draws a rule across its parent — vertical inside a row, horizontal elsewhere.
`link("Docs", "https://…")` is a line of text that opens the URL in the browser; a headless `click:` on it is accepted and opens nothing.

`text` carries typography and a box of its own.
`bold=`, `italic=`, `mono=` and `underline=` are the typography; `wrap="nowrap"` or `wrap="ellipsis"` (with a `width=` to clip against) and `max_lines=` control the wrapping; `background=`, `padding=` and `border_radius=` paint a box behind the text, which is how a status pill is written (`demo/badges.py`).
Each of these takes what a style value takes anywhere else — a literal or a state read — so a pill's background can follow state.

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
    count: int = 1
    price: float = 2.5

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

    def set_count(self, n: int) -> None:
        self.count = n

    def set_price(self, p: float) -> None:
        self.price = p


checkbox("Dark mode", checked=Settings.dark, on_change=Settings.set_dark)
switch("Wi-Fi", checked=Settings.wifi, on_change=Settings.set_wifi)
slider(value=Settings.volume, min=0.0, max=10.0, step=1.0, on_change=Settings.set_volume)
select(options=Settings.fruits, selected=Settings.fruit, on_change=Settings.pick_fruit)
radio_group(options=Settings.fruits, selected=Settings.fruit, on_change=Settings.pick_fruit)
tab_bar(labels=Settings.tabs, active=Settings.tab, on_change=Settings.pick_tab)
segmented(options=Settings.fruits, selected=Settings.fruit, on_change=Settings.pick_fruit)
int_field(Settings.count, min=1, max=99, on_change=Settings.set_count)
number_field(Settings.price, min=0.0, max=100.0, step=0.5, on_change=Settings.set_price)
```

- **checkbox / switch**: a label and `checked=`. The handler receives the new bool. In verification scripts, `click:<label>` toggles.
- **slider**: `value=` plus `min=` / `max=` / `step=`. The handler receives the new float. The script verb is `slide:<value>` (clamped to the range, snapped to the step).
- **select / radio_group / tab_bar / segmented**: the list of options and the current position. The handler receives the chosen **index**. The script verb is `select:<label>`. `segmented` is the same contract painted as one joined pill group, the current segment filled in.
- **number_field / int_field**: a typed number. Typing reports nothing; `enter`, an arrow key or leaving the field commits — the text is parsed with Python's `float()` / `int()` rules, clamped into `min=` / `max=` (both 0 = no range), snapped to `step=`, and the handler runs only when the value changed. Text that is not a number is dropped, and the field shows the app's value again. In scripts, `input:<text>` commits in one step.
- **text_field**: the value and `on_change=`. `multiline=True` makes it a field that holds paragraphs — it wraps, `enter` writes a newline instead of submitting, the caret moves by visual line, and `rows=` says how many lines are visible.

Every element also takes `tooltip="…"`: the window shows it when the pointer rests there, and it is in the dump either way, so a verification script sees it.
Two more riders reach assistive technology: `role=` overrides the role an element derives (a screen reader's "button", "heading", "list" and so on) and `a11y_label=` is the name it is read by; the `a11y` step of a headless script prints that tree (`demo/labels.py`).

Switching tab content is a plain `if` / `elif` under the `tab_bar`.

## Handlers and control flow

Handlers can be passed in three forms:
a lambda (a tuple for multiple operations, `lambda: (a.set(x), b.set(y))`), a module-level def, and a store's bound method (`on_click=Cart.clear`).

A decorator compiles too. Decoration happens at import and the compiled app never runs the module, so the wrapper is folded into the handler it decorates:

```python
def announced(f):
    def wrapper():
        status.set("working")
        f()
        status.set("done")

    return wrapper

@announced
def save():
    fs.write_text(path, body())
```

The decorator is a def of one argument that either returns that argument or defines a wrapper calling it once.
A decorator that takes arguments of its own, or calls the function twice, or uses its value, is refused by name.

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

Available: `if` / `elif` / `else`, `while` (`while True:` included), `for` (over `range()`, list states, list fields, list-typed parameters), `break` / `continue`, and locals (reassignable, as in Python).
`log("…")` writes a line to stderr from either run, and `assert` / `raise` end the statement the way Python's exception does — the app keeps running.
Conditions take a bool directly (`if on:`), chain comparisons (`0 < n < 10`, the middle read once), and bind with `:=`.
A conditional expression (`a if c else b`) is written in a handler, over int, float, str or bool.
A pure helper (parameters and return annotated, body ending in `return expression`) is callable from handlers and from view text; it may return early from a branch, call itself, take `list[...]` parameters and default arguments, and return a value class or a list.

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

## Strings

Strings work as they do in Python: the methods, the length, indexing and slicing, `in`, and the conversions.

```python
name.set(raw().strip().upper())
parts.set(raw().split(","))
name.set(", ".join(parts()))
first.set(raw()[0] + raw()[1:4])          # a code point, then a slice
n.set(len(raw()) + raw().find("a"))
if "ada" in raw().lower():
    tag.set("found")
n.set(int("42") + int(2.5) + round(2.5))  # round-half-to-even, as Python does
```

As with Python's arithmetic, the two runs use different code here — CPython's own method while you develop, a Rust twin written to answer exactly the same thing once compiled, failures included, so `int("x")` stops that statement in both — and the gate is what holds them together.

Format specs are Python's, in views and in handlers alike.

```python
text(f"{total():,}")            # 1,234,567
text(f"{ratio():.1%}")          # 12.5%
text(f"{name():>10}")           # right-aligned in ten columns
text(f"{value():.2e}")          # 1.50e+00
```

## Lists, charts, virtualized lists

Append to a list by concatenating and putting it back.
The shipped app compiles this to a one-element append, so there is no copy cost.

```python
items.set(items() + [x])     # append
items.set([])                # clear
len(items())                 # count
```

The rest of Python's list vocabulary works in handlers: `in`, slices, `sorted` / `reversed` / `min` / `max` / `sum`, comprehensions, `enumerate` and `zip`, a stepped `range`, and joining two lists.
A local list carries its element type in the annotation, which is what the compiled side reads.

```python
out: list[str] = []
for i, s in enumerate(items()):
    if s != "":
        out = out + [f"{i}: {s}"]
items.set(sorted(out))
best.set(max(scores()))
```

Indexing reads an element, with Python's meaning: a negative index counts from the back, and an index past the end stops that statement in both runs.

```python
first.set(names()[0])        # a state read, indexed
tail.set(names()[-1])        # last element (too short: the statement aborts)
for i in range(len(Cart.items)):
    Cart.items[i] = "-"      # `self.xs[i]` inside the store says the same
```

Charts draw lists of float or int.

```python
values: State[list[float]] = State([])
line_chart(values(), height=120.0)
bar_chart(Metrics.svc_reqs, labels=Metrics.svc_names, height=100.0)
bar_chart(Books.profit, labels=Books.months, axis=True)          # negative months hang below the zero line
line_chart(series=Traffic.lines, colors=["accent", "#f38ba8"], axis=True)
```

The range spans the data and always contains zero, so a negative value hangs below the zero line; `min=` / `max=` pin it instead.
`axis=True` adds tick labels and gridlines.
`series=` takes a `list[list[float]]` field for several lines or bar groups, `colors=` names one color per series, and `color=` colors a single series (`demo/charts.py`).
`progress(value)` fills a track: `width=` / `height=` size it, `label=` captions it, and `indeterminate=True` sweeps a segment instead, for work with no known length.

Long lists go to `list_view`.
It is **virtualized**: the row builder `row(i)` is called only for the visible range (a dozen or so calls even at 100k rows).

```python
def row(i):
    return text(items()[i])

list_view(len(items()), row, item_height=22.0, height=200.0)
list_view(len(items()), row, item_height=22.0, grow=1.0)   # fill the parent's remaining height
```

A table is a `list_view` with a header and column tracks.
`table(columns, count, row)` calls `row(i)` for the visible rows only, and the builder returns a `row` of one cell per column; `widths=` are the tracks' shares.
`selected=` tints a row and `on_select` receives the clicked row's index; `sort=` / `descending=` draw the header's arrow and `on_sort` receives the clicked column's index — the app re-sorts its own lists.
In scripts, `select:<first cell>` picks a row and `click:<column>` sorts (`demo/roster.py`).

```python
def cells(i: int):
    return row(text(Roster.names[i]), text(f"{Roster.scores[i]}"))

table(["member", "score"], len(Roster.names), cells, widths=[2.0, 1.0],
      selected=Roster.sel, on_select=Roster.pick,
      sort=Roster.sort_col, descending=Roster.desc, on_sort=Roster.sort_by, grow=1.0)
```

The row index is an int the row can use anywhere: in the text, in a condition, and in the row's own handlers.

```python
def line(i):
    with row(spacing=6):
        text(f"{i + 1}. {items()[i]}")
        if i == Sel.idx:
            text("*")
        button("delete", on_click=lambda: Sel.drop(i))

list_view(len(items()), line, item_height=24.0, height=200.0)
```

## Dicts

Read with `.get`, write per key, count with `len`, iterate with `sorted()`.
A key is any str the app can name — a literal, a state read, a loop variable.

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
Arms take guards and `|` alternatives, and a guard that fails falls through to the arms below it, as Python's does.

```python
match health():
    case Degraded(services) if services > 3:
        text("badly degraded")
    case Healthy() | Degraded(_):
        text("fine enough")
    case _:
        text("down")
```

## Optional and Enum

Optional works in state and in fields (`last: int | None = None`).
Narrowing is as shown in the walrus sections.

Enums are ordinary `class Mood(Enum)` and compile as-is.
`match` arms are `Mood.MEMBER` or `_`, and missing arms are reported.
In text they render exactly as Python does: `Mood.HAPPY`.
`.name` and `.value` read what Python reads (`auto()` counts from 1), and `for m in Mood:` walks the members in declaration order.

`match` also takes int, float, str and bool values, with `|` alternatives and guards:

```python
match code():
    case 0 | 1:
        note.set("early")
    case n if n > 100:
        note.set("far")
    case _:
        note.set("middle")
```

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

A component can also take a callback or a `State` cell, which is how a child talks back to the caller.

```python
@component
def field(label: str, cell: State[str]):
    with row(spacing=6):
        text(label)
        text_field(cell(), on_change=cell.set)

field("name", name)
field("city", city)
```

A handler and a cell live in the caller, so a component that takes one becomes a view per call site — two calls that pass the same thing share one.

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

A style value can come from state as well as from a literal: `size=zoom()`, `color=Look.tone`, `padding=Look.pad * 2`.
The view re-reads it after every event, like everything else it shows.

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

Use it with `from yokan import fs, sqlite, http, math, json, time, strings, random, clipboard, notify`.
Each one calls the same function, implemented in Rust, during development and after shipping alike.
The shipped binary needs no Python.
Call them from handlers (views stay pure).

- **fs**: `read_text` / `write_text` / `append_text` / `exists` / `read_text_or` / `list_dir` (the names in a directory, sorted) / `make_dir` / `remove` / `app_dir(name)` (the directory this app may keep its own files in, created if it is not there yet)
  — plus the platform's own panels, `open_dialog(title)` and `save_dialog(name)`, which answer with a path or `""` when the person cancelled. A dialog waits for a person, so it runs inside `task(...)`; a verification script answers it with `file:<path>`.
- **sqlite**: `exec` / `query_text` / `query_int` / `query_rows` / `query_int_or` / `query_text_or` / `query_rows_or` (SQLite bundled. `query_text` answers column 0 of each row, `query_rows` every column. Wrap aggregates in COALESCE and pin the order with ORDER BY)
- **http**: `get_text(url)` / `get_text_or` / `get_text_with(url, headers)` / `post_text(url, body)` / `post_text_or` / `status(url)` (synchronous; `get_text` takes a deadline in milliseconds as a second argument, `post_text` a content type as a third)
- **math**: `sqrt` / `sin` / `cos` / `pow` / `fabs` / `floor` / `ceil` / `pi`
- **json**: `get_text` / `get_int` / `get_float` / `get_bool` / `length` / `has` (looked up by dotted paths like `"items.0.title"`), and `dumps(value)`, which writes a str, int, float, bool, a list of one of those, or a dict with str keys — a dict in key order
- **time**: `now_ms`, `format_ms(ms, "%Y-%m-%d")` (UTC. In verification scripts, pass a fixed ms), `format_local_ms(ms, fmt)` (the machine's own zone, from the same zone database in both runs), `local_offset_minutes(ms)`, `sleep_ms(ms)` (blocking; inside `task` the compiled run awaits it)
- **strings**: `to_int(s, default)` / `to_float(s, default)` (numeric parsing where broken input becomes the default)
- **random**: `seed(n)` / `int(lo, hi)` (inclusive on both ends) / `float()` (seed it and the sequence repeats)
- **clipboard**: `set_text(s)` / `get_text()` — the system clipboard. A window exchanges it with every other application; a headless run keeps it to itself, so a copy and a paste are checked like any other interaction
- **notify**: `send(title, body)` — an OS notification, delivered through Notification Center when the app runs as an `.app` bundle (`--app`); a bare dev run and headless runs drop it quietly

Every sqlite call takes one more argument, a list of values to bind:

```python
sqlite.exec(DB, "INSERT INTO expenses VALUES (?, ?, ?)", [item, str(yen), cat])
sqlite.query_int_or(DB, "SELECT COALESCE(SUM(amount),0) FROM expenses WHERE cat=?", 0, ["food"])
```

Write `?` where the value goes and pass it beside the statement.
An apostrophe in `item` is then an apostrophe, and text a user typed can never become SQL.
Values bind as text and the column's affinity converts, so an INTEGER column stores the number.

A whole row comes back as a `list[str]`, so a result is a `list[list[str]]`:

```python
@store
class Ledger:
    raw: list[list[str]] = []
    rows: list[str] = []

    def load(self) -> None:
        self.raw = sqlite.query_rows_or(DB, "SELECT name, amount, cat FROM expenses ORDER BY rowid")
        self.rows = []
        for r in self.raw:
            self.rows = self.rows + [f"{r[0]}  ¥{r[1]}  ({r[2]})"]
```

The line is written in Python rather than assembled in SQL.

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

Annotate every parameter and the return: int, float, str, bool, `list[...]` and `dict[str, ...]` of those, a value class, and `T | None`.
Compiled extensions like numpy work inside escapes.

## Heavy work, timers and keys

Never block a handler (the window freezes).
`task` does the work on a worker thread and runs the continuation on the UI thread when it finishes.

```python
def start():
    busy.set(True)
    task(fetch_data, on_done=lambda v: (busy.set(False), data.set(v)))
```

`on_error=` runs during development only; a failing standard-library call is caught with `try` / `except` around the call itself.

`work` must not build UI elements; it just returns a value, and `task` is the last statement of its handler (in Python the statements after it run before the work finishes).
Headless runs wait for task completion before taking the next step, so flows containing tasks are testable.
Both runs do the same thing with it: during development the work is a Python thread, and the compiled app awaits the standard-library calls inside it, which is what puts them on the engine's pool.
Pure computation inside a task stays where it is written — what moves off the UI thread is the `fs`, `sqlite`, `http` or `time.sleep_ms` call.

`every(seconds, cb)` is a timer, declared at module level (or under the `__main__` guard) and started with the app.

```python
def tick():
    n.set(n() + 1)

every(1.0, tick)
```

It is a declaration, not a call you make later: both runs start it when the app starts, and both fire it off the same clock — a frame in a window, an `advance:<ms>` in a headless script, so a minute of ticks is gate-checkable.

Keys are declared the same way.
`shortcut(chord, handler)` binds a chord, and `on_key(handler)` sees every key as the chord it was.

```python
def save():
    fs.write_text(path, body())

shortcut("cmd+s", save)
on_key(lambda k: last.set(k))
```

The chord is spelled the way the platform spells it — `cmd+s`, `shift-tab`, `ctrl+alt+k` — and `-` reads the same as `+`.
While a text field has the caret, plain keys go on typing into it and only chords carrying cmd or ctrl reach the app.
A headless script presses one with `key:cmd+s`, so a shortcut is a checked interaction like a click.

`menu_item(menu, name, handler)` puts the same handler in the application's menu bar.

```python
menu_item("File", "Save", save)
menu_item("File", "Clear", clear)
```

Declaration order is menu order, the window hands the bar to the platform, and a script picks an item by name with `menu:Save`.

`on_file_drop(handler)` is the same kind of declaration for a file dragged onto the window: the handler receives its path, and a script drops one with `drop:<path>`.


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

The step vocabulary is `click[@n]:<label>` (a button, a link, or a table's column header), `input[@n]:<text>`, `submit[@n]`, `slide[@n]:<value>`, `select[@n]:<label>` (a chooser's option, or a table's row by its first cell), `advance:<ms>`, `theme:light|dark`, `a11y`, `mem`, `dump`.
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
Every one of them passes the gate, except the two that hold state in a dict (`run(state={...})`) — those are development-only by design, and the gallery says so on each.

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
- **Reading a local assigned in only one branch.** Had that branch not run, Python would raise NameError. Assign in both if and else and it reads fine.
- **Negative exponents on `int ** int`.** The result's type would change at runtime; make either side a float and it can be written.
- **Compiling dict state (`run(state={...})`).** It runs during development, but the compiled truth is typed `State`.
- **Calling Protocol-bound helpers from views** (handlers can call them).
- **Calling value-class methods from views** (handlers can; views read fields).
- **Calling a store or model method from a view.** Building the screen only reads state, and a method may write to it; the read-only form is a `@property`, which a view reads like a field.
- **Iterating a list of models directly in a view.** Today, assemble the display strings on the store side and hand them to `list_view`.
- **A `Weak` field on a store.** A store is an owner; the non-owning reference belongs on the model side (the back pointer).
- **Type names the native side already uses, such as `Vec`.** Refused by name; pick another (`V2`, say).
- **Statements at module level.** The compiled app reads the module's declarations (imports, `State`, classes, defs, `style()`, type aliases, literal constants, `every(...)` timers, the `__main__` guard) and never executes it, so a `count.set(5)` or a `fs.write_text(...)` outside a function is refused by name. Startup work goes in a def passed as `run(view, on_start=setup)`.
- **Starting a timer from a handler.** A timer is a declaration (`every(1.0, tick)` at module level), so what a handler changes is what the tick reads.
- **`task`'s `on_error=`.** The failure path waits on the error union; catch a failing standard-library call with `try` / `except` around the call.
- A component's `local` is **identified by call site**. Reordering the calls reassigns the states.
- Placing the same element object **twice**. Constructors consume their children.
- **A method that returns `T | None`.** Scalars, lists, value classes and enums come back from a store or model method; an Optional return is not in the dialect yet.
- **A local dict**, and a local list without an annotation (`out: list[str] = []` says what the compiled side needs to know).
- **str methods beyond the common set**: `.title()`, `.zfill()`, `.format()`, `.encode()` and the rest. `.upper()`, `.lower()`, `.strip()` / `.lstrip()` / `.rstrip()`, `.split()`, `.join()`, `.startswith()`, `.endswith()`, `.replace()`, `.find()`, `.count()`, `len(s)`, `s[i]`, `s[a:b]` and `in` are in.
- **Format specs beyond fill, align, sign, width, `,`, precision and `d` / `f` / `e` / `%` / `s`** (`#`, `b` / `o` / `x`, `n`, `g`).
- **Iterating a dict's `.values()` / `.items()`.** Python walks them in insertion order, the compiled dict by key; iterate `sorted(d())` and read `d().get(k, default)`.
- **Some control flow**: nested defs (a closure has no compiled shape — define helpers at module level) and a conditional expression in a view (branch the elements with `if` there).
- **A component parameter that is a value class or an enum**, and a body that is not one container (a top-level `if`, or several elements — wrap them in a `column`). Callback and State parameters work: a component that takes one becomes a view per call site.
- **`tuple` and `set`.** A tuple has no compiled shape yet; a Python set iterates in an order the compiled side would not reproduce, so it is refused rather than reordered. A `list` covers both today.
- **`@py` signatures beyond scalars, lists, str-keyed dicts, value classes and Optionals** (models, nested containers).
- **`print`.** It writes to stdout, which is where a headless run's screen dump goes; `log("…")` writes the same line to stderr in both runs.
- **In the standard library**: reading a time back from text, file metadata (size, times) and copying or renaming, streaming or binary downloads, and nested json writing (a value inside a written dict or list is a str, int, float or bool).
- **Around the new elements**: a table's columns cannot be resized by dragging, and its rows have no keyboard navigation or multi-select; charts have no hover readout and no legend; `select` has no keyboard operation; a tooltip's appearance is not something a script can hover for (its text is in the dump). Each waits on a verb the headless harness does not have yet.
- **A second window.** One app, one window today: the engine's window root is written for a single view, and a headless run's dump is that one tree. Shortcuts, the clipboard, the menu bar, file dialogs, dropped files, tooltips and the multi-line field are all in.
- **Decorator shapes beyond a plain wrapper**: one that takes arguments of its own, one whose wrapper calls the function twice or uses its value. A decorator that returns the function, or a wrapper calling it once, compiles.
- **At the Rust-crate boundary, payload-carrying enums and methods on a twin do not cross yet.** Scalars, String, Lists, Optionals, str-keyed dicts, structs (nested and width-annotated fields included), enums, and Result (compound returns too) all do. The two that remain each wait on something specific: payload enums on rpi-gen itself, methods on impl-splicing onto an rpi-declared struct. Enum- or list-typed fields inside a struct stay out too; every call outside the set is refused with a named reason.
- All measurements are macOS/arm64. Other platforms are not measured yet.

This list is updated every time a design lands.
The design principles behind it are collected in [DESIGN.md](DESIGN.md).
