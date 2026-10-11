"""Every vector rulec wrote for `二つの区分`, through `two_bands` and
`two_bands_traced`: each input is called both ways, and what came out and the
rows that matched are printed, one line each.

Written by tools/rule_apps.py from the vectors beside it.
"""
from yokan import State, column, run, text

from two_bands import Handling, Kilogram, ShipBand, two_bands, two_bands_traced


cases: State[list[int]] = State([
    0,
    2,
    3,
    10,
    11,
    30,
])


def check() -> None:
    for a0 in cases():
        out = two_bands(Kilogram(a0))
        print(out.fee, out.charge)
        got, rows = two_bands_traced(Kilogram(a0))
        print(got.fee, got.charge, len(rows))
        for r in rows:
            if r.label != "":
                print(r.table, r.row, r.label)
            else:
                print(r.table, r.row)


def view():
    with column():
        text("two_bands: 6 vectors")


if __name__ == "__main__":
    run(view, on_start=check)
