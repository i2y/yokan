# /// script
# requires-python = ">=3.14"
# dependencies = ["numpy"]
# ///
"""numpy inside the native app: the escape imports numpy, and
--bundle installs it (from this file's own PEP 723 block) into the
shipped runtime's site-packages.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import bar_chart, button, column, py, run, State, text  # noqa: E402


@py
def stats(xs: list[float]) -> list[float]:
    import numpy as np

    a = np.array(xs)
    return [float(a.mean()), float(a.std())]


values: State[list[float]] = State([3.0, 5.0, 2.0, 8.0])
mean: State[float] = State(0.0)
std: State[float] = State(0.0)


def compute():
    r = stats(values())
    mean.set(r[0])
    std.set(r[1])


def view():
    with column(spacing=10, padding=14):
        bar_chart(values(), height=100.0)
        button("stats (numpy)", on_click=compute)
        text(f"mean {mean():.2f} · std {std():.2f}", size=16)


if __name__ == "__main__":
    run(view, title="pystats")
