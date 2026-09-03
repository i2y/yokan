---
name: yokan
description: Writing desktop apps with Yokan, a compiler for a statically typed subset of Python. Develop on CPython with live reload, ship a native binary, and verify the two runs against each other. Trigger when the user asks for a yokan app, imports yokan, or wants a native (non-web) desktop UI written in Python.
---

# Yokan — agent guide

Yokan is a compiler for a statically typed subset of Python that
builds native desktop apps. Inside the subset, code behaves exactly
as Python: while you develop, the whole app runs on real CPython
with a state-preserving live reload; `yokan build` turns the same
source into a machine-code executable; and `yokan gate` replays one
interaction script against both runs and byte-compares the screens.
What the subset cannot take is refused by name when you check,
translate or build — behavior never changes silently. `yokan check
app.py` is the fast one: it prints the first refusal as
`file:line:col: …` and says nothing when the app is inside the
dialect. The closing section
lists what is refused and why; write inside that boundary from the
start instead of discovering it at build time.

This guide follows the language tour (`TOUR.md`) section by
section and says the same things more briefly. When the two
disagree, the tour is right.

Spelling: import everything bare —
`from yokan import State, store, model, value, component, local,
slot, py, button, column, row, text, text_field, run, …`. A
namespace alias (`import yokan as ui`, then `ui.button`) compiles
identically.

## The shape of every app

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

```console
$ uv run app.py                                    # develop: CPython + live reload
$ yokan gate app.py --script "click:+1,click:+1"  # verify: both runs, same script
$ yokan build app.py --release                    # ship: the native binary
```

- State is declared at module level with `State`, read with
  `count()`, written with `count.set(v)`. Always annotate: the
  annotation is where the compiled type comes from.
- `view()` opens one root container with `with` and builds the
  screen from state. It is pure: no writes, no standard-library,
  crate or `@py` calls inside it.
- Handlers (`on_click`, `on_change`, …) do the mutating; the view
  re-runs after every event.
- The `if __name__ == "__main__":` guard is required (the reload
  machinery would otherwise start a second app) and holds
  `run(...)` only.
- Module level holds declarations: imports, `State`, classes,
  defs, `style()`, type aliases, literal constants, `every(...)`
  timers, the guard. A
  statement there (`count.set(5)`, `fs.write_text(...)`) is
  refused by name; startup work goes in a def passed as
  `run(view, on_start=setup)`. `every(1.0, tick)` IS a
  declaration and belongs there.
- A literal constant (`LIMIT = 10`, `NAMES = ["a", "b"]`) is read
  by name from handlers and views — it is a declaration, not
  state.

## Holding state

Three tools, chosen by what is being held: a **single value** is a
`State[T]`; a **coherent area** (a cart, settings, one screen) is a
`@store`; **objects you create many of and want the screen to
follow** are a `@model`. Data itself belongs in value classes on
store fields.

### State

```python
count: State[int] = State(0)
name: State[str] = State("")
show: State[bool] = State(False)
items: State[list[str]] = State([])
prices: State[dict[str, int]] = State({"apple": 120})
```

Types: int, str, float, bool, lists and dicts (str keys) of those,
Optional (`int | None`), Enum, value classes and sum types. An int
state checks the 64-bit range on every write, in both runs.

### Stores

```python
@store
class Cart:
    items: list[str] = []
    total: int = 0

    def add(self, name: str, price: int) -> None:
        self.items = self.items + [name]
        self.total += price

    def clear(self) -> None:
        Cart.reset_total()          # a sibling is called through the class name
        self.items = []

    def reset_total(self) -> None:
        self.total = 0

button("add", on_click=lambda: Cart.add("apple", 120))
button("clear", on_click=Cart.clear)       # a bound method is a handler
text(f"n={len(Cart.items)} total={Cart.total}")
```

The decorator returns the singleton, so the class name is the
store. Fields take the same types as `State` and are read directly
in views (`Cart.total`); writes go through methods. Methods take
int/float/str/bool, `list[...]` of those, value classes and Enums,
by position or by keyword, with defaults, and are written like
handlers. Stores can call each other's methods.

