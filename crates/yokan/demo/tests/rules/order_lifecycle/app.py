"""Every vector rulec wrote for `order_lifecycle`, through `order_lifecycle` and
`order_lifecycle_traced`: each input is called both ways, and what came out and the
rows that matched are printed, one line each.

Written by tools/rule_apps.py from the vectors beside it.
"""
from yokan import State, column, run, text

from order_lifecycle import INITIAL, is_final, JPYInclTax, order_lifecycle, order_lifecycle_traced, OrderEvent, OrderState


cases: State[list[tuple[OrderState, OrderEvent, int]]] = State([
    (OrderState.RECEIVED, OrderEvent.PAY, 0),
    (OrderState.RECEIVED, OrderEvent.CANCEL, 0),
    (OrderState.RECEIVED, OrderEvent.SHIP, 0),
    (OrderState.PAID, OrderEvent.SHIP, 0),
    (OrderState.PAID, OrderEvent.CANCEL, 0),
    (OrderState.PAID, OrderEvent.PAY, 0),
    (OrderState.SHIPPED, OrderEvent.DELIVER, 0),
    (OrderState.SHIPPED, OrderEvent.PAY, 0),
    (OrderState.DELIVERED, OrderEvent.PAY, 0),
    (OrderState.CANCELLED, OrderEvent.PAY, 0),
    (OrderState.RECEIVED, OrderEvent.PAY, 999999),
    (OrderState.RECEIVED, OrderEvent.PAY, 1000000),
    (OrderState.PAID, OrderEvent.CANCEL, 1000000),
    (OrderState.PAID, OrderEvent.DELIVER, 0),
    (OrderState.SHIPPED, OrderEvent.SHIP, 0),
    (OrderState.DELIVERED, OrderEvent.SHIP, 0),
    (OrderState.DELIVERED, OrderEvent.DELIVER, 0),
    (OrderState.DELIVERED, OrderEvent.CANCEL, 0),
    (OrderState.CANCELLED, OrderEvent.SHIP, 0),
    (OrderState.CANCELLED, OrderEvent.DELIVER, 0),
    (OrderState.CANCELLED, OrderEvent.CANCEL, 0),
    (OrderState.PAID, OrderEvent.PAY, 1),
    (OrderState.PAID, OrderEvent.PAY, 999999),
    (OrderState.SHIPPED, OrderEvent.PAY, 1),
    (OrderState.SHIPPED, OrderEvent.PAY, 999999),
    (OrderState.SHIPPED, OrderEvent.PAY, 1000000),
    (OrderState.DELIVERED, OrderEvent.PAY, 1),
    (OrderState.DELIVERED, OrderEvent.PAY, 999999),
    (OrderState.DELIVERED, OrderEvent.PAY, 1000000),
    (OrderState.CANCELLED, OrderEvent.PAY, 1),
    (OrderState.CANCELLED, OrderEvent.PAY, 999999),
    (OrderState.CANCELLED, OrderEvent.PAY, 1000000),
    (OrderState.RECEIVED, OrderEvent.SHIP, 1),
    (OrderState.RECEIVED, OrderEvent.SHIP, 999999),
    (OrderState.RECEIVED, OrderEvent.SHIP, 1000000),
    (OrderState.RECEIVED, OrderEvent.DELIVER, 1),
    (OrderState.RECEIVED, OrderEvent.DELIVER, 999999),
    (OrderState.RECEIVED, OrderEvent.DELIVER, 1000000),
    (OrderState.RECEIVED, OrderEvent.CANCEL, 1),
    (OrderState.RECEIVED, OrderEvent.CANCEL, 999999),
    (OrderState.SHIPPED, OrderEvent.CANCEL, 3000),
])

