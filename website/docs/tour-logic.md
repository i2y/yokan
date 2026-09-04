# Flow and data

The [tour](tour.md) continues: what handlers can do, arithmetic with CPython's meaning, lists, charts, dicts and tuples.

## Handlers and control flow

Handlers can be passed in three forms:
a lambda (a tuple for multiple operations, `lambda: (a.set(x), b.set(y))`), a module-level def, and a store's bound method (`on_click=Cart.clear`).

A decorator compiles too. Decoration happens at import and the compiled app never runs the module, so the wrapper is folded into the handler it decorates:

```python
def announced(f):
    def wrapper():
        status.set("working")
        f()
        status.set("done")

    return wrapper

@announced
def save():
    fs.write_text(path, body())
```

The decorator is a def of one argument that either returns that argument or defines a wrapper calling it once.
A decorator that takes arguments of its own, or calls the function twice, or uses its value, is refused by name.

The body of a def handler compiles with its real control flow.

```python
def double(v: int) -> int:          # a pure helper becomes a native function
    return v * 2

def tally():
    total.set(0)
    for i in range(1, 6):
        if i == 3:
            continue
        total.set(total() + double(i))
```

Available: `if` / `elif` / `else`, `while` (`while True:` included), `for` (over `range()`, list states, list fields, list-typed parameters), `break` / `continue`, and locals (reassignable, as in Python).
`log("…")` writes a line to stderr from either run, and `assert` / `raise` end the statement the way Python's exception does — the app keeps running.
Conditions take a bool directly (`if on:`), chain comparisons (`0 < n < 10`, the middle read once), and bind with `:=`.
A conditional expression (`a if c else b`) is written in a handler, over int, float, str or bool.
A pure helper (parameters and return annotated, body ending in `return expression`) is callable from handlers and from view text; it may return early from a branch, call itself, take `list[...]` parameters and default arguments, and return a value class or a list.

A local assigned in **both** the if and the else reads fine after the branch, as in Python.

```python
def judge():
    n = score()
    if n > 20:
        verdict = "high"
    else:
        verdict = "low"
    grade.set(verdict)      # readable after the branch
```

Reading a local assigned in only one branch is refused (had the branch not run, Python would raise NameError there).

Optional narrowing is written with the walrus.

```python
if (v := sel()) is not None:
    text(f"picked {v}")      # v is bound only inside this branch
else:
    text("(none)")
```

## Arithmetic

