# /// script
# requires-python = ">=3.14"
# ///
"""class ↔ @model and trait ↔ Protocol. A model is an observed
object: Python objects and native handles are both references, so
identity agrees from the start. A Protocol base routes its methods
into a native `impl`, and a Protocol-typed helper compiles to a
bounded generic fn — static dispatch, no boxing.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from typing import Protocol  # noqa: E402

from yokan import button, column, model, run, State, text  # noqa: E402


class Shape(Protocol):
    def area(self) -> float: ...


@model
class Circle(Shape):
    r: float = 1.0
    hits: int = 0

    def grow(self, by: float) -> None:
        self.r += by
        self.hits += 1

    def area(self) -> float:
        return self.r * self.r * 3.0


left = Circle()
right = Circle()
total: State[float] = State(0.0)


def area_of(s: Shape) -> float:
    return s.area()


def bump():
    left.grow(0.5)
    right.grow(2.0)
    total.set(area_of(left) + area_of(right))


def view():
    with column(spacing=8, padding=12):
        text(f"L={left.hits} R={right.hits} total={total():.2f}")
        button("bump", on_click=bump)


if __name__ == "__main__":
    run(view, title="models")
