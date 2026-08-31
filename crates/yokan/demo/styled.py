# /// script
# requires-python = ">=3.14"
# ///
"""Named styles and the theme scope. A style is a plain dict of
element kwargs (`ui.style`), applied with `**` — plain Python when
interpreted, a native style block when compiled; `|` merges styles
and `theme=` scopes a palette over a subtree. Tokens like "accent"
resolve in one shared place, so a palette flip re-colors the
interpreted and the compiled app identically.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

import yokan as ui  # noqa: E402
from yokan import State  # noqa: E402

chip = ui.style(size=18, color="accent")
key = ui.style(background="#313244", hover_background="#45475a")
hot = ui.style(background="#fab387")
key_hot = key | hot

mode: State[str] = State("dark")
n: State[int] = State(0)


def flip():
    if mode() == "dark":
        mode.set("light")
    else:
        mode.set("dark")


def view():
    with ui.column(spacing=8, padding=12, background="panel", theme=mode()):
        ui.text(f"n={n()}", **chip)
        with ui.row(spacing=6):
            ui.button("+1", on_click=lambda: n.set(n() + 1), **key)
            ui.button("flip", on_click=flip, **key_hot)


if __name__ == "__main__":
    ui.run(view, title="styled")
