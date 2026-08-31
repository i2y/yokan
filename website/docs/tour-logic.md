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

Available: `if` / `elif` / `else`, `while`, `for` (over `range()`, list states, list fields, list-typed parameters), `break` / `continue`, and locals (reassignable, as in Python).
A pure helper (parameters and return annotated, body ending in `return expression`) is callable from handlers and from view text.

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
    ui.text(f"picked {v}")      # v is bound only inside this branch
else:
    ui.text("(none)")
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

## Lists, charts, virtualized lists

Append to a list by concatenating and putting it back.
The shipped app compiles this to a one-element append, so there is no copy cost.

```python
items.set(items() + [x])     # append
items.set([])                # clear
len(items())                 # count
```

Indexing from the back takes a literal.

```python
r = names()
tail.set(r[-1])              # last element (too short: the statement aborts)
```

Charts draw lists of float or int.

```python
values: State[list[float]] = State([])
ui.line_chart(values(), height=120.0)
ui.bar_chart(Metrics.svc_reqs, labels=Metrics.svc_names, height=100.0)
```

Long lists go to `list_view`.
It is **virtualized**: the row builder `row(i)` is called only for the visible range (a dozen or so calls even at 100k rows).

```python
def row(i):
    return ui.text(items()[i])

ui.list_view(len(items()), row, item_height=22.0, height=200.0)
ui.list_view(len(items()), row, item_height=22.0, grow=1.0)   # fill the parent's remaining height
```

## Dicts

Read with `.get`, write per key, count with `len`, iterate with `sorted()`.

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

