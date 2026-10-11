"""Every vector rulec wrote for `nationwide_freight`, through `nationwide_freight` and
`nationwide_freight_traced`: each input is called both ways, and what came out and the
rows that matched are printed, one line each.

Written by tools/rule_apps.py from the vectors beside it.
"""
from yokan import State, column, run, text

from nationwide_freight import Element, JPYInclTax, nationwide_freight, nationwide_freight_traced, Pick, Zone


cases: State[list[list[Element]]] = State([
    [],
    [Element(Zone.KINKI, JPYInclTax(0), JPYInclTax(0))],
    [Element(Zone.KINKI, JPYInclTax(1000), JPYInclTax(0))],
    [Element(Zone.KINKI, JPYInclTax(1001), JPYInclTax(0))],
    [Element(Zone.REMOTE, JPYInclTax(0), JPYInclTax(0))],
    [Element(Zone.REMOTE, JPYInclTax(1000), JPYInclTax(0))],
    [Element(Zone.REMOTE, JPYInclTax(1001), JPYInclTax(0))],
    [Element(Zone.REMOTE, JPYInclTax(1000000), JPYInclTax(100000)), Element(Zone.REMOTE, JPYInclTax(1000000), JPYInclTax(100000))],
    [Element(Zone.REMOTE, JPYInclTax(1000000), JPYInclTax(100000)), Element(Zone.KINKI, JPYInclTax(1000000), JPYInclTax(100000))],
    [Element(Zone.REMOTE, JPYInclTax(1000000), JPYInclTax(100000)), Element(Zone.REMOTE, JPYInclTax(1000), JPYInclTax(100000))],
    [Element(Zone.REMOTE, JPYInclTax(1000000), JPYInclTax(100000)), Element(Zone.KINKI, JPYInclTax(1000), JPYInclTax(100000))],
    [Element(Zone.KINKI, JPYInclTax(1000000), JPYInclTax(100000)), Element(Zone.REMOTE, JPYInclTax(1000000), JPYInclTax(100000))],
    [Element(Zone.KINKI, JPYInclTax(1000000), JPYInclTax(100000)), Element(Zone.KINKI, JPYInclTax(1000000), JPYInclTax(100000))],
    [Element(Zone.KINKI, JPYInclTax(1000000), JPYInclTax(100000)), Element(Zone.REMOTE, JPYInclTax(1000), JPYInclTax(100000))],
    [Element(Zone.KINKI, JPYInclTax(1000000), JPYInclTax(100000)), Element(Zone.KINKI, JPYInclTax(1000), JPYInclTax(100000))],
    [Element(Zone.REMOTE, JPYInclTax(1000), JPYInclTax(100000)), Element(Zone.REMOTE, JPYInclTax(1000000), JPYInclTax(100000))],
    [Element(Zone.REMOTE, JPYInclTax(1000), JPYInclTax(100000)), Element(Zone.KINKI, JPYInclTax(1000000), JPYInclTax(100000))],
    [Element(Zone.REMOTE, JPYInclTax(1000), JPYInclTax(100000)), Element(Zone.REMOTE, JPYInclTax(1000), JPYInclTax(100000))],
    [Element(Zone.REMOTE, JPYInclTax(1000), JPYInclTax(100000)), Element(Zone.KINKI, JPYInclTax(1000), JPYInclTax(100000))],
    [Element(Zone.KINKI, JPYInclTax(1000), JPYInclTax(100000)), Element(Zone.REMOTE, JPYInclTax(1000000), JPYInclTax(100000))],
    [Element(Zone.KINKI, JPYInclTax(1000), JPYInclTax(100000)), Element(Zone.KINKI, JPYInclTax(1000000), JPYInclTax(100000))],
    [Element(Zone.KINKI, JPYInclTax(1000), JPYInclTax(100000)), Element(Zone.REMOTE, JPYInclTax(1000), JPYInclTax(100000))],
    [Element(Zone.KINKI, JPYInclTax(500), JPYInclTax(800)), Element(Zone.KINKI, JPYInclTax(2000), JPYInclTax(1500))],
])


def check() -> None:
    for a0 in cases():
        out = nationwide_freight(a0)
        print(out)
        got, rows = nationwide_freight_traced(a0)
        print(got, len(rows))
        for r in rows:
            if r.label != "":
                print(r.table, r.row, r.label)
            else:
                print(r.table, r.row)


def view():
    with column():
        text("nationwide_freight: 23 vectors")


if __name__ == "__main__":
    run(view, on_start=check)
