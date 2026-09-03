# /// script
# requires-python = ">=3.14"
# ///
"""Python-semantics operations, gated: `/` `//` `%` `**`, bare
float/bool/enum text, negative indexing, dict iteration (the keys
in the order they went in, the values, and sorted()),
if/else locals that outlive the branch, @value, list-typed store
method parameters. The interpreted run uses the real
operators and str(); the compiled run reproduces CPython's results
exactly — the gate proves they print the same bytes.
"""
import os
import sys
from enum import Enum

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import button, column, row, run, State, store, text, value  # noqa: E402


@value
class Point:
    x: int
    y: int = 0


class Mood(Enum):
    HAPPY = 1
    GRUMPY = 2


q: State[float] = State(0.0)
big: State[float] = State(0.0)
fd: State[int] = State(0)
md: State[int] = State(0)
ffd: State[float] = State(0.0)
fmd: State[float] = State(0.0)
p2: State[int] = State(0)
pf: State[float] = State(0.0)
flag: State[bool] = State(False)
mood: State[Mood] = State(Mood.HAPPY)
grade: State[str] = State("-")
last_key: State[str] = State("-")
walked: State[str] = State("-")
spend: State[int] = State(0)
tail: State[str] = State("-")
pt: State[Point] = State(Point(3, 4))
prices: State[dict[str, int]] = State({"cherry": 300, "apple": 120, "banana": 80})
names: State[list[str]] = State(["ada", "erik", "momo"])


@store
class Bag:
    joined: str = "-"

    def take(self, xs: list[str]) -> None:
        self.joined = ""
        for x in xs:
            self.joined = self.joined + x

    def spot(self, p: Point) -> None:
        self.joined = f"({p.x}, {p.y})"


def crunch():
    q.set(1 / 3)
    big.set(9007199254740993 / 3)
    fd.set(-7 // 2)
    md.set(7 % -2)
    ffd.set(-7.5 // 2.0)
    fmd.set(-1.0 % 0.3)
    p2.set(2 ** 10)
    pf.set(2.0 ** -2)
    flag.set(True)
    mood.set(Mood.GRUMPY)
    n = 25
    if n > 20:
        verdict = "high"
    else:
        verdict = "low"
    grade.set(verdict)


def walk():
    for k in sorted(prices()):
        last_key.set(k)
    # A dict walks in the order its keys went in, which is not the
    # sorted order above — cherry, apple, banana.
    order = ""
    for k in prices():
        order = order + k[0]
    walked.set(order)
    n = 0
    for v in prices().values():
        n = n + v
    spend.set(n)
    r = names()
    tail.set(r[-1])
    Bag.take(names())
    Bag.spot(pt())


def view():
    with column(spacing=6, padding=12):
        text(f"q = {q()}")
        text(f"big = {big()}")
        text(f"floor {fd()}  mod {md()}")
        text(f"ffloor = {ffd()}  fmod = {fmd()}")
        text(f"pow {p2()}  fpow = {pf()}")
        text(f"flag = {flag()}  mood = {mood()}")
        text(f"grade = {grade()}  doubled = {p2() * 2 + 1}")
        text(f"last key = {last_key()}  tail = {tail()}")
        text(f"walked = {walked()}  spend = {spend()}")
        text(f"bag = {Bag.joined}")
        with row(spacing=6):
            button("crunch", on_click=crunch)
            button("walk", on_click=walk)


if __name__ == "__main__":
    run(view, title="pyops")
