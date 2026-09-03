# The demo gallery

[日本語版](README.ja.md)

Every demo is one file (opsboard and multi are directories) and runs
as-is from `crates/yokan/`:

```console
$ uv run demo/counter.py            # substitute any demo's name
$ ./tools/gate_all.sh               # gate-check every demo at once
```

The three numpy demos (pystats / csv_viewer / app) run with
`uv run --with numpy`. Two demos — `app` and `csv_viewer` —
use dict state and are development-only by design (see the tour's
[What does not work yet](../TOUR.md#what-does-not-work-yet)); they are listed here, not gated. All screenshots show
the initial state, right after launch.

## Start here

#### counter — the smallest app. The same app in two other spellings: counter_state.py (typed State cells) and counter_with.py
<img src="screenshots/counter.png" width="360">

#### opsboard — the flagship: a three-module dashboard (two stores, a sum-typed health model, charts, a virtualized alert feed, theme flip, fs report export)
<img src="screenshots/opsboard.png" width="720">

#### forms — the full set of form controls: checkbox / switch / slider / select / radio_group / tab_bar; each handler receives the one new value
<img src="screenshots/forms.png" width="360">

#### calc — the classic keypad calculator: the layout is all `grow` (rows share the height, keys share each row, the zero key takes two shares), so resizing scales the pad with no dead space
<img src="screenshots/calc.png" width="300">

#### calcgrid — the same calculator on `grid(columns=4, rows=5)`: equal tracks, one container instead of five rows, and the zero key spans two cells with `col_span=2`
<img src="screenshots/calcgrid.png" width="300">

## Holding state

#### stores — named stores: the class name IS the singleton, and stores call each other's methods
<img src="screenshots/stores.png" width="360">

#### models — @model and Protocol: observed objects, and statically dispatched interfaces
<img src="screenshots/models.png" width="360">

#### links — models referencing models: owning `Node | None`, non-owning `Weak[Node]` back pointers (no cycles, so dropping the root frees the chain)
<img src="screenshots/links.png" width="360">

#### stateful — @component + local: a component with its own state per call site
<img src="screenshots/stateful.png" width="360">

#### lookup — dict cells: reads via `.get(key, default)` and `in`, writes in place with `cell[k] = v`
<img src="screenshots/lookup.png" width="360">

#### mixer — a fields-only @store: annotated fields, direct assignment, the screen follows
<img src="screenshots/mixer.png" width="360">

## Values and types

#### points — Value classes (frozen dataclasses): updates are functional, via `replace`
<img src="screenshots/points.png" width="360">

#### vecops — operators on Value classes: define `__add__` / `__sub__` / `__mul__` and `+` `-` `*` mean that
<img src="screenshots/vecops.png" width="360">

#### geometry — static dispatch through Protocol: the trait story, compiled
<img src="screenshots/geometry.png" width="360">

#### moods — Enum, Optional and animation
<img src="screenshots/moods.png" width="360">

#### pyops — CPython's own arithmetic: `/` `//` `%` `**`, negative indexing, sorted() — byte-identical in both runs
<img src="screenshots/pyops.png" width="360">

#### pytext — bare float / bool / Enum text renders exactly as Python's str()
<img src="screenshots/pytext.png" width="360">

## Control flow and errors

#### flow — real control flow in handlers: if / elif / while / for / break / continue
<img src="screenshots/flow.png" width="360">

#### edges — containment, demonstrated: out-of-bounds and overflow stop the same statement the same way in both runs, and the app keeps running
<img src="screenshots/edges.png" width="360">

#### tryfetch — the full try/except form: catch a failing http call, with `f"{e}"` identical in both runs
<img src="screenshots/tryfetch.png" width="360">

## UI elements

#### todo — the classic todo list
<img src="screenshots/todo.png" width="360">

#### table — data_table: the first `row` is the header, later `row`s are data rows shaded in alternation, and the frame comes with the element
<img src="screenshots/table.png" width="360">

#### dialog — the modal: existing IS being open, so wrap it in `if`
<img src="screenshots/dialog.png" width="360">

#### trend — line and bar charts
<img src="screenshots/trend.png" width="360">

#### styled — named styles (`style` + `**` splat + `|` merge) and theme scopes
<img src="screenshots/styled.png" width="360">

#### cards — components with slots (components that take children)
<img src="screenshots/cards.png" width="360">

## The standard library

#### keys — shortcuts, keys and the clipboard: `shortcut("cmd+s", save)`, `on_key(typed)`, `clipboard.set_text` / `get_text`, pressed in a script with `key:cmd+s`

#### files — yokan.fs: write, append, list a directory, remove (both runs call the same implementation)
<img src="screenshots/files.png" width="360">

#### dbnotes — yokan.sqlite: shape rows with SQL, order with ORDER BY
<img src="screenshots/dbnotes.png" width="360">

#### ledger — a practical app: a household ledger in sqlite, every value a bound parameter
<img src="screenshots/ledger.png" width="360">

#### webfetch — yokan.http: GET, headers, POST, status (an @py fixture server runs in both runs, so the gate needs no network)
<img src="screenshots/webfetch.png" width="360">

#### reader — an http + json feed reader
<img src="screenshots/reader.png" width="360">

#### stdlib — math / json (read and write) / time (UTC and local)
<img src="screenshots/stdlib.png" width="360">

#### dice — yokan.random: seed it and both runs draw the same sequence
<img src="screenshots/dice.png" width="360">

#### postcard — an image, a vector icon, and `notify.send` (delivered through Notification Center when the app runs as an `.app` bundle)
<img src="screenshots/postcard.png" width="360">

## A Rust crate of your own

#### rustcrate — Rust crates, added with `yokan add`: a local path crate and a crates.io version crate side by side, called by their own snake_case names. The pyproject spelling of the same declaration is `demo/proj/`
<img src="screenshots/rustcrate.png" width="360">

#### dashboard — every(): a timer declared at module level, ticking in both runs (the gate steps it with `advance:`)
<img src="screenshots/dashboard.png" width="360">

#### tasks — task(): slow work off the UI thread, in both runs
<img src="screenshots/tasks.png" width="360">

## Escapes and development-only

#### pystats — @py + numpy: escaped functions ship with CPython embedded in the release binary
<img src="screenshots/pystats.png" width="360">

#### multi — a multi-module app (state.py and widgets.py; helpers become components)
<img src="screenshots/multi.png" width="360">

#### app — a dashboard with numpy (development-only: dict state)
<img src="screenshots/app.png" width="360">

#### csv_viewer — a 100k-row virtualized table + numpy (development-only: dict state)
<img src="screenshots/csv_viewer.png" width="360">

