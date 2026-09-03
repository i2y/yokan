# /// script
# requires-python = ">=3.14"
# ///
"""spacer and divider: a flex filler and a rule. The header row's
spacer pushes "ping" to the row's far edge; the footer row's spacer
does the same for the ping count. divider() draws the rules —
default weight between the header and the body, a heavier
theme-colored one between the body's two sections.

Develop:  uv run demo/layout.py
Ship:     python3 yokan_gate.py gate demo/layout.py --script "click:ping"
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import button, column, divider, row, run, spacer, State, text  # noqa: E402


pings: State[int] = State(0)


def view():
    return column(
        row(
            text("Layout", size=18),
            spacer(),
            button("ping", on_click=lambda: pings.set(pings() + 1)),
        ),
        divider(),
        column(
            text("Section one", size=14),
            text("spacer() takes the slack a row leaves behind."),
            divider(thickness=2.0, color="accent"),
            text("Section two", size=14),
            text("divider() draws a rule across its parent."),
            spacing=6,
        ),
        row(
            spacer(),
            text(f"pings: {pings()}"),
        ),
        spacing=12,
        padding=16,
    )


if __name__ == "__main__":
    run(view, title="layout")
