# /// script
# requires-python = ">=3.14"
# ///
"""List state: a todo app with a virtualized list, fully in the dialect.

items: State[list[str]] — the annotation is what makes `[]`-style
list state translatable at all. The row builder becomes a `.pix`
`for` repeater; submit appends via the push pattern.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

import yokan as ui  # noqa: E402
from yokan import State  # noqa: E402

items: State[list[str]] = State(["milk"])
draft: State[str] = State("")


def add(t: str):
    items.set(items() + [t])
    draft.set("")


def row(i: int):
    return ui.text(items()[i])


def view():
    with ui.column(spacing=10, padding=14):
        ui.text(f"todo — {len(items())} items", size=16)
        ui.text_field(
            draft(),
            placeholder="add and press enter",
            on_change=draft.set,
            on_submit=add,
        )
        ui.list_view(len(items()), row, item_height=24.0, height=280.0)
        ui.button("clear", on_click=lambda: items.set([]))


if __name__ == "__main__":
    ui.run(view, title="todo")
