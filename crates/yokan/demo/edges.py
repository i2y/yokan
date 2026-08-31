# /// script
# requires-python = ">=3.14"
# ///
"""Containment, proven by the gate: every predictable failure an
admitted program can reach fails the SAME way interpreted and
compiled — the statement aborts before writing, earlier statements'
effects stay, the app keeps running.

  oob     — a local subscript past the end
  grow    — i64 overflow (the write is refused before it happens,
            in both runs)
  partial — first statement lands, second fails: ordering agrees
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import button, column, run, State, text  # noqa: E402

xs: State[list[int]] = State([7])
picked: State[int] = State(0)
big: State[int] = State(4611686018427387904)
steps: State[int] = State(0)


def oob():
    r = xs()
    picked.set(r[5])


def grow():
    big.set(big() * 4)


def partial():
    steps.set(steps() + 1)
    r = xs()
    picked.set(r[9])


def view():
    with column(spacing=8, padding=12):
        text(f"picked={picked()} steps={steps()}")
        button("oob", on_click=oob)
        button("grow", on_click=grow)
        button("partial", on_click=partial)


if __name__ == "__main__":
    run(view, title="edges")
