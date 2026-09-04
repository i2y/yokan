# Verify and ship

The [tour](tour.md) concludes: type checkers, headless runs, the gate, shipping — and what does not work yet.

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

## Headless runs and the gate

Running without a window is where verification starts.

```console
$ PIXIE_SCRIPT="click:+1,input:Momo" uv run app.py
```

The step vocabulary is `click[@n]:<label>` (a button, a link, or a table's column header), `input[@n]:<text>`, `submit[@n]`, `slide[@n]:<value>`, `select[@n]:<label>` (a chooser's option, or a table's row by its first cell), `advance:<ms>`, `theme:light|dark`, `a11y`, `mem`, `dump`.
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
The steps are collected on the [Installation](installation.md) page.

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
- **Iterating a list of models directly in a view.** Today, assemble the display strings on the store side and hand them to `list_view`.
- **On a canvas**: no mouse, no tilemap and no camera offset; coordinates are whole pixels (a float is refused and asks for `int(...)`); the scale is a number the app declares rather than a fit to the window, because the painted size would then depend on a window the dump cannot see; and a sprite's PNG is found next to the app, so a missing one paints nothing. What a canvas paints is not readable by assistive technology either — it reports as one image, and `a11y_label=` is the honest way to say what is on it.
- **A `Weak` field on a store.** A store is an owner; the non-owning reference belongs on the model side (the back pointer).
- **Type names the native side already uses, such as `Vec`.** Refused; pick another (`V2`, say).
- **Statements at module level.** The compiled app reads the module's declarations (imports, `State`, classes, defs, `style()`, type aliases, literal constants, `every(...)` timers, the `__main__` guard) and never executes it, so a `count.set(5)` or a `fs.write_text(...)` outside a function is refused. Startup work goes in a def passed as `run(view, on_start=setup)`.
- **Starting a timer from a handler.** A timer is a declaration (`every(1.0, tick)` at module level), so what a handler changes is what the tick reads.
- **`task`'s `on_error=`.** The failure path waits on the error union; catch a failing standard-library call with `try` / `except` around the call.
- A component's `local` is **identified by call site**. Reordering the calls reassigns the states.
- Placing the same element object **twice**. Constructors consume their children.
- **A method that returns `T | None`.** Scalars, lists, value classes and enums come back from a store or model method; an Optional return is not in the dialect yet.
- **A local dict**, and a local list without an annotation (`out: list[str] = []` says what the compiled side needs to know).
- **str methods that answer something the dialect has no shape for**: `.encode()` (bytes), `.format()` and `.translate()` (a template or a table built at run time), `.casefold()` (its mapping expands `ß` to `ss`, which is a different Unicode table from the one the case methods use). What is in: `.partition()`, `.rpartition()`, `.upper()`, `.lower()`, `.title()`, `.capitalize()`, `.swapcase()`, `.strip()` / `.lstrip()` / `.rstrip()` (with or without a set of characters), `.split()`, `.splitlines()`, `.join()`, `.startswith()`, `.endswith()`, `.replace()`, `.find()`, `.rfind()`, `.index()`, `.rindex()`, `.count()`, `.zfill()`, `.ljust()`, `.rjust()`, `.center()`, `.expandtabs()`, `.removeprefix()`, `.removesuffix()`, the `.is…()` family, `len(s)`, `s[i]`, `s[a:b]` and `in`.
- **Format specs beyond fill, align, sign, width, `,`, precision and `d` / `f` / `e` / `%` / `s`** (`#`, `b` / `o` / `x`, `n`, `g`).
- **Some control flow**: nested defs (a closure has no compiled shape — define helpers at module level) and a conditional expression in a view (branch the elements with `if` there).
- **A component parameter that is a value class or an enum**, and a body that is not one container (a top-level `if`, or several elements — wrap them in a `column`). Callback and State parameters work: a component that takes one becomes a view per call site.
- **`set`.** A Python set iterates in an order the compiled side would not reproduce, so it is refused rather than reordered; a `list` covers it. A tuple is in — see [Tuples](tour-logic.md#tuples) — but only where its shape is written out: a tuple that a Rust crate would have to answer is not carried yet, which is why `re.findall` still refuses a pattern with two groups or more.
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
The design principles behind it are collected in [DESIGN.md](https://github.com/i2y/yokan/blob/main/crates/yokan/DESIGN.md).
