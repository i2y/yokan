# /// script
# requires-python = ">=3.14"
# ///
"""task — slow work off the UI thread, in both runs.

`task(work, on_done=...)` hands the work to a worker: during
development that is a Python thread, and the compiled app awaits the
standard-library call inside it, which puts it on the engine's pool.
Either way the window keeps drawing — the counter button stays
clickable while the work runs — and `on_done` lands the result.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import (  # noqa: E402
    State,
    button,
    column,
    row,
    run,
    spinner,
    task,
    text,
    time,
)

busy: State[bool] = State(False)
result: State[str] = State("—")
n: State[int] = State(0)


def slow_work() -> int:
    time.sleep_ms(1500)
    return 1_500


def start():
    busy.set(True)
    task(slow_work, on_done=lambda v: (busy.set(False), result.set(f"waited {v} ms")))


def view():
    with column(spacing=12, padding=16):
        text("task — the UI thread never blocks", size=13, color="#8a8f98")
        with row(spacing=8):
            button("start slow work", on_click=start)
            button(f"+1 ({n()})", on_click=lambda: n.set(n() + 1))
        if busy():
            with row(spacing=8):
                spinner(size=16.0)
                text("working…", color="#8a8f98")
        else:
            text(f"result: {result()}", size=18)


if __name__ == "__main__":
    run(view, title="tasks")
