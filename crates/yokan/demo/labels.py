# /// script
# requires-python = ">=3.14"
# ///
"""Accessibility riders, gated: `role=` overrides the role an element
derives, `a11y_label=` is the name assistive technology reads instead
of what the element would otherwise derive. Mirrors
examples/labels/labels.pix — the `a11y` headless step prints the
resulting tree, the same one a platform adapter would be handed.

Develop:  uv run demo/labels.py
Ship:     python3 yokan_gate.py gate demo/labels.py --script "a11y,click:save,a11y"
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import button, column, progress, row, run, State, svg, text, text_field  # noqa: E402


title: State[str] = State("Reports")
query: State[str] = State("")
# `role=` takes a value: the summary line is a heading until there is
# a result under it, and then it is not (mirrors labels.pix).
summary_role: State[str] = State("heading")


def save():
    summary_role.set("label")


def find(q: str):
    query.set(q)


def view():
    with column(spacing=8, padding=12):
        text(title(), size=22, role="heading")
        with row(spacing=6, role="group", a11y_label="toolbar"):
            svg("demo/assets/yokan.svg", width=20, height=20, a11y_label="Yokan")
            svg("demo/assets/search.svg", width=20, height=20, a11y_label="Search")
            # The one element carrying tooltip=, role=, a11y_label=
            # AND animate= together — proving the wrapper nesting
            # (Semantics, then Tooltip, then Anim) matches pixie's
            # own codegen byte for byte.
            button(
                "save",
                on_click=save,
                animate=150,
                role="button",
                a11y_label="Save the report",
                tooltip="Save this report",
            )
        text_field(query(), placeholder="search", on_change=find, a11y_label="search")
        text("1 of 4 saved", role=summary_role())
        progress(0.4)


if __name__ == "__main__":
    run(view, title="labels", width=420, height=320)
