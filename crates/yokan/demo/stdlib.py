# /// script
# requires-python = ">=3.14"
# ///
"""The standard library: math, json, time. Not "Python's stdlib
reimplemented": the interpreted and the compiled app call the SAME
implementation, so there is no fidelity gap to chase — yokan.math
is yokan.math everywhere, and the gate arbitrates.

json reads a path out of a document and writes a value back;
`time.format_ms` is UTC and `time.format_local_ms` is the machine's
own zone, from the same zone database in both runs.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import button, column, row, run, State, text  # noqa: E402
from yokan import json, math, time  # noqa: E402

hyp: State[float] = State(0.0)
who: State[str] = State("-")
score: State[int] = State(0)
day: State[str] = State("-")
here: State[str] = State("-")
doc: State[str] = State("-")
scores: State[list[int]] = State([3, 5, 8])


def measure():
    hyp.set(math.sqrt(3.0 * 3.0 + 4.0 * 4.0))


def parse():
    who.set(json.get_text('{"name": "momo", "scores": [3, 5, 8]}', "name"))
    score.set(json.get_int('{"name": "momo", "scores": [3, 5, 8]}', "scores.2"))


def stamp():
    day.set(time.format_ms(0, "%Y-%m-%d"))
    here.set(time.format_local_ms(0, "%Y-%m-%d %H:%M"))


def write():
    # the writer follows the value's type; a map is written in key order
    doc.set(json.dumps({"name": "momo", "team": "yokan"}))


def write_list():
    doc.set(json.dumps(scores()))


def view():
    with column(spacing=8, padding=12):
        text(f"hyp={hyp():.1f} who={who()} score={score()} day={day()}")
        text(f"local={here()}")
        text(f"doc={doc()}")
        with row(spacing=6):
            button("measure", on_click=measure)
            button("parse", on_click=parse)
            button("stamp", on_click=stamp)
            button("write", on_click=write)
            button("write list", on_click=write_list)


if __name__ == "__main__":
    run(view, title="stdlib")