Python's arithmetic operators work as-is.
Besides `+`, `-`, `*`: `/` (the result is always float), `//` (floor toward negative infinity), `%` (the result takes the divisor's sign) and `**` all compile to **exactly Python's results**.

```python
q.set(1 / 3)          # 0.3333333333333333
d.set(-7 // 2)        # -4
r.set(7 % -2)         # -1
p.set(2 ** 10)        # 1024
```

Division by zero and overflow are exceptions in Python, and in the shipped app they surface the same way — the statement aborts, the app does not crash.
Write `int ** int` exponents as non-negative literals (a negative exponent would change the result's type at runtime; with either side a float, negative exponents are fine).
Do fallible `/` `//` `%` `**` inside handlers and hand the view the result.

`and`, `or`, `not` work in conditions as-is.
They also work as bool values (`both.set(hot() and not cold())`).
Using `and` / `or` as a value on non-bools is refused (Python returns **one of the operands themselves** there, which is a different thing from a truth value).

## Strings

Strings work as they do in Python: the methods, the length, indexing and slicing, `in`, and the conversions.

```python
name.set(raw().strip().upper())
parts.set(raw().split(","))
name.set(", ".join(parts()))
first.set(raw()[0] + raw()[1:4])          # a code point, then a slice
n.set(len(raw()) + raw().find("a"))
if "ada" in raw().lower():
    tag.set("found")
n.set(int("42") + int(2.5) + round(2.5))  # round-half-to-even, as Python does
```

As with Python's arithmetic, the two runs use different code here — CPython's own method while you develop, a Rust twin written to answer exactly the same thing once compiled, failures included, so `int("x")` stops that statement in both — and the gate is what holds them together.

Format specs are Python's, in views and in handlers alike.

```python
text(f"{total():,}")            # 1,234,567
text(f"{ratio():.1%}")          # 12.5%
text(f"{name():>10}")           # right-aligned in ten columns
text(f"{value():.2e}")          # 1.50e+00
```

## Lists, charts, virtualized lists

Append to a list by concatenating and putting it back.
The shipped app compiles this to a one-element append, so there is no copy cost.

```python
items.set(items() + [x])     # append
items.set([])                # clear
len(items())                 # count
```

The rest of Python's list vocabulary works in handlers: `in`, slices, `sorted` / `min` / `max` / `sum`, comprehensions, `enumerate` and `zip`, a stepped `range`, and joining two lists.
A local list carries its element type in the annotation, which is what the compiled side reads.

```python
out: list[str] = []
for i, s in enumerate(items()):
    if s != "":
        out = out + [f"{i}: {s}"]
items.set(sorted(out))
best.set(max(scores()))
```

The operations that say nothing about the element — `in`, a slice, `+`, `[::-1]` — take a list of anything the app can hold, a value class or a tuple included.
Comparing needs to know what to compare, so `sorted`, `min` and `max` take a `key=` for those, and a `reverse=` to turn the order around.
The key is a lambda of one element or the name of a helper that takes one, and sorting is stable: elements with equal keys keep the order they came in, with `reverse=True` as well.

```python
by_score = sorted(players(), key=lambda p: p.score, reverse=True)
leader = max(players(), key=lambda p: p.score)
names = [p.name for p in players()]
newest = entries()[::-1]
```

`reversed(xs)` is Python's iterator, so it walks a list backwards in a `for`; where a list is wanted, `xs[::-1]` is the one that is a list in Python too.

Indexing reads an element, with Python's meaning: a negative index counts from the back, and an index past the end stops that statement in both runs.

```python
first.set(names()[0])        # a state read, indexed
tail.set(names()[-1])        # last element (too short: the statement aborts)
for i in range(len(Cart.items)):
    Cart.items[i] = "-"      # `self.xs[i]` inside the store says the same
```

Charts draw lists of float or int.

```python
values: State[list[float]] = State([])
line_chart(values(), height=120.0)
bar_chart(Metrics.svc_reqs, labels=Metrics.svc_names, height=100.0)
bar_chart(Books.profit, labels=Books.months, axis=True)          # negative months hang below the zero line
line_chart(series=Traffic.lines, colors=["accent", "#f38ba8"], axis=True)
```

The range spans the data and always contains zero, so a negative value hangs below the zero line; `min=` / `max=` pin it instead.
`axis=True` adds tick labels and gridlines.
`series=` takes a `list[list[float]]` field for several lines or bar groups, `colors=` names one color per series, and `color=` colors a single series (`demo/charts.py`).
`progress(value)` fills a track: `width=` / `height=` size it, `label=` captions it, and `indeterminate=True` sweeps a segment instead, for work with no known length.

Long lists go to `list_view`.
It is **virtualized**: the row builder `row(i)` is called only for the visible range (a dozen or so calls even at 100k rows).

```python
def row(i):
    return text(items()[i])

list_view(len(items()), row, item_height=22.0, height=200.0)
list_view(len(items()), row, item_height=22.0, grow=1.0)   # fill the parent's remaining height
```

A table is a `list_view` with a header and column tracks.
`table(columns, count, row)` calls `row(i)` for the visible rows only, and the builder returns a `row` of one cell per column; `widths=` are the tracks' shares.
`selected=` tints a row and `on_select` receives the clicked row's index; `sort=` / `descending=` draw the header's arrow and `on_sort` receives the clicked column's index — the app re-sorts its own lists.
In scripts, `select:<first cell>` picks a row and `click:<column>` sorts (`demo/roster.py`).

```python
def cells(i: int):
    return row(text(Roster.names[i]), text(f"{Roster.scores[i]}"))

table(["member", "score"], len(Roster.names), cells, widths=[2.0, 1.0],
      selected=Roster.sel, on_select=Roster.pick,
      sort=Roster.sort_col, descending=Roster.desc, on_sort=Roster.sort_by, grow=1.0)
```

The row index is an int the row can use anywhere: in the text, in a condition, and in the row's own handlers.

```python
def line(i):
    with row(spacing=6):
        text(f"{i + 1}. {items()[i]}")
        if i == Sel.idx:
            text("*")
        button("delete", on_click=lambda: Sel.drop(i))

list_view(len(items()), line, item_height=24.0, height=200.0)
```

## Dicts

Read with `.get`, write per key, count with `len`, walk it like a Python dict.
A key is any str the app can name — a literal, a state read, a loop variable.

```python
prices["cherry"] = 200                 # per-key write
picked.set(prices().get("apple", -1))  # read: default when missing
if "cherry" in prices(): ...           # membership
len(prices())                          # count

def scan():
    for k in prices():                 # insertion order, as Python walks it
        last.set(k)
    for v in prices().values():        # the same order
        total.set(total() + v)
    for k in sorted(prices()):         # key order, when that is what you mean
        first.set(k)
```

A compiled dict remembers the order its keys went in, so a walk visits them in the order Python does.
Bare `d[k]` reads are refused: they raise `KeyError` when the key is missing, and `.get(key, default)` says what a missing key means.
`.items()` walks the pairs, in the same insertion order.

## Tuples

A tuple is a value with a part for each position, written and read the way Python writes one.

```python
pair: State[tuple[str, int]] = State(("momo", 4))
rows: State[list[tuple[str, int]]] = State([])

def measure(word: str) -> tuple[str, int]:
    return (word.upper(), len(word))

def scan():
    label, n = measure("hello")          # unpacking
    first = pair()[0]                    # a part, by a literal position
    whole, rest = divmod(n, 3)
    for name, count in rows():           # a pair per row
        total.set(total() + count)
    for key, value in prices().items():  # and a dict walks as pairs
        seen.set(seen() + key)
```

The parts have types of their own, so a tuple is indexed by a literal position: a computed index would have no one type to be.
Two parts or more, and the same shape can be a state, a field, a list's element, a parameter and a return.