A method with a return annotation ends in `return <expression>` and
answers a handler (`Cart.count()`); a view reads state instead, so
the read-only form is a `@property` — an expression with a name,
read like a field. A `@staticmethod` is a plain function living in
the class and is callable from views too.

### Models

```python
@model
class Node:
    label: str = "n"
    kid: Node | None = None
    parent: Weak[Node] = None       # the back reference does not own

@store
class Tree:
    root: Node | None = None

    def build(self) -> None:
        a = Node()
        a.label = "alpha"
        b = Node()
        a.kid = b
        b.parent = a                # no cycle: parent is Weak
        self.root = a
```

Fields need defaults; construct with `Node()` and set fields in
handlers. An owning reference is `Node | None` or `list[Node]`;
the non-owning back reference is `Weak[Node]` (`from yokan import
Weak`; a checker sees `Node | None`, a dead target reads None).
Read a reference with the walrus: `if (r := Tree.root) is not
None: text(f"root: {r.label}")`. Models are reference-counted in
the compiled run and cycles are never collected, so back pointers
are `Weak` — the CPython you develop on does collect cycles, which
is the one place the two runs' memory behavior differs.

### Value classes

```python
from dataclasses import replace

@value
class V2:
    x: int
    y: int = 0

    def __add__(self, o: "V2") -> "V2":
        return V2(self.x + o.x, self.y + o.y)

    def dot(self, o: "V2") -> int:
        return self.x * o.x + self.y * o.y

sel: State[V2] = State(V2(3, 4))
sel.set(replace(sel(), x=10))          # functional update
c.set(a() + b())                       # + goes to __add__ in both runs
text(f"x={sel().x}")                   # views read fields
```

`@value` is `@dataclass(frozen=True)` (that spelling works too).
Updates are `replace`. Methods are a single `return expression`;
`__add__` / `__sub__` / `__mul__` give `+` `-` `*` their meaning;
plain methods are called from handlers (views read fields). A
field may hold another value class declared earlier.

### Sum types

```python
@value
class Healthy: pass
@value
class Degraded: services: int

type Health = Healthy | Degraded
health: State[Health] = State(Healthy())

match health():                        # in handlers and in views
    case Healthy():
        text("nominal")
    case Degraded(services):
        text(f"{services} degraded")
```

Missing arms are compile errors. Variant fields take no defaults,
and a variant belongs to one sum type.

### Enum, Optional, Protocol

- `class Mood(Enum)` compiles as-is. `match` arms are `Mood.MEMBER`
  or `_`; missing arms are reported. In text an enum renders as
  Python does (`Mood.HAPPY`). Compare with `==`.
- Optional works in state and in fields (`last: int | None =
  None`); narrow with the walrus, `if (v := sel()) is not None:`,
  in handlers and views alike (`v` is bound only in that branch).
- `typing.Protocol` with method stubs is an interface: a model
  listing it as a base implements it, and a helper with a
  Protocol-typed parameter is statically dispatched. Such helpers
  are called from handlers, not views.

## Views

