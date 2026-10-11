"""Every vector rulec wrote for `cart_shipping_fee`, through `cart_shipping_fee` and
`cart_shipping_fee_traced`: each input is called both ways, and what came out and the
rows that matched are printed, one line each.

Written by tools/rule_apps.py from the vectors beside it.
"""
from yokan import State, column, run, text

from cart_shipping_fee import cart_shipping_fee, cart_shipping_fee_traced, Element, JPY, MemberTier


cases: State[list[tuple[MemberTier, list[Element]]]] = State([
    (MemberTier.REGULAR, []),
    (MemberTier.REGULAR, [Element(JPY(99999))]),
    (MemberTier.REGULAR, [Element(JPY(3000))]),
    (MemberTier.REGULAR, [Element(JPY(2999))]),
    (MemberTier.REGULAR, [Element(JPY(5000))]),
    (MemberTier.REGULAR, [Element(JPY(4999))]),
    (MemberTier.PREMIUM, [Element(JPY(3000))]),
    (MemberTier.PREMIUM, [Element(JPY(2999))]),
    (MemberTier.PREMIUM, [Element(JPY(4999))]),
    (MemberTier.PREMIUM, [Element(JPY(5000))]),
    (MemberTier.REGULAR, [Element(JPY(3000)), Element(JPY(2500))]),
    (MemberTier.PREMIUM, [Element(JPY(3000)), Element(JPY(2500))]),
    (MemberTier.REGULAR, [Element(JPY(1200))]),
    (MemberTier.REGULAR, [Element(JPY(2000)), Element(JPY(1500))]),
    (MemberTier.PREMIUM, [Element(JPY(2000)), Element(JPY(1500))]),
])


def check() -> None:
    for a0, a1 in cases():
        out = cart_shipping_fee(a0, a1)
        print(out)
        got, rows = cart_shipping_fee_traced(a0, a1)
        print(got, len(rows))
        for r in rows:
            if r.label != "":
                print(r.table, r.row, r.label)
            else:
                print(r.table, r.row)


def view():
    with column():
        text("cart_shipping_fee: 15 vectors")


if __name__ == "__main__":
    run(view, on_start=check)
