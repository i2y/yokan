# /// script
# requires-python = ">=3.14"
# ///
"""task — slow work off the UI thread, in both runs.

`task(work, on_done=..., on_progress=...)` hands the work to a
worker: during development that is a Python thread, and the compiled
app awaits the standard-library call inside it, which puts it on the
engine's pool. Either way the window keeps drawing — the counter
button stays clickable while the work runs — and `on_done` lands the
result.

The work is not silent while it runs: `report(fraction, note)` says
where it has got to, from wherever it is running, and `on_progress`
hears it on the UI thread. Every report is heard, and the last one
lands before `on_done` does.
"""
import os
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import (  # noqa: E402
    State,
    button,
    column,
    progress,
    report,
    row,
    run,
    task,
    text,
)

busy: State[bool] = State(False)
result: State[str] = State("—")
done: State[float] = State(0.0)
step: State[str] = State("")
heard: State[int] = State(0)
n: State[int] = State(0)


def slow_work() -> int:
    time.sleep(0.5)
    report(0.33, "500 ms")
    time.sleep(0.5)
    report(0.66, "1000 ms")
    time.sleep(0.5)
    report(1.0, "1500 ms")
    return 1_500


def moved(fraction: float, note: str):
    done.set(fraction)
    step.set(note)
    heard.set(heard() + 1)


def start():
    busy.set(True)
    task(
        slow_work,
        on_done=lambda v: (busy.set(False), result.set(f"waited {v} ms")),
        on_progress=moved,
    )


def view():
    with column(spacing=12, padding=16):
        text("task — the UI thread never blocks", size=13, color="#8a8f98")
        with row(spacing=8):
            button("start slow work", on_click=start)
            button(f"+1 ({n()})", on_click=lambda: n.set(n() + 1))
        progress(done(), width=260.0)
        text(f"{step()} · {heard()} reports", size=13, color="#8a8f98")
        if busy():
            text("working…", color="#8a8f98")
        else:
            text(f"result: {result()}", size=18)


if __name__ == "__main__":
    run(view, title="tasks")
