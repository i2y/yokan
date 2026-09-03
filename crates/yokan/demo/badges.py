# /// script
# requires-python = ">=3.14"
# ///
"""Typography, wrapping and the box a label draws behind itself.
A text can be bold, italic, monospaced or underlined; it can stop
wrapping (`wrap="nowrap"`), clip with an ellipsis (`wrap="ellipsis"`
plus a `width`), or clamp to `max_lines`; and `background`,
`padding` and the border props turn it into a status pill. The pill
colors are named once as styles and composed with `|`, and the last
one follows state — a style value is a value, so `flip` re-colors it
without a second element.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import button, column, row, run, State, style, text  # noqa: E402

pill = style(size=11, color="#11111b", padding=4, border_radius=10)
ok = style(background="#2fa84f")
warn = style(background="#fab387")
crit = style(background="#f38ba8")

pill_ok = pill | ok
pill_warn = pill | warn
pill_crit = pill | crit

tint: State[str] = State("#45475a")
hot: State[bool] = State(False)


def flip():
    hot.set(not hot())
    if hot():
        tint.set("#f38ba8")
    else:
        tint.set("#45475a")


def view():
    with column(spacing=8, padding=12):
        text("Badges", size=20, bold=True)
        with row(spacing=6):
            text("● OK", **pill_ok)
            text("● WARN", **pill_warn)
            text("● CRIT", **pill_crit)
            text(
                "● BUILD",
                size=11,
                color="#cdd6f4",
                background=tint(),
                padding=4,
                border_radius=10,
                border_width=1,
                border_color="#585b70",
            )
        button("flip", on_click=flip)
        text("commit 9f2c1ab8e04d", mono=True, size=12)
        text("an underlined note", underline=True)
        text("in italics, for contrast", italic=True)
        # An ellipsis needs a bounded box to clip against.
        text(
            "a single line far too long for the box it was given, so it ends in an ellipsis",
            wrap="ellipsis",
            width=260,
        )
        # The clamp is the other half: this one wraps, then stops.
        text(
            "a paragraph that wraps at the window's width and then stops after two lines, "
            "because a clamped label is what a card summary wants",
            max_lines=2,
            width=260,
        )


if __name__ == "__main__":
    run(view, title="badges")
