---
name: yokan
description: Writing native desktop UIs in Python with yokan — a GPU-rendered window (pixie's gpui engine) driven from real CPython. Trigger when the user asks for a yokan app, imports yokan, or wants a native (non-web) Python UI with instant start and live reload.
---

# yokan — agent guide

yokan is a Python extension module: real CPython (numpy and the
whole ecosystem work) driving a native, GPU-rendered window. No
browser, no server. Apps are one file, runnable with
`uv run app.py` — declare `dependencies = ["yokan"]` in the PEP 723
header and uv fetches it from PyPI.

Spelling convention: the docs use `import yokan as ui` for the UI
surface (`ui.text`, `ui.column`, `ui.run`) so element-emitting
lines stand out, and import everything that is not UI bare:
`from yokan import State, store, model, value, component,
py, local`. Bare element imports compile identically
(`from yokan import button, column, run, …`) — the alias is a
house style, not a requirement. `@yokan.store` (module-prefixed)
works too.

## The shape of every app

Two view spellings, freely mixable — both build the same tree:

```python
def view(s):                      # functional
    return ui.column(ui.text(f"n={s['n']}"), spacing=12)

def view(s):                      # declarative (return nothing)
    with ui.column(spacing=12):
        ui.text(f"n={s['n']}")
        with ui.row():
            ui.button("+1", on_click=lambda: s.update(n=s["n"] + 1))
```

Dict state still RUNS on CPython but no longer compiles — typed
cells are the compiled story (they validate the native i64 range on
every write; a plain dict cannot).

Typed State cells (the compiled story):

```python
count: State[int] = State(0)
name: State[str] = State("")

def view():                       # zero-arg: cells close over module scope
    with ui.column(spacing=12):
        ui.text(f"count: {count()}")
        ui.button("+1", on_click=lambda: count.set(count() + 1))
        ui.text_field(name(), on_change=name.set)   # bound .set is a handler
```

Annotate cells (`State[int]`) — the annotation is the type source
for native compilation. Reads are `count()`, writes `count.set(v)`;
reload preserves cell values by name. Do not mix cells with a state
dict in one app.

List cells and the patterns that translate natively:

```python
items: State[list[str]] = State([])       # annotate lists — always
values: State[list[float]] = State([1.0]) # fine as CHART DATA

items.set(items() + [x])   # append (translates to an in-place push)
items.set([])              # clear
len(items())               # count
ui.list_view(len(items()), row)   # row = def row(i): return ui.text(items()[i])
ui.line_chart(values(), height=120.0)
```

Bare float/bool/enum TEXT renders exactly as CPython's str() in
both tiers (`2.0`, `True`, `Mood.HAPPY`) — the compiled side
reproduces CPython's results exactly. `.Nf` specs remain available for fixed
decimals. `+`/`-`/`*` arithmetic works inside text holes.

CPython escapes — `@py` keeps a function REAL Python in both
tiers (interpreted in dev, embedded CPython in the native build):

```python
@py
def slug(t: str) -> str:      # annotate every param AND the return
    import re                  # imports go inside the escape
    return re.sub(r"[^a-z0-9]+", "-", t.lower()).strip("-")
```

Call escapes from handlers and put results in state; sole decorator;
types are int/float/str/bool/list[...]. The native binary needs a
CPython on the machine for escaped parts (stdlib free; third-party
must be importable there).

Shipping an `@py` app self-contained: add `--bundle` to the gate
(`yokan gate app.py --script … --release --bundle`; the `yokan`
command ships with the wheel — `uv tool install yokan`) — the dist
folder carries its own python runtime, and the app's declared
`dependencies` (PEP 723 inline block, or the nearest
`pyproject.toml` when the app is a project — `yokan` itself is
filtered out) are installed into its site-packages; the target machine needs nothing. `--onefile` goes
further: ONE distributable file (~17 MB stdlib-only, ~21 MB with
numpy) that unpacks into the user cache on first run and starts in
~40 ms after; the gate replays its script through that single file.
When the app has PEP 723 deps, run the gate itself under
`uv run --with <dep>` so the CPython tier can import them too.
To just produce the artifact without the two-tier comparison:
`yokan build app.py --release [--bundle|--onefile]` (no --script,
no deps needed on the build machine). Compiling needs the pixie repo
(auto-found in-tree/from cwd, else `PIXIE_REPO=…`); `translate`
works anywhere.

The standard library — `yokan.fs` and friends: one native
implementation shared by the interpreted and the compiled app, so
it works identically in both, and needs no CPython in the shipped
binary:

`ui` stays the UI-building alias; modules import from the package:

```python
from yokan import fs

def save():
    n.set(fs.write_text("out/note.txt", "hi"))   # -> bytes written
def load():
    content.set(fs.read_text("out/note.txt"))    # errors abort the
flag.set(fs.exists("out/note.txt"))              # statement, app lives
```

Call them from handlers only (views stay pure). More modules
(sqlite, http) follow the same shape.

Styling — named bags are plain dicts, used by splat; `|`
composes; `theme=` scopes a subtree's palette:

```python
chip = ui.style(size=18, color="accent")     # tokens resolve per theme
key  = ui.style(background="#313244", hover_background="#45475a")
hot  = key | ui.style(background="#fab387")  # native style merge

ui.text(f"n={n()}", **chip)                  # one ** per element
with ui.column(theme=mode()):                # "light"/"dark" cell or literal
    ...
```

Form controls — one shape: value in from state, handler receives
THE NEW VALUE (one argument) and writes it back. `ui.checkbox(label,
checked=..., on_change=cb)` / `ui.switch(...)` (cb gets a bool;
script step: `click:<label>` toggles) · `ui.slider(value=, min=,
max=, step=, on_change=cb)` (cb gets a float; script `slide:<v>`,
clamped + step-snapped) · `ui.select(options=, selected=,
on_change=cb)` / `ui.radio_group(...)` / `ui.tab_bar(labels=,
active=, on_change=cb)` (cb gets the chosen INDEX; script
`select:<label>`). Bindings are state/store-field reads; tab
content switches with a plain `if` on the active index.

Standard library, continued — `from yokan import math, json,
time` (same shared-implementation rule; yokan.math is yokan's
math in BOTH tiers, so results match by construction): `math.sqrt/sin/cos/pow/fabs/
floor/ceil/pi`; `json.get_text/get_int/get_float/get_bool/length/
has(src, "a.b.0")` (typed, panics-with-path on mismatch,
contained); `time.now_ms()`, `time.format_ms(ms, "%Y-%m-%d")`
(UTC; feed fixed ms in gate scripts).

Enums — plain `class Mood(Enum)` compiles to a native enum;
`match` is the native `case` (exhaustiveness checked; arms are
`Mood.MEMBER` or `_`), in handlers and view bodies. Enum cells:
`State[Mood] = State(Mood.HAPPY)`; never render an enum in
text — match it to a string.

Optionals — `State[int | None]`, `.set(None)`, and narrowing by
walrus (Python's own `if let`):

```python
if (v := sel()) is not None:
    ui.text(f"picked {v}")      # v is bound, branch-scoped
else:
    ui.text("(none)")
```

Animation — `animate=` (ms) / `easing=` ("linear|in|out|inOut") /
`enter=True` / `exit=True` on text and containers; frames come
from the shared kernel clock, so `advance:60` in a script stands
inside the tween identically in both tiers.

Error handling, in order of reach-for: (1) `*_or` totals —
`fs.read_text_or(p, "")`, `http.get_text_or(url, "")`,
`sqlite.query_int_or(p, sql, 0)`, `sqlite.query_text_or(p, sql)`
(→ [] on failure) — the default for the standard library; (2) try/except when
the failure reason matters; (3) uncaught = contained.

try/except is the full Python form: any statements in the body
(`a = risky(...)` locals live on after), multiple clauses, tuples
(`except (ValueError, KeyError) as e:`), `else`, `finally` (finally
needs a catch-all clause — the message says why). Matching for
escape exceptions is real CPython's:

```python
try:
    num.set(risky(mode))
except ValueError as e:  note.set(f"VE: {e}")
except KeyError as e:    note.set(f"KE: {e}")
except Exception:        note.set("other")
```

Errors — `try/except` is native `case ok/err` for @py escape
calls (a raised Python exception's `str(e)` is the message in BOTH
tiers) and for the fallible
standard-library calls (http.get_text, fs.read_text,
sqlite.query_int): one
statement in the try, `except Exception as e` binds the message
(same string in both tiers). Anything uncaught stays contained.

```python
try:
    body.set(http.get_text(url))
except Exception as e:
    status.set(f"offline: {e}")
```

Optional/enum types work on store/model FIELDS as well as cells
(`last: int | None = None`, `trend: Mood = Mood.HAPPY`, match over
`self.trend`, view-narrow `(t := Tracker.last) is not None`).
Buttons take animate=/easing= too.

Helpers — full bodies (ifs, locals, reassignment) ending in
`return expr`; callable from HANDLERS and VIEW TEXT (they compile
to native receiver-less static fns). Locals behave like Python
locals (mutable). Protocol-bounded helpers remain handler-only.

`yokan.random` — seeded and deterministic: `random.seed(n)`,
`random.int(lo, hi)` (inclusive), `random.float()`. Seed in your
reset handler and gate scripts replay identical sequences.

Sum types — frozen dataclasses + a `type` alias compile to a
native payload enum; `match` destructures everywhere:

```python
@dataclass(frozen=True)
class Circle: r: float
@dataclass(frozen=True)
class Rect: w: float; h: float
type Shape = Circle | Rect

sel: State[Shape] = State(Circle(2.0))
match sel():                       # handlers AND view bodies
    case Circle(r): ui.text(f"r={r:.1f}")   # float binds need .Nf
    case Rect(w, h): ...
```

Variant fields take no defaults; a variant belongs to one union.

Value types — `@value` (shorthand for `@dataclass(frozen=True)`;
both spellings compile) makes a native struct. Value classes take
METHODS too: operator dunders `__add__`/`__sub__`/`__mul__` give
`+`/`-`/`*` their meaning (both tiers), plain methods are
handler-callable; bodies are a single `return expr`. Bool logic as
a VALUE works over bools (`hot() and not cold()`). Immutability is what
makes Python references and native values mean the same thing:

```python
from dataclasses import dataclass, replace

@dataclass(frozen=True)
class Point:
    x: int
    y: int = 0

sel: State[Point] = State(Point(3, 4))
sel.set(replace(sel(), x=10))      # functional update, both tiers
ui.text(f"x={sel().x}")            # field reads in views/handlers
```

Observed objects — `@model` (instantiable; fields need
defaults; methods compile with the full handler dialect). Models
REFERENCE models: owning `kid: Node | None = None` / `kids:
list[Node] = []`, non-owning back pointer `parent: Weak[Node] =
None` (from yokan import Weak; compiled twin is pixie's weak
prop — a dead target reads as None). Wire references in handlers
(`a.kid = b`; construct with `Node()`); narrow reads with the
walrus. Stores hold models the same way (`root: Node | None`) but
never weakly (owners are strong):

```python
@model
class Circle:
    r: float = 1.0
    def grow(self, by: float) -> None:
        self.r += by

left = Circle()                    # module-level instances
ui.button("grow", on_click=lambda: left.grow(0.5))   # def handlers may call left.grow(...)
ui.text(f"r is set")               # views read left.<field> reactively
```

Interfaces — `typing.Protocol` with method stubs compiles to a
native trait; a model listing it as a base implements it, and a
Protocol-typed helper becomes a statically-dispatched generic:

```python
class Shape(Protocol):
    def area(self) -> float: ...

@model
class Circle(Shape):
    r: float = 1.0
    def area(self) -> float:       # protocol methods: single return
        return self.r * self.r * 3.0

def area_of(s: Shape) -> float:    # bounded generic natively
    return s.area()
```

Startup — `ui.run(view, on_start=Store.load)` runs once after
mount, contained (a failing start prints; the app opens). Gate
persistent apps with `--fresh path/to/file.db` so the interpreted
run's writes never leak into the compiled run's startup read.

Window — `ui.run(view, title="OpsBoard", width=1100, height=820)`:
title and size cross into the compiled binary too (baked through
the project's pixie.toml `[window]`). width/height are logical
pixels and come as a pair; omitted = the engine default (420x560).
Headless dumps never contain them, so they are gate-neutral.

Dict defaults read anywhere: `d().get("key", fallback)` (and
`Store.field.get(...)`) works in handlers AND view text — total in
both tiers. Loop variables are Int and render in holes
(`f"items.{i}.title"` builds dynamic json paths).

Named stores — `@store`: a singleton with fields AND methods
(the decorator returns the instance, so the class name is the
store). Fields take cell-grade types; methods use the full handler
dialect; stores may call each other; bound methods work as
handlers. Method params take int/float/str/bool, list[...] of
those, a value class, or an enum:

```python
@store
class Cart:
    items: list[str] = []
    total: int = 0
    def add(self, name: str, price: int) -> None:
        self.items = self.items + [name]   # -> native push
        self.total += price

ui.button("add", on_click=lambda: Cart.add("apple", 120))
ui.button("clear", on_click=Cart.clear)    # bound method, no lambda
ui.text(f"n={len(Cart.items)} total={Cart.total}")
```

Grouped state is a fields-only `@store` — direct reads in views
(`Mixer.volume`), writes through methods, no instance line.
(There is no separate bundle decorator.)

Components with children — declare `@component(slots=True)`,
place them with `ui.slot()`, pass them with `with`:

```python
@component(slots=True)
def card(title: str):
    with ui.column(border_width=1.0, border_color="accent", padding=8):
        ui.text(title, size=18)
        ui.slot()

with card("counters"):        # children keep use-site identity
    counter("a", 1)
    counter("b", 10)
```

Per-instance components — `@component` + `ui.local`:

```python
@component
def counter(label: str, step: int):
    n: State[int] = local(0)     # per-call-site state, survives rebuilds
    with ui.row():
        ui.text(f"{label}: {n()}")
        ui.button(f"+{step}", on_click=lambda: n.set(n() + step))
```

Identity is positional (reordering call sites reassigns state).
Float text: allowed ONLY with an explicit spec — `f"{x:.1f}"`;
bare `f"{x}"` of a float stays untranslatable.

Multi-module apps work: put cells in one module, helpers in
another, `from state import count` as usual. Helpers that return an
element become native components when compiled — annotate their
parameters (`def badge(label: str):`), keep them stateless over
cells, and don't reference parameters inside their handlers.

Conditionals — Python `if` in with-blocks, ternaries in functional
style; conditions are bool cells or explicit comparisons:

```python
show: State[bool] = State(False)   # bool: conditions yes, TEXT no

if show():
    with ui.modal():                     # no open= — presence IS openness
        ui.text("confirm?")
        ui.button("yes", on_click=lambda: (done.set(True), show.set(False)))
else:
    ui.text("(closed)")
```

Tuple-bodied lambdas (`lambda: (a.set(x), b.set(y))`) are the
idiom for multi-action handlers in lambda position; use a def for
anything longer.

def-handlers compile with real control flow — if/elif/else, while,
for over range() or a list cell, break/continue — plus locals and
pure helper fns (annotated, single return) that become NATIVE free
functions:

```python
def double(v: int) -> int:          # compiles to a native fn
    return v * 2

def tally():
    total.set(0)
    for i in range(1, 6):
        if i == 3:
            continue
        total.set(total() + double(i))
```

Arithmetic: `/` `//` `%` `**` compile with CPython's exact results
(true division always float; floor/mod follow the divisor's sign;
zero/overflow abort the statement contained, both tiers). int**int
needs a non-negative literal exponent. Compute the fallible four in
handlers; `+`/`-`/`*` are fine in text holes too.

Rules that bite: a local assigned on BOTH arms of if/else (elif
chains included) outlives the branch like in Python; a one-armed
assignment is still block-scoped (Python would NameError the
untaken path). Loop vars do not outlive the loop. `xs[-1]`-style
LITERAL negative indexing works on locals; a variable that turns
out negative does not.

Dict cells — order-free operations only (Python orders dicts by
insertion, native maps by key, so iteration stays out BY NAME):

```python
prices: State[dict[str, int]] = State({"apple": 120})
prices["cherry"] = 200                 # per-key write, in place BOTH tiers
picked.set(prices().get("apple", -1))  # total read: default on missing
if "cherry" in prices(): ...           # membership; len(prices()) counts
```

Bare `d[k]` reads are refused (Python raises where native answers
nil) — `.get(key, default)` is the read. Iterate a dict with
`for k in sorted(d())` (key order — identical in both tiers);
bare iteration stays refused (insertion order is Python-only).

More standard library: `strings.to_int(s, default)` (total numeric
parsing — bad input becomes the default in BOTH tiers) and
`sqlite.query_int(path, sql)` (scalar aggregates; wrap in
COALESCE). `from yokan import sqlite, http` —
`sqlite.exec(path, sql) -> int`, `sqlite.query_text(path, sql) ->
list[str]` (column 0 as text; ORDER BY for determinism), and
`http.get_text(url) -> str` (blocking). Same shared-implementation
rule as fs; call from handlers.

In a `with` block, constructors auto-append to the open container;
passing an element explicitly as a child argument moves it there
instead. A with-style view must build exactly one root and return
nothing. Containers usable as `with`: column/row/grid/stack/
scroll_view/h_scroll_view/data_table/modal.

```python
import yokan as ui

def view(s):                      # called on every rebuild; PURE over s
    return ui.column(
        ui.text(f"count: {s['count']}", size=34),
        ui.button("+1", on_click=lambda: s.update(count=s["count"] + 1)),
        spacing=12, padding=16,
    )

if __name__ == "__main__":        # REQUIRED guard (reload re-execs the module)
    ui.run(view, state={"count": 0}, title="counter")
```

- **State is any Python object** you own (a dict is idiomatic).
  Event callbacks mutate it; the view re-runs after every event.
- **view() must be pure over state**: build the tree from `s`, no
  side effects. Handlers (`on_click`, `on_change`, ...) do the
  mutating.
- **`if __name__ == "__main__":` is required.** Live reload
  re-executes the file under a different module name and swaps
  `view` while the state object survives; without the guard the
  reload would try to start a second app (it is a no-op, but keep
  the guard).

## Rules that bite

1. **An element can appear once.** Constructors consume their
   children; placing one element twice raises. Build fresh elements
   each call.
2. **TextField is controlled**: pass the current value from state
   and write it back in `on_change` —
   `ui.text_field(s["q"], on_change=lambda t: s.update(q=t))`.
3. **`ui.every(seconds, cb)` must be called before `ui.run`** (put
   it under the `__main__` guard). Timer changes need an app
   restart; reloads keep the original timers.
4. **`list_view` is virtualized**: `ui.list_view(count, row)` calls
   `row(i)` only for visible rows (~14–17 of 100k). Never
   pre-build big lists as columns; use `list_view`.
5. Charts take sequences of floats: `ui.bar_chart(data, height=140.0)`;
   numpy arrays work (`.tolist()` not required, but cheap and safe).
6. Expensive recomputation belongs in handlers, not in `view()` —
   e.g. recompute a filtered index in `on_change` and store it in
   state; `view()` just reads it.
7. Colors are hex strings (`"#8a8f98"`); sizes are px floats; `0.0`
   usually means "engine default"; `grow=1.0` fills the parent's
   main axis.

## Catalog

`text` `button` `text_field` `column` `row` `grid` (`columns=`,
`rows=`; a button inside spans cells with `col_span=`/`row_span=`)
`stack` `list_view` `scroll_view` `h_scroll_view` `data_table`
`modal` `image` `svg` `bar_chart` `line_chart` `progress`
`spinner` — plus `every(seconds, cb)`, `task(work, on_done,
on_error)` and `run(view, state=None, title="pixie", watch=True,
theme=None)` (`theme="light"|"dark"`; Cmd+T flips live). Buttons
take `color`/`hover_background`/`active_background`/`border_*`;
containers take `border_radius`/`border_width`/`border_color`.

## Slow work: ui.task

Never block a handler — `time.sleep`, requests, file crunching all
freeze the window. Instead:

```python
def start():
    s.update(busy=True)
    ui.task(fetch_data, on_done=lambda v: s.update(busy=False, data=v),
            on_error=lambda e: s.update(busy=False, error=str(e)))
```

`work` runs on a worker thread — it must NOT call `ui.*` or touch
elements; return a value instead. `on_done`/`on_error` run on the UI
thread and may mutate state freely; a rebuild follows automatically.
Callable from handlers, timer ticks, other callbacks, or before
`run()`. Headless scripts wait for task completion deterministically,
so `_headless` tests cover task flows too.

## Running and reload

```console
$ uv run app.py        # PEP 723 header may declare numpy etc.
```

Edit `view()` while the window is open and save: the window updates
in place; state (and typed TextField content bound to state)
survives. A Python exception in a handler prints to the terminal and
the app keeps running; an exception in `view()` renders an error
line instead of the tree.

## Verification habit

Verify WITHOUT a window first: `PIXIE_SCRIPT="click:+1,input:hello"
uv run app.py` runs the app headless and dumps the element tree
before and after the steps (steps: `click:<label>`,
`input[@n]:<text>`, `submit[@n]`, `advance:<ms>`, `theme:light|dark`,
`a11y`, `mem`). Assert on the dump in tests via
`ui._headless(view, state, script) -> str`. Timers do not run
headless. Then run windowed once to confirm the look. For
virtualized lists, `PIXIE_TRACE_LAZY=1` prints the built row
ranges — `building rows 0..17 of 100000` is what correct looks
like.
