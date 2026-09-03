# /// script
# requires-python = ">=3.14"
# ///
"""The riders: the cross-cutting keyword arguments EVERY element
takes. `width=`/`min_width=` put a box around one, `disabled=` dims
it and stops taking its clicks, `theme=` scopes a palette over a
subtree, `animate=`/`easing=` tween what changes, `col_span=` places
an element on a grid's tracks, `role=` names it for assistive
technology and `tooltip=` is the line the window shows under the
pointer. None of them belongs to any one element: each is a wrapper
the compiler puts around whatever it is written on, so the same
spelling works on a spacer, a segmented chooser, a field, a link or
a rule — and both runs build the same tree, which is what the gate
compares. Locking proves it: while `locked` is true the save button
and the field are inert, in the window and in a script alike.

Develop:  uv run demo/riders.py
Ship:     python3 yokan_gate.py gate demo/riders.py --script "click:lock,click:save,input:typed,dump,click:lock,click:save,dump"
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import (  # noqa: E402
    button,
    column,
    divider,
    grid,
    link,
    row,
    run,
    segmented,
    spacer,
    store,
    text,
    text_field,
)


@store
class Locks:
    locked: bool = False
    saves: int = 0
    # The palette the spacer's subtree resolves its tokens in — a
    # rider takes a read, not just a literal, so the lock switches it.
    mode: str = "dark"
    tab: int = 0
    note: str = "draft"

    def flip(self) -> None:
        self.locked = not self.locked
        if self.locked:
            self.mode = "light"
        else:
            self.mode = "dark"

    def save(self) -> None:
        self.saves = self.saves + 1

    def pick(self, i: int) -> None:
        self.tab = i

    def edit(self, t: str) -> None:
        self.note = t


def view():
    with column(spacing=10, padding=14):
        text("riders", size=20, role="heading")
        with row(spacing=8):
            text(f"mode: {Locks.mode}  saves: {Locks.saves}", size=12)
            # A theme scope on a spacer: the rider is the element's,
            # whichever element it is.
            spacer(grow=1.0, theme=Locks.mode)
            button("lock", on_click=Locks.flip, tooltip="flip the lock")
        segmented(
            options=["read", "write"],
            selected=Locks.tab,
            on_change=Locks.pick,
            animate=120,
            easing="out",
        )
        # A box around the section: 260 wide, never under 200.
        with column(width=260.0, min_width=200.0, spacing=8, padding=8, background="panel"):
            # The field takes two of the grid's three tracks, and goes
            # inert with the lock.
            with grid(columns=3, spacing=8):
                text("note", size=12)
                text_field(
                    Locks.note,
                    on_change=Locks.edit,
                    col_span=2,
                    disabled=Locks.locked,
                )
            button("save", on_click=Locks.save, disabled=Locks.locked, tooltip="count a save")
        link("Docs", "https://i2y.github.io/yokan/", role="button")
        divider(tooltip="the end of the riders")


if __name__ == "__main__":
    run(view, title="riders")
