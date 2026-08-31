# /// script
# requires-python = ">=3.14"
# ///
"""Chart data from a list[float] cell — the float rule in action:

float TEXT is out of the dialect (str(2.0) diverges across tiers),
but float DATA is fine: both tiers dump chart values through the
same kernel renderer, and the gate proves it.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

import yokan as ui  # noqa: E402
from yokan import State  # noqa: E402

values: State[list[float]] = State([3.0, 5.0, 2.0])
limit: State[float] = State(4.5)


def bump():
    values.set(values() + [8.0])


def raise_limit():
    limit.set(limit() + 0.5)


def view():
    with ui.column(spacing=10, padding=14):
        ui.text(f"points: {len(values())}", size=14)
        ui.line_chart(values(), height=120.0)
        ui.bar_chart(values(), height=90.0)
        ui.text(f"limit: {limit():.1f}", size=12, color="#8a8f98")
        with ui.row(spacing=8):
            ui.button("add point", on_click=bump)
            ui.button("raise limit", on_click=raise_limit)


if __name__ == "__main__":
    ui.run(view, title="trend")
