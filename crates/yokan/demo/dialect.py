# /// script
# requires-python = ">=3.14"
# ///
"""The everyday Python the dialect used to refuse, gated: a method
that answers `int | None`, a local dict counted up and read back, a
conditional expression inside a view, `d[k]` caught as a KeyError,
`print` as a checked output, an early `return` inside a branch, and a
comparison used as a value.

`print` writes to stdout and the screens go to the file `PIXIE_DUMP`
names, so the gate compares both: the dumps and what the app printed.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import State, button, column, row, run, store, text  # noqa: E402

words: State[list[str]] = State(["ripe", "green", "ripe", "gold", "ripe", "green"])
counted: State[str] = State("")
found: State[str] = State("")
flagged: State[bool] = State(False)


@store
class Basket:
    n: int = 0
    # An optional field, and a method that answers one.
    picked: int | None = None

    def pick(self) -> int | None:
        if self.n > 2:
            return self.n
        return None

    # A conditional expression, nested, read by the view as a
    # property.
    @property
    def grade(self) -> str:
        return "many" if self.n > 4 else ("some" if self.n > 2 else "few")

    # An early return inside a branch, and an `if` / `else` where both
    # sides answer.
    def rung(self) -> int:
        if self.n > 4:
            return 2
        elif self.n > 2:
            return 1
        else:
            return 0

    def add(self) -> None:
        self.n = self.n + 1


def take() -> None:
    print("rung", Basket.rung())
    v = Basket.pick()
    if v is not None:
        Basket.picked = v
        print("picked", v)
    else:
        print("nothing to pick")


def tally() -> None:
    # A local dict: counted up, read with `.get`, asked with `in`,
    # walked in sorted order.
    counts: dict[str, int] = {}
    for w in words():
        counts[w] = counts.get(w, 0) + 1
    best = ""
    most = 0
    for k in sorted(counts):
        if counts.get(k, 0) > most:
            most = counts.get(k, 0)
            best = k
    counted.set(f"{best}={most} of {len(counts)}")
    # A comparison as a value.
    flagged.set(most > 2 and "ripe" in counts)
    print("tallied", len(counts), "kinds", sep=" ", end="\n")


def look() -> None:
    counts: dict[str, int] = {}
    for w in words():
        counts[w] = counts.get(w, 0) + 1
    # `d[k]` is the read Python answers with a KeyError, and what a
    # `try` catches is the read itself, bound to a name.
    try:
        gold = counts["gold"]
        found.set(f"gold {gold}")
    except KeyError as e:
        found.set(f"no {e}")
    try:
        plum = counts["plum"]
        found.set(f"{found()}, plum {plum}")
    except KeyError as e:
        found.set(f"{found()}, no {e}")


def view() -> None:
    with column(spacing=8, padding=12):
        # A conditional expression, where a view used to have none.
        text(f"n: {Basket.n} ({Basket.grade})", size=20)
        if (v := Basket.picked) is not None:
            text(f"picked: {v}")
        else:
            text("nothing picked")
        text(f"counted: {counted()}, ripe-heavy: {flagged()}")
        text(f"found: {found()}")
        with row(spacing=6):
            button("add", on_click=Basket.add)
            button("take", on_click=take)
            button("tally", on_click=tally)
            button("look", on_click=look)


if __name__ == "__main__":
    run(view)
