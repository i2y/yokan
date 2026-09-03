# /// script
# requires-python = ">=3.14"
# ///
"""progress: `value` stays the only required prop — `width`/`height`
size the track, `label` draws a dim line above it, and
`indeterminate` ignores `value` and sweeps a segment instead, for
work with no known length.

Develop:  uv run demo/loading.py
Ship:     python3 yokan_gate.py gate demo/loading.py --script "click:step,click:step,dump,click:busy"
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import button, column, progress, row, run, State, text  # noqa: E402


ratio: State[float] = State(0.25)
busy: State[bool] = State(False)


def step():
    if ratio() >= 1.0:
        ratio.set(0.0)
    else:
        ratio.set(ratio() + 0.25)


def toggle_busy():
    busy.set(not busy())


def view():
    return column(
        text(f"ratio: {ratio()}"),
        progress(ratio(), label="Uploading"),
        progress(ratio(), width=240, height=6),
        progress(ratio(), indeterminate=busy()),
        row(
            button("step", on_click=step),
            button("busy", on_click=toggle_busy),
            spacing=8,
        ),
        spacing=12,
        padding=16,
    )


if __name__ == "__main__":
    run(view, title="loading")