The catalog: `text`, `link`, `button`, `text_field`, `number_field`,
`int_field`, `checkbox`, `switch`, `slider`, `select`, `radio_group`,
`tab_bar`, `segmented`, `column`, `row`, `grid`, `stack`, `spacer`,
`divider`, `list_view`, `table`, `scroll_view`, `h_scroll_view`,
`data_table`, `modal`, `image`, `svg`, `bar_chart`, `line_chart`,
`progress`, `spinner`. Containers are opened with `with`; elements
add themselves to the open container. `grid(columns=, rows=)` lays
equal tracks and a button spans cells with `col_span=` /
`row_span=`. In `data_table` the first `row` child is the header
and later `row` children are data rows shaded in alternation;
columns line up when the cells of one column share a `grow`. An
element object is placed once; build fresh ones on every call.
`spacer()` takes a row's or column's remaining space, `divider()`
draws a rule (vertical in a row), `link(label, url)` opens a URL.
`text` takes `bold=` / `italic=` / `mono=` / `underline=`,
`wrap="nowrap"|"ellipsis"` with `width=`, `max_lines=`, and a box
(`background=`, `padding=`, `border_radius=`) — a status pill is a
text with a box. `table(columns, count, row)` is a `list_view` with a
header and column tracks: `row(i)` returns a `row` of one cell per
column, `widths=` are the tracks' shares, `selected=` / `on_select`
and `sort=` / `descending=` / `on_sort` hand indices back and the app
re-sorts its own lists. Charts take `min=` / `max=`, `axis=True`,
`series=` (a `list[list[float]]` field) with `colors=`; negatives hang
below the zero line. `progress(value)` takes `width=` / `height=`,
`label=` and `indeterminate=True`. Every element takes the same
shared properties: `tooltip=`, `role=` / `a11y_label=` (not on checkbox, switch
or progress, which are named by their own label), `disabled=` (dimmed
and inert, in the window and in scripts), `width=` / `height=` /
`min_width=` / `max_width=` (an element with its own `width=` /
`height=` keeps them), `theme=`, `animate=` / `easing=` / `enter=` /
`exit=`, and `col_span=` / `row_span=`.

**Text holes** are f-strings. int, str, float, bool and Enum
values render exactly as Python's `str()` (`2.0`, `True`,
`Mood.HAPPY`); Python's format specs work (`:.1f`, `:,`, `:>8`,
`:.1%`, `:.2e`) in views and handlers alike; `+` `-` `*` compute in a
hole (`f"{n * 2 + 1}"`). `/` `//` `%` `**` can fail, so compute
them in a handler and render the result. `len(items())`,
`d().get("k", fallback)` and pure helpers are fine in holes.

**Structure** inside a `with` block is element calls, nested `with`
blocks, `if` / `elif` / `else` and `match`. No loops and no locals
in views: long lists go to `list_view`, derived values go to store
fields or helpers. A modal is open by existing, so wrap it in an
`if`:

```python
if show():
    with modal():
        text("confirm?")
        button("yes", on_click=lambda: (done.set(True), show.set(False)))
```

**Lists and charts.** `items.set(items() + [x])` appends (the
compiled run does an in-place push), `items.set([])` clears,
`len(items())` counts, `items[0] = v` writes one slot, and a
literal `xs[-1]` reads from the back. Charts draw lists of float or
int: `line_chart(values(), height=120.0)`, `bar_chart(data,
labels=names, height=100.0)`. `list_view(len(items()), row,
item_height=22.0, height=200.0)` is virtualized — `row(i)` runs
only for visible rows and returns `text(items()[i])`; `grow=1.0`
fills the parent instead of `height=`.

**Components.** `@component` with `local` for per-instance state;
`@component(slots=True)` takes children at `slot()`:

```python
@component
def counter(label: str, step: int):
    n: State[int] = local(0)           # per call site, survives rebuilds
    with row(spacing=6):
        text(f"{label}: {n()}")
        button(f"+{step}", on_click=lambda: n.set(n() + step))

@component(slots=True)
def card(title: str):
    with column(border_width=1.0, border_color="accent", padding=8):
        text(title, size=18)
        slot()

with card("counters"):
    counter("a", 1)
    counter("b", 10)
```

Parameters are str and int. `local` identity is the call site:
reordering calls reassigns the states.

**Styles and themes.** `style()` makes a named dict; splat one per
element with `**`; compose with `|`. Colors are hex literals or
theme tokens (`windowBg`, `panel`, `surface`, `surfaceHover`,
`border`, `text`, `textDim`, `accent`). `theme=` on a container
scopes a palette and takes a literal or a state read; theming the
root follows down to the window ground.

```python
chip = style(size=18, color="accent")
key = style(background="surface", hover_background="surfaceHover")
hot = key | style(background="#fab387")

text(f"n={n()}", **chip)
with column(background="windowBg", grow=1.0, theme=mode()):
    ...
```

