# /// script
# requires-python = ">=3.14"
# dependencies = ["numpy"]
# ///
"""numpy inside the native app: the escape imports numpy, and
--bundle installs it (from this file's own PEP 723 block) into the
shipped runtime's site-packages.

The escape runs inside a `task`, which is what keeps a long Python
call off the UI thread in the compiled app as well as in the
development run. From in there it can say where it has got to:
`report(fraction, note)` inside the escape reaches this app's
`on_progress`, on whatever thread the Python happened to run on.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import bar_chart, button, column, py, progress, run, State, task, text  # noqa: E402


@py
def stats(xs: list[float]) -> list[float]:
    from yokan import report

    import numpy as np

    report(0.5, "mean")
    a = np.array(xs)
    m = float(a.mean())
    report(1.0, "spread")
    return [m, float(a.std())]


values: State[list[float]] = State([3.0, 5.0, 2.0, 8.0])
mean: State[float] = State(0.0)
std: State[float] = State(0.0)
done: State[float] = State(0.0)
step: State[str] = State("idle")


def landed(r: list[float]):
    mean.set(r[0])
    std.set(r[1])


def moved(fraction: float, note: str):
    done.set(fraction)
    step.set(note)


def compute():
    xs = values()
    task(lambda: stats(xs), on_done=landed, on_progress=moved)


def view():
    with column(spacing=10, padding=14):
        bar_chart(values(), height=100.0)
        button("stats (numpy)", on_click=compute)
        progress(done(), width=220.0, label=step())
        text(f"mean {mean():.2f} · std {std():.2f}", size=16)


if __name__ == "__main__":
    run(view, title="pystats")
