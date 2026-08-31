# /// script
# requires-python = ">=3.14"
# ///
"""Multi-module apps: state lives in state.py, view helpers in
widgets.py. Helpers compile as reusable components; the build
flattens the module graph into the one compiled program.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))

import yokan as ui  # noqa: E402
from state import count  # noqa: E402
from widgets import badge, header  # noqa: E402


def view():
    with ui.column(spacing=10, padding=14):
        header()
        badge("multi-module")
        ui.button("+1", on_click=lambda: count.set(count() + 1))


if __name__ == "__main__":
    ui.run(view, title="multi")
