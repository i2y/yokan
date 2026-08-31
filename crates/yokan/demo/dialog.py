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

from yokan import button, column, modal, row, run, State, text  # noqa: E402

show: State[bool] = State(False)
status: State[str] = State("undecided")


def accept():
    status.set("accepted")
    show.set(False)


def decline():
    status.set("declined")
    show.set(False)


def view():
    with column(spacing=10, padding=14):
        text(f"status: {status()}", size=16)
        button("open dialog", on_click=lambda: show.set(True))
        if show():
            with modal():
                text("accept the terms?", size=18)
                with row(spacing=8):
                    button("accept", on_click=accept)
                    button("decline", on_click=decline)
        else:
            text("(dialog closed)", size=12, color="#8a8f98")


if __name__ == "__main__":
    run(view, title="dialog")
