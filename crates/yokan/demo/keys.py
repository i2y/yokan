# /// script
# requires-python = ">=3.14"
# ///
"""Keys and the clipboard. `shortcut(chord, handler)` declares a chord
the app answers, and `on_key(handler)` sees every key as the chord it
was. The chord is spelled the way the platform spells it (`cmd+s`,
`shift-tab`, `ctrl+alt+k`); a headless script presses one with
`key:cmd+s`, so a shortcut is checked by the gate like any other
interaction. `clipboard.set_text` / `get_text` copy and paste: a
window exchanges the text with every other application, a headless
run keeps it to itself. `menu_item(menu, name, handler)` puts the
same handlers in the application's menu bar, and a script picks one
with `menu:Save`.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import (  # noqa: E402
    button,
    column,
    menu_item,
    on_key,
    row,
    run,
    shortcut,
    State,
    text,
)
from yokan import clipboard  # noqa: E402

count: State[int] = State(0)
saved: State[int] = State(0)
last: State[str] = State("-")
pasted: State[str] = State("(nothing)")


def bump():
    count.set(count() + 1)


def save():
    saved.set(count())


def clear():
    count.set(0)
    saved.set(0)


def typed(key: str):
    last.set(key)


def copy_count():
    clipboard.set_text(f"count={count()}")


def paste():
    pasted.set(clipboard.get_text())


menu_item("Count", "Save", save)
menu_item("Count", "Clear", clear)

shortcut("cmd+s", save)
shortcut("cmd+shift+r", clear)
shortcut("cmd+shift+c", copy_count)
shortcut("cmd+shift+v", paste)
on_key(typed)


def view():
    with column(spacing=8, padding=12):
        text(f"count: {count()}  saved: {saved()}")
        text(f"last key: {last()}")
        text(f"pasted: {pasted()}")
        with row(spacing=6):
            button("+1", on_click=bump)
            button("save", on_click=save)
            button("copy", on_click=copy_count)
            button("paste", on_click=paste)


if __name__ == "__main__":
    run(view, title="keys")
