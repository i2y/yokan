"""Every vector rulec wrote for `retirement_pay`, through `retirement_pay` and
`retirement_pay_traced`: each input is called both ways, and what came out and the
rows that matched are printed, one line each.

Written by tools/rule_apps.py from the vectors beside it.
"""
from yokan import State, column, run, text

from retirement_pay import JPY, ReasonKind, retirement_pay, retirement_pay_traced


cases: State[list[tuple[int, ReasonKind, int]]] = State([
    (1, ReasonKind.RETIREMENTAGE, 100000),
    (9, ReasonKind.RETIREMENTAGE, 100000),
    (10, ReasonKind.RETIREMENTAGE, 100000),
    (24, ReasonKind.RETIREMENTAGE, 100000),
    (25, ReasonKind.RETIREMENTAGE, 100000),
    (1, ReasonKind.DEATH, 100000),
    (1, ReasonKind.VOLUNTARY, 100000),
    (39, ReasonKind.RETIREMENTAGE, 100000),
    (40, ReasonKind.VOLUNTARY, 100000),
    (2, ReasonKind.VOLUNTARY, 100000),
    (2, ReasonKind.DEATH, 100000),
    (9, ReasonKind.VOLUNTARY, 100000),
    (9, ReasonKind.DEATH, 100000),
    (10, ReasonKind.VOLUNTARY, 100000),
    (10, ReasonKind.DEATH, 100000),
    (11, ReasonKind.VOLUNTARY, 100000),
    (11, ReasonKind.DEATH, 100000),
    (24, ReasonKind.VOLUNTARY, 100000),
    (24, ReasonKind.DEATH, 100000),
    (25, ReasonKind.VOLUNTARY, 100000),
    (25, ReasonKind.DEATH, 100000),
    (26, ReasonKind.VOLUNTARY, 100000),
    (26, ReasonKind.DEATH, 100000),
    (39, ReasonKind.VOLUNTARY, 100000),
    (39, ReasonKind.DEATH, 100000),
    (40, ReasonKind.DEATH, 100000),
    (2, ReasonKind.RETIREMENTAGE, 100001),
    (2, ReasonKind.RETIREMENTAGE, 999999),
    (2, ReasonKind.RETIREMENTAGE, 1000000),
    (9, ReasonKind.RETIREMENTAGE, 100001),
    (9, ReasonKind.RETIREMENTAGE, 999999),
    (9, ReasonKind.RETIREMENTAGE, 1000000),
    (10, ReasonKind.RETIREMENTAGE, 100001),
    (10, ReasonKind.RETIREMENTAGE, 999999),
    (10, ReasonKind.RETIREMENTAGE, 1000000),
    (11, ReasonKind.RETIREMENTAGE, 100001),
    (11, ReasonKind.RETIREMENTAGE, 999999),
    (11, ReasonKind.RETIREMENTAGE, 1000000),
    (24, ReasonKind.RETIREMENTAGE, 100001),
    (24, ReasonKind.RETIREMENTAGE, 999999),
    (24, ReasonKind.RETIREMENTAGE, 1000000),
    (25, ReasonKind.RETIREMENTAGE, 100001),
    (25, ReasonKind.RETIREMENTAGE, 999999),
    (25, ReasonKind.RETIREMENTAGE, 1000000),
    (26, ReasonKind.RETIREMENTAGE, 100001),
    (26, ReasonKind.RETIREMENTAGE, 999999),
    (26, ReasonKind.RETIREMENTAGE, 1000000),
    (39, ReasonKind.RETIREMENTAGE, 100001),
    (39, ReasonKind.RETIREMENTAGE, 999999),
    (39, ReasonKind.RETIREMENTAGE, 1000000),
    (40, ReasonKind.RETIREMENTAGE, 100001),
    (40, ReasonKind.RETIREMENTAGE, 999999),
    (40, ReasonKind.RETIREMENTAGE, 1000000),
    (1, ReasonKind.VOLUNTARY, 100001),
    (1, ReasonKind.VOLUNTARY, 999999),
    (1, ReasonKind.VOLUNTARY, 1000000),
    (1, ReasonKind.DEATH, 100001),
    (1, ReasonKind.DEATH, 999999),
    (1, ReasonKind.DEATH, 1000000),
    (3, ReasonKind.RETIREMENTAGE, 300000),
    (3, ReasonKind.VOLUNTARY, 300000),
    (30, ReasonKind.DEATH, 300000),
])


def check() -> None:
    for a0, a1, a2 in cases():
        out = retirement_pay(a0, a1, JPY(a2))
        print(out)
        got, rows = retirement_pay_traced(a0, a1, JPY(a2))
        print(got, len(rows))
        for r in rows:
            if r.label != "":
                print(r.table, r.row, r.label)
            else:
                print(r.table, r.row)


def view():
    with column():
        text("retirement_pay: 62 vectors")


if __name__ == "__main__":
    run(view, on_start=check)
