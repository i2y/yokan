# /// script
# requires-python = ">=3.14"
# ///
"""Slots: a component that takes CHILDREN. Declare it
@component(slots=True), place them with ui.slot(), pass them by
`with card(...):` — the native twin is pixie's `Slot { }` splice,
and the children keep use-site identity, so stateful components
inside a slot hold independent per-instance state.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

import yokan as ui  # noqa: E402
from yokan import component, local, State  # noqa: E402


@component(slots=True)
def card(title: str):
    with ui.column(spacing=4, padding=8, border_width=1.0, border_color="accent", border_radius=8):
        ui.text(title, size=18)
        ui.slot()


@component
def counter(label: str, step: int):
    n: State[int] = local(0)
    with ui.row(spacing=6):
        ui.text(f"{label}: {n()}")
        ui.button(f"+{step}", on_click=lambda: n.set(n() + step))


def view():
    with ui.column(spacing=10, padding=16):
        with card("counters"):
            counter("a", 1)
            counter("b", 10)
        ui.text("outside the card", size=12)


if __name__ == "__main__":
    ui.run(view, title="cards")
