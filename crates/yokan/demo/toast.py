# /// script
# requires-python = ">=3.14"
# ///
"""A message that appears over the app and goes away on its own.

A toast is open by existing, the way a modal is: put it behind
`if showing():` rather than passing a flag. Give it `duration_ms` and
it closes itself that many milliseconds later by calling `on_close` —
which is where the app clears what the `if` reads. The countdown runs
on the framework's own clock, so a headless script says `advance:1500`
and sees the same thing a person waiting would.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import button, column, row, run, State, text, toast  # noqa: E402

note: State[str] = State("nothing saved yet")
saved: State[bool] = State(False)
hint: State[bool] = State(True)


def save():
    note.set("saved")
    saved.set(True)


def done():
    saved.set(False)
    note.set("the toast closed itself")


def dismiss():
    hint.set(False)


def view():
    with column(spacing=10, padding=14):
        text(f"{note()}", size=16)
        with row(spacing=8):
            button("save", on_click=save)
            button("dismiss", on_click=dismiss)
        if hint():
            toast("welcome - press save")
        if saved():
            toast("Saved", duration_ms=1500, on_close=done)


if __name__ == "__main__":
    run(view, title="toast")
