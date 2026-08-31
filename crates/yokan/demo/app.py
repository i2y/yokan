# /// script
# requires-python = ">=3.14"
# dependencies = ["numpy"]
# ///
"""yokan demo: real CPython + numpy driving pixie's gpui engine.

Build the module, then run:
    cargo build -p yokan --release --features extension-module
    cp <target>/release/libyokan.dylib crates/yokan/yokan.so
    uv run crates/yokan/demo/app.py

While it runs, edit view() below and save — the window updates in
place; the session id, the count and the typed name all survive
(state lives on, only the view function is re-executed).
"""
import os
import random
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

import numpy as np  # noqa: E402
import yokan as ui  # noqa: E402


def view(s):
    xs = np.linspace(0.0, 2.0 * np.pi, 36)
    wave = ((np.sin(xs + s["phase"]) + 1.0) * 0.5).tolist()
    greeting = f"Hello, {s['name']}!" if s["name"] else "type your name below"
    return ui.column(
        ui.text(f"yokan — session #{s['sid']}", size=13, color="#8a8f98"),
        ui.text(f"count: {s['count']}", size=34),
        ui.row(
            ui.button("+1", on_click=lambda: s.update(count=s["count"] + 1)),
            ui.button("+10", on_click=lambda: s.update(count=s["count"] + 10)),
            ui.button("wave", on_click=lambda: s.update(phase=s["phase"] + 0.7)),
            spacing=8,
        ),
        ui.text_field(
            s["name"],
            placeholder="your name",
            on_change=lambda t: s.update(name=t),
        ),
        ui.text(greeting, size=16),
        ui.bar_chart(wave, height=140.0),
        ui.text("edit view() and save — state survives the reload", size=12, color="#8a8f98"),
        spacing=12,
        padding=16,
    )


if __name__ == "__main__":
    ui.run(
        view,
        state={"sid": random.randint(1000, 9999), "count": 0, "phase": 0.0, "name": ""},
        title="yokan",
    )
