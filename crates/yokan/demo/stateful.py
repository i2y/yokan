# /// script
# requires-python = ">=3.14"
# ///
"""Per-instance state: @component + local. Each call site owns
its own `n`; identity is positional (the no-key rule), and the state
survives rebuilds and reloads.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import button, column, component, local, row, run, State, text  # noqa: E402


@component
def counter(label: str, step: int):
    n: State[int] = local(0)
    with row(spacing=6):
        text(f"{label}: {n()}")
        button(f"+{step}", on_click=lambda: n.set(n() + step))


def view():
    with column(spacing=10, padding=14):
        text("two counters, one component, separate state", size=13, color="#8a8f98")
        counter("a", 1)
        counter("b", 10)


if __name__ == "__main__":
    run(view, title="stateful")