Sizes are px floats and `0.0` means the engine default; `grow=1.0`
fills the parent's main axis. Other style values are literals —
to change a size or color with state, branch with `if`.

**Animation.** `animate=` (ms), `easing="linear|in|out|inOut"`,
`enter=True` / `exit=True` on text, buttons and containers.
Frames come from the shared clock, so `advance:<ms>` in a script
lands identically in both runs.

## Form controls

Value in from state; the handler receives **the one new value**
and writes it back. Bind to a `State` or a store field.

| element | value in | handler gets | script step |
|---|---|---|---|
| `text_field(value, placeholder=, on_change=, on_submit=)` | str | str | `input:<text>`, `submit` |
| `checkbox(label, checked=, on_change=)` / `switch(...)` | bool | bool | `click:<label>` |
| `slider(value=, min=, max=, step=, on_change=)` | float | float | `slide:<v>` |
| `select(options=, selected=, on_change=)` / `radio_group(...)` | list, index | index | `select:<label>` |
| `tab_bar(labels=, active=, on_change=)` | list, index | index | `select:<label>` |
| `segmented(options=, selected=, on_change=)` | list, index | index | `select:<label>` |
| `number_field(value, min=, max=, step=, on_change=)` / `int_field(...)` | float / int | float / int | `input:<text>` (commits) |

`text_field(name(), on_change=name.set)` binds a str state
directly. Tab content switches with an `if` / `elif` on the active
index. Options and labels come from a list state or field.

## Handlers and control flow

Three forms: a lambda (a tuple for several operations, `lambda:
(a.set(x), b.set(y))`), a module-level def, and a store's bound
method (`on_click=Cart.clear`). A def body compiles with real
control flow:

```python
def double(v: int) -> int:             # a pure helper: annotated, ends in return
    return v * 2

def tally():
    total.set(0)
    for i in range(1, 6):
        if i == 3:
            continue
        total.set(total() + double(i))
```

- `if` / `elif` / `else`, `while`, `for` (over `range()` with one or
  two arguments, a list state, a list field, a list parameter, or
  `sorted(d())`), `break` / `continue`, locals (reassignable).
- A local assigned in **both** arms of an `if` / `else` reads after
  the branch; assigned in one arm only, it is refused (Python would
  raise NameError on the other path). Loop variables do not outlive
  the loop.
- Conditions are bools (a state, a field, a local, a parameter,
  `while True:`) or explicit comparisons — `if name() == "":`, not
  `if name():`, because a str has no truth value here. `and` / `or`
  / `not` work in conditions and as bool values over bools; `0 < n
  < 10` chains (the middle is read once) and `:=` binds.
- `a if c else b` is written in a handler over int/float/str/bool;
  in a view, branch the elements with `if` / `else`.
- Helpers may return early from a branch, call themselves, take
  `list[...]` parameters and defaults, and return a value class or
  a list.
- Arithmetic is exactly Python's: `/` is always float, `//` floors,
  `%` takes the divisor's sign, `**` is power. Division by zero and
  overflow abort the statement in both runs; the app lives on.
  `int ** int` takes a non-negative literal exponent.
- Dicts: `prices["cherry"] = 200` writes a key, `d().get(k,
  default)` reads, `"k" in d()` tests, `len(d())` counts, `for k in
  sorted(d())` iterates in key order. Bare `d[k]` and bare
  iteration are refused.
- Lists: `xs[i]` reads an element with Python's meaning (a
  negative index counts from the back, past the end aborts the
  statement); `xs[i] = v` writes one. `len(xs)`, `Cart.xs` and
  `self.xs` all say the same thing inside a store. `in`, slices,
  `sorted` / `reversed` / `min` / `max` / `sum`, comprehensions,
  `enumerate` / `zip`, a stepped `range` and joining two lists all
  work; a local list is annotated (`out: list[str] = []`).
- `log("…")` writes a line to stderr from either run; `assert` and
  `raise` end the statement the way Python's exception does.
