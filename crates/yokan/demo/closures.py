# /// script
# requires-python = ">=3.14"
# ///
"""Functions as values, gated: a lambda held in an annotated local, a
nested def that captures a local, a `Callable` field a store is armed
with and swapped later, a closure handed to a method that declares
one, `map` over a function value and a function value as a sort key.

Captures are by value, taken where the closure is made, and the two
places Python's variable capture would disagree are refused by name:
writing to a captured local, and letting a closure that took a loop
variable outlive the iteration.
"""
import os
import sys
from typing import Callable

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import State, button, column, row, run, store, text  # noqa: E402

xs: State[list[int]] = State([1, 2, 3])
mapped: State[int] = State(0)


@store
class Pipeline:
    n: int = 1
    # A callback field: the store is armed with one and can be armed
    # with another while the app runs.
    step: Callable[[int], int] = lambda x: x + 1
    label: str = "add one"

    def advance(self) -> None:
        f = self.step
        self.n = f(self.n)

    def harder(self) -> None:
        self.step = lambda x: x * x
        self.label = "squares"

    def reset(self) -> None:
        self.n = 1
        self.step = lambda x: x + 1
        self.label = "add one"

    # A method that takes a function value, and one that hands it one.
    def apply(self, g: Callable[[int], int]) -> None:
        self.n = g(self.n)

    def through(self) -> None:
        self.apply(lambda x: x * 10)


def double_all() -> None:
    twice: Callable[[int], int] = lambda x: x * 2
    doubled = list(map(twice, xs()))
    mapped.set(sum(doubled))


def offset() -> None:
    # A nested def, capturing a local by value: `base` is the number
    # that was there when the closure was made.
    base = Pipeline.n

    def add(x: int) -> int:
        return x + base

    Pipeline.n = add(10)


def ordered() -> None:
    # A function value as a sort key: the keys are built once, and the
    # list comes back in their order.
    down: Callable[[int], int] = lambda x: -x
    xs.set(sorted(xs(), key=down))
    mapped.set(xs()[0])


def counted() -> None:
    # A closure made inside a loop and called there: both runs read
    # the same value, so it is allowed.
    total = 0
    for i in xs():
        scale: Callable[[int], int] = lambda x: x * i
        total = total + scale(2)
    mapped.set(total)


def view() -> None:
    with column(spacing=8, padding=12):
        text(f"n: {Pipeline.n}", size=24)
        text(f"step: {Pipeline.label}")
        text(f"mapped: {mapped()}")
        with row(spacing=6):
            button("advance", on_click=Pipeline.advance)
            button("harder", on_click=Pipeline.harder)
            button("through", on_click=Pipeline.through)
        with row(spacing=6):
            button("offset", on_click=offset)
            button("double", on_click=double_all)
            button("counted", on_click=counted)
            button("ordered", on_click=ordered)
            button("reset", on_click=Pipeline.reset)


if __name__ == "__main__":
    run(view)
