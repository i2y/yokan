# Types

**You write the types at the edges. Inside a function they are
inferred. Where neither can answer, the build refuses and says what to
write instead.**

That is the whole rule. The rest of this page is what "the edges"
means, what inference reaches, and which of the three checks answers
which question.

## Where you write them

An edge is anywhere a value outlives the function it was made in, or
leaves the dialect entirely.

| Edge | Written as |
|---|---|
| Module state | `count: State[int] = State(0)` |
| Store and model fields | `total: int = 0` |
| Value class fields | `x: float` |
| Helper parameters and return | `def area_of(s: Shape) -> float:` |
| Component parameters | `def card(title: str, n: int):` |
| A `@py` signature | every parameter, and the return |

The reason is the same each time: the compiled side lays the value out
in memory before the app runs, so it has to know the shape before the
first line executes. A `State[int]` is a cell of a machine integer,
not a box that finds out later.

## Where they are inferred

A local takes its type from what it is assigned. Nothing below is
annotated, and every type is known:

```python
def restock():
    n = count()                 # int   — count is State[int]
    label = f"{n} left"         # str   — an f-string is a str
    names = items()             # list[str]
    first = names[0]            # str   — an element of list[str]
    total = n * 2 + 1           # int   — int arithmetic
```

Inference is local and it does not guess. When the right-hand side
carries no type of its own, you write one:

```python
out: list[str] = []             # an empty list has no element type
```

That is the shape of most annotations you will write inside a
function: a container that starts empty.

## The vocabulary

| Type | Notes |
|---|---|
| `int` `float` `str` `bool` | machine integers and doubles; `str` is Python's |
| `list[T]` | `T` is a scalar, a value class, an enum, or a list |
| `dict[str, T]` | string keys; iterates in insertion order in both runs |
| `tuple[A, B]` | a fixed shape, written out |
| `@value` class | data: fields, no identity |
| `@model` class | observed and shared; reached by reference |
| `Enum` | Python's `enum.Enum` |
| `Protocol` | an interface; see [Generics](#generics) below |
| <code>T &#124; None</code> | an optional, consumed by comparison or `match` |
| `State[T]` | a cell |
| `Weak[T]` | a non-owning reference; reads as <code>T &#124; None</code> |

The three shapes of state are `State`, `@store` and `@model`, and the
[language tour](tour.md#holding-state) walks the choice between them.

## What checks what

Three answers arrive at three different moments, and they answer
different questions.

**Your editor.** Type stubs ship with the wheel, so pyright and
Pylance check a Yokan app the way they check any Python. `@store`
binds the singleton to the class name, `@value` and `@model` carry
their field constructors, and `Weak[Node]` reads as `Node | None`.
This is Python's own type system, live as you type.

**`yokan check app.py`.** The dialect's boundary: is this shape one
the compiler can take? No compiler is started, so the answer comes in
about a second, and it comes as a file, line and column with the fix
named:

```console
$ yokan check app.py
app.py:14:5: not in the dialect — a `list[str]` local starts from a
list literal or another list of the same type
    part = line.split("\t")
           ^
```

**The build.** Underneath the dialect sits the compiler's own checker,
and it is the one that catches a type that neither of the first two
could see.

None of the three is the gate. `yokan gate` compares *behaviour*: it
replays the same clicks through the development build and the shipped
binary and byte-compares the screens. Types answer "will this
compile"; the gate answers "do the two runs agree".

## Generics

An interface is a `typing.Protocol`, and a helper that takes one
becomes a generic function:

```python
class Shape(Protocol):
    def area(self) -> float: ...


@model
class Circle(Shape):
    r: float = 1.0

    def area(self) -> float:
        return self.r * self.r * 3.0


def area_of(s: Shape) -> float:
    return s.area()
```

`yokan translate app.py` shows what that becomes:

```
fn area_of<P0: Shape>(s: P0) Float {
```

A type parameter bounded by the interface. Dispatch is static and the
compiler emits one copy per type it is called with, so there is no
lookup at run time and nothing is erased.

You never write the type argument. `area_of(circle)` solves `P0` from
the argument, one call site at a time.

Container types are parametric too — `list[T]`, `dict[str, T]`,
`State[T]` — and those you do write out, because a container's element
type is part of its layout.

## What it will not infer

Each of these is refused by name rather than guessed at, and the
reason is the same in every case: the compiled side would have to
choose, and a choice it made silently could differ from what CPython
does.

- **An empty container** has no element type. Annotate it.
- **A local assigned in only one branch.** Had that branch not run,
  Python would raise `NameError`; assign in both and it reads fine.
- **A local a text hole cannot render.** A float or a bool in
  `f"{x}"` asks for a State or a pinned `.2f`, because Python's `str()`
  of a float is a specification, not a formatting default.
- **A method returning `T | None`.** Scalars, lists, value classes and
  enums come back from a store or model method; an optional return is
  not in the dialect yet.

The complete list, each item with its reason, is at the end of the
tour: [What does not work yet](tour-ship.md#what-does-not-work-yet).

## Where Python is not enough

When a function needs more of Python than the dialect takes, mark it
`@py` and it stays real Python. Values then cross a boundary, and
what may cross is spelled out in
[Crossing to Python](escapes.md).
