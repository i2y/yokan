# /// script
# requires-python = ">=3.14"
#
# [tool.yokan.crates]
# deunicode = "1"
# hexfmt = { path = "native/hexfmt" }
# ///
"""Rust crates, declared and called — one by path, one by
crates.io version (added with `yokan add`). The `[tool.yokan.crates]` block
names it; `crates.hexfmt.…` calls it — through an auto-built pyo3
door while developing, through the derived binding in the release
build. One implementation, both runs, and the gate compares them.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from enum import Enum

import yokan as ui  # noqa: E402
from yokan import crates, store, value  # noqa: E402


@value
class Span:
    lo: int
    hi: int


class Grade(Enum):
    Fine = 1
    Odd = 2


@value
class Packed:
    id: int
    weight: int


@value
class Framed:
    span: Span
    packed: Packed


@store
class Out:
    samples: list[float] = [1.0, 2.0, 6.0]
    encoded: str = "-"
    romaji: str = "-"
    total: int = 0
    mean: float = 0.0
    half: int | None = None
    span_lo: int = 0
    span_hi: int = 0
    span_w: int = 0
    verdict: str = "-"
    pack_id: int = 0
    heavy: bool = False
    nums: list[int] = []
    parse_msg: str = "-"

    hello: str = "-"
    even_msg: str = "-"
    csum: int = 0
    o_count: int = 0
    fr_sum: int = 0
    fr_id: int = 0

    def run(self) -> None:
        self.encoded = crates.hexfmt.encode("yokan")
        self.total = crates.hexfmt.add(40, 2)
        self.mean = crates.hexfmt.avg(self.samples)
        self.romaji = crates.deunicode.deunicode("ようかん")
        self.half = crates.hexfmt.halve(10)
        self.hello = crates.hexfmt.greet(None)
        moved = crates.hexfmt.shift(Span(3, 8), 10)
        self.span_lo = moved.lo
        self.span_hi = moved.hi
        self.span_w = crates.hexfmt.width(moved)
        g = crates.hexfmt.judge(7)
        self.verdict = crates.hexfmt.describe(g)
        p = crates.hexfmt.pack(9, 1200)
        self.pack_id = p.id
        self.heavy = crates.hexfmt.heavier(p, 1000)
        counts = crates.hexfmt.char_counts("yokan yokan")
        self.csum = crates.hexfmt.total_counts(counts)
        self.o_count = counts.get("o", 0)
        fr = crates.hexfmt.frame(Span(3, 8), Packed(7, 500))
        self.fr_sum = crates.hexfmt.frame_sum(fr)
        self.fr_id = fr.packed.id

    def check(self) -> None:
        try:
            self.total = crates.hexfmt.parse_even("41")
        except Exception as e:
            self.even_msg = f"{e}"
        try:
            self.nums = crates.hexfmt.parse_all("4, 5, six")
        except Exception as e:
            self.parse_msg = f"{e}"
        try:
            self.nums = crates.hexfmt.parse_all("4, 5, 6")
        except Exception as e:
            self.parse_msg = f"{e}"


def view():
    with ui.column(spacing=8, padding=12):
        ui.text(f"encoded: {Out.encoded}")
        ui.text(f"romaji: {Out.romaji}")
        ui.text(f"total: {Out.total}")
        ui.text(f"mean: {Out.mean:.2f}")
        if (h := Out.half) is not None:
            ui.text(f"half: {h}  {Out.hello}")
        else:
            ui.text(f"half: (none)  {Out.hello}")
        ui.text(f"even: {Out.even_msg}")
        ui.text(f"span: {Out.span_lo}..{Out.span_hi} w={Out.span_w}")
        ui.text(f"judge(7): {Out.verdict}")
        ui.text(f"packed: id={Out.pack_id} heavy={Out.heavy}")
        ui.text(f"nums: {len(Out.nums)} parse: {Out.parse_msg}")
        ui.text(f"counts: sum={Out.csum} o={Out.o_count}")
        ui.text(f"framed: sum={Out.fr_sum} id={Out.fr_id}")
        with ui.row(spacing=6):
            ui.button("run", on_click=Out.run)
            ui.button("check", on_click=Out.check)


if __name__ == "__main__":
    ui.run(view, title="rustcrate", on_start=Out.run)
