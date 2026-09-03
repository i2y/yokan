# /// script
# requires-python = ">=3.14"
# ///
"""Charts that can say what they mean: a profit-and-loss bar chart
whose losing months hang below the zero line, and a two-series line
chart of requests against errors.

`axis=True` puts the range's ends and the zero line in the margin
with a faint gridline across the plot at each; `series=` takes one
`list[list[float]]` field, one inner list per line, and `colors=`
names them.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import (
    bar_chart,
    button,
    column,
    line_chart,
    row,
    run,
    store,
    style,
    text,
)

heading = style(size=18, color="accent")
faint = style(size=12, color="#8a8f98")


@store
class Book:
    months: list[str] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun"]
    profit: list[float] = [12.0, -8.0, 4.0, -3.0, 15.0, -6.0]
    requests: list[float] = [40.0, 55.0, 48.0, 62.0, 70.0, 58.0]
    errors: list[float] = [3.0, 9.0, 5.0, 12.0, 6.0, 4.0]
    traffic: list[list[float]] = [
        [40.0, 55.0, 48.0, 62.0, 70.0, 58.0],
        [3.0, 9.0, 5.0, 12.0, 6.0, 4.0],
    ]
    n: int = 6

    def next_month(self) -> None:
        self.n = self.n + 1
        # A deterministic next month, so both runs read the same
        # numbers and the gate can byte-compare them.
        p = 1.0 * (self.n * 7 % 41) - 18.0
        self.profit = self.profit + [p]
        self.months = self.months + [f"M{self.n}"]
        self.requests = self.requests + [1.0 * (self.n * 13 % 50) + 30.0]
        self.errors = self.errors + [1.0 * (self.n * 5 % 14)]
        # `series=` reads ONE list[list[float]], so the two flat
        # series are collected into it after each shift.
        self.traffic = []
        self.traffic = self.traffic + [self.requests]
        self.traffic = self.traffic + [self.errors]


def advance():
    Book.next_month()


def view():
    with column(spacing=12, padding=16):
        text("Profit and loss", **heading)
        text("negative months hang below the zero line", **faint)
        bar_chart(Book.profit, labels=Book.months, axis=True, height=150.0)
        text("Traffic", **heading)
        text("requests and errors, one color each", **faint)
        line_chart(
            series=Book.traffic,
            labels=Book.months,
            colors=["accent", "#f38ba8"],
            axis=True,
            max=90.0,
            height=150.0,
        )
        with row(spacing=8):
            button("next month", on_click=advance)


if __name__ == "__main__":
    run(view, title="charts")
