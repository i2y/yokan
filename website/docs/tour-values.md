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

## Optional and Enum

Optional works in state and in fields (`last: int | None = None`).
Narrowing is as shown in the walrus sections.

Enums are ordinary `class Mood(Enum)` and compile as-is.
`match` arms are `Mood.MEMBER` or `_`, and missing arms are reported.
In text they render exactly as Python does: `Mood.HAPPY`.
`.name` and `.value` read what Python reads (`auto()` counts from 1), and `for m in Mood:` walks the members in declaration order.

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

