# /// script
# requires-python = ">=3.14"
# ///
"""A list you can pick a row from.

`list_view` takes `selected` and `on_select` the way `table` does:
the marked row is data the app owns, and clicking a row asks the app
to move the mark rather than moving it behind the app's back. The
rows are whatever the builder returns, so a verification script picks
one by what it says — the first text anywhere in the row.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import State, button, column, list_view, row, run, text  # noqa: E402

names: State[list[str]] = State(["ada", "bo", "cy", "dee", "eve"])
sizes: State[list[int]] = State([12, 7, 31, 4, 19])
picked: State[int] = State(-1)


def choose(i: int) -> None:
    picked.set(i)


def clear() -> None:
    picked.set(-1)


def line(i: int):
    with row(spacing=8):
        text(names()[i], size=14)
        text(f"{sizes()[i]} kb", size=12, color="#7aa2f7")


def view() -> None:
    with column(spacing=10, padding=14):
        text("pick a row", size=20)
        list_view(
            len(names()),
            line,
            item_height=28.0,
            height=180.0,
            selected=picked(),
            on_select=choose,
        )
        text(f"picked: {picked()}")
        button("clear", on_click=clear)


if __name__ == "__main__":
    run(view, title="picklist")
