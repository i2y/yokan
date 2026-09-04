# /// script
# requires-python = ">=3.14"
# ///
"""The standard library, in its two halves.

`math`, `random` and `statistics` are Python's own, written the way
Python writes them. During development the app imports CPython's
module; the shipped binary calls a twin written against it, and a
table of answers CPython printed holds the twin to CPython. Seed the
generator and the two runs walk the same sequence.

`json.dumps` is Python's too, and writes what CPython writes: keys in
the order they went in, `", "` between the parts, non-ASCII escaped.
Reading a path out of a document is Yokan's own, under `jsondoc`,
because Python's `json` has no such thing;
`datetime` is Python's as well: a date is a value that adds a
timedelta, subtracts another date and formats itself, and the twin
answers what CPython answers.

`clock.format_ms` is UTC and `clock.format_local_ms` is the machine's
own zone, from the same zone database in both runs. Python's own
`time` is there too, for the clock itself.
"""
import json
import math
import os
import random
import statistics
import sys
import time
from datetime import date, timedelta

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import button, column, row, run, State, text  # noqa: E402
from yokan import clock, jsondoc  # noqa: E402

hyp: State[float] = State(0.0)
spread: State[str] = State("-")
rolls: State[str] = State("-")
who: State[str] = State("-")
score: State[int] = State(0)
day: State[str] = State("-")
here: State[str] = State("-")
ticked: State[str] = State("-")
due: State[date] = State(date(2026, 1, 1))
plan: State[str] = State("-")
doc: State[str] = State("-")
scores: State[list[int]] = State([3, 5, 8])


def measure():
    hyp.set(math.sqrt(3.0 * 3.0 + 4.0 * 4.0))


def schedule():
    # A date is a value: arithmetic, comparison and formatting all
    # answer what Python answers, down to the weekday's name.
    due.set(date(2026, 1, 1) + timedelta(weeks=6))
    span = due() - date(2026, 1, 1)
    plan.set(f"{due()} ({due().strftime('%A')}) in {span.days} days")


def summarize():
    xs: list[float] = [0.1, 0.2, 0.3]
    # An exact mean, as CPython computes it: 0.2, not the
    # 0.20000000000000004 a plain sum would give.
    spread.set(f"{statistics.mean(xs)} sd={statistics.stdev([1.5, 2.5, 4.75]):.4f}")


def roll():
    # Seeded, so both runs walk the same sequence.
    random.seed(20260904)
    out = ""
    for _i in range(5):
        out = out + f"{random.randint(1, 6)}"
    rolls.set(f"{out} u={random.uniform(0.0, 1.0):.4f}")


def parse():
    who.set(jsondoc.get_text('{"name": "momo", "scores": [3, 5, 8]}', "name"))
    score.set(jsondoc.get_int('{"name": "momo", "scores": [3, 5, 8]}', "scores.2"))


def stamp():
    # A clock reads differently in every run, so what a gate compares
    # is the shape, not the moment: this one only asks that the two
    # readings are ordered.
    lo = time.monotonic()
    time.sleep(0.001)
    if time.monotonic() > lo:
        ticked.set("yes")
    else:
        ticked.set("no")
    day.set(clock.format_ms(0, "%Y-%m-%d"))
    here.set(clock.format_local_ms(0, "%Y-%m-%d %H:%M"))


def write():
    # A literal nests as deep as it is written out.
    doc.set(json.dumps({"name": "momo", "team": "yokan", "tags": ["a", "b"]}))


def write_list():
    doc.set(json.dumps(scores()))


def view():
    with column(spacing=8, padding=12):
        text(f"hyp={hyp():.1f} who={who()} score={score()} day={day()}")
        text(f"local={here()}  ticked={ticked()}")
        text(f"exact={spread()}")
        text(f"due={due()} plan={plan()}")
        text(f"rolls={rolls()}")
        text(f"tau={math.tau:.5f} floor={math.floor(hyp())}")
        text(f"doc={doc()}")
        with row(spacing=6):
            button("measure", on_click=measure)
            button("stats", on_click=summarize)
            button("due", on_click=schedule)
            button("roll", on_click=roll)
        with row(spacing=6):
            button("parse", on_click=parse)
            button("stamp", on_click=stamp)
            button("write", on_click=write)
            button("write list", on_click=write_list)


if __name__ == "__main__":
    run(view, title="stdlib")
