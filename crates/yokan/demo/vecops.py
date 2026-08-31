# /// script
# requires-python = ">=3.14"
# ///
"""Operator overloading on value classes, gated: __add__ / __sub__ /
__mul__ become the operator's meaning in both tiers, and plain value
methods are handler-callable. Bool logic as a VALUE (and / or / not
over bools) rides along.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import button, column, run, State, text, value  # noqa: E402


@value
class V2:
    x: int
    y: int

    def __add__(self, o: "V2") -> "V2":
        return V2(self.x + o.x, self.y + o.y)

    def __sub__(self, o: "V2") -> "V2":
        return V2(self.x - o.x, self.y - o.y)

    def __mul__(self, k: int) -> "V2":
        return V2(self.x * k, self.y * k)

    def dot(self, o: "V2") -> int:
        return self.x * o.x + self.y * o.y


a: State[V2] = State(V2(1, 2))
b: State[V2] = State(V2(10, 20))
c: State[V2] = State(V2(0, 0))
d: State[int] = State(0)
both: State[bool] = State(False)
hot: State[bool] = State(True)
cold: State[bool] = State(False)


def combine():
    c.set(a() + b() * 2 - V2(1, 1))
    d.set(a().dot(b()))
    both.set(hot() and not cold())


def view():
    with column(spacing=6, padding=12):
        text(f"c = ({c().x}, {c().y})")
        text(f"dot = {d()}  both = {both()}")
        button("combine", on_click=combine)


if __name__ == "__main__":
    run(view, title="vecops")
