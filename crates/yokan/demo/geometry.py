# /// script
# requires-python = ">=3.14"
# ///
"""Sum types: frozen dataclasses joined by a `type` alias compile to
a native payload enum, and `match` destructures in handlers AND view
bodies — this required fixing a real substrate contradiction (the
checker demanded view patterns the emitter refused; both tiers can
bind them now).
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from dataclasses import dataclass  # noqa: E402

from yokan import button, column, row, run, State, text  # noqa: E402


@dataclass(frozen=True)
class Circle:
    r: float


@dataclass(frozen=True)
class Rect:
    w: float
    h: float


@dataclass(frozen=True)
class Dot:
    pass


type Shape = Circle | Rect | Dot

sel: State[Shape] = State(Dot())
area: State[float] = State(0.0)


def mk_circle():
    sel.set(Circle(2.0))


def mk_rect():
    sel.set(Rect(3.0, 4.0))


def measure():
    match sel():
        case Circle(r):
            area.set(r * r * 3.0)
        case Rect(w, h):
            area.set(w * h)
        case Dot():
            area.set(0.0)


def view():
    with column(spacing=8, padding=12):
        text(f"area={area():.1f}")
        match sel():
            case Circle(r):
                text(f"circle r={r:.1f}")
            case Rect(w, h):
                text(f"rect {w:.1f} x {h:.1f}")
            case Dot():
                text("just a dot")
        with row(spacing=6):
            button("circle", on_click=mk_circle)
            button("rect", on_click=mk_rect)
            button("measure", on_click=measure)


if __name__ == "__main__":
    run(view, title="geometry")
