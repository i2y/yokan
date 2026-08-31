# /// script
# requires-python = ">=3.14"
# ///
"""The dialect reference: everything in this file translates to .pix.

Develop:  uv run demo/counter.py
Ship:     python3 yokan_gate.py gate demo/counter.py --script "click:+1,input:Momo"
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import button, column, row, run, State, text, text_field  # noqa: E402


count: State[int] = State(0)
name: State[str] = State("")


def view():
    return column(
        text(f"count: {count()}", size=34),
        row(
            button("+1", on_click=lambda: count.set(count() + 1)),
            button("+10", on_click=lambda: count.set(count() + 10)),
            button("reset", on_click=lambda: count.set(0)),
            spacing=8,
        ),
        text_field(name(), placeholder="your name", on_change=name.set),
        text(f"hello, {name()}"),
        spacing=12,
        padding=16,
    )


if __name__ == "__main__":
    run(view, title="counter")
