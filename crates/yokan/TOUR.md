# Yokan language tour

Yokan is a compiler for desktop apps: it takes a statically typed
subset of Python to native code.
This tour is one pass over how the apps are written.
Every piece of code in this tour runs as-is on today's tree.
What Yokan cannot do yet is collected, with reasons, at the end
([What does not work yet](#what-does-not-work-yet)).

A Yokan app is an ordinary Python file.
During development it runs on real CPython; when you ship, the same source compiles to a native binary.
Whether the two behave the same is checked by `yokan gate`, which replays a script of clicks and keystrokes through both and compares the screens byte for byte ([Headless runs and the gate](#headless-runs-and-the-gate)).

## Table of contents

1. [The smallest app](#the-smallest-app)
2. [Holding state](#holding-state)
3. [Writing views](#writing-views)
4. [Form controls](#form-controls)
5. [Handlers and control flow](#handlers-and-control-flow)
6. [Arithmetic](#arithmetic)
7. [Strings](#strings)
8. [Lists, charts, virtualized lists](#lists-charts-virtualized-lists)
9. [The canvas](#the-canvas)
10. [Dicts](#dicts)
11. [Tuples](#tuples)
12. [Value classes and interfaces](#value-classes-and-interfaces)
13. [Memory](#memory)
14. [Sum types and match](#sum-types-and-match)
15. [Optional and Enum](#optional-and-enum)
16. [Components](#components)
17. [Shared properties](#shared-properties)
18. [Styles and themes](#styles-and-themes)
19. [Animation](#animation)
20. [The window](#the-window)
21. [Error handling](#error-handling)
22. [The standard library](#the-standard-library)
23. [Calling a Rust crate](#calling-a-rust-crate)
24. [CPython escapes](#cpython-escapes)
25. [Heavy work, timers and keys](#heavy-work-timers-and-keys)
26. [Working with type checkers](#working-with-type-checkers)
27. [Testing](#testing)
28. [Headless runs and the gate](#headless-runs-and-the-gate)
29. [Shipping](#shipping)
30. [A real app](#a-real-app)
31. [What does not work yet](#what-does-not-work-yet)

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

`yokan init app.py` writes this file, with the title taken from the file name.
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

A **store** (`@store`) holds fields and methods together.
There is one instance of it, and the class name is that instance: `Cart.add(...)` calls, `Cart.total` reads.
An app can have as many stores as it likes, and they can call each other's methods (`demo/stores.py`).

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

The elements, by what they are for:

- **Arranging**: `column`, `row`, `grid`, `stack` (children on top of each other), `spacer`, `divider`.
  `grid(columns=, rows=)` lays equal tracks, and a child spans cells with `col_span=` / `row_span=` (`demo/calcgrid.py`).
  `spacer()` takes the space its row or column has left (`grow=` shares it between several); `divider()` draws a rule across its parent, vertical inside a row.
- **Input**: `button`, and the [form controls](#form-controls).
- **Showing**: `text`, `link`, `image`, `svg`, `progress`, `spinner`, `bar_chart`, `line_chart`.
  `link("Docs", "https://…")` opens the URL in the browser; a headless `click:` on it opens nothing.
- **Showing many**: `list_view`, `table`, `data_table`, `scroll_view` / `h_scroll_view`.
  `data_table`'s first `row` child is the header and the rest are data rows shaded in alternation; columns line up when the cells of one column carry the same `grow` (`demo/table.py`).
- **Layering**: `modal`.

`text` carries typography and a box of its own.
The typography is `bold=`, `italic=`, `mono=` and `underline=`.
The wrapping is `wrap="nowrap"` or `wrap="ellipsis"` (with a `width=` to clip against) and `max_lines=`.
The box is `background=`, `padding=` and `border_radius=`, which is how a status pill is written (`demo/badges.py`).
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
A decorator that takes arguments of its own, or calls the function twice, or uses its value, is refused.

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

The rest of Python's list vocabulary works in handlers: `in`, slices, `sorted` / `min` / `max` / `sum`, comprehensions, `enumerate` and `zip`, a stepped `range`, and joining two lists.
A local list carries its element type in the annotation, which is what the compiled side reads.

```python
out: list[str] = []
for i, s in enumerate(items()):
    if s != "":
        out = out + [f"{i}: {s}"]
items.set(sorted(out))
best.set(max(scores()))
```

The operations that say nothing about the element — `in`, a slice, `+`, `[::-1]` — take a list of anything the app can hold, a value class or a tuple included.
Comparing needs to know what to compare, so `sorted`, `min` and `max` take a `key=` — a lambda of one element, or the name of a helper that takes one — and `sorted` takes a `reverse=`.
Sorting is stable either way: elements with equal keys stay in the order they came in.

```python
by_score = sorted(players(), key=lambda p: p.score, reverse=True)
leader = max(players(), key=lambda p: p.score)
names = [p.name for p in players()]
newest = entries()[::-1]
```

`reversed(xs)` is Python's iterator, so it walks a list backwards in a `for`; where a list is wanted, `xs[::-1]` is the one that is a list in Python too.

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

## The canvas

A canvas is a grid of virtual pixels you paint command by command.
`width` and `height` count those pixels and `scale` says how many logical ones each of them takes, so `canvas(160, 120, scale=4)` occupies 640x480 on screen.
The commands go in the block.

```python
with canvas(160, 120, scale=4, background=0, palette=Game.palette):
    rect(Game.x, Game.y, 8, 8, 7)
    circle(30, 20, 4, 12)
    pixel_text(4, 4, f"SCORE {Game.score}", 7)
```

Every color is a **number**: the index of a color in `palette`, a list of hex colors the app declares.
Numbering the colors is how tools for pixel art work, so drawing code written for one moves here with its numbers unchanged.
An index past the end paints the last color, so an off-by-one is visible rather than invisible; a canvas with an empty palette paints magenta.

```python
@store
class Game:
    palette: list[str] = ["#000000", "#2b335f", "#7e2072", "#19959c"]
```

The commands are `pixel`, `line`, `rect`, `rect_outline`, `circle`, `circle_outline`, `triangle`, `triangle_outline`, `sprite` and `pixel_text`.
Coordinates are whole numbers — a pixel grid has no half pixels, so a float is refused and asks for `int(...)`.
`sprite(x, y, source, u, v, w, h)` copies a rectangle of a PNG onto the canvas; `colkey=` is the palette index that is not copied, and `flip_x=` / `flip_y=` mirror it.
`pixel_text` writes in the canvas's own 4x6 font, on the pixel grid.

A `for` inside the canvas is the ordinary loop: what its body paints joins the frame where it stands.

```python
with canvas(160, 120, scale=4, palette=Game.palette):
    for e in Game.enemies:
        sprite(e.x, e.y, "assets/sheet.png", 0, 16, 8, 8, colkey=0)
```

It walks a list the view can read directly — a `State` cell, a store field, a model's own field — whose elements are scalars or value classes, and `for i, e in enumerate(...)` binds the index beside the element.
`for i in range(2):` works too, and is written out where it stands: the bounds are written-out numbers (up to 64 of them) because the loop becomes the elements it would have produced.
The same loops work in any container, not only in a canvas.

A drawing command is not an element: it takes none of the [shared properties](#shared-properties), nothing in a canvas can be clicked, and a canvas is one image in the accessibility tree — an `a11y_label=` on it is the only way to say what it paints.
What the dump prints is the frame itself, one command per line, so `yokan gate` compares what the two runs would have painted.

```console
Canvas(160x120, scale=4, bg=#000000)[
  Rect(56, 100, 8, 8, #eeeeee)
  PixelText(4, 4, "SCORE 1250", #eeeeee)
]
```

## Dicts

Read with `.get`, write per key, count with `len`, walk it like a Python dict.
A key is any str the app can name — a literal, a state read, a loop variable.

```python
prices["cherry"] = 200                 # per-key write
picked.set(prices().get("apple", -1))  # read: default when missing
if "cherry" in prices(): ...           # membership
len(prices())                          # count


def scan():
    for k in prices():                 # insertion order, as Python walks it
        last.set(k)
    for v in prices().values():        # the same order
        total.set(total() + v)
    for k in sorted(prices()):         # key order, when that is what you mean
        first.set(k)
```

A compiled dict remembers the order its keys went in, so a walk visits them in the order Python does.
Bare `d[k]` reads are refused: they raise `KeyError` when the key is missing, and `.get(key, default)` says what a missing key means.
`.items()` walks the pairs, in the same insertion order.

A dict of lists groups:

```python
groups: State[dict[str, list[str]]] = State({})

for w in words():
    groups[w[0]] = groups().get(w[0], []) + [w]
```

## Tuples

A tuple puts several values together as one, written and read the way Python writes one.

```python
pair: State[tuple[str, int]] = State(("momo", 4))
rows: State[list[tuple[str, int]]] = State([])


def measure(word: str) -> tuple[str, int]:
    return (word.upper(), len(word))


def scan():
    label, n = measure("hello")          # unpacking
    first = pair()[0]                    # a part, by a literal position
    whole, rest = divmod(n, 3)
    for name, count in rows():           # a pair per row
        total.set(total() + count)
    for key, value in prices().items():  # and a dict walks as pairs
        seen.set(seen() + key)
```

The parts have types of their own, so a tuple is indexed by a literal position: a computed index would have no one type to be.
Two parts or more, and the same shape can be a state, a field, a list's element, a parameter and a return.

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
`check` warns in its place: when the field types close a loop through two or more model classes, and when a handler writes the round trip (`a.kid = b`, then `b.parent = a`).

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

## Shared properties

Every element also takes these shared properties, under the same names and with the same meaning:

- **`tooltip="…"`**: shows a line when the pointer rests there, and it is in the dump either way, so a verification script sees it.
- **`role=` / `a11y_label=`**: `role=` overrides the role an element derives (a screen reader's "button", "heading", "list" and so on), and `a11y_label=` is the name it is read by; a headless script's `a11y` step prints that tree (`demo/labels.py`). A `checkbox`, a `switch` and a `progress` are named by their own label, so they take no `a11y_label=`.
- **`disabled=True`**: dims an element and makes it inert. The window does not press it, a script step aimed at it is accepted and does nothing, and the dump shows the state.
- **`width=` / `height=` / `min_width=` / `max_width=`**: size it. An element with its own `width=` / `height=` (`button`, `image`, `svg`, `text`, the charts, `progress`) keeps those.
- **`theme=`, `animate=` / `easing=` / `enter=` / `exit=`, `col_span=` / `row_span=`**: covered under [Styles and themes](#styles-and-themes), [Animation](#animation) and `grid` respectively (`demo/shared.py`).

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
`padding=` is the inset between the window and your tree: 16 px unless you say otherwise, and `padding=0.0` lets the app paint to the window's edge — which is what an app that IS a picture wants, a canvas or a map.
The declaration is baked into the compiled binary as well.
`quit()` asks the window to close, from any handler.
A headless run has no window to close, so a script runs its remaining steps and both runs print the same dumps — which is why a quit is not something the gate can check.
`on_start` is a handler that runs once right after mount, and a failure prints and continues (use it for loading startup data or seeding the RNG).
It is also the only place for startup work: module level holds declarations, and a statement there (`count.set(5)`, say, or `fs.write_text(...)`) is refused, because the compiled app reads the module and never executes it.

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

It comes in two halves, told apart by where the name comes from.
Neither half puts Python in the shipped binary.
How far each module reaches into Python's, function by function, is on the [coverage page](https://i2y.github.io/yokan/support/), which is generated rather than written.

**Python's own modules**, written the way Python writes them: `import math`, `import random`, `import statistics`, `import json`, `import datetime`, `import time`, `import re`, `import string`, `import textwrap`, `import bisect`, `import heapq`, `import collections`, `import itertools`.
During development the app imports CPython's module and CPython runs it.
The shipped binary calls a twin written against CPython's semantics.
What it answers, how it fails and the wording of the error are all the same.
`math.sqrt(-1)` raises where Python raises; `statistics.mean([0.1, 0.2, 0.3])` is `0.2`, the exact answer, not the `0.20000000000000004` a plain sum gives; `random.seed(1)` starts the same Mersenne Twister sequence in both runs; `json.dumps` writes what CPython writes, down to the `", "` between the parts and the `\uXXXX` escapes; a `date` adds a `timedelta`, subtracts another date and formats itself the way Python's does; and a regular expression is compiled by CPython itself while the app translates, so the shipped binary runs the very array Python would have run — the backtracking, the groups and the flags are CPython's, not a second dialect of them.

```python
import json, math, random, re, statistics
from datetime import date, timedelta


def measure():
    hyp.set(math.sqrt(3.0 * 3.0 + 4.0 * 4.0))     # 5.0
    spread.set(statistics.stdev([1.5, 2.5, 4.75]))
    random.seed(42)
    roll.set(random.randint(1, 6))
    doc.set(json.dumps({"name": "momo", "tags": ["a", "b"]}))
    due.set(date(2026, 1, 1) + timedelta(weeks=6))  # 2026-02-12, a Thursday
    mail.set(re.findall(r"\w+@[\w.]+", line())[0])   # a Match has no shape here


def view():
    text(f"circumference: {math.tau * r():.3f}")   # pure, so a view may ask
```

`math` and `statistics` are pure, so a view can call them; `random` moves a generator on, so it belongs in a handler like the rest.
An unseeded generator is as unrepeatable here as it is in Python — seed it and the gate can hold the two runs to one sequence.

**Yokan's own modules** cover files, a database, the network, the clipboard, notifications, sound and the keyboard: `from yokan import fs, sqlite, http, jsondoc, clock, strings, clipboard, notify, audio, keys`.
Python can do most of this too, through `pathlib`, `sqlite3` and `urllib`.
What differs is how many implementations there are.
`math` and `re` are CPython's modules during development and Rust twins after shipping.
These call the same Rust function in both runs, so the two runs cannot answer differently.
None of them uses a Python module's name, because a Python name is a promise that CPython decides the answers.
Reading a JSON document by a dotted path is `jsondoc`, not `json`, and the machine's own time zone is `clock`, not `time`.
Call them from handlers (views stay pure).

- **fs**: `read_text` / `write_text` / `append_text` / `exists` / `read_text_or` / `list_dir` (the names in a directory, sorted) / `make_dir` / `remove` / `app_dir(name)` (the directory this app may keep its own files in, created if it is not there yet)
  — plus the platform's own panels, `open_dialog(title)` and `save_dialog(name)`, which answer with a path or `""` when the person cancelled. A dialog waits for a person, so it runs inside `task(...)`; a verification script answers it with `file:<path>`.
- **sqlite**: `exec` / `query_text` / `query_int` / `query_rows` / `query_int_or` / `query_text_or` / `query_rows_or` (SQLite bundled. `query_text` answers column 0 of each row, `query_rows` every column. Wrap aggregates in COALESCE and pin the order with ORDER BY)
- **http**: `get_text(url)` / `get_text_or` / `get_text_with(url, headers)` / `post_text(url, body)` / `post_text_or` / `status(url)` (synchronous; `get_text` takes a deadline in milliseconds as a second argument, `post_text` a content type as a third)
- **jsondoc**: `get_text` / `get_int` / `get_float` / `get_bool` / `length` / `has` — reads into a JSON document by a dotted path like `"items.0.title"`, which Python's `json` has no verb for. Writing is Python's `json.dumps`.
- **clock**: `format_ms(ms, "%Y-%m-%d")` (UTC. In verification scripts, pass a fixed ms), `format_local_ms(ms, fmt)` (the machine's own zone, from the same zone database in both runs), `local_offset_minutes(ms)` — the machine's zone, which Python's `time` reaches only through a struct. Reading the clock is Python's `time`, and calendar work is Python's `datetime`.
- **strings**: `to_int(s, default)` / `to_float(s, default)` (numeric parsing where broken input becomes the default)
- **clipboard**: `set_text(s)` / `get_text()` — the system clipboard. A window exchanges it with every other application; a headless run keeps it to itself, so a copy and a paste are checked like any other interaction
- **notify**: `send(title, body)` — an OS notification, delivered through Notification Center when the app runs as an `.app` bundle (`--app`); a bare dev run and headless runs drop it quietly
- **audio**: `play(path, volume=1.0)` / `stop()` — a WAV starts and the call returns; several play together, and `volume` is a level between 0 and 1 (loud is the one mistake a sound cannot take back, so something that plays several times a second should ask for less). A SCRIPTED run is silent, so a gate never needs a machine with speakers and a sound never reaches a dump; a machine with no audio device, or a file that cannot be read, plays nothing rather than failing the app. Only an app that imports this links a sound device, which is about 1.3 MB of binary
- **keys**: `down(k)` / `pressed(k)` / `released(k)` — the keyboard as a device, read from a timer's tick; see [The window](#the-window) for the chords that come to a handler instead

How far the Python half reaches, module by module:

- **math** — everything but six members, each refused with its reason: `prod` and `sumprod` answer an int or a float depending on the list, and `gamma`, `lgamma`, `erf` and `erfc` are computed by CPython itself rather than by the platform.
- **random** — `seed`, `random`, `randint`, `randrange`, `getrandbits`, `uniform`, `gauss`, `choice`, `sample`.
- **statistics** — `mean`, `fmean`, `median`, `mode`, `variance`, `pvariance`, `stdev`, `pstdev`, over `list[float]`. A list of ints is refused: CPython answers an int for `mean([1, 2, 3])` and a float for `mean([1, 2, 4])`, so there is no one type it could have.
- **json** — `dumps`, with CPython's defaults and no keyword arguments.
- **time** — `time`, `time_ns`, `monotonic`, `monotonic_ns`, `perf_counter`, `perf_counter_ns`, `sleep`.
- **re** — `findall`, `sub`, `split`, `escape`, and `re.search(p, s) is not None` (with `match` and `fullmatch`) as the test. The pattern is a literal, because it is compiled while the app translates.
- **datetime** — `date`, `datetime` and `timedelta`, all of them naive: construction, `today` / `now` / `fromisoformat` / `fromtimestamp` / `fromordinal` / `combine`, the parts (`.year`, `.hour`, `.days`, …), `isoformat`, `strftime`, `weekday`, `toordinal`, `timestamp`, `total_seconds`, arithmetic and comparison. A value renders in a hole the way `str()` renders it.
- **collections** — `Counter`, over a list of str: the dict of counts, keyed in first-seen order, with `.most_common()` and `.total()` beside everything a dict answers. A Counter held in a `State` reads back as the dict it is, so take the counts out before storing it.
- **itertools** — `chain`, `pairwise`, `accumulate`, `combinations`, `permutations`, `product`. Each answers an iterator in Python, so each is what a `for` walks here.
- **string / textwrap / bisect / heapq** — the nine constants; `dedent` and `indent`; `bisect_left` and `bisect_right`; `nsmallest` and `nlargest`.

```python
c = Counter(votes())                       # {"ivy": 3, "momo": 2, "ada": 1}
for name, n in c.most_common(2):           # by count, ties in first-seen order
    board.set(board() + f"{name}:{n} ")

for a, b in itertools.pairwise(readings()):
    steps.set(steps() + [b - a])
```

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

`yokan gate` and `yokan build` set the crate up for both runs.
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
Anything that cannot cross is refused, and the error says what and why.
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
It must not read app state either — it runs off the UI thread, where state cannot be reached: read what it needs above the task, hand the value in, and write what comes back in `on_done`.
Headless runs wait for task completion before taking the next step, so flows containing tasks are testable.
Both runs do the same thing with it: during development the work is a Python thread, and the compiled app awaits the calls inside it, which is what puts them on the engine's pool.
Pure computation stays where it is written — what moves off the UI thread is the `fs`, `sqlite`, `http` or `time.sleep` call, and a `@py` escape, which is how a minute of Python keeps the window drawing.

The work is not silent while it runs.
`report(fraction, note)` says where it has got to — from the work itself, or from inside a `@py` escape it called — and `on_progress` hears it on the UI thread like any other handler.

```python
def moved(fraction: float, note: str):
    done.set(fraction)
    step.set(note)


def start():
    task(count_primes, on_done=counted, on_progress=moved)
```

Inside an escape, `report` is imported the way anything else is: `from yokan import report`.
Every report is heard, and the last one lands before `on_done` does; called outside a task it does nothing.
What a report *says* is the work's business and may depend on the machine, but that it arrives is not — so counting them is something the gate can compare.

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

A chord is a message; a key that is *held* is something else, and `keys` answers that.

```python
from yokan import keys

def tick():
    if keys.down("left"):
        Game.steer(-1)
    if keys.pressed("space"):
        Game.fire()

every(0.033, tick)
```

`keys.down(name)` is "held right now", `keys.pressed(name)` is "went down since the last tick" and `keys.released(name)` its opposite.
A name is one bare key — `left`, `space`, `z` — and the modifiers answer under their own names (`shift`, `cmd`, `ctrl`, `alt`), so `down("left")` is true whether or not shift is held with it.

Read them from a tick, not from a view: a view is rebuilt on the framework's schedule, so what it read there would be a moment the app never chose (the dialect refuses it for the same reason it refuses a clock in a view).
What `pressed` and `released` saw is spent by the tick that read it, so holding a key fires once however many frames it stays down.
A script presses one with `keydown:left` and lets go with `keyup:left`, and `key:<chord>` is both halves at once — which is why a game is gate-checkable frame by frame.

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
`yokan check app.py` answers that half: it checks every module the app imports, prints the first refusal in the `file:line:col` form, and says nothing when the app is inside the dialect.
No compiler is started, so it is the check to run while editing.

`check` also warns.
A warning is not a refusal: the app translates, and something in it is still worth changing.

```console
$ yokan check app.py
app.py:37:9: warning — these assignments make a reference cycle: b.parent → a.kid → b. The compiled run counts references and never frees a cycle (the CPython you develop on collects it), so write the reference that points back as `Weak[...]`.
            b.parent = a
            ^
```

Today's warnings are about memory.
A cycle is never freed in the compiled run while the CPython you develop on collects it, so the two runs differ.
No dump shows that difference, which makes it the one thing the gate cannot check for you.

Two shapes are caught.
One is the field types: strong references that close a loop through two or more model classes (`Kid.owner → Parent.kids → Kid`).
The other is a handler that writes the round trip (`a.kid = b`, then `b.parent = a`).
The second catches a model that references its own class, where the types alone cannot tell a ring from a list or a tree.

`--strict` turns a warning into a failure.

## Testing

An app is a Python module, so its tests are Python tests, in whatever runner you already use.
`yokan.headless(view, state, script)` runs the app against a script with no window and answers the screen as text — the dump before the steps, then the dump after — and a test asserts on that.

Between edits, `yokan show` is the same run from the command line, and it can leave a picture behind.

```console
$ yokan show app.py --script "keydown:left,advance:33,advance:33" --frames shots/ --scale 3
Column[Canvas(160x120, scale=4, bg=#000000)[
  Sprite(assets/sheet.png, 0,0 8x8 at 54,100)
  PixelText(4, 4, "SCORE 0", #eeeeee)
]]

3 frames in shots/
```

It refuses first (so a mistake costs a second, not a compile), runs without a window, prints the screen, and with `--frames` writes a PNG of each step's canvas — `--gif` assembles those into one file if ffmpeg is around.
No display is involved, so it works over ssh and in CI, and what it draws is the same rasterizer the window uses.
That is the fast half of the loop; `yokan gate` is the slow half, and it is the one that proves the shipped run agrees.

```python
# test_app.py
import app                       # the module; its run(...) is under __main__
from yokan import headless


def test_clicking_counts():
    out = headless(app.view, None, "click:+1,click:+1")
    assert "count: 2" in out


def test_typing_greets():
    out = headless(app.view, None, "input:Momo")
    assert "Momo" in out
```

```console
$ uv run --with pytest python -m pytest
2 passed
```

The script vocabulary is the one in the next section, so anything a person can do to the app is something a test can do: click, type, press a key, drop a file, let a second pass with `advance:1000`.
Handlers, store methods and value classes are ordinary Python too, so the parts that are only computation can be tested by calling them.

What a test like this checks is the development run, which is CPython.
Whether the shipped binary agrees is the other half, and that is what `yokan gate` answers — the same script through both runs, byte-compared.
The two are complementary: a unit test says the app does the right thing, and the gate says the compiled app does the same thing.
How `check`, `show` and the gate fit together into a loop an agent can work on its own is written up at [Building with an agent](https://i2y.github.io/yokan/agents/).

## Headless runs and the gate

Running without a window is where verification starts.

```console
$ PIXIE_SCRIPT="click:+1,input:Momo" uv run app.py
```

`PIXIE_SCRIPT` is an environment variable, and every Yokan app reads it — the development run and a release binary alike.
Set it and the app skips the window: it dumps the screen, replays the steps you listed, dumps the screen again, and exits.
It is what `yokan gate` and `yokan show --script` set for you, so one script text works in all three places.
The name is the substrate's because the code that reads it is: the harness belongs to [pixie](https://github.com/i2y/yokan/blob/main/docs/PIXIE.md), the layer Yokan compiles through, and is spelled the same way there.

The step vocabulary is `click[@n]:<label>` (a button, a link, or a table's column header), `input[@n]:<text>`, `submit[@n]`, `slide[@n]:<value>`, `select[@n]:<label>` (a chooser's option, or a table's row by its first cell), `key:<chord>`, `keydown:<key>` / `keyup:<key>`, `menu:<item>`, `file:<path>`, `drop:<path>`, `advance:<ms>`, `theme:light|dark`, `a11y`, `mem`, `dump`.
`@n` picks the n-th match in tree order, so a row of identical buttons is reachable (`click@2:delete`).
`dump` prints the screen at that point in the script, which is what makes an intermediate state checked and not just the first and last.
A comma inside text is written `\,` (`input:hello\, world`).
The screen tree is dumped to stdout before and after the steps, and from tests `yokan.headless(view, state, script)` returns the same string.

The **gate** replays the same script against the development build and the shipped build, and diffs the dumps.

```console
$ yokan gate app.py --script "click:+1,input:Momo" --release
GATE OK — 2 dump lines identical in both runs
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

The native build has one prerequisite: a Rust toolchain.
The crates it compiles against live in the repository, and the first native build fetches the checkout matching your version into `~/.cache/yokan/` — inside a checkout it uses that one, and `PIXIE_REPO` points it anywhere else.
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

What lies outside this range is refused, and never silently given another behavior.
A refusal names the file, line and column and quotes the line:

```console
$ yokan build app.py --release
widgets.py:5:40: not in the dialect — text() does not take `weight=`
        return text(label, size=12, weight=2)
                                           ^
```

What Yokan cannot do as of today, with the reason for each refusal:

- **Bare `d[k]` reads.** The read form is `.get(key, default)`, where the caller decides what a missing key means.
- **A local, a parameter or a loop variable that takes a field's name**, inside a store or model method. Python keeps `score` and `self.score` apart; the compiled side reads a field by its bare name, so the two runs would mean different things by it. Rename the local — `score_` reads the same in Python.
- **Reading a local assigned in only one branch.** Had that branch not run, Python would raise NameError. Assign in both if and else and it reads fine.
- **Negative exponents on `int ** int`.** The result's type would change at runtime; make either side a float and it can be written.
- **Compiling dict state (`run(state={...})`).** It runs during development, but the compiled truth is typed `State`.
- **Calling Protocol-bound helpers from views** (handlers can call them).
- **Calling value-class methods from views** (handlers can; views read fields).
- **Calling a store or model method from a view.** Building the screen only reads state, and a method may write to it; the read-only form is a `@property`, which a view reads like a field.
- **Iterating a list of models directly in a view.** A view's `for` walks a list of scalars or value classes; for models, assemble the display strings on the store side and hand them to `list_view`.
- **On a canvas**: no mouse, no tilemap and no camera offset; coordinates are whole pixels (a float is refused and asks for `int(...)`); the scale is a number the app declares rather than a fit to the window, because the painted size would then depend on a window the dump cannot see; and a sprite's PNG is found next to the app, so a missing one paints nothing. What a canvas paints is not readable by assistive technology either — it reports as one image, and `a11y_label=` is the honest way to say what is on it.
- **A `Weak` field on a store.** A store is an owner; the non-owning reference belongs on the model side (the back pointer).
- **Type names the native side already uses, such as `Vec`.** Refused; pick another (`V2`, say).
- **Statements at module level.** The compiled app reads the module's declarations (imports, `State`, classes, defs, `style()`, type aliases, literal constants, `every(...)` timers, the `__main__` guard) and never executes it, so a `count.set(5)` or a `fs.write_text(...)` outside a function is refused. Startup work goes in a def passed as `run(view, on_start=setup)`.
- **Starting a timer from a handler.** A timer is a declaration (`every(1.0, tick)` at module level), so what a handler changes is what the tick reads.
- **`task`'s `on_error=`.** The failure path waits on the error union; catch a failing standard-library call with `try` / `except` around the call.
- **Stopping a task once it has started.** `report` says where the work is; nothing says stop, so a task runs to its end.
- A component's `local` is **identified by call site**. Reordering the calls reassigns the states.
- Placing the same element object **twice**. Constructors consume their children.
- **A method that returns `T | None`.** Scalars, lists, value classes and enums come back from a store or model method; an Optional return is not in the dialect yet.
- **A local dict**, and a local list without an annotation (`out: list[str] = []` says what the compiled side needs to know).
- **str methods that answer something the dialect has no shape for**: `.encode()` (bytes), `.format()` and `.translate()` (a template or a table built at run time), `.casefold()` (its mapping expands `ß` to `ss`, which is a different Unicode table from the one the case methods use). What is in: `.partition()`, `.rpartition()`, `.upper()`, `.lower()`, `.title()`, `.capitalize()`, `.swapcase()`, `.strip()` / `.lstrip()` / `.rstrip()` (with or without a set of characters), `.split()`, `.splitlines()`, `.join()`, `.startswith()`, `.endswith()`, `.replace()`, `.find()`, `.rfind()`, `.index()`, `.rindex()`, `.count()`, `.zfill()`, `.ljust()`, `.rjust()`, `.center()`, `.expandtabs()`, `.removeprefix()`, `.removesuffix()`, the `.is…()` family, `len(s)`, `s[i]`, `s[a:b]` and `in`.
- **Format specs beyond fill, align, sign, width, `,`, precision and `d` / `f` / `e` / `%` / `s`** (`#`, `b` / `o` / `x`, `n`, `g`).
- **Some control flow**: nested defs (a closure has no compiled shape — define helpers at module level) and a conditional expression in a view (branch the elements with `if` there).
- **A component parameter that is a value class or an enum**, and a body that is not one container (a top-level `if`, or several elements — wrap them in a `column`). Callback and State parameters work: a component that takes one becomes a view per call site.
- **`set`.** A Python set iterates in an order the compiled side would not reproduce, so it is refused rather than reordered; a `list` covers it. A tuple is in — see [Tuples](#tuples) — but only where its shape is written out: a tuple that a Rust crate would have to answer is not carried yet, which is why `re.findall` still refuses a pattern with two groups or more.
- **`@py` signatures beyond scalars, lists, str-keyed dicts, value classes and Optionals** (models, nested containers).
- **`print`.** It writes to stdout, which is where a headless run's screen dump goes; `log("…")` writes the same line to stderr in both runs.
- **In Yokan's own modules**: file metadata (size, times) and copying or renaming, and streaming or binary downloads.
- **In Python's modules**: six members of `math` (each refused with its reason), `random`'s `shuffle` (it reorders a list in place, and a list lives in a `State` — take a new order with `random.sample(xs(), len(xs()))` and write it back) and its distributions beyond `gauss`, and `statistics` over a list of ints (its answer would be an int or a float depending on the values). From `datetime`: an aware value (`timezone`, `tzinfo`), `datetime.time`, `replace`, `strptime`, a `date` in a list or a dict, and a `date` as a helper's parameter. `strftime` takes the directives CPython gives a meaning of its own; `%c`, `%x`, `%X` and `%-d` are refused, because what they answer is the machine's business. `json.loads` is refused too: what it answers has no shape until it runs, so reads go through `jsondoc`'s paths, and a `json.dumps` of a value the app is holding reaches one level of nesting where a literal reaches any. From `re`: a `Match` (`re.search` used as a value), and a pattern built at run time — both refused, the second one pointing at `@py`. From the small modules: what rearranges a list in place (`heapq.heappush`, `bisect.insort`), because a list lives in a `State` here, and `textwrap.wrap` / `fill` / `shorten`, which split words with a regular expression of CPython's own. From `collections`: everything but `Counter` — `defaultdict` (what a missing key answers is asked at the read here), `deque` (it works in place, and a list lives in a `State`), `namedtuple` (a `@value` class says it with types), `OrderedDict` (a dict here already keeps its order) and `ChainMap`. From `itertools`: what never ends (`count`, `cycle`, `repeat`), what yields an iterator of its own (`groupby`, `tee`), what takes a function (`starmap`, `takewhile`, `filterfalse`) and `batched`, whose last tuple is a different shape from the rest. Modules that stay out for a reason the refusal names: `pathlib`, `os`, `decimal`, `hashlib`, `base64`, `zoneinfo`.
- **Around the new elements**: a table's columns cannot be resized by dragging, and its rows have no keyboard navigation or multi-select; charts have no hover readout and no legend; `select` has no keyboard operation; a tooltip's appearance is not something a script can hover for (its text is in the dump). Each waits on a verb the headless harness does not have yet.
- **A second window.** One app, one window today: the engine's window root is written for a single view, and a headless run's dump is that one tree. Shortcuts, the clipboard, the menu bar, file dialogs, dropped files, tooltips and the multi-line field are all in.
- **Decorator shapes beyond a plain wrapper**: one that takes arguments of its own, one whose wrapper calls the function twice or uses its value. A decorator that returns the function, or a wrapper calling it once, compiles.
- **At the Rust-crate boundary, payload-carrying enums and methods on a twin do not cross yet.** Scalars, String, Lists, Optionals, str-keyed dicts, structs (nested and width-annotated fields included), enums, and Result (compound returns too) all do. The two that remain each wait on something specific: payload enums on rpi-gen itself, methods on impl-splicing onto an rpi-declared struct. Enum- or list-typed fields inside a struct stay out too; every call outside the set is refused, and the error says what and why.
- All measurements are macOS/arm64. Other platforms are not measured yet.

This list is updated every time a design lands.
The design principles behind it are collected in [DESIGN.md](DESIGN.md).
