# /// script
# requires-python = ">=3.14"
# ///
"""List state: a todo app with a virtualized list, fully in the dialect.

items: State[list[str]] — the annotation is what makes `[]`-style
list state translatable at all. The row builder becomes a `.pix`
`for` repeater, and the row index is an ordinary int inside it: the
number, the marker on the row that is done, and that row's own
button all read the same `i`.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import button, column, list_view, row, run, State, text, text_field  # noqa: E402

items: State[list[str]] = State(["milk"])
draft: State[str] = State("")
done: State[int] = State(-1)


def add(t: str):
    items.set(items() + [t])
    draft.set("")


def line(i: int):
    with row(spacing=8):
        text(f"{i + 1}. {items()[i]}")
        if i == done():
            text("done", color="accent")
        button("done", on_click=lambda: done.set(i))


def view():
    with column(spacing=10, padding=14):
        text(f"todo — {len(items())} items", size=16)
        text_field(
            draft(),
            placeholder="add and press enter",
            on_change=draft.set,
            on_submit=add,
        )
        list_view(len(items()), line, item_height=26.0, height=280.0)
        button("clear", on_click=lambda: items.set([]))


if __name__ == "__main__":
    run(view, title="todo")
