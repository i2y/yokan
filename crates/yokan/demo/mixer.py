# /// script
# requires-python = ">=3.14"
# ///
"""Grouped state is a fields-only @store: annotated fields, direct
reads in views (`Mixer.volume`), writes through methods — no
separate instance line, and methods are there the day you need one.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import button, column, row, run, store, text, text_field  # noqa: E402


@store
class Mixer:
    volume: int = 5
    title: str = "untitled"
    muted: bool = False

    def louder(self) -> None:
        self.volume += 1

    def set_muted(self, on: bool) -> None:
        self.muted = on

    def rename(self, t: str) -> None:
        self.title = t


def view():
    with column(spacing=10, padding=14):
        text(f"{Mixer.title} — vol {Mixer.volume}", size=16)
        with row(spacing=8):
            button("+1", on_click=Mixer.louder)
            button("mute", on_click=lambda: Mixer.set_muted(True))
            button("unmute", on_click=lambda: Mixer.set_muted(False))
        if Mixer.muted:
            text("(muted)", size=12, color="#8a8f98")
        text_field(Mixer.title, placeholder="title", on_change=Mixer.rename)


if __name__ == "__main__":
    run(view, title="mixer")
