# /// script
# requires-python = ">=3.14"
# ///
"""A long Python job, off the UI thread, saying where it has got to.

The work is a `@py` escape — real Python, run on an embedded CPython
inside the compiled binary — and `task` puts it on a worker in both
runs, so the window keeps drawing while it grinds: the counter button
stays clickable throughout.

From inside the escape, `report(fraction, note)` reaches this app's
`on_progress`, which runs on the UI thread like any other handler.
Every report is heard, and the last one lands before `on_done` does —
which is what makes `4 reports` in the dump a checked claim rather
than a hope.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import (  # noqa: E402
    State,
    button,
    column,
    progress,
    py,
    row,
    run,
    task,
    text,
)


@py
def count_primes(limit: int, chunks: int) -> int:
    from yokan import report

    found = 0
    step = limit // chunks
    for c in range(chunks):
        lo = c * step + 2
        hi = lo + step
        found += sum(
            1
            for n in range(lo, hi)
            if all(n % d for d in range(2, int(n**0.5) + 1))
        )
        report((c + 1) / chunks, f"below {hi}")
    return found


limit: State[int] = State(20_000)
found: State[int] = State(0)
done: State[float] = State(0.0)
step: State[str] = State("idle")
heard: State[int] = State(0)
n: State[int] = State(0)


def counted(total: int):
    found.set(total)


def moved(fraction: float, note: str):
    done.set(fraction)
    step.set(note)
    heard.set(heard() + 1)


def start():
    n_max = limit()
    task(lambda: count_primes(n_max, 4), on_done=counted, on_progress=moved)


def view():
    with column(spacing=12, padding=16):
        text("a Python job on a worker — the window keeps drawing", size=13, color="#8a8f98")
        with row(spacing=8):
            button("count", on_click=start)
            button(f"+1 ({n()})", on_click=lambda: n.set(n() + 1))
        progress(done(), width=260.0, label=step())
        text(f"{found()} primes below {limit()} · {heard()} reports", size=16)


if __name__ == "__main__":
    run(view, title="pyjob")
