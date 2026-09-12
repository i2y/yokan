# /// script
# requires-python = ">=3.14"
# ///
"""A split is two panes and a divider you drag.

`ratio` is the share the first pane takes, and it is the app's own
number: the handler receives the new one and writes it back, which is
what moves the divider. That is the slider's contract with a different
gesture, so `slide:` drives it headless with no verb of its own — and
because a ratio is a fraction, the widget has no min= / max=: a pane
that may not vanish says so once, in the handler.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import column, run, split, store, text  # noqa: E402


@store
class Panes:
    ratio: float = 0.5
    rows: float = 0.35

    def widen(self, r: float) -> None:
        # The floor under each pane, kept where the app keeps the
        # number rather than on the widget: one owner, not two.
        self.ratio = min(0.8, max(0.2, r))

    def lower(self, r: float) -> None:
        self.rows = min(0.85, max(0.15, r))


def view():
    with column(spacing=10, padding=14, grow=1.0):
        text(f"files {Panes.ratio}, output below {Panes.rows}", size=13)
        split(
            column(
                text("files", size=13),
                padding=12,
                background="#313244",
                grow=1.0,
            ),
            split(
                column(
                    text("editor", size=13),
                    padding=12,
                    background="#45475a",
                    grow=1.0,
                ),
                column(
                    text("output", size=13),
                    padding=12,
                    background="#181825",
                    grow=1.0,
                ),
                ratio=Panes.rows,
                vertical=True,
                on_change=Panes.lower,
            ),
            ratio=Panes.ratio,
            on_change=Panes.widen,
        )


if __name__ == "__main__":
    run(view, title="split")
