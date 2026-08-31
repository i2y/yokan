"""The pyproject spelling: no PEP 723 block — the crate declaration
lives in this directory's pyproject.toml under [tool.yokan.crates].
Same call surface, same two doors, same gate.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))

import yokan as ui  # noqa: E402
from yokan import crates, store  # noqa: E402


@store
class Out:
    encoded: str = "-"
    total: int = 0

    def run(self) -> None:
        self.encoded = crates.hexfmt.encode("proj")
        self.total = crates.hexfmt.add(20, 22)


def view():
    with ui.column(spacing=8, padding=12):
        ui.text(f"encoded: {Out.encoded}")
        ui.text(f"total: {Out.total}")
        ui.button("run", on_click=Out.run)


if __name__ == "__main__":
    ui.run(view, title="proj", on_start=Out.run)
