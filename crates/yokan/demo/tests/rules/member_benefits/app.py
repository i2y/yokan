"""Every vector rulec wrote for `member_benefits`, through `member_benefits` and
`member_benefits_traced`: each input is called both ways, and what came out and the
rows that matched are printed, one line each.

Written by tools/rule_apps.py from the vectors beside it.
"""
from yokan import State, column, run, text

from member_benefits import Gram, JPYInclTax, member_benefits, member_benefits_traced, MemberClass, TierClass, WeightClass


cases: State[list[tuple[MemberClass, int, int, int]]] = State([
    (MemberClass.BASIC, 1, 0, 0),
    (MemberClass.BASIC, 2000, 0, 0),
    (MemberClass.BASIC, 2001, 0, 0),
    (MemberClass.BASIC, 10000, 0, 0),
    (MemberClass.BASIC, 10001, 0, 0),
    (MemberClass.PREMIUM, 1, 0, 0),
    (MemberClass.PREMIUM, 2001, 0, 0),
    (MemberClass.BASIC, 1, 2999, 0),
    (MemberClass.BASIC, 1, 3000, 0),
    (MemberClass.BASIC, 1, 5, 0),
    (MemberClass.BASIC, 1, 50, 0),
    (MemberClass.PREMIUM, 2, 0, 0),
    (MemberClass.PREMIUM, 1999, 0, 0),
    (MemberClass.PREMIUM, 2000, 0, 0),
    (MemberClass.PREMIUM, 9999, 0, 0),
    (MemberClass.PREMIUM, 10000, 0, 0),
    (MemberClass.PREMIUM, 10001, 0, 0),
    (MemberClass.PREMIUM, 19999, 0, 0),
    (MemberClass.PREMIUM, 20000, 0, 0),
    (MemberClass.PREMIUM, 1, 1, 0),
    (MemberClass.PREMIUM, 1, 999999, 0),
    (MemberClass.PREMIUM, 1, 1000000, 0),
    (MemberClass.PREMIUM, 1, 0, 1),
    (MemberClass.PREMIUM, 1, 0, 99999),
    (MemberClass.PREMIUM, 1, 0, 100000),
    (MemberClass.BASIC, 2, 1, 0),
    (MemberClass.BASIC, 2, 999999, 0),
    (MemberClass.BASIC, 2, 1000000, 0),
    (MemberClass.BASIC, 1999, 1, 0),
    (MemberClass.BASIC, 1999, 999999, 0),
    (MemberClass.BASIC, 1999, 1000000, 0),
    (MemberClass.BASIC, 2000, 1, 0),
    (MemberClass.BASIC, 2000, 999999, 0),
    (MemberClass.BASIC, 2000, 1000000, 0),
    (MemberClass.BASIC, 2001, 1, 0),
    (MemberClass.BASIC, 2001, 999999, 0),
    (MemberClass.BASIC, 2001, 1000000, 0),
    (MemberClass.BASIC, 9999, 1, 0),
    (MemberClass.BASIC, 9999, 999999, 0),
    (MemberClass.BASIC, 9999, 1000000, 0),
    (MemberClass.BASIC, 10000, 1, 0),
    (MemberClass.BASIC, 10000, 999999, 0),
    (MemberClass.BASIC, 10000, 1000000, 0),
    (MemberClass.BASIC, 10001, 1, 0),
    (MemberClass.BASIC, 10001, 999999, 0),
    (MemberClass.BASIC, 10001, 1000000, 0),
    (MemberClass.BASIC, 19999, 1, 0),
    (MemberClass.BASIC, 19999, 999999, 0),
    (MemberClass.BASIC, 19999, 1000000, 0),
    (MemberClass.BASIC, 20000, 1, 0),
    (MemberClass.BASIC, 20000, 999999, 0),
    (MemberClass.BASIC, 20000, 1000000, 0),
    (MemberClass.BASIC, 2, 0, 1),
    (MemberClass.BASIC, 2, 0, 99999),
    (MemberClass.BASIC, 2, 0, 100000),
    (MemberClass.BASIC, 1999, 0, 1),
    (MemberClass.BASIC, 1999, 0, 99999),
    (MemberClass.BASIC, 1999, 0, 100000),
    (MemberClass.BASIC, 2000, 0, 1),
    (MemberClass.BASIC, 2000, 0, 99999),
    (MemberClass.BASIC, 2000, 0, 100000),
    (MemberClass.BASIC, 2001, 0, 1),
    (MemberClass.BASIC, 2001, 0, 99999),
    (MemberClass.BASIC, 2001, 0, 100000),
    (MemberClass.BASIC, 9999, 0, 1),
    (MemberClass.BASIC, 9999, 0, 99999),
    (MemberClass.BASIC, 9999, 0, 100000),
    (MemberClass.BASIC, 10000, 0, 1),
    (MemberClass.BASIC, 10000, 0, 99999),
    (MemberClass.BASIC, 10000, 0, 100000),
    (MemberClass.BASIC, 10001, 0, 1),
    (MemberClass.BASIC, 10001, 0, 99999),
    (MemberClass.BASIC, 10001, 0, 100000),
    (MemberClass.BASIC, 19999, 0, 1),
    (MemberClass.BASIC, 19999, 0, 99999),
    (MemberClass.BASIC, 19999, 0, 100000),
    (MemberClass.BASIC, 20000, 0, 1),
    (MemberClass.BASIC, 20000, 0, 99999),
    (MemberClass.BASIC, 20000, 0, 100000),
    (MemberClass.BASIC, 1, 1, 1),
    (MemberClass.BASIC, 1, 1, 99999),
    (MemberClass.BASIC, 1, 1, 100000),
    (MemberClass.BASIC, 1, 999999, 1),
    (MemberClass.BASIC, 1, 999999, 99999),
    (MemberClass.BASIC, 1, 999999, 100000),
    (MemberClass.BASIC, 1, 1000000, 1),
    (MemberClass.BASIC, 1, 1000000, 99999),
    (MemberClass.BASIC, 1, 1000000, 100000),
    (MemberClass.BASIC, 1000, 2000, 500),
    (MemberClass.PREMIUM, 5000, 20000, 2000),
    (MemberClass.BASIC, 15000, 5000, 0),
])


def check() -> None:
    for a0, a1, a2, a3 in cases():
        out = member_benefits(a0, Gram(a1), JPYInclTax(a2), JPYInclTax(a3))
        print(out.total, out.pts)
        got, rows = member_benefits_traced(a0, Gram(a1), JPYInclTax(a2), JPYInclTax(a3))
        print(got.total, got.pts, len(rows))
        for r in rows:
            if r.label != "":
                print(r.table, r.row, r.label)
            else:
                print(r.table, r.row)


def view():
    with column():
        text("member_benefits: 91 vectors")


if __name__ == "__main__":
    run(view, on_start=check)
