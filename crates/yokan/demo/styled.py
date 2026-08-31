# /// script
# requires-python = ">=3.14"
# ///
"""Named styles and the theme scope. A style is a plain dict of
element kwargs (`style`), applied with `**` — plain Python when
interpreted, a native style block when compiled; `|` merges styles
and `theme=` scopes a palette over a subtree. Tokens like "accent"
resolve in one shared place, so a palette flip re-colors the
interpreted and the compiled app identically.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import button, column, row, run, State, style, text  # noqa: E402

chip = style(size=18, color="accent")
key = style(background="#313244", hover_background="#45475a")
hot = style(background="#fab387")
key_hot = key | hot

mode: State[str] = State("dark")
n: State[int] = State(0)


def flip():
    if mode() == "dark":
        mode.set("light")
    else:
        mode.set("dark")


def view():
    with column(spacing=8, padding=12, background="panel", theme=mode()):
        text(f"n={n()}", **chip)
        with row(spacing=6):
            button("+1", on_click=lambda: n.set(n() + 1), **key)
            button("flip", on_click=flip, **key_hot)


if __name__ == "__main__":
    run(view, title="styled")
