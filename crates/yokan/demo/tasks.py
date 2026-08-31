# /// script
# requires-python = ">=3.14"
# ///
"""yokan dogfood #3: task — slow work off the UI thread.

Click "start slow work": a worker computes for ~1.5 s while the
spinner keeps spinning and the counter button stays clickable — the
UI thread never blocks. The result lands via on_done.
"""
import os
import sys
import time
from yokan import button, column, row, run, spinner, task, text

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))


state = {"busy": False, "result": "—", "n": 0}


def slow_work():
    time.sleep(1.5)
    return sum(i * i for i in range(200_000))


def start():
    if state["busy"]:
        return
    state.update(busy=True)
    task(slow_work, on_done=lambda v: state.update(busy=False, result=f"{v:,}"))


def view(s):
    if s["busy"]:
        status = row(spinner(size=16), text("working…", color="#8a8f98"), spacing=8)
    else:
        status = text(f"result: {s['result']}", size=18)
    return column(
        text("task — the UI thread never blocks", size=13, color="#8a8f98"),
        row(
            button("start slow work", on_click=start),
            button(f"+1 ({s['n']})", on_click=lambda: state.update(n=s["n"] + 1)),
            spacing=8,
        ),
        status,
        spacing=12,
        padding=16,
    )


if __name__ == "__main__":
    run(view, state=state, title="tasks")
