"""The pyproject spelling: no PEP 723 block — the crate declaration
lives in this directory's pyproject.toml under [tool.yokan.crates].
Same call surface, same two doors, same gate.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))

from yokan import button, column, crates, run, store, text  # noqa: E402


@store
class Out:
    encoded: str = "-"
    total: int = 0

    def run(self) -> None:
        self.encoded = crates.hexfmt.encode("proj")
        self.total = crates.hexfmt.add(20, 22)


def view():
    with column(spacing=8, padding=12):
        text(f"encoded: {Out.encoded}")
        text(f"total: {Out.total}")
        button("run", on_click=Out.run)


if __name__ == "__main__":
    run(view, title="proj", on_start=Out.run)
