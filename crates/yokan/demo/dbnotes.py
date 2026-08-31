# /// script
# requires-python = ">=3.14"
# ///
"""sqlite from the standard library: one bundled implementation
serves the interpreted and the compiled app alike. Rows come back
as column-0 text — shape the row with SQL, order with ORDER BY
(determinism is the app's SQL, not the module's guess).
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

import yokan as ui  # noqa: E402
from yokan import State  # noqa: E402
from yokan import sqlite  # noqa: E402

changed: State[int] = State(0)
rows: State[list[str]] = State([])


def setup():
    sqlite.exec("demo/.gate/notes.db", "CREATE TABLE IF NOT EXISTS notes(t TEXT)")
    sqlite.exec("demo/.gate/notes.db", "DELETE FROM notes")
    changed.set(sqlite.exec("demo/.gate/notes.db", "INSERT INTO notes VALUES ('alpha'),('beta'),('gamma')"))


def load():
    rows.set(sqlite.query_text("demo/.gate/notes.db", "SELECT t FROM notes ORDER BY t"))


def row(i):
    return ui.text(rows()[i])


def view():
    with ui.column(spacing=8, padding=12):
        ui.text(f"inserted={changed()} rows={len(rows())}")
        with ui.row(spacing=6):
            ui.button("setup", on_click=setup)
            ui.button("load", on_click=load)
        ui.list_view(len(rows()), row, item_height=22.0, height=120.0)


if __name__ == "__main__":
    ui.run(view, title="dbnotes")
