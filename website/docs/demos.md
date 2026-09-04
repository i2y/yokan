# Demos

Every demo is one file (opsboard and multi are directories) and runs
as-is from `crates/yokan/` in the repository:

```console
$ cd crates/yokan
$ uv run demo/counter.py            # substitute any demo's name
$ ./tools/gate_all.sh               # gate-check every demo at once
```

The three numpy demos (pystats / csv_viewer / app) run with
`uv run --with numpy`. Two demos — `app` and `csv_viewer` —
use dict state and are development-only by design (see the
[What does not work yet](tour-ship.md#what-does-not-work-yet) section); they are listed here, not gated. All screenshots show
the initial state, right after launch.

## Start here

#### counter — the smallest app. The same app in two other spellings: counter_state.py (typed State cells) and counter_with.py
<img src="images/demos/counter.png" width="360">

#### opsboard — the flagship: a three-module dashboard (two stores, a sum-typed health model, charts, a virtualized alert feed, theme flip, fs report export)
<img src="images/demos/opsboard.png" width="720">

#### forms — the full set of form controls: checkbox / switch / slider / select / radio_group / tab_bar; each handler receives the one new value
<img src="images/demos/forms.png" width="360">

#### calc — the classic keypad calculator: the layout is all `grow`, so resizing scales the whole pad with no dead space
<img src="images/demos/calc.png" width="300">

#### calcgrid — the same calculator on `grid(columns=4, rows=5)`; the zero key spans two cells with `col_span=2`
<img src="images/demos/calcgrid.png" width="300">

## Holding state

#### stores — named stores: the class name IS the singleton, and stores call each other's methods
<img src="images/demos/stores.png" width="360">

#### models — @model and Protocol: observed objects, and statically dispatched interfaces
<img src="images/demos/models.png" width="360">

#### links — models referencing models: owning `Node | None`, non-owning `Weak[Node]` back pointers (no cycles, so dropping the root frees the chain)
<img src="images/demos/links.png" width="360">

#### stateful — @component + local: a component with its own state per call site
<img src="images/demos/stateful.png" width="360">

#### lookup — dict cells: reads via `.get(key, default)` and `in`, writes in place with `cell[k] = v`
<img src="images/demos/lookup.png" width="360">

#### mixer — a fields-only @store: annotated fields, direct assignment, the screen follows
<img src="images/demos/mixer.png" width="360">

## Values and types

#### points — Value classes (frozen dataclasses): updates are functional, via `replace`
<img src="images/demos/points.png" width="360">

#### vecops — operators on Value classes: define `__add__` / `__sub__` / `__mul__` and `+` `-` `*` mean that
<img src="images/demos/vecops.png" width="360">

#### geometry — static dispatch through Protocol: the trait story, compiled
<img src="images/demos/geometry.png" width="360">

#### moods — Enum, Optional and animation
<img src="images/demos/moods.png" width="360">

#### pyops — CPython's own arithmetic: `/` `//` `%` `**`, negative indexing, sorted() — byte-identical in both runs
<img src="images/demos/pyops.png" width="360">

#### pytext — bare float / bool / Enum text renders exactly as Python's str()
<img src="images/demos/pytext.png" width="360">

## Control flow and errors

#### flow — real control flow in handlers: if / elif / while / for / break / continue
<img src="images/demos/flow.png" width="360">

#### edges — containment, demonstrated: out-of-bounds and overflow stop the same statement the same way in both runs, and the app keeps running
<img src="images/demos/edges.png" width="360">

#### tryfetch — the full try/except form: catch a failing http call, with `f"{e}"` identical in both runs
<img src="images/demos/tryfetch.png" width="360">

## UI elements

#### todo — the classic todo list
<img src="images/demos/todo.png" width="360">

#### table — data_table: the first `row` is the header, later `row`s are data rows shaded in alternation, and the frame comes with the element
<img src="images/demos/table.png" width="360">

#### dialog — the modal: existing IS being open, so wrap it in `if`
<img src="images/demos/dialog.png" width="360">

#### trend — line and bar charts
<img src="images/demos/trend.png" width="360">

#### styled — named styles (`style` + `**` splat + `|` merge) and theme scopes
<img src="images/demos/styled.png" width="360">

#### cards — components with slots (components that take children)
<img src="images/demos/cards.png" width="360">

#### layout — spacer and divider: a spacer pushes the button to the row's edge, a divider draws a rule (thicker and accent-colored between the sections)
<img src="images/demos/layout.png" width="360">

#### about — link: text that opens a URL, beside a button that copies one to the clipboard
<img src="images/demos/about.png" width="360">

#### badges — text with a box of its own: status pills, a monospaced hash, an underlined note, an ellipsis and a two-line clamp
<img src="images/demos/badges.png" width="360">

#### filter — segmented: the toggle-button chooser over a filtered list
<img src="images/demos/filter.png" width="360">

#### quantities — number_field and int_field: typed numeric inputs that commit on enter, clamp into the range and snap to the step
<img src="images/demos/quantities.png" width="360">

#### loading — progress with a label, a size, and an indeterminate sweep for work with no known length
<img src="images/demos/loading.png" width="360">

#### charts — negative values below the zero line, a pinned range, an axis with gridlines, and two series with their own colors
<img src="images/demos/charts.png" width="360">

#### roster — table: a virtualized table with column tracks, row selection and header sort (the app re-sorts its own lists)
<img src="images/demos/roster.png" width="360">

#### labels — the accessibility properties `role=` and `a11y_label=`, printed by a script's `a11y` step
<img src="images/demos/labels.png" width="360">

#### shared — the shared properties on elements that could not take them before: a themed spacer, an animated segmented, a field spanning two grid tracks, a link with a role, a divider with a tooltip, a disabled button and field, a sized column
<img src="images/demos/shared.png" width="360">

## The standard library

#### picker — file dialogs and dropped files: `fs.open_dialog` / `save_dialog` inside a `task`, `on_file_drop`; a script answers with `file:<path>` and drops with `drop:<path>`
<img src="images/demos/picker.png" width="360">

#### keys — shortcuts, keys, the clipboard and the menu bar: `shortcut("cmd+s", save)`, `on_key(typed)`, `clipboard.set_text` / `get_text`, `menu_item("Count", "Save", save)` — driven in a script with `key:cmd+s` and `menu:Save`
<img src="images/demos/keys.png" width="360">

#### files — yokan.fs: write, append, list a directory, remove (both runs call the same implementation)
<img src="images/demos/files.png" width="360">

#### dbnotes — yokan.sqlite: shape rows with SQL, order with ORDER BY
<img src="images/demos/dbnotes.png" width="360">

#### ledger — a practical app: a household ledger in sqlite, every value a bound parameter
<img src="images/demos/ledger.png" width="360">

#### webfetch — yokan.http: GET, headers, POST, status (an @py fixture server runs in both runs, so the gate needs no network)
<img src="images/demos/webfetch.png" width="360">

#### reader — an http + jsondoc feed reader
<img src="images/demos/reader.png" width="360">

#### stdlib — Python's `math`, `random`, `statistics`, `json`, `datetime`, `time` and `re`, and Yokan's jsondoc and clock
<img src="images/demos/stdlib.png" width="360">

#### dice — Python's `random`: seed it and both runs draw the same sequence
<img src="images/demos/dice.png" width="360">

#### postcard — an image, a vector icon, and `notify.send` (delivered through Notification Center when the app runs as an `.app` bundle)
<img src="images/demos/postcard.png" width="360">

## A Rust crate of your own

#### rustcrate — Rust crates, added with `yokan add`: a local path crate and a crates.io version crate side by side, called by their own snake_case names. The pyproject spelling of the same declaration is `demo/proj/`
<img src="images/demos/rustcrate.png" width="360">

#### dashboard — every(): a timer declared at module level, ticking in both runs (the gate steps it with `advance:`)
<img src="images/demos/dashboard.png" width="360">

#### tasks — task(): slow work off the UI thread, in both runs
<img src="images/demos/tasks.png" width="360">

## Escapes and development-only

#### pystats — @py + numpy: escaped functions ship with CPython embedded in the release binary
<img src="images/demos/pystats.png" width="360">

#### multi — a multi-module app (state.py and widgets.py; helpers become components)
<img src="images/demos/multi.png" width="360">

#### app — a dashboard with numpy (development-only: dict state)
<img src="images/demos/app.png" width="360">

#### csv_viewer — a 100k-row virtualized table + numpy (development-only: dict state)
<img src="images/demos/csv_viewer.png" width="360">

