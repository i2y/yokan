# /// script
# requires-python = ">=3.14"
# dependencies = ["numpy"]
# ///
"""yokan dogfood #1: 100,000 rows at native scroll speed.

Type in the filter box; rows render through a virtualized ListView,
so Python builds only the visible window (~14 rows of 100k). Run
with PIXIE_TRACE_LAZY=1 to watch the requested ranges.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

import numpy as np  # noqa: E402
import yokan as ui  # noqa: E402

N = 100_000
rng = np.random.default_rng(7)
CATS = ["alpha", "beta", "gamma", "delta", "epsilon"]
STEMS = ["kuro", "shiro", "aka", "ao", "momo", "yuki", "hana", "sora"]
TAILS = ["maru", "suke", "chan", "gou", "ta", "emon"]
names = [f"{STEMS[i % 8]}{TAILS[(i // 8) % 6]}-{i:06d}" for i in range(N)]
cats = [CATS[i % 5] for i in range(N)]
values = np.round(rng.normal(50.0, 20.0, N), 2)


def matches(q):
    if not q:
        return list(range(N))
    q = q.lower()
    return [i for i in range(N) if q in names[i] or q in cats[i]]


def view(s):
    idx = s["idx"]

    def render_row(k):
        i = idx[k]
        return ui.row(
            ui.text(f"{i:06d}", size=12, color="#8a8f98"),
            ui.text(names[i], grow=1.0),
            ui.text(cats[i], size=12, color="#7aa2f7"),
            ui.text(f"{values[i]:.2f}", align="right"),
            spacing=12,
        )

    return ui.column(
        ui.text("csv viewer — 100k rows, virtualized", size=13, color="#8a8f98"),
        ui.text_field(s["q"], placeholder="filter…", on_change=lambda t: s.update(q=t, idx=matches(t))),
        ui.text(f"{len(idx):,} / {N:,} rows match", size=12),
        ui.list_view(len(idx), render_row, item_height=26.0, height=430.0),
        spacing=10,
        padding=14,
    )


if __name__ == "__main__":
    ui.run(view, state={"q": "", "idx": list(range(N))}, title="csv viewer")
