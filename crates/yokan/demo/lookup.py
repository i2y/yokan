# /// script
# requires-python = ">=3.14"
# ///
"""Dict cells. The order question is DECIDED: iteration stays out
(Python orders by insertion, native maps by key — admitting either
would lie), and everything order-free is in: per-key writes land in
place in both tiers (`prices["cherry"] = 200` is pixie's
`m[k] = v`), reads are total via .get(key, default), membership
guards conditions, len counts.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import button, column, row, run, State, text  # noqa: E402

prices: State[dict[str, int]] = State({"apple": 120, "banana": 80})
picked: State[int] = State(0)
label: State[str] = State("none")


def pick_apple():
    picked.set(prices().get("apple", -1))
    if "cherry" in prices():
        label.set("cherry known")
    else:
        label.set("no cherry")


def add_cherry():
    prices["cherry"] = 200
    picked.set(prices().get("cherry", -1))
    if "cherry" in prices():
        label.set("cherry known")


def miss():
    picked.set(prices().get("durian", -7))


def view():
    with column(spacing=8, padding=12):
        text(f"picked={picked()} n={len(prices())} {label()}")
        text(f"apple costs {prices().get('apple', -1)} right now", size=12)
        with row(spacing=6):
            button("apple", on_click=pick_apple)
            button("cherry", on_click=add_cherry)
            button("miss", on_click=miss)


if __name__ == "__main__":
    run(view, title="lookup")
