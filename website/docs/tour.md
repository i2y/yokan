# Yokan language tour

Yokan is a compiler for desktop apps: it takes a statically typed
subset of Python to native code.
This tour is one pass over how the apps are written.
Every piece of code in this tour runs as-is on today's tree.
What Yokan cannot do yet is collected, with reasons, in
[What does not work yet](tour-ship.md#what-does-not-work-yet) at the end of the tour.

A Yokan app is an ordinary Python file.
During development it runs on real CPython; when you ship, the same source compiles to a native binary.
Whether the two behave the same is checked by `yokan gate`, which replays a script of clicks and keystrokes through both and compares the screens byte for byte ([Headless runs and the gate](tour-ship.md#headless-runs-and-the-gate)).

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

Every element also takes the shared properties below, under the same names and with the same meaning; no element takes some of them and refuses others.
`tooltip="…"` shows a line when the pointer rests there, and it is in the dump either way, so a verification script sees it.
`role=` overrides the role an element derives (a screen reader's "button", "heading", "list" and so on) and `a11y_label=` is the name it is read by; the `a11y` step of a headless script prints that tree (`demo/labels.py`). A checkbox, a switch and a progress bar are named by their own label, so they take no `a11y_label=`.
`disabled=True` dims an element and makes it inert: the window does not press it, a script step aimed at it is accepted and does nothing, and the dump shows the state.
`width=`, `height=`, `min_width=` and `max_width=` size any element; an element that has its own `width=` / `height=` (button, image, svg, text, the charts, progress) keeps them.
`theme=`, `animate=` / `easing=` / `enter=` / `exit=`, and `col_span=` / `row_span=` likewise go on any element, not only the ones the earlier sections showed them on (`demo/shared.py` puts one of each where it could not go before).

Switching tab content is a plain `if` / `elif` under the `tab_bar`.

