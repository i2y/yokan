# /// script
# requires-python = ">=3.14"
# ///
"""`yokan.fs` from the standard library: the interpreted and the
compiled app call the SAME implementation, so the gate arbitrates a
single truth (write 25 bytes, read them back).
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

import yokan as ui  # noqa: E402
from yokan import State  # noqa: E402
from yokan import fs  # noqa: E402

content: State[str] = State("(not loaded)")
wrote: State[int] = State(0)


def save():
    wrote.set(fs.write_text("demo/.gate/fs_probe.txt", "hello from one rust crate"))


def load():
    content.set(fs.read_text("demo/.gate/fs_probe.txt"))


def view():
    with ui.column(spacing=8, padding=12):
        ui.text(f"content: {content()}")
        ui.text(f"wrote: {wrote()} bytes")
        ui.button("save", on_click=save)
        ui.button("load", on_click=load)


if __name__ == "__main__":
    ui.run(view, title="files")
