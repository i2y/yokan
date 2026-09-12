# /// script
# requires-python = ">=3.14"
# ///
"""A button that opens a short menu.

`menu_button` is a `select` with no current value: the label stays
put, the options are data, and choosing one calls the handler with
its index. Nothing new is needed to verify it — `select:<option>`
picks from a menu the way it picks from a select, and the options
are in the dump whether the menu is open or not, because what the
app offers is not engine state.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import column, menu_button, row, run, store, text  # noqa: E402


@store
class Doc:
    actions: list[str] = ["Rename", "Duplicate", "Delete"]
    exports: list[str] = ["CSV", "JSON"]
    note: str = "nothing chosen"

    def act(self, i: int) -> None:
        self.note = f"chose {self.actions[i]}"

    def export(self, i: int) -> None:
        self.note = f"exported as {self.exports[i]}"


def view() -> None:
    with column(spacing=10, padding=14):
        text("a button that opens a menu", size=20)
        with row(spacing=8):
            menu_button("Actions", Doc.actions, on_select=Doc.act)
            menu_button("Export", Doc.exports, on_select=Doc.export)
        text(f"{Doc.note}")


if __name__ == "__main__":
    run(view, title="menubutton")
