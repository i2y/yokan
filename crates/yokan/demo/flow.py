# /// script
# requires-python = ">=3.14"
# ///
"""Handler control flow, natively compiled: if/elif/else, while,
for-over-range and for-over-list with break/continue, and a pure
helper fn that lowers to a native free fn (not an escape — the
computation itself compiles). Locals are block-scoped natively, so
the translator refuses reads that Python would leak.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import button, column, row, run, State, text  # noqa: E402

count: State[int] = State(0)
total: State[int] = State(0)
status: State[str] = State("start")


def double(v: int) -> int:
    return v * 2


def step():
    count.set(count() + 1)
    if count() > 3 and count() < 100:
        status.set("big")
    elif count() == 3:
        status.set("three")
    else:
        status.set("small")


def tally():
    total.set(0)
    for i in range(1, 6):
        if i == 3:
            continue
        total.set(total() + double(i))


def bump3():
    while count() < 3:
        count.set(count() + 1)


def find():
    for i in range(0, 10):
        if i * i > 10:
            count.set(i)
            break


def view():
    with column(spacing=8, padding=12):
        text(f"count={count()} total={total()} status={status()}")
        with row(spacing=6):
            button("step", on_click=step)
            button("tally", on_click=tally)
            button("bump3", on_click=bump3)
            button("find", on_click=find)


if __name__ == "__main__":
    run(view, title="flow")
