# Crossing to Python

`@py` marks a function that stays **real Python**. The dialect does
not compile its body; it compiles the *call*, and arranges for the
body to run on CPython either way.

```python
@py
def stats(xs: list[float]) -> list[float]:
    import numpy as np

    a = np.array(xs)
    return [float(a.mean()), float(a.std())]
```

Two things cross that boundary: the arguments on the way in, and the
answer on the way back. This page is what happens to them.

## The two runs are not symmetrical

While you develop, `@py` is the identity. Your function is called by
the same CPython that is running the rest of the app, with your own
objects, and **nothing is converted at all**. The annotations are not
even read at run time.

Once compiled, the function's source is carried inside the binary and
runs on an embedded CPython. Now the two sides speak different
languages, and every value is converted on the way in and on the way
back — following the annotations exactly.

That asymmetry is why the signature is required. It is not decoration
for a type checker: it is the contract the compiled run implements,
and `yokan gate` is what proves the two runs came out the same.

## What may cross

| Written as | Goes over as | Rust side |
|---|---|---|
| `int` | a machine integer | `i64` |
| `float` | a double | `f64` |
| `str` | text | `&str` in, `String` back |
| `bool` | a boolean | `bool` |
| `list[T]` | a list of scalars | `Vec<T>` |
| `dict[str, T]` | a string-keyed table | `HashMap<String, T>` |
| a `@value` class | a frozen dataclass | a struct of the same fields |
| <code>T &#124; None</code> | the value, or `None` | `Option<T>` |

`T` is `int`, `float`, `str` or `bool` throughout. Anything outside
this table is refused with the reason, not silently reshaped.

## Going in

Say the app calls `stats(values())` where `values` is a
`State[list[float]]`.

**Development run.** The list held in the State is a Python list. It
is passed. That is all.

**Compiled run.** The call site is a native one, and the list is the
compiler's own list of doubles. Before Python sees it, the generated
crate builds a Python object from it:

- an `int` becomes a Python `int`, a `float` a `float`, a `bool` a
  `bool`
- a `str` becomes a Python `str`
- a `list[T]` becomes a Python `list` of the converted elements
- a `dict[str, T]` becomes a Python `dict`
- a value class becomes an instance of a frozen dataclass with the
  same field names, which the escape module declares for exactly this
  purpose
- `None` stays `None`; anything else in a `T | None` is the value
  itself

Inside the function you are holding ordinary Python objects. `xs` is a
list you can slice, hand to numpy, or iterate.

## Coming back

The return annotation is read the other way.

**Development run.** The object your `return` produced is the value
the handler receives. Again, no conversion.

**Compiled run.** The returned Python object is read back into the
declared type:

- a Python `int` / `float` / `bool` becomes the machine value
- a `str` becomes the compiler's string
- a `list` becomes a `list[T]`, element by element
- a `dict` becomes a `dict[str, T]`
- a dataclass instance becomes the value class, field by field
- `None` becomes the empty half of `T | None`

If the object does not fit the annotation — a `str` returned where the
signature says `int` — the read fails, and it fails loudly rather than
guessing. See [When Python raises](#when-python-raises).

## A value class, both directions

This is the crossing worth seeing whole, because the same shape is
declared twice and the two declarations have to agree.

```python
@value
class Reading:
    label: str
    value: float


@py
def normalise(r: Reading) -> Reading:
    return Reading(r.label.strip().lower(), r.value / 100.0)
```

On the Yokan side `Reading` is a value class: two fields, no identity.
On the compiled side it becomes a Rust struct of the same two fields,
and the escape module gets a matching

```python
@dataclass(frozen=True)
class Reading:
    label: str
    value: float
```

so the Python you wrote can build one and read one. The fields must be
`int`, `float`, `str` or `bool`; a value class holding a list or
another value class does not cross yet, and says so.

## When Python raises

Without a `try`, an exception is not swallowed. The traceback is
printed — the real one, from the real interpreter — and the statement
that called the escape does not complete. The app keeps running; the
handler that raised does not.

With a `try`, the same clause runs in both runs:

```python
def compute():
    try:
        n = parse(text())
    except ValueError as e:
        error.set(f"{e}")
        return
    total.set(n)
```

In the development run that is Python's own `try`. In the compiled
run, the exception is caught **inside** the embedded interpreter by a
dispatcher the compiler generates, which answers with which clause
matched and the message — so `f"{e}"` renders the same bytes on both
sides. The clauses you wrote are what that dispatcher is built from.

## Inside a task

An escape can take a while. Put it in a `task` and the window keeps
drawing:

```python
def start():
    xs = values()
    task(lambda: stats(xs), on_done=landed, on_progress=moved)
```

Compiled, the escape is *awaited*: it runs on the engine's thread pool
rather than on the thread that paints. From inside the escape,
`report(fraction, note)` reaches this task's `on_progress`:

```python
@py
def transcribe(path: str) -> list[str]:
    from yokan import report

    report(0.0, "loading the model")
    ...
```

That import is the same line in both runs. The development run imports
the real module; the compiled binary's embedded interpreter has no
yokan package at all, so the generated crate installs a one-function
`yokan` module for it, reaching the same channel.

One rule comes with it: **a task's work does not touch app state.** It
runs off the UI thread, where a `State` cannot be read. Read what it
needs above the task, hand the value in, and write what comes back in
`on_done` — which is what `xs = values()` is doing in the example
above.

## What does not cross

Each of these is refused when you run `yokan check`, with the shapes
that do cross named in the message:

- **Nested containers.** A `list[list[int]]` or a `dict[str, list[…]]`
  in a signature. One level, both directions.
- **A `list` of value classes.** The elements cross one at a time or
  not at all; a list of them does not yet.
- **A `@model` class.** A model is observed and shared, and identity
  is not something to copy across an interpreter boundary.
- **A dict keyed by anything but `str`.**
- **A value class holding anything but scalars.**

## Shipping it

An app with no `@py` links no Python at all. An app that has one ships
CPython inside: `--bundle` produces a folder, `--onefile` a single
file, and `--app` either shape as a macOS `.app`. The person receiving
it installs nothing. The details are in
[Verify and ship](tour-ship.md#shipping).
