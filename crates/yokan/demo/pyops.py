# /// script
# requires-python = ">=3.14"
# ///
"""Python-semantics operations, gated: `/` `//` `%` `**`, bare
float/bool/enum text, negative indexing, dict iteration (the keys
in the order they went in, the values, `.items()`, and sorted()),
tuples (a literal, a part, unpacking, a pair loop, a tuple return),
ordering by a key (`sorted`/`min`/`max` with `key=`, `reverse=`),
comprehensions and `[::-1]` over a value class, if/else locals that
outlive the branch, @value, list-typed store method parameters.
The interpreted run uses the real operators and str(); the compiled
run reproduces CPython's results exactly — the gate proves they print
the same bytes.
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
paired: State[str] = State("-")
sized: State[tuple[str, int]] = State(("-", 0))
spend: State[int] = State(0)
tail: State[str] = State("-")
pt: State[Point] = State(Point(3, 4))
prices: State[dict[str, int]] = State({"cherry": 300, "apple": 120, "banana": 80})
names: State[list[str]] = State(["ada", "erik", "momo"])
people: State[list[Point]] = State([Point(3, 1), Point(1, 5), Point(2, 5)])
ranked: State[str] = State("-")


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


def measure(word: str) -> tuple[str, int]:
    return (word.upper(), len(word))


def pairs():
    # A dict walks as pairs, a tuple comes back from a helper, and
    # `divmod` answers the two numbers Python says it does.
    s = ""
    for k, v in prices().items():
        s = s + f"{k[0]}{v}"
    label, n = measure("momo")
    whole, rest = divmod(len(s), 3)
    sized.set((label, n))
    paired.set(f"{s} {label}{n} {whole}r{rest} {sized()[1]}")


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


def rank(p: Point) -> int:
    return p.y


def order():
    # A key says which part to compare, so an order works for a value
    # class as much as for a number, and the key can be a lambda or a
    # helper. Sorting is stable: the two points with y=5 keep the
    # order they came in, and `reverse=True` keeps it as well — it
    # turns the comparison around, not the answer.
    by_y = sorted(people(), key=lambda p: p.y)
    down = sorted(people(), key=rank, reverse=True)
    lo = min(people(), key=lambda p: p.x)
    hi = max(people(), key=lambda p: p.x)
    xs = [p.x for p in people()]
    back = people()[::-1]
    high = sorted(xs, reverse=True)
    ranked.set(
        f"{by_y[0].x}{by_y[1].x}{by_y[2].x} {down[0].x}{down[1].x} "
        f"{lo.x}{hi.x} {xs[0]} {back[0].x} {high[0]}"
    )


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
        text(f"paired = {paired()}")
        text(f"bag = {Bag.joined}")
        text(f"ranked = {ranked()}")
        with row(spacing=6):
            button("crunch", on_click=crunch)
            button("walk", on_click=walk)
            button("pairs", on_click=pairs)
            button("order", on_click=order)


if __name__ == "__main__":
    run(view, title="pyops")
