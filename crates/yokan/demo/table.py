# /// script
# requires-python = ">=3.14"
# ///
"""data_table draws the table itself: the first `row` inside it is
the header, every later `row` is a data row shaded in alternation,
and the frame around them comes with the element. Columns line up
because the cells of one column carry the same `grow` share.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import button, column, data_table, row, run, store, text  # noqa: E402


@store
class Fleet:
    api: int = 42
    db: int = 17
    cache: int = 8
    edge: int = 95
    polls: int = 0

    def refresh(self) -> None:
        self.polls += 1
        self.api = (self.api * 3 + 29) % 140
        self.db = (self.db * 5 + 11) % 140
        self.cache = (self.cache * 7 + 3) % 140
        self.edge = (self.edge * 2 + 47) % 140


def health(ms: int) -> str:
    label = "ok"
    if ms > 60:
        label = "watch"
    if ms > 100:
        label = "slow"
    return label


def view():
    with column(spacing=10, padding=14):
        text(f"fleet latency — {Fleet.polls} polls", size=16)
        with data_table():
            with row(spacing=8):
                text("service", grow=2.0)
                text("latency", grow=1.0, align="right")
                text("health", grow=1.0, align="center")
            with row(spacing=8):
                text("api", grow=2.0)
                text(f"{Fleet.api} ms", grow=1.0, align="right")
                text(f"{health(Fleet.api)}", grow=1.0, align="center")
            with row(spacing=8):
                text("db", grow=2.0)
                text(f"{Fleet.db} ms", grow=1.0, align="right")
                text(f"{health(Fleet.db)}", grow=1.0, align="center")
            with row(spacing=8):
                text("cache", grow=2.0)
                text(f"{Fleet.cache} ms", grow=1.0, align="right")
                text(f"{health(Fleet.cache)}", grow=1.0, align="center")
            with row(spacing=8):
                text("edge", grow=2.0)
                text(f"{Fleet.edge} ms", grow=1.0, align="right")
                text(f"{health(Fleet.edge)}", grow=1.0, align="center")
        button("refresh", on_click=Fleet.refresh)


if __name__ == "__main__":
    run(view, title="table")
