# /// script
# requires-python = ">=3.14"
# ///
"""segmented replaces a row of if/else-styled buttons with one bound
chooser: the accent-filled segment IS the current filter, and
picking another segment reruns `on_change` with its 0-based index —
the shape opsboard's alert filter needs three buttons and an
if/else per button for (`demo/opsboard/app.py`, the `Alerts.filter`
row).

Develop:  uv run demo/filter.py
Ship:     python3 yokan_gate.py gate demo/filter.py --script "select:crit,dump,select:all"
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import column, list_view, run, segmented, store, text  # noqa: E402


@store
class Alerts:
    # `self.levels[i]` (a list indexed by an arbitrary Int) is not in
    # the dialect yet — only a row builder's OWN driving list may be
    # indexed by its row number — and a plain method call (like
    # `.startswith()`) is not a valid `if` condition either, only a
    # bool cell/field or a comparison is. So the split mirrors
    # opsboard's `Alerts.rebuild()`: one list per severity, `pick`
    # branches on the INDEX (an Int comparison) instead of a String
    # field, and appends whichever lists the chosen segment covers.
    levels: list[str] = ["all", "crit", "warn"]
    level: int = 0
    crit_rows: list[str] = [
        "crit  09:02  payments p95 breach — circuit breaker armed",
        "crit  09:11  db failover triggered",
        "crit  09:20  worker pool exhausted",
    ]
    warn_rows: list[str] = [
        "warn  09:05  error budget burn 2x on web",
        "warn  09:14  cache hit rate below 80%",
        "warn  09:24  edge latency above SLO",
    ]
    visible: list[str] = [
        "crit  09:02  payments p95 breach — circuit breaker armed",
        "crit  09:11  db failover triggered",
        "crit  09:20  worker pool exhausted",
        "warn  09:05  error budget burn 2x on web",
        "warn  09:14  cache hit rate below 80%",
        "warn  09:24  edge latency above SLO",
    ]

    def pick(self, i: int) -> None:
        self.level = i
        self.visible = []
        if i == 0:
            for r in self.crit_rows:
                self.visible = self.visible + [r]
            for r in self.warn_rows:
                self.visible = self.visible + [r]
        elif i == 1:
            for r in self.crit_rows:
                self.visible = self.visible + [r]
        else:
            for r in self.warn_rows:
                self.visible = self.visible + [r]


def alert_row(i):
    return text(Alerts.visible[i], size=12)


def view():
    with column(spacing=10, padding=14):
        text("alert filter", size=16)
        segmented(options=Alerts.levels, selected=Alerts.level, on_change=Alerts.pick)
        text(f"{len(Alerts.visible)} shown", size=12, color="textDim")
        list_view(len(Alerts.visible), alert_row, item_height=22.0, height=150.0)


if __name__ == "__main__":
    run(view, title="filter")
