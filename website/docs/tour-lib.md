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

Use it with `from yokan import fs, sqlite, http, math, json, time, strings, random, notify`.
Each one calls the same function, implemented in Rust, during development and after shipping alike.
The shipped binary needs no Python.
Call them from handlers (views stay pure).

- **fs**: `read_text` / `write_text` / `exists` / `read_text_or`
- **sqlite**: `exec` / `query_text` / `query_int` / `query_int_or` / `query_text_or` (SQLite bundled. Wrap aggregates in COALESCE and pin the order with ORDER BY)
- **http**: `get_text` / `get_text_or` (synchronous)
- **math**: `sqrt` / `sin` / `cos` / `pow` / `fabs` / `floor` / `ceil` / `pi`
- **json**: `get_text` / `get_int` / `get_float` / `get_bool` / `length` / `has` (looked up by dotted paths like `"items.0.title"`)
- **time**: `now_ms`, `format_ms(ms, "%Y-%m-%d")` (UTC. In verification scripts, pass a fixed ms)
- **strings**: `to_int(s, default)` / `to_float(s, default)` (numeric parsing where broken input becomes the default)
- **random**: `seed(n)` / `int(lo, hi)` (inclusive on both ends) / `float()` (seed it and the sequence repeats)
- **notify**: `send(title, body)` — an OS notification, delivered through Notification Center when the app runs as an `.app` bundle (`--app`); a bare dev run and headless runs drop it quietly

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

Annotate every parameter and the return (int / float / str / bool / list[...]).
Compiled extensions like numpy work inside escapes.

## Heavy work and timers

Never block a handler (the window freezes).
`task` does the work on a worker thread and runs the continuation on the UI thread when it finishes.

```python
def start():
    busy.set(True)
    task(fetch_data,
            on_done=lambda v: (busy.set(False), data.set(v)),
            on_error=lambda e: (busy.set(False), err.set(str(e))))
```

`work` must not build UI elements; it just returns a value.
Headless runs wait for task completion before taking the next step, so flows containing tasks are testable.
Today `task` runs in the development run only, and the compiler refuses the call by name until the compiled run has a worker thread to give it (see [What does not work yet](tour-ship.md#what-does-not-work-yet)).
Until then, a compiled handler that fetches over `http` waits for the reply, and the window waits with it.

`every(seconds, cb)` is a timer with a seconds interval.
Call it before `run`.
Timers are a development-run feature: rather than shipping an app that starts without the timer, the compiler refuses the `every` call by name (see [What does not work yet](tour-ship.md#what-does-not-work-yet)).

