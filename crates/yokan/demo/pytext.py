# /// script
# requires-python = ">=3.14"
# ///
"""@py — a CPython escape compiled INTO the native app.

`slug` stays real Python (stdlib `re`) in both tiers: interpreted on
CPython, and run on an EMBEDDED CPython inside the pixie binary,
bridged through pixie's own [crates] binding machinery. The gate
proves both tiers agree.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

import yokan as ui  # noqa: E402
from yokan import py, State  # noqa: E402


@py
def slug(t: str) -> str:
    import re

    return re.sub(r"[^a-z0-9]+", "-", t.lower()).strip("-")


title: State[str] = State("")
slugged: State[str] = State("")


def retitle(t: str):
    title.set(t)
    slugged.set(slug(t))


def view():
    with ui.column(spacing=10, padding=14):
        ui.text("type a title — the slug is computed by real Python", size=13, color="#8a8f98")
        ui.text_field(title(), placeholder="title", on_change=retitle)
        ui.text(f"slug: {slugged()}", size=16)


if __name__ == "__main__":
    ui.run(view, title="pytext")
