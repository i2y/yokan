# Values and types

The [tour](tour.md) continues: value classes, interfaces, memory, sum types, Optional and Enum.

## Value classes and interfaces

Data itself lives in **value classes**.
A class marked `@value` compiles to a native struct (it is the same thing as `@dataclass(frozen=True)`, and that spelling works too).
Immutability is what makes "a Python reference" and "a native value" mean the same thing, so updates are functional, via `replace`.
A field may hold another value class declared earlier (nested values).

```python
from dataclasses import replace


@value
class Point:
    x: int
    y: int = 0

sel: State[Point] = State(Point(3, 4))
sel.set(replace(sel(), x=10))
text(f"x={sel().x}")
```

Value classes can have methods.
Define the operator dunders (`__add__`, `__sub__`, `__mul__`) and `+` `-` `*` take those meanings (during development Python itself runs them; the shipped app calls the same computation, compiled).
A body is a single `return expression` (an immutable value has nothing to assign to).

```python
@value
class V2:
    x: int
    y: int

    def __add__(self, o: "V2") -> "V2":
        return V2(self.x + o.x, self.y + o.y)

    def __mul__(self, k: int) -> "V2":
        return V2(self.x * k, self.y * k)

    def dot(self, o: "V2") -> int:
        return self.x * o.x + self.y * o.y

c.set(a() + b() * 2)      # operators go to the dunders
d.set(a().dot(b()))       # plain methods from handlers
```

Interfaces are `typing.Protocol`.
A model that lists a Protocol with method stubs as a base implements it, and a helper taking a Protocol-typed parameter compiles to a statically dispatched generic function.

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

## Memory

There is nothing to free by hand.
Two shapes cover everything.

- **Values** (Value classes, lists, dicts, strings) mean copies.
  If the place you passed one to changes it, your side does not change.
  The release build shares the storage until the moment of the write (copy-on-write), so passing a large list costs no duplication.
- **Models** (and the stores that hold them) are references.
  The release build reference-counts them and frees at the very assignment that drops the last owner.
  No collector scans the heap; nothing pauses.

The daily habits fall out of those two.

- Keep data in Value classes and lists, on store fields. Reserve models for what is shared, mutated, and watched by the screen.
- A model created inside a handler and never handed out is freed when the handler ends — the temporaries a loop makes included.
- Cut an ownership chain (`self.root = None`) and everything below it is freed together; a surviving `Weak` reads back None.

Cycles are the one exception.
Objects that own each other never let go, and the release build does not free them (a leak, never a crash).
The habit is to not build cycles: make the back reference `Weak`.
One honest note: the CPython you develop on does collect cycles, so a cycle you build by mistake is the one place the two runs' memory behavior differs.
The gate compares screens; memory is outside what it checks.
`check` warns in its place: when the field types close a loop through two or more model classes, and when a handler writes the round trip (`a.kid = b`, then `b.parent = a`).

The number of live objects is countable at any time with the headless `mem` step.

## Sum types and match

Bundle value classes with a `type` alias and you have a set of alternatives — a sum type with payloads.
`match` works in handlers and in views, with destructuring.

```python
@value
class Healthy: pass


@value
class Degraded: services: int


@value
class Outage: service: str

type Health = Healthy | Degraded | Outage

health: State[Health] = State(Healthy())

# in the view:
match health():
    case Healthy():
        text("ALL SYSTEMS NOMINAL")
    case Degraded(services):
        text(f"DEGRADED — {services} service(s)")
    case Outage(service):
        text(f"OUTAGE — {service} is down")
```

Missing arms are reported at compile time.
Variant fields cannot take defaults, and a variant belongs to exactly one sum type.
Arms take guards and `|` alternatives, and a guard that fails falls through to the arms below it, as Python's does.

```python
match health():
    case Degraded(services) if services > 3:
        text("badly degraded")
    case Healthy() | Degraded(_):
        text("fine enough")
    case _:
        text("down")
```

## Optional and Enum

Optional works in state and in fields (`last: int | None = None`).
Narrowing is as shown in the walrus sections.

Enums are ordinary `class Mood(Enum)` and compile as-is (`import enum` with `class Mood(enum.Enum)` too).
`match` arms are `Mood.MEMBER` or `_`, and missing arms are reported.
In text they render exactly as Python does: `Mood.HAPPY`.
`.name` and `.value` read what Python reads (`auto()` counts from 1), and `for m in Mood:` walks the members in declaration order.
`repr(m)` is what Python writes for a member, `<Mood.HAPPY: 1>`, and `repr` of an int, a float, a bool or bytes is what it writes for those.
A str's `repr` is not in the dialect yet: Python escapes what Unicode's own table says it cannot print, and the compiled side does not carry that table.

`match` also takes int, float, str and bool values, with `|` alternatives and guards:

```python
match code():
    case 0 | 1:
        note.set("early")
    case n if n > 100:
        note.set("far")
    case _:
        note.set("middle")
```

## Names for types

A typed module names its types before it uses them, and the dialect reads those names the way a type checker does.

```python
import enum
from typing import NamedTuple, NewType, TypeAlias

Gram = NewType("Gram", int)          # a unit: an int to the compiled side


class Size(enum.Enum):
    SMALL = "small"
    LARGE = "large"
    HUGE = "huge"


class Parcel(NamedTuple):
    weight: Gram
    size: Size


Parcels = list[Parcel]               # another name for a type
Fees: TypeAlias = dict[str, int]     # the same, said outright
type Labels = list[str]              # and in the newer spelling
_len = len                           # a builtin under a name of the module's own

OVERSIZE = frozenset({Size.LARGE, Size.HUGE})


def oversize_weight(parcels: Parcels) -> Gram:
    total = Gram(0)
    for p in parcels:
        if p.size in OVERSIZE:
            total = Gram(total + p.weight)
    return total
```

A NewType is a name a type checker keeps apart from its base, and nothing at run time: `Gram(5)` answers the 5 it was given.
So it is its base wherever an annotation names it, and a call to it is held to that base (`Gram(1.5)` is refused, because Python would hand back the float).
`isinstance(x, Gram)` is refused for the reason Python raises TypeError there: a NewType has no class.
An alias is the type it names, in any of the three spellings; a union of value classes is a sum type, and keeps its own spelling (`type Shape = Circle | Rect`).
Another name for a builtin, a function or a class is that thing wherever it is called, except inside a function that binds the original name itself: there Python's alias still reads the builtin, so the call is refused and asks for the function's own name to change.
A `frozenset` of literals, and a set written in place (`x in {A, B}`), answer `in` and `not in`: the one question a set answers the same way in both runs, whatever order it would walk in.

A class or a local can also take a name the compiled side uses for something of its own: a class called `Element` or `List`, a local called `state`, `case` or `_`.
A class with no decorator (a NamedTuple, an Enum, an exception) and a function's own parameters and locals are given another name on the compiled side, where nobody sees it; the interpreted run, a `str()` of an Enum member and every message keep the name you wrote.
A store, a model or a value class keeps its name, because something finds it by that name (the interpreted run finds a store's methods, and a value class twins a Rust struct), so for them the name is refused and another one asked for.
