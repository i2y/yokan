# /// script
# requires-python = ">=3.14"
# ///
"""The same app as counter.py, spelled declaratively with `with`.

Two spellings, one tree: the translator emits identical .pix for
both, and the gate proves both against the same native binary shape.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

import yokan as ui  # noqa: E402
from yokan import State  # noqa: E402


count: State[int] = State(0)
name: State[str] = State("")


def view():
    with ui.column(spacing=12, padding=16):
        ui.text(f"count: {count()}", size=34)
        with ui.row(spacing=8):
            ui.button("+1", on_click=lambda: count.set(count() + 1))
            ui.button("+10", on_click=lambda: count.set(count() + 10))
            ui.button("reset", on_click=lambda: count.set(0))
        ui.text_field(name(), placeholder="your name", on_change=name.set)
        ui.text(f"hello, {name()}")


if __name__ == "__main__":
    ui.run(view, title="counter")
