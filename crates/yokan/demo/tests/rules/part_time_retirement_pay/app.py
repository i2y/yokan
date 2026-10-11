"""Every vector rulec wrote for `part_time_retirement_pay`, through `part_time_retirement_pay` and
`part_time_retirement_pay_traced`: each input is called both ways, and what came out and the
rows that matched are printed, one line each.

Written by tools/rule_apps.py from the vectors beside it.
"""
from yokan import State, column, run, text

from part_time_retirement_pay import EndKind, JPY, part_time_retirement_pay, part_time_retirement_pay_traced


cases: State[list[tuple[int, EndKind, int]]] = State([
    (1, EndKind.TERMEND, 100000),
    (2, EndKind.TERMEND, 100000),
    (3, EndKind.TERMEND, 100000),
    (1, EndKind.TERMEND, 100001),
    (1, EndKind.TERMEND, 499999),
    (1, EndKind.TERMEND, 500000),
    (2, EndKind.TERMEND, 100001),
    (2, EndKind.TERMEND, 499999),
    (2, EndKind.TERMEND, 500000),
    (3, EndKind.TERMEND, 100001),
    (3, EndKind.TERMEND, 499999),
    (3, EndKind.TERMEND, 500000),
    (3, EndKind.TERMEND, 300000),
    (3, EndKind.RESIGNATION, 300000),
])


def check() -> None:
    for a0, a1, a2 in cases():
        out = part_time_retirement_pay(a0, a1, JPY(a2))
        print(out)
        got, rows = part_time_retirement_pay_traced(a0, a1, JPY(a2))
        print(got, len(rows))
        for r in rows:
            if r.label != "":
                print(r.table, r.row, r.label)
            else:
                print(r.table, r.row)


def view():
    with column():
        text("part_time_retirement_pay: 14 vectors")


if __name__ == "__main__":
    run(view, on_start=check)
