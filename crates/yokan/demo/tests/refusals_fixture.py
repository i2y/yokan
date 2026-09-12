"""An app with several refusals, for the test that counts them.

Not a demo: nothing here translates, which is the point. It sits
under `tests/` so the demo sweep does not try to gate it, and it is
named so pytest does not collect it as a test.

Four refusals, of four kinds, in four places: a state type the
dialect has no shape for, a comprehension inside a store method, an
unannotated local list inside a handler, and a comparison rendered as
text inside the view. One `check` should report all four.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))

from yokan import State, button, column, run, store, text  # noqa: E402

count: State[int] = State(0)
tags: State[set] = State(set())


@store
class Cart:
    n: int = 0

    def sums(self) -> None:
        self.n = sum([x for x in [1, 2, 3]])


def gather() -> None:
    xs = []
    count.set(len(xs))


def view() -> None:
    with column(spacing=8, padding=12):
        text(f"{count()}")
        text(f"{count() > 1}")
        button("gather", on_click=gather)
        button("sum", on_click=Cart.sums)


if __name__ == "__main__":
    run(view)
