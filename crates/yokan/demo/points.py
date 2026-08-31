# /// script
# requires-python = ">=3.14"
# ///
"""struct ↔ frozen dataclass. `frozen=True` is the admission ticket:
an immutable value cannot expose Python's reference aliasing, so it
means the same thing as a native COW value by construction. Updates
are `dataclasses.replace` — a new value, both tiers.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from dataclasses import dataclass, replace  # noqa: E402

from yokan import button, column, row, run, State, text  # noqa: E402


@dataclass(frozen=True)
class Point:
    x: int
    y: int = 0


sel: State[Point] = State(Point(3, 4))
dist: State[int] = State(0)


def move_right():
    sel.set(replace(sel(), x=sel().x + 5))


def swap():
    sel.set(Point(sel().y, sel().x))


def measure():
    p = sel()
    dist.set(p.x * p.x + p.y * p.y)


def view():
    with column(spacing=8, padding=12):
        text(f"p=({sel().x}, {sel().y}) d2={dist()}")
        with row(spacing=6):
            button("right", on_click=move_right)
            button("swap", on_click=swap)
            button("measure", on_click=measure)


if __name__ == "__main__":
    run(view, title="points")
