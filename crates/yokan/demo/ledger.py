# /// script
# requires-python = ">=3.14"
# ///
"""A practical app: a household ledger. Everything on the stack at
once — a named store with methods over sqlite, dict/list fields, a
chart, styles, typed text input (`strings.to_int` is total: bad
input becomes the default, identically in both tiers) — and it
ships as one file.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

import yokan as ui  # noqa: E402
from yokan import State, store  # noqa: E402
from yokan import sqlite, strings  # noqa: E402

DB = "demo/.gate/ledger.db"

heading = ui.style(size=20, color="accent")
faint = ui.style(size=12, color="#8a8f98")

name: State[str] = State("")
amount: State[str] = State("")


@store
class Ledger:
    count: int = 0
    grand: int = 0
    food: int = 0
    transit: int = 0
    fun: int = 0
    totals: dict[str, int] = {}
    chart: list[float] = []
    rows: list[str] = []

    def reset(self) -> None:
        sqlite.exec("demo/.gate/ledger.db", "CREATE TABLE IF NOT EXISTS expenses(name TEXT, amount INTEGER, cat TEXT)")
        sqlite.exec("demo/.gate/ledger.db", "DELETE FROM expenses")
        self.load()

    def add(self, item: str, yen: int, cat: str) -> None:
        if yen > 0:
            sqlite.exec("demo/.gate/ledger.db", f"INSERT INTO expenses VALUES ('{item}', {yen}, '{cat}')")
            self.load()

    def load(self) -> None:
        # the *_or family: a missing table reads as clean zeros —
        # return-value defaults are the ergonomic default; try/except
        # is for when the failure REASON matters (see tryfetch).
        self.count = sqlite.query_int_or("demo/.gate/ledger.db", "SELECT COUNT(*) FROM expenses", 0)
        self.grand = sqlite.query_int_or("demo/.gate/ledger.db", "SELECT COALESCE(SUM(amount),0) FROM expenses", 0)
        f = sqlite.query_int_or("demo/.gate/ledger.db", "SELECT COALESCE(SUM(amount),0) FROM expenses WHERE cat='food'", 0)
        t = sqlite.query_int_or("demo/.gate/ledger.db", "SELECT COALESCE(SUM(amount),0) FROM expenses WHERE cat='transit'", 0)
        n = sqlite.query_int_or("demo/.gate/ledger.db", "SELECT COALESCE(SUM(amount),0) FROM expenses WHERE cat='fun'", 0)
        self.food = f
        self.transit = t
        self.fun = n
        self.totals = {}
        self.totals["food"] = f
        self.totals["transit"] = t
        self.totals["fun"] = n
        self.chart = []
        self.chart = self.chart + [1.0 * f]
        self.chart = self.chart + [1.0 * t]
        self.chart = self.chart + [1.0 * n]
        self.rows = sqlite.query_text_or("demo/.gate/ledger.db", "SELECT name || '  ¥' || amount || '  (' || cat || ')' FROM expenses ORDER BY rowid")


def row(i):
    return ui.text(Ledger.rows[i])


def add_food():
    Ledger.add(name(), strings.to_int(amount(), 0), "food")


def add_transit():
    Ledger.add(name(), strings.to_int(amount(), 0), "transit")


def add_fun():
    Ledger.add(name(), strings.to_int(amount(), 0), "fun")


def view():
    with ui.column(spacing=10, padding=14, background="panel"):
        ui.text("ledger", **heading)
        with ui.row(spacing=6):
            ui.text_field(name(), placeholder="item", on_change=name.set)
            ui.text_field(amount(), placeholder="yen", on_change=amount.set)
        with ui.row(spacing=6):
            ui.button("food", on_click=add_food)
            ui.button("transit", on_click=add_transit)
            ui.button("fun", on_click=add_fun)
        ui.bar_chart(Ledger.chart, height=100.0)
        ui.list_view(len(Ledger.rows), row, item_height=22.0, height=110.0)
        ui.text(f"entries={Ledger.count} total=¥{Ledger.grand}")
        ui.text(f"food ¥{Ledger.food} · transit ¥{Ledger.transit} · fun ¥{Ledger.fun}", **faint)
        with ui.row(spacing=6):
            ui.button("load", on_click=Ledger.load)
            ui.button("reset", on_click=Ledger.reset)


if __name__ == "__main__":
    ui.run(view, title="ledger", on_start=Ledger.load)
