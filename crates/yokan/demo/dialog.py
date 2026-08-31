# /// script
# requires-python = ">=3.14"
# ///
"""Conditional rendering: a modal behind `if show():`.

bool cells follow the same split as floats: bool TEXT is out of the
dialect, bool CONDITIONS are in. The Python `if` becomes .pix's view
`if/else`; the modal needs no open= — presence IS openness.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

import yokan as ui  # noqa: E402
from yokan import State  # noqa: E402

show: State[bool] = State(False)
status: State[str] = State("undecided")


def accept():
    status.set("accepted")
    show.set(False)


def decline():
    status.set("declined")
    show.set(False)


def view():
    with ui.column(spacing=10, padding=14):
        ui.text(f"status: {status()}", size=16)
        ui.button("open dialog", on_click=lambda: show.set(True))
        if show():
            with ui.modal():
                ui.text("accept the terms?", size=18)
                with ui.row(spacing=8):
                    ui.button("accept", on_click=accept)
                    ui.button("decline", on_click=decline)
        else:
            ui.text("(dialog closed)", size=12, color="#8a8f98")


if __name__ == "__main__":
    ui.run(view, title="dialog")
