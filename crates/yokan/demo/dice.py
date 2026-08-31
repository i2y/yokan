# /// script
# requires-python = ">=3.14"
# ///
"""yokan.random (seeded — the gate seeds explicitly, so both tiers
walk the same sequence) and helpers grown up: full statement bodies,
callable from VIEW text because they compile to native `static fn`s
(no receiver, no World — view-safe by definition; making that true
took teaching pixie's view lowering AND its interpreter to call
statics).
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

import yokan as ui  # noqa: E402
from yokan import State  # noqa: E402
from yokan import random  # noqa: E402

rolls: State[list[int]] = State([])
total: State[int] = State(0)


def rank(v: int) -> str:
    label = "low"
    if v > 9:
        label = "high"
    return label


def reset():
    random.seed(42)
    rolls.set([])
    total.set(0)


def roll():
    v = random.int(1, 6)
    rolls.set(rolls() + [v])
    total.set(total() + v)


def view():
    with ui.column(spacing=8, padding=12):
        ui.text(f"rolls={len(rolls())} total={total()} rank={rank(total())}")
        with ui.row(spacing=6):
            ui.button("reset", on_click=reset)
            ui.button("roll", on_click=roll)


if __name__ == "__main__":
    ui.run(view, title="dice")
