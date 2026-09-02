# /// script
# requires-python = ">=3.14"
# ///
"""A live dashboard: every() drives the updates, in both runs.

`every(1.0, tick)` at module level is a declaration — the compiled
app starts the timer with the app, and a headless run steps it with
`advance:<ms>`, so a minute of ticks is gate-checkable. The samples
come from the seeded RNG both runs share, and the history is a
fixed ring the tick writes by index.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import (  # noqa: E402
    State,
    column,
    every,
    line_chart,
    progress,
    random,
    row,
    run,
    spinner,
    text,
)

SLOTS = 12

hist: State[list[float]] = State([0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0])
at: State[int] = State(0)
ticks: State[int] = State(0)
cur: State[float] = State(0.25)


def setup():
    random.seed(7)


def tick():
    step = random.float() * 0.4 - 0.2
    v = cur() + step
    if v < 0.0:
        v = 0.0
    if v > 1.0:
        v = 1.0
    cur.set(v)
    hist[at()] = v
    at.set((at() + 1) % SLOTS)
    ticks.set(ticks() + 1)


every(1.0, tick)


def view():
    with column(spacing=12, padding=16):
        with row(spacing=8):
            text("load, sampled every second", size=13, color="#8a8f98", grow=1.0)
            spinner(size=16.0)
        text(f"{cur():.2f}", size=40)
        progress(cur())
        line_chart(hist(), height=120.0)
        text(f"{ticks()} ticks · {SLOTS} slots", size=12, color="#8a8f98")


if __name__ == "__main__":
    run(view, title="loadavg", on_start=setup)
