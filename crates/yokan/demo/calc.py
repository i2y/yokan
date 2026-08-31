# /// script
# requires-python = ">=3.14"
# ///
"""A calculator — the classic keypad, in the dialect. The layout is
all `grow`: the root column fills the window, every row shares the
height, keys share each row's width, and the zero key takes two
shares (`grow=2`), so resizing the window scales the whole pad with
no dead space. Styles are dicts merged with `|`.
`strings.to_float` is total (bad text parses as the default), so
the arithmetic needs no try."""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

import yokan as ui  # noqa: E402
from yokan import store, strings  # noqa: E402


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


key = ui.style(
    grow=1, size=20, background="panel",
    hover_background="#45475a", active_background="#585b70",
)
fun_tint = ui.style(background="#313244", color="#a6adc8")
fun = key | fun_tint
op_tint = ui.style(
    background="#fab387", color="#1e1e2e",
    hover_background="#f8c49b", active_background="#f5e0dc",
)
op = key | op_tint
wide_tint = ui.style(grow=2, basis=8)
wide = key | wide_tint
readout = ui.style(size=40, color="text", align="right", grow=1.4)
keys = ui.style(spacing=8, grow=1)


def view():
    with ui.column(spacing=8, padding=16, grow=1):
        ui.text(f"{Calc.display}", **readout)
        with ui.row(**keys):
            ui.button("C", on_click=Calc.clear, **fun)
            ui.button("±", on_click=Calc.negate, **fun)
            ui.button("%", on_click=Calc.percent, **fun)
            ui.button("÷", on_click=lambda: Calc.do_op("÷"), **op)
        with ui.row(**keys):
            ui.button("7", on_click=lambda: Calc.press("7"), **key)
            ui.button("8", on_click=lambda: Calc.press("8"), **key)
            ui.button("9", on_click=lambda: Calc.press("9"), **key)
            ui.button("×", on_click=lambda: Calc.do_op("×"), **op)
        with ui.row(**keys):
            ui.button("4", on_click=lambda: Calc.press("4"), **key)
            ui.button("5", on_click=lambda: Calc.press("5"), **key)
            ui.button("6", on_click=lambda: Calc.press("6"), **key)
            ui.button("-", on_click=lambda: Calc.do_op("-"), **op)
        with ui.row(**keys):
            ui.button("1", on_click=lambda: Calc.press("1"), **key)
            ui.button("2", on_click=lambda: Calc.press("2"), **key)
            ui.button("3", on_click=lambda: Calc.press("3"), **key)
            ui.button("+", on_click=lambda: Calc.do_op("+"), **op)
        with ui.row(**keys):
            ui.button("0", on_click=lambda: Calc.press("0"), **wide)
            ui.button(".", on_click=Calc.dot, **key)
            ui.button("=", on_click=Calc.equals, **op)


if __name__ == "__main__":
    ui.run(view, title="calc")
