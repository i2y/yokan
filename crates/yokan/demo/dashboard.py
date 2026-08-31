# /// script
# requires-python = ">=3.14"
# ///
"""yokan dogfood #2: a live dashboard — ui.every() drives updates.

Stdlib only: samples os.getloadavg() once a second, keeps a minute
of history, renders a line chart, a progress track and a spinner.
"""
import os
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

import yokan as ui  # noqa: E402

NCPU = os.cpu_count() or 1
state = {"hist": [], "ticks": 0, "started": time.time()}


def tick():
    load = os.getloadavg()[0]
    state["hist"] = (state["hist"] + [load])[-60:]
    state["ticks"] += 1


def view(s):
    hist = s["hist"] or [0.0]
    cur = hist[-1]
    up = int(time.time() - s["started"])
    return ui.column(
        ui.row(
            ui.text("1-minute load average", size=13, color="#8a8f98", grow=1.0),
            ui.spinner(size=16),
            spacing=8,
        ),
        ui.text(f"{cur:.2f}", size=40),
        ui.progress(min(cur / NCPU, 1.0)),
        ui.line_chart(hist, height=120.0),
        ui.text(f"{s['ticks']} ticks · up {up}s · {NCPU} cpus", size=12, color="#8a8f98"),
        spacing=12,
        padding=16,
    )


if __name__ == "__main__":
    ui.every(1.0, tick)
    ui.run(view, state=state, title="loadavg")
