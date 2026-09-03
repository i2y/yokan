# Yokan language tour

Yokan is a compiler for desktop apps: it takes a statically typed
subset of Python to native code.
This tour is one pass over how the apps are written.
Every piece of code in this tour runs as-is on today's tree.
What Yokan cannot do yet is collected, with reasons, in
[What does not work yet](tour-ship.md#what-does-not-work-yet) at the end of the tour.

A Yokan app is an ordinary Python file.
During development it runs on real CPython; when you ship, the same source compiles to a native binary.
And the **gate** replays the same interaction script against both the development build and the shipped build and byte-diffs the results, verifying per app that the two behave the same.
Everything this tour calls "compiled" has passed that check.
The sections below will not repeat this.

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
- **text_field**: the value and `on_change=`. `multiline=True` makes it a field that holds paragraphs — it wraps, `enter` writes a newline instead of submitting, the caret moves by visual line, and `rows=` says how many lines are visible.

Every element also takes `tooltip="…"`: the window shows it when the pointer rests there, and it is in the dump either way, so a verification script sees it.

Switching tab content is a plain `if` / `elif` under the `tab_bar`.

