# Libraries and crates

The [tour](tour.md) continues: error handling, the standard library, your own Rust crates and the CPython escapes.

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
How far each module reaches into Python's, function by function, is on the [coverage page](support.md), which is generated rather than written.

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


