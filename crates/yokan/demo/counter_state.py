# /// script
# requires-python = ">=3.14"
# ///
"""The same app, third spelling: typed State cells.

count: State[int] — the annotation is the type source for
translation (annotation-first; survives where literal inference
dies). Reads are `count()`, writes are `count.set(v)`, and a bound
`name.set` works directly as a text handler.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import button, column, row, run, State, text, text_field  # noqa: E402

count: State[int] = State(0)
name: State[str] = State("")


def view():
    with column(spacing=12, padding=16):
        text(f"count: {count()}", size=34)
        with row(spacing=8):
            button("+1", on_click=lambda: count.set(count() + 1))
            button("+10", on_click=lambda: count.set(count() + 10))
            button("reset", on_click=lambda: count.set(0))
        text_field(name(), placeholder="your name", on_change=name.set)
        text(f"hello, {name()}")


if __name__ == "__main__":
    run(view, title="counter")
