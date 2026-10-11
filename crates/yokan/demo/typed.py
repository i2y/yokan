# /// script
# requires-python = ">=3.14"
# ///
"""The typed Python a module writes at its top, gated: a NewType for a
unit, type aliases, a builtin under another name, a frozenset constant
asked with `in`, a NamedTuple read by name, by position and by
unpacking, `isinstance` answered from what a value is declared as, an
optional narrowed inside an `or`, a local list grown with `.append`,
and an exception class that carries what it was raised with.

This is the shape a code generator writes, and nothing in it needs a
word changed to compile. Every answer is printed as well as shown, so
the gate compares both channels.
"""
import enum
import os
import sys
from typing import NamedTuple, NewType

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import State, button, column, row, run, text  # noqa: E402

# A unit, which a type checker keeps apart from a plain int and which
# is the int itself at run time.
Gram = NewType("Gram", int)


class Size(enum.Enum):
    SMALL = "small"
    LARGE = "large"
    HUGE = "huge"


class Parcel(NamedTuple):
    """One parcel on the desk."""

    label: str
    weight: Gram
    size: Size
    note: str = ""


# Another name for a type, in both spellings.
Parcels = list[Parcel]
type Lines = list[str]

# A builtin under a name of the module's own.
_len = len

# Asked with `in` only: a set answers membership the same way in both
# runs, whatever order it would walk in.
OVERSIZE = frozenset({Size.LARGE, Size.HUGE})


class TooHeavy(ValueError):
    """A parcel the desk cannot take, and what it weighed."""

    def __init__(self, what: str, value: str | None = None) -> None:
        super().__init__(what)
        self.what = what
        self.value = value

    def __str__(self) -> str:
        return self.what if self.value is None else f"{self.what}: {self.value}"


desk: State[list[Parcel]] = State([
    Parcel("books", Gram(1800), Size.SMALL),
    Parcel("lamp", Gram(4200), Size.LARGE, "fragile"),
    Parcel("tent", Gram(2600), Size.HUGE),
])
summary: State[str] = State("")
pick: State[str] = State("")


def fee(p: Parcel) -> int:
    # The declaration answers this one, so both runs agree on it
    # before either runs; the branch it rules out is never read.
    if not isinstance(p.weight, int) or isinstance(p.weight, bool):
        raise TooHeavy("weight is not a whole number of grams", repr(p.weight))
    if p.weight > 20000:
        raise TooHeavy("over the desk's limit", repr(p.weight))
    # Assigned on every way out of the chain; the last way raises.
    if p.size in OVERSIZE:
        base = 1200
    elif p.size is Size.SMALL:
        base = 500
    else:
        raise TooHeavy("no rate for this size")
    # A note costs extra.
    return base + (300 if p.note != "" else 0)


def heaviest(ps: Parcels) -> Parcel | None:
    best: Parcel | None = None
    for p in ps:
        # Narrowed inside the `or`: past `best is None`, best holds one.
        if best is None or p.weight > best.weight:
            best = p
    return best


def weigh() -> None:
    lines: Lines = []
    total: int = 0
    for label, weight, size, note in desk():
        f = fee(Parcel(label, weight, size, note))
        total += f
        lines.append(f"{label} {f}")
    summary.set(f"{_len(lines)} parcels, {total} yen: {', '.join(lines)}")
    print(summary())
    print(f"{total:>8,} yen, {total / _len(lines):.1f} a parcel")


def heavy() -> None:
    best = heaviest(desk())
    name = best.label if best is not None else "nothing"
    first = desk()[0]
    print(name, first[0], first[-1] == "", isinstance(first, tuple))
    # A rule called inside `print`, handed an Enum local: the call's
    # answer is the text, its arguments are values.
    size = first.size
    print(fee(Parcel("copy", first.weight, size)), size)
    # An optional asked which object it holds, as a generator writes it
    # for an optional input: `is True` is False when it holds nothing.
    light: bool | None = None
    if best is not None:
        light = best.weight < 4000
    asked: bool | None = None
    heavy_one = light is False
    yes = asked is True
    not_no = asked is not False
    print(heavy_one, yes, not_no)
    pick.set(f"heaviest: {name}")


def wrap() -> None:
    # A NamedTuple is a value: `_replace` answers a new one.
    lamp = desk()[1]
    wrapped = lamp._replace(note="wrapped", weight=Gram(lamp.weight + 300))
    print(wrapped.note, wrapped.weight, wrapped == lamp)
    desk.set(desk() + [wrapped])


def overload() -> None:
    # An exception that is not caught ends its handler in both runs:
    # the line before it prints, the state after it is not written.
    print("weighing a piano")
    fee(Parcel("piano", Gram(250000), Size.HUGE))
    summary.set("never reached")


def view() -> None:
    with column(spacing=8, padding=12):
        if len(desk()) > 0:
            text(f"{len(desk())} parcels on the desk", size=18)
        else:
            text("the desk is empty", size=18)
        text(f"{summary()}")
        text(f"{pick()}")
        with row(spacing=6):
            button("weigh", on_click=weigh)
            button("heaviest", on_click=heavy)
            button("wrap", on_click=wrap)
            button("overload", on_click=overload)


if __name__ == "__main__":
    run(view)
