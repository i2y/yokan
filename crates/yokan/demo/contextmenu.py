# /// script
# requires-python = ">=3.14"
# ///
"""A menu the right button opens.

`context_menu` wraps one element with the items it offers. The items
are the app's own data, so they are in a dump whether the menu is
open or not, and a verification script picks from them with the step
every chooser takes — `select:<item>` — without the click that opens
the panel. Where that panel goes is the window's business, the way a
`select`'s open list is.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import column, context_menu, run, store, text  # noqa: E402


@store
class Board:
    items: list[str] = ["Rename", "Duplicate", "Delete"]
    note: str = "right-click the card"

    def pick(self, i: int) -> None:
        self.note = f"chose {self.items[i]}"


def view() -> None:
    with column(spacing=10, padding=14):
        text("a menu the right button opens", size=20)
        with context_menu(options=Board.items, on_select=Board.pick):
            with column(padding=16, background="#313244", border_radius=8.0):
                text("a card", size=14)
        text(f"{Board.note}")


if __name__ == "__main__":
    run(view, title="contextmenu")