# A state machine's call sequences: whether a call starts one, the state it
# starts in (a placeholder where it carries on), then the other inputs.
steps: State[list[tuple[bool, OrderState, OrderEvent, int]]] = State([
    (True, OrderState.RECEIVED, OrderEvent.SHIP, 0),
    (False, OrderState.RECEIVED, OrderEvent.PAY, 0),
    (True, OrderState.RECEIVED, OrderEvent.SHIP, 0),
    (False, OrderState.RECEIVED, OrderEvent.SHIP, 0),
    (True, OrderState.RECEIVED, OrderEvent.SHIP, 0),
    (False, OrderState.RECEIVED, OrderEvent.CANCEL, 0),
    (True, OrderState.RECEIVED, OrderEvent.PAY, 0),
    (False, OrderState.RECEIVED, OrderEvent.PAY, 0),
    (False, OrderState.RECEIVED, OrderEvent.PAY, 0),
    (True, OrderState.RECEIVED, OrderEvent.PAY, 0),
    (False, OrderState.RECEIVED, OrderEvent.PAY, 0),
    (False, OrderState.RECEIVED, OrderEvent.SHIP, 0),
    (True, OrderState.RECEIVED, OrderEvent.PAY, 0),
    (False, OrderState.RECEIVED, OrderEvent.PAY, 0),
    (False, OrderState.RECEIVED, OrderEvent.CANCEL, 0),
    (True, OrderState.RECEIVED, OrderEvent.PAY, 0),
    (False, OrderState.RECEIVED, OrderEvent.CANCEL, 0),
    (False, OrderState.RECEIVED, OrderEvent.PAY, 0),
    (True, OrderState.RECEIVED, OrderEvent.PAY, 0),
    (False, OrderState.RECEIVED, OrderEvent.SHIP, 0),
    (False, OrderState.RECEIVED, OrderEvent.PAY, 0),
    (False, OrderState.RECEIVED, OrderEvent.PAY, 0),
    (True, OrderState.RECEIVED, OrderEvent.PAY, 0),
    (False, OrderState.RECEIVED, OrderEvent.SHIP, 0),
    (False, OrderState.RECEIVED, OrderEvent.PAY, 0),
    (False, OrderState.RECEIVED, OrderEvent.DELIVER, 0),
    (True, OrderState.RECEIVED, OrderEvent.PAY, 0),
    (False, OrderState.RECEIVED, OrderEvent.SHIP, 0),
    (False, OrderState.RECEIVED, OrderEvent.DELIVER, 0),
    (False, OrderState.RECEIVED, OrderEvent.PAY, 0),
    (False, OrderState.RECEIVED, OrderEvent.PAY, 0),
    (True, OrderState.RECEIVED, OrderEvent.CANCEL, 0),
    (False, OrderState.RECEIVED, OrderEvent.PAY, 0),
    (False, OrderState.RECEIVED, OrderEvent.PAY, 0),
    (True, OrderState.RECEIVED, OrderEvent.PAY, 3000),
    (False, OrderState.RECEIVED, OrderEvent.SHIP, 3000),
    (False, OrderState.RECEIVED, OrderEvent.DELIVER, 3000),
    (True, OrderState.RECEIVED, OrderEvent.PAY, 3000),
    (False, OrderState.RECEIVED, OrderEvent.CANCEL, 3000),
    (False, OrderState.RECEIVED, OrderEvent.PAY, 3000),
])


def check() -> None:
    for a0, a1, a2 in cases():
        out = order_lifecycle(a0, a1, JPYInclTax(a2))
        print(out.next_state.value, out.refund, out.accepted)
        got, rows = order_lifecycle_traced(a0, a1, JPYInclTax(a2))
        print(got.next_state.value, got.refund, got.accepted, len(rows))
        for r in rows:
            if r.label != "":
                print(r.table, r.row, r.label)
            else:
                print(r.table, r.row)
    print(INITIAL.value)
    for s in OrderState:
        if is_final(s):
            print(s.value)
    st = INITIAL
    for start, s0, b0, b1 in steps():
        if start:
            st = s0
        out = order_lifecycle(st, b0, JPYInclTax(b1))
        print(out.next_state.value, out.refund, out.accepted)
        got, rows = order_lifecycle_traced(st, b0, JPYInclTax(b1))
        print(got.next_state.value, got.refund, got.accepted, len(rows))
        for r in rows:
            if r.label != "":
                print(r.table, r.row, r.label)
            else:
                print(r.table, r.row)
        st = out.next_state


def view():
    with column():
        text("order_lifecycle: 41 vectors")


if __name__ == "__main__":
    run(view, on_start=check)
