# Flow and data

The [tour](tour.md) continues: what handlers can do, arithmetic with CPython's meaning, lists, charts and dicts.

## Handlers and control flow

Handlers can be passed in three forms:
a lambda (a tuple for multiple operations, `lambda: (a.set(x), b.set(y))`), a module-level def, and a store's bound method (`on_click=Cart.clear`).

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
```

Long lists go to `list_view`.
It is **virtualized**: the row builder `row(i)` is called only for the visible range (a dozen or so calls even at 100k rows).

```python
def row(i):
    return text(items()[i])

list_view(len(items()), row, item_height=22.0, height=200.0)
list_view(len(items()), row, item_height=22.0, grow=1.0)   # fill the parent's remaining height
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

Read with `.get`, write per key, count with `len`, iterate with `sorted()`.
A key is any str the app can name — a literal, a state read, a loop variable.

```python
prices["cherry"] = 200                 # per-key write
picked.set(prices().get("apple", -1))  # read: default when missing
if "cherry" in prices(): ...           # membership
len(prices())                          # count

def scan():
    for k in sorted(prices()):         # iterates in key order
        last.set(k)
```

Bare `d[k]` reads and bare `for k in d` are refused.
Missing keys and iteration order are the places where Python is particular, and `get` / `sorted()` are the forms that state the intent.