- Strings: `+`, `==`, `<`, `"-" * 3`, f-strings with Python's
  format specs, `len(s)`, `s[i]`, `s[a:b]`, `in`, and the common
  methods (`.upper()`, `.lower()`, the `.strip()` family,
  `.split()`, `.join()`, `.startswith()`, `.endswith()`,
  `.replace()`, `.find()`, `.count()`). `int()` / `float()` /
  `str()` / `bool()` / `round()` convert, failing the way Python
  raises; `strings.to_int(s, default)` is the total form.
- Helpers: parameters annotated int/float/str/bool (or a
  Protocol), the return annotated int/float/str/bool, body ending
  in `return expression`; callable from handlers and from view
  text. Lists, value classes and Optionals do not cross a helper
  yet; store methods take them.
- A store method calls a sibling through the class name
  (`Cart.reset_total()`), not bare.

**Errors**, in order of reach-for: `*_or` totals
(`fs.read_text_or(p, "")`, `http.get_text_or(url, "")`,
`sqlite.query_int_or(p, sql, 0)`); then `try` / `except` in its
full Python form (several statements, per-exception clauses,
tuples, `else`, `finally`; `@py` exceptions are caught with
Python's own message); then nothing — an uncaught failure aborts
its statement and the app keeps running.

```python
try:
    body.set(http.get_text(url))
except Exception as e:
    status.set(f"offline: {e}")
```

## The standard library

Two layers, told apart by where the name comes from.

**Python's own**: `import math`, `import random`, `import
statistics`, written as Python writes them. Development imports
CPython's module; the shipped binary calls a twin held to CPython by
a table CPython printed, error messages included. `math` and
`statistics` are pure, so a view may call them; `random` moves a
generator on, so it stays in a handler. Seed it and the two runs walk
one sequence.

**Yokan's own**: `from yokan import fs, sqlite, http, json, time,
strings, clipboard, notify`. One implementation in Rust serves both
runs; the shipped binary needs no Python. Call it from handlers only.

- **fs**: `read_text` / `write_text` / `append_text` / `exists` /
  `read_text_or` / `list_dir` (sorted names) / `make_dir` /
  `remove` / `app_dir(name)` (this app's own directory, created if
  it is missing)
- **sqlite**: `exec(path, sql) -> int` / `query_text(path, sql) ->
  list[str]` (column 0 as text; `ORDER BY` for determinism) /
  `query_rows` (every column, `list[list[str]]`) / `query_int` /
  `query_int_or` / `query_text_or` / `query_rows_or` — wrap
  aggregates in `COALESCE`. Every one takes a trailing `params`
  list: write `?` in the statement and pass the values beside it
  (`sqlite.exec(db, "INSERT INTO t VALUES (?, ?)", [name,
  str(n)])`, `sqlite.query_int_or(db, sql, 0, ["food"])`). Never
  splice user text into SQL.
- **http**: `get_text(url[, timeout_ms])` / `get_text_or` /
  `get_text_with(url, headers)` / `post_text(url, body[,
  content_type])` / `post_text_or` / `status(url)` (synchronous;
  inside `task` the compiled run awaits it)
- **json**: `get_text` / `get_int` / `get_float` / `get_bool` /
  `length` / `has` by dotted path (`"items.0.title"`), and
  `dumps(value)` for a str, int, float, bool, a list of one of
  those, or a str-keyed dict (written in key order)
- **time**: `now_ms()`, `format_ms(ms, "%Y-%m-%d")` (UTC; pass a
  fixed ms in verification scripts), `format_local_ms(ms, fmt)`
  (the machine's zone), `local_offset_minutes(ms)`
- **strings**: `to_int(s, default)` / `to_float(s, default)`
- **clipboard**: `set_text(s)` / `get_text()` — a window shares it
  with every other application, a headless run keeps it to itself,
  so copy-and-paste is gate-checkable
- **notify**: `send(title, body)` — delivered when the app runs as
  an `.app` bundle; dev and headless runs drop it quietly

From Python's side: all of `math` but eight members (`frexp`,
`modf`, `prod`, `sumprod`, `gamma`, `lgamma`, `erf`, `erfc`, each
refused by name with its reason); `random`'s `seed`, `random`,
`randint`, `randrange`, `getrandbits`, `uniform`, `gauss`, `choice`,
`sample` (no `shuffle` — it reorders in place, and a list lives in a
`State`); `statistics`' `mean`, `fmean`, `median`, `mode`,
`variance`, `pvariance`, `stdev`, `pstdev` over `list[float]`.

Determinism is the rule underneath: fixed times, seeded randomness
(`random.seed(n)` in `on_start` or a reset handler so scripts
replay), and `--fresh path/to/file.db` on the gate for apps that
persist.

## Rust crates

`yokan add app.py deunicode 1` (crates.io) or `yokan add app.py
hexfmt --path native/hexfmt` declares a crate in the PEP 723 block
(or pyproject.toml for a project); `from yokan import crates` and
`crates.deunicode.deunicode(s)` calls it, same implementation in
both runs. Scalars, str, lists and Optionals of those, str-keyed
dicts, structs and enums with a same-shaped `@value` twin in the
app, and `Result` cross the boundary; anything else is refused by
name.

## CPython escapes

```python
@py
def slug(t: str) -> str:
    import re                          # imports live inside the escape
    return re.sub(r"[^a-z0-9]+", "-", t.lower()).strip("-")
```

An `@py` function stays real Python in both runs (numpy works).
Sole decorator; annotate every parameter and the return with
int/float/str/bool, `list[...]` or `dict[str, ...]` of those, a
value class, or `T | None`; call it from handlers.
A shipped app with escapes needs CPython: `--bundle` ships a
runtime folder, `--onefile` one file (about 17 MB; 21 MB with
numpy).

## Heavy work and timers

`task(work, on_done=)` runs `work` off the UI thread and the
continuation on it; headless runs wait for it, and it is the last
statement of its handler (in Python the statements after it run
first). Both runs do it: a Python thread during development, and
in the compiled app the standard-library calls inside the work are
awaited, which is what moves them. `on_error=` is not compiled —
catch a failing call with `try` / `except` around it.

`every(seconds, cb)` is a timer DECLARED at module level (or under
the `__main__` guard) and started with the app. Both runs fire it
off the same clock: a frame in a window, an `advance:<ms>` in a
headless script, so ticks are gate-checkable.

`shortcut("cmd+s", save)` and `on_key(typed)` are declared the same
way: a chord and its handler, or one handler that sees every key as
the chord it was. The chord is spelled the way the platform spells
it (`cmd+s`, `shift-tab`, `ctrl+alt+k`; `-` reads the same as `+`).
While a text field has the caret, plain keys keep going into it and
only chords carrying cmd or ctrl reach the app. A headless script
presses one with `key:cmd+s`.

`menu_item("File", "Save", save)` puts a handler in the application
menu bar — declaration order is menu order, and a script picks one
with `menu:Save`. `on_file_drop(handler)` takes a file dragged onto
the window (script: `drop:<path>`), and `fs.open_dialog(title)` /
`fs.save_dialog(name)` open the platform's panels from inside a
`task` (script: `file:<path>` answers the next one).

A user decorator is folded into the function it decorates: the
decorator is a def of one argument that returns it, or a wrapper
that calls it once. Anything else is refused by name.

`text_field(..., multiline=True, rows=3)` is a field that holds
paragraphs; every element takes `tooltip="…"`.

## The window and startup

`run(view, title="OpsBoard", width=1100, height=820,
on_start=boot)`. Title and size are baked into the binary; width
and height come as a pair in logical pixels. `on_start` runs once
after mount (a failure prints and the app opens) and is the one
place for startup work — loading data, seeding the RNG.

## Headless runs, the gate, shipping

```console
$ yokan check app.py                                  # refusals only, no compiler
$ PIXIE_SCRIPT="click:+1,input:Momo" uv run app.py    # dump before/after, no window
$ yokan gate app.py --script "click:+1,input:Momo" --release
GATE OK — 2 dump lines identical across tiers
$ yokan build app.py --release [--app] [--bundle | --onefile]
```

Steps: `click[@n]:<label>`, `input[@n]:<text>`, `submit[@n]`,
`slide[@n]:<value>`, `select[@n]:<label>`, `advance:<ms>`,
`theme:light|dark`, `a11y`, `mem`, `dump`. `@n` picks the n-th
match in tree order; `dump` prints the screen mid-script; a comma
in text is `\,`. From tests, `yokan._headless(view, None, script)`
returns the same string. Apps with PEP 723 dependencies run the
gate under `uv run --with <dep>`. Building needs the repository
checkout (or `PIXIE_REPO`); `translate` works anywhere. `--app`
makes `dist/<Title>.app` (a `<stem>.png` beside the file becomes
the icon); it excludes `--onefile`.

Verify headless first; then run windowed once and look at it — the
gate proves the two runs agree, not that the window looks right.

## Type checkers

Stubs ship with the package; pyright (Pylance) works as-is, and
`@store` methods type as bound calls (`Settings.set_dark(True)`,
`on_change=Settings.set_dark`). mypy does not apply class-decorator
transformations and misreports store methods; use pyright.

## What does not work yet

Same list as the tour's closing section; each is refused by name.

- Iterating a dict in insertion order (compiled dicts are
  key-ordered): use `sorted(d())`.
- Bare `d[k]` reads: use `.get(key, default)`.
- Reading a local assigned in one branch only.
- Negative exponents on `int ** int`: make a side float.
- Compiling dict state (`run(state={...})`): development only;
  the compiled form is typed `State`.
- Calling Protocol-bound helpers, value-class methods, or a store
  or model method from views (handlers can; views read fields and
  `@property`).
- Iterating a list of models in a view: assemble strings on the
  store side and hand them to `list_view`.
- A `Weak` field on a store (stores own; the back pointer lives on
  the model).
- Type names the compiled side uses, such as `Vec`: pick another.
- Statements at module level: startup work goes in
  `run(view, on_start=setup)`.
- Starting a timer from a handler: `every(1.0, tick)` is a
  module-level declaration.
- `task`'s `on_error=`: catch a failing call with try/except.
- A component's `local` is identified by call site.
- Placing one element object twice.
- A method that returns `T | None` (scalars, lists, value classes
  and enums do come back).
- A local dict, and a local list without an annotation
  (`out: list[str] = []`).
- str methods beyond `.upper()` / `.lower()` / `.strip()` family /
  `.split()` / `.join()` / `.startswith()` / `.endswith()` /
  `.replace()` / `.find()` / `.count()` (those, `len(s)`, `s[i]`,
  `s[a:b]` and `in` are in).
- Format specs beyond fill, align, sign, width, `,`, precision and
  `d` / `f` / `e` / `%` / `s`.
- Iterating a dict's `.values()` / `.items()` (insertion order in
  Python, key order compiled): iterate `sorted(d())`.
- Nested defs (no closures — helpers go at module level) and a
  conditional expression inside a view.
- `print`: stdout carries the headless dump, so `log("…")` writes
  to stderr in both runs instead.
- Component parameters that are value classes or enums, and a body
  that is not one container (a top-level `if`, or several elements
  — wrap them in a `column`). Callbacks and State parameters work:
  the component becomes a view per call site.
- `tuple` and `set`: a tuple has no compiled shape, and a Python
  set iterates in an order the compiled side would not reproduce.
- `@py` signatures beyond scalars, lists, str-keyed dicts, value classes and Optionals.
- Standard library: reading a time back from text, file metadata
  and copy/rename, streaming or binary downloads, nested json
  writing (a value inside a written dict or list is a scalar).
- A second window: one app, one window (the engine's window root
  takes a single view, and a headless dump is that one tree).
- Decorators beyond a plain wrapper: one taking arguments of its
  own, or whose wrapper calls the function twice or uses its value.
- At the Rust-crate boundary: payload-carrying enums and methods
  on a twin; enum- or list-typed struct fields.
- macOS on Apple silicon is the measured platform.
