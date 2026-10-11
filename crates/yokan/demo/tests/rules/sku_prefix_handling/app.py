"""Every vector rulec wrote for `sku_prefix_handling`, through `sku_prefix_handling` and
`sku_prefix_handling_traced`: each input is called both ways, and what came out and the
rows that matched are printed, one line each.

Written by tools/rule_apps.py from the vectors beside it.
"""
from yokan import State, column, run, text

from sku_prefix_handling import HandlingKind, sku_prefix_handling, sku_prefix_handling_traced


cases: State[list[tuple[str, int]]] = State([
    ("FZ-", 1),
    ("CH-", 1),
    ("FZ-", 20),
    ("FZ-", 19),
    ("CH-", 20),
    ("FZ-0001", 1),
    ("CH-0001", 1),
    ("CL-", 1),
    ("CL-0001", 1),
    ("z", 1),
    ("FZ-", 2),
    ("FZ-", 21),
    ("FZ-", 199),
    ("FZ-", 200),
    ("FZ-0001", 2),
    ("FZ-0001", 19),
    ("FZ-0001", 20),
    ("FZ-0001", 21),
    ("FZ-0001", 199),
    ("FZ-0001", 200),
    ("CH-", 2),
    ("CH-", 19),
    ("CH-", 21),
    ("CH-", 199),
    ("CH-", 200),
    ("CH-0001", 2),
    ("CH-0001", 19),
    ("CH-0001", 20),
    ("CH-0001", 21),
    ("CH-0001", 199),
    ("CH-0001", 200),
    ("CL-", 2),
    ("CL-", 19),
    ("CL-", 20),
    ("CL-", 21),
    ("CL-", 199),
    ("CL-", 200),
    ("CL-0001", 2),
    ("CL-0001", 19),
    ("CL-0001", 20),
    ("CL-0001", 21),
    ("CL-0001", 199),
    ("CL-0001", 200),
    ("z", 2),
    ("z", 19),
    ("z", 20),
    ("z", 21),
    ("z", 199),
    ("z", 200),
    ("FZ-1001", 1),
    ("CH-2002", 5),
    ("CL-0003", 5),
    ("AB-0004", 30),
    ("AB-0004", 3),
])


def check() -> None:
    for a0, a1 in cases():
        out = sku_prefix_handling(a0, a1)
        print(out.value)
        got, rows = sku_prefix_handling_traced(a0, a1)
        print(got.value, len(rows))
        for r in rows:
            if r.label != "":
                print(r.table, r.row, r.label)
            else:
                print(r.table, r.row)


def view():
    with column():
        text("sku_prefix_handling: 54 vectors")


if __name__ == "__main__":
    run(view, on_start=check)
