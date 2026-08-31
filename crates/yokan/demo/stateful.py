# /// script
# requires-python = ">=3.14"
# ///
"""Per-instance state: @component + ui.local. Each call site owns
its own `n`; identity is positional (the no-key rule), and the state
survives rebuilds and reloads.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

import yokan as ui  # noqa: E402
from yokan import component, local, State  # noqa: E402


@component
def counter(label: str, step: int):
    n: State[int] = local(0)
    with ui.row(spacing=6):
        ui.text(f"{label}: {n()}")
        ui.button(f"+{step}", on_click=lambda: n.set(n() + step))


def view():
    with ui.column(spacing=10, padding=14):
        ui.text("two counters, one component, separate state", size=13, color="#8a8f98")
        counter("a", 1)
        counter("b", 10)


if __name__ == "__main__":
    ui.run(view, title="stateful")
