#!/usr/bin/env python3
"""Print what CPython answers, so the compiled run's twins can be held
to it.

    uv run tools/gen_expected.py           # every table
    uv run tools/gen_expected.py math      # one module
    uv run tools/gen_expected.py --check   # fail if a table is stale

A table lands in `crates/yokan-stdlib/tests/expected/<module>.txt` and
`crates/yokan-stdlib/tests/expected.rs` reads it. The interpreted run
IS CPython, so nothing here tests it; what the table does is hold the
compiled run's twin to the same answers, which is the half of the
promise the gate cannot see (the gate proves the two runs AGREE — a
twin that is wrong the same way in both would still pass).

A table is only true of the CPython that printed it: the version is
the first line, and moving Python means regenerating and reading the
diff. Doubles are written as their 16 hex digits, so nothing is lost
to a decimal round-trip.

Rows read `name arg… -> result`, values tagged by type:

    i:-12   an int          s:hello        a str (percent-escaped)
    f:hex   a double        !Name:message  the exception CPython raised
    b:0     a bool

`~>` in place of `->` means "within one ulp": the answer comes from
the platform's libm rather than from IEEE-754, so CPython and the
twin agree exactly on this machine but a table is read on others.
"""

import math
import os
import struct
import sys
import urllib.parse

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "..", "yokan-stdlib", "tests", "expected")

# The answers IEEE-754 pins down, and the ones a C library decides.
# `sqrt` is exactly rounded by the standard; `sin` is whatever libm
# does, and CPython and Rust only agree because they call the same
# one. Anything listed here is compared within an ulp instead.
LIBM = {"sin", "cos", "tan", "exp", "log", "log2", "log10", "atan", "asin", "acos", "pow"}

# Boundary values first, then a few ordinary ones. A double that
# survives a decimal round-trip is not the interesting case.
#
# What is deliberately absent: inputs whose CPython answer is an int
# too wide for 64 bits (`floor(1e300)`). The dialect's int is 64 bits
# and the twin stops the statement there, which is a decided
# constraint rather than a disagreement with CPython, so no row here
# can state it.
FLOATS = (
    0.0, -0.0, 1.0, -1.0, 0.5, -0.5, 2.0, 2.5, -2.75,
    0.1, 0.3, 1e-5, 1e-300, 5e-324, 1.5e300, 1e16, 9007199254740992.0,
    3.141592653589793, 2.718281828459045, 123456.789, -123456.789,
    1.7976931348623157e308,
)
NONNEG = tuple(x for x in FLOATS if not (x < 0))


def cases_math():
    """`math`, as far as the standard library reaches today. Phase by
    phase this grows; the row format does not."""
    yield from (("sqrt", (x,)) for x in NONNEG)
    yield ("sqrt", (-1.0,))          # ValueError, and the table says so
    yield from (("sin", (x,)) for x in FLOATS)
    yield from (("cos", (x,)) for x in FLOATS)
    yield from (("fabs", (x,)) for x in FLOATS)
    # floor and ceil answer an int, which is 64 bits wide here, so the
    # inputs stay inside that range on purpose.
    small = (0.0, -0.0, 1.0, -1.0, 0.5, -0.5, 2.5, -2.75, 0.1, -0.1, 1e16, -1e16,
             math.inf, -math.inf, math.nan)
    yield from (("floor", (x,)) for x in small)
    yield from (("ceil", (x,)) for x in small)
    yield from (("pow", (a, b)) for a in (0.0, 1.0, 2.0, -2.0, 0.5, 10.0)
                for b in (0.0, 1.0, 2.0, -1.0, 0.5, 3.0, -0.5))
    # The range error, and the infinities pow answers rather than
    # refuses.
    yield from (("pow", args) for args in (
        (10.0, 400.0), (10.0, -400.0), (math.inf, 2.0), (-math.inf, 3.0),
        (math.inf, -1.0), (2.0, math.inf), (0.5, math.inf), (math.nan, 0.0),
        (1.0, math.nan), (math.nan, 2.0), (-1.0, math.inf),
    ))
    yield ("pi", ())


MODULES = {"math": (math, cases_math)}


def enc(v) -> str:
    if isinstance(v, BaseException):
        return f"!{type(v).__name__}:{urllib.parse.quote(str(v), safe='')}"
    if isinstance(v, bool):
        return "b:1" if v else "b:0"
    if isinstance(v, int):
        return f"i:{v}"
    if isinstance(v, float):
        return "f:%016x" % struct.unpack("<Q", struct.pack("<d", v))[0]
    if isinstance(v, str):
        return "s:" + urllib.parse.quote(v, safe="")
    raise SystemExit(f"no encoding for {v!r} ({type(v).__name__})")


def render(name: str) -> str:
    mod, cases = MODULES[name]
    out = [
        f"# CPython {sys.version.split()[0]} — printed by tools/gen_expected.py, not by hand.",
    ]
    for fn, args in cases():
        target = getattr(mod, fn)
        try:
            got = target(*args) if callable(target) else target
        except Exception as e:  # noqa: BLE001 — the answer IS the exception
            got = e
        arrow = "~>" if fn in LIBM else "->"
        cells = " ".join(enc(a) for a in args)
        out.append(f"{fn} {cells} {arrow} {enc(got)}".replace("  ", " "))
    return "\n".join(out) + "\n"


def main() -> int:
    args = sys.argv[1:]
    check = "--check" in args
    names = [a for a in args if not a.startswith("-")] or sorted(MODULES)
    os.makedirs(OUT, exist_ok=True)
    stale = []
    for name in names:
        if name not in MODULES:
            sys.exit(f"no cases for `{name}` — have {', '.join(sorted(MODULES))}")
        path = os.path.join(OUT, f"{name}.txt")
        text = render(name)
        if check:
            have = open(path).read() if os.path.isfile(path) else ""
            if have != text:
                stale.append(name)
            continue
        open(path, "w").write(text)
        print(f"{os.path.relpath(path)}: {len(text.splitlines()) - 1} rows")
    if stale:
        sys.exit(f"stale: {', '.join(stale)} — run `uv run tools/gen_expected.py`")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
