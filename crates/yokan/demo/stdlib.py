# /// script
# requires-python = ">=3.14"
# ///
"""The standard library: math, json, time. Not "Python's stdlib
reimplemented": the interpreted and the compiled app call the SAME
implementation, so there is no fidelity gap to chase — yokan.math
is yokan.math everywhere, and the gate arbitrates.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

import yokan as ui  # noqa: E402
from yokan import State  # noqa: E402
from yokan import json, math, time  # noqa: E402

hyp: State[float] = State(0.0)
who: State[str] = State("-")
score: State[int] = State(0)
day: State[str] = State("-")


def measure():
    hyp.set(math.sqrt(3.0 * 3.0 + 4.0 * 4.0))


def parse():
    who.set(json.get_text('{"name": "momo", "scores": [3, 5, 8]}', "name"))
    score.set(json.get_int('{"name": "momo", "scores": [3, 5, 8]}', "scores.2"))


def stamp():
    day.set(time.format_ms(0, "%Y-%m-%d"))


def view():
    with ui.column(spacing=8, padding=12):
        ui.text(f"hyp={hyp():.1f} who={who()} score={score()} day={day()}")
        with ui.row(spacing=6):
            ui.button("measure", on_click=measure)
            ui.button("parse", on_click=parse)
            ui.button("stamp", on_click=stamp)


if __name__ == "__main__":
    ui.run(view, title="stdlib")
