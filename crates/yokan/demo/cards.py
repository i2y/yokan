# /// script
# requires-python = ">=3.14"
# ///
"""Slots: a component that takes CHILDREN. Declare it
@component(slots=True), place them with slot(), pass them by
`with card(...):` — the native twin is pixie's `Slot { }` splice,
and the children keep use-site identity, so stateful components
inside a slot hold independent per-instance state.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import (
    button,
    column,
    component,
    local,
    row,
    run,
    slot,
    State,
    text,
)


@component(slots=True)
def card(title: str):
    with column(spacing=4, padding=8, border_width=1.0, border_color="accent", border_radius=8):
        text(title, size=18)
        slot()


@component
def counter(label: str, step: int):
    n: State[int] = local(0)
    with row(spacing=6):
        text(f"{label}: {n()}")
        button(f"+{step}", on_click=lambda: n.set(n() + step))


def view():
    with column(spacing=10, padding=16):
        with card("counters"):
            counter("a", 1)
            counter("b", 10)
        text("outside the card", size=12)


if __name__ == "__main__":
    run(view, title="cards")
