# /// script
# requires-python = ">=3.14"
# ///
"""File dialogs. A dialog waits for a person, so it runs inside a
`task`: the call blocks on the worker while the window keeps drawing,
and the answer arrives in `on_done`. A headless run has no person, so
the script is the person — a `file:<path>` step is the answer the
next dialog gets, which is what makes a flow that opens a file
replayable and comparable across both runs.

A file dragged onto the window arrives the same way: `on_file_drop`
declares what happens to the path, and a script drops one with
`drop:<path>`.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import (  # noqa: E402
    button,
    column,
    on_file_drop,
    row,
    run,
    State,
    task,
    text,
)
from yokan import fs  # noqa: E402

chosen: State[str] = State("(nothing yet)")
body: State[str] = State("")
saved: State[str] = State("(not saved)")


def pick_file() -> str:
    return fs.open_dialog("Choose a file")


def took(path: str):
    chosen.set(path)
    if path != "":
        body.set(fs.read_text_or(path, "(unreadable)"))


def open_one():
    task(pick_file, on_done=took)


def pick_target() -> str:
    return fs.save_dialog("notes.txt")


def wrote(path: str):
    if path != "":
        fs.write_text(path, body())
        saved.set(path)


def save_as():
    task(pick_target, on_done=wrote)


def dropped(path: str):
    chosen.set(path)
    body.set(fs.read_text_or(path, "(unreadable)"))


on_file_drop(dropped)


def view():
    with column(spacing=8, padding=12):
        text(f"chosen: {chosen()}")
        text(f"first line: {body()[:40]}")
        text(f"saved to: {saved()}")
        with row(spacing=6):
            button("open…", on_click=open_one, tooltip="the platform's own panel")
            button("save as…", on_click=save_as)


if __name__ == "__main__":
    run(view, title="picker")
