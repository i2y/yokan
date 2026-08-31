# /// script
# requires-python = ">=3.14"
# ///
"""The calculator again, on a grid. `grid(columns=4, rows=5)`
lays equal tracks, every key fills its cell, and the zero key says
`col_span=2` — the whole keypad is one container instead of five
rows. Same store as demo/calc.py; only the view differs."""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import button, column, grid, run, store, strings, style, text  # noqa: E402


@store
class Calc:
    display: str = "0"
    acc: float = 0.0
    op: str = ""
    fresh: bool = True
    has_dot: bool = False

    def press(self, d: str) -> None:
        if self.fresh:
            self.display = d
            self.fresh = False
            self.has_dot = False
        elif self.display == "0":
            self.display = d
        else:
            self.display = self.display + d

    def dot(self) -> None:
        if self.fresh:
            self.display = "0."
            self.fresh = False
            self.has_dot = True
        elif not self.has_dot:
            self.display = self.display + "."
            self.has_dot = True

    def negate(self) -> None:
        v = strings.to_float(self.display, 0.0)
        if v != 0.0:
            self.display = f"{0.0 - v}"
            self.fresh = False

    def percent(self) -> None:
        v = strings.to_float(self.display, 0.0)
        self.display = f"{v / 100.0}"
        self.fresh = True
        self.has_dot = False

    def apply(self, nxt: str) -> None:
        if self.fresh and self.op != "":
            self.op = nxt
            return
        cur = strings.to_float(self.display, 0.0)
        if self.op == "":
            self.acc = cur
        if self.op == "+":
            self.acc = self.acc + cur
        if self.op == "-":
            self.acc = self.acc - cur
        if self.op == "×":
            self.acc = self.acc * cur
        if self.op == "÷":
            if cur == 0.0:
                self.display = "Error"
                self.acc = 0.0
                self.op = ""
                self.fresh = True
                return
            self.acc = self.acc / cur
        self.display = f"{self.acc}"
        self.op = nxt
        self.fresh = True

    def do_op(self, o: str) -> None:
        Calc.apply(o)

    def equals(self) -> None:
        Calc.apply("")

    def clear(self) -> None:
        self.display = "0"
        self.acc = 0.0
        self.op = ""
        self.fresh = True
        self.has_dot = False


key = style(
    size=20, background="panel",
    hover_background="#45475a", active_background="#585b70",
)
fun_tint = style(background="#313244", color="#a6adc8")
fun = key | fun_tint
op_tint = style(
    background="#fab387", color="#1e1e2e",
    hover_background="#f8c49b", active_background="#f5e0dc",
)
op = key | op_tint
readout = style(size=40, color="text", align="right", grow=1.4)


def view():
    with column(spacing=8, padding=16, grow=1):
        text(f"{Calc.display}", **readout)
        with grid(columns=4, rows=5, spacing=8, grow=5):
            button("C", on_click=Calc.clear, **fun)
            button("±", on_click=Calc.negate, **fun)
            button("%", on_click=Calc.percent, **fun)
            button("÷", on_click=lambda: Calc.do_op("÷"), **op)
            button("7", on_click=lambda: Calc.press("7"), **key)
            button("8", on_click=lambda: Calc.press("8"), **key)
            button("9", on_click=lambda: Calc.press("9"), **key)
            button("×", on_click=lambda: Calc.do_op("×"), **op)
            button("4", on_click=lambda: Calc.press("4"), **key)
            button("5", on_click=lambda: Calc.press("5"), **key)
            button("6", on_click=lambda: Calc.press("6"), **key)
            button("-", on_click=lambda: Calc.do_op("-"), **op)
            button("1", on_click=lambda: Calc.press("1"), **key)
            button("2", on_click=lambda: Calc.press("2"), **key)
            button("3", on_click=lambda: Calc.press("3"), **key)
            button("+", on_click=lambda: Calc.do_op("+"), **op)
            button("0", on_click=lambda: Calc.press("0"), col_span=2, **key)
            button(".", on_click=Calc.dot, **key)
            button("=", on_click=Calc.equals, **op)


if __name__ == "__main__":
    run(view, title="calcgrid")
