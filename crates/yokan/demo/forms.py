# /// script
# requires-python = ">=3.14"
# ///
"""The form controls, gated: checkbox / switch (click by label
toggles), slider (`slide:` steps), select / radio_group / tab_bar
(`select:` steps) — every handler receives the new value as its one
argument.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import (
    checkbox,
    column,
    radio_group,
    run,
    select,
    slider,
    State,
    store,
    switch,
    tab_bar,
    text,
)


@store
class Settings:
    dark: bool = False
    wifi: bool = True
    volume: float = 5.0
    fruits: list[str] = ["apple", "banana", "cherry"]
    fruit: int = 0
    sizes: list[str] = ["small", "medium", "large"]
    size: int = 1
    tabs: list[str] = ["General", "Details", "About"]
    tab: int = 0

    def set_dark(self, on: bool) -> None:
        self.dark = on

    def set_wifi(self, on: bool) -> None:
        self.wifi = on

    def set_volume(self, v: float) -> None:
        self.volume = v

    def pick_fruit(self, i: int) -> None:
        self.fruit = i

    def pick_size(self, i: int) -> None:
        self.size = i

    def pick_tab(self, i: int) -> None:
        self.tab = i


def view():
    with column(spacing=10, padding=14):
        checkbox("Dark mode", checked=Settings.dark, on_change=Settings.set_dark)
        switch("Wi-Fi", checked=Settings.wifi, on_change=Settings.set_wifi)
        slider(value=Settings.volume, min=0.0, max=10.0, step=1.0, on_change=Settings.set_volume)
        select(options=Settings.fruits, selected=Settings.fruit, on_change=Settings.pick_fruit)
        radio_group(options=Settings.sizes, selected=Settings.size, on_change=Settings.pick_size)
        tab_bar(labels=Settings.tabs, active=Settings.tab, on_change=Settings.pick_tab)
        if Settings.tab == 0:
            text("general panel", size=12)
        elif Settings.tab == 1:
            text("details panel", size=12)
        else:
            text("about panel", size=12)
        text(f"dark={Settings.dark}  wifi={Settings.wifi}  vol={Settings.volume:.1f}")
        text(f"fruit#{Settings.fruit}  size#{Settings.size}  tab#{Settings.tab}")


if __name__ == "__main__":
    run(view, title="forms", width=460, height=420)
