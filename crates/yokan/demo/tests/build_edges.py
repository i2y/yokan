"""Python that `check` used to pass and the compiled build then stopped
on, each written the way Python writes it and gated here.

A float compared with an int, exactly, the int a literal or not; a
float literal Python spells with an exponent; a sum printed as a sum; a
helper handed a plain value or None for an optional parameter, and one
that answers an optional; an optional narrowed past an `if x is None:`
that leaves; a list of optionals written out and grown; an optional
field or State read copied into an optional slot; a store method handed
None. Every answer is printed, so the gate compares both channels.
"""
import math
from dataclasses import dataclass

from yokan import State, column, run, store, text

level: State[float] = State(2.5)
count: State[int] = State(3)
picked: State[int | None] = State(7)
board: State[list[int | None]] = State([None, 1, None])


@dataclass(frozen=True)
class Row:
    name: str
    qty: int | None


@store
class Bag:
    last: int | None = None

    def put(self, v: int | None) -> None:
        self.last = v


def twice(n: int) -> int:
    return n * 2


def tally(on: bool | None, n: int | None = None) -> int:
    if on is True:
        return 1
    if n is not None:
        return n
    return 0


def pick(n: int) -> int | None:
    if n > 2:
        return n
    return None


def qty_of(r: Row) -> int | None:
    return r.qty


def shown(v: int | None) -> int:
    if v is None:
        return -1
    return v + 1


def filled(xs: list[int | None]) -> int:
    total = 0
    for x in xs:
        if x is None:
            continue
        total = total + x
    return total


def start() -> None:
    # A float against an int, a literal or not, either side.
    x = level()
    n = count()
    big = 9007199254740993           # 2**53 + 1: no float equals it
    edge = 9007199254740992.0        # 2**53
    top = 9223372036854775807
    print(x < 0, 0 < x, x == 2, 1 <= x < n, x < n, n <= x, x != n)
    print(edge < big, edge == big, big > edge, 9223372036854775808.0 > top)
    print(math.nan < n, math.nan != n, math.inf > top, x < twice(n), twice(1) > x)
    print(n + 1, x + 1, 1e-05, 1e16 + x)

    # Optionals handed in, answered, narrowed and copied.
    print(tally(True), tally(None, 4), tally(False), tally(None))
    print(shown(pick(5)), shown(pick(1)))
    r = Row("a", 3)
    s = Row("b", None)
    q: int | None = r.qty
    a = shown(q)
    q = s.qty
    print(a, shown(q), shown(qty_of(r)), shown(Row("c", r.qty).qty))
    picked.set(s.qty)
    seen = shown(picked())
    picked.set(5)
    print(seen, shown(picked()))
    Bag.put(None)
    Bag.put(r.qty)
    kept: int | None = Bag.last
    print(shown(kept))

    # Lists of optionals, written out and grown.
    xs: list[int | None] = [1, None, 3]
    xs.append(None)
    xs.append(2)
    board.set(board() + [None])
    board.set(board() + [5])
    ys = board() + [None, 7]
    print(len(xs), filled(board()), filled(ys))


def view():
    with column(spacing=6, padding=12):
        if level() > 2:
            text("above two")
        text("over" if level() > count() else "under")
        text(f"{len(board())} on the board")


if __name__ == "__main__":
    run(view, on_start=start)
