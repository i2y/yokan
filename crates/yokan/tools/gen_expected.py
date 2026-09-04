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
    b:0     a bool          [f:..,f:..]    a list
    u:      None            {s:k=i:1,..}   an object, in order

`~>` in place of `->` means "within one ulp": the answer comes from
the platform's libm rather than from IEEE-754, so CPython and the
twin agree exactly on this machine but a table is read on others.
"""

import json
import math
import os
import random
import statistics
import struct
import sys
import urllib.parse

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "..", "yokan-stdlib", "tests", "expected")

# The answers IEEE-754 pins down, and the ones a C library decides.
# `sqrt` is exactly rounded by the standard; `sin` is whatever libm
# does, and CPython and Rust only agree because they call the same
# one. Anything listed here is compared within an ulp instead.
#
# `hypot`, `dist` and `fma` are deliberately NOT here: CPython
# computes the first two itself rather than calling the platform, and
# `fma` is exactly rounded by the standard, so the twin has to match
# to the bit.
LIBM = {
    "sin", "cos", "tan", "sinh", "cosh", "tanh", "exp", "exp2", "expm1",
    "log", "log1p", "log2", "log10", "atan", "atan2", "asin", "acos",
    "asinh", "acosh", "atanh", "cbrt", "pow",
}

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


# The values an int-taking function is asked about: small, boundary,
# and large enough to reach the 64-bit edge.
INTS = (0, 1, 2, 3, 5, 10, 20, 21, 62, 63, -1, -5, 100, 1000)
SMALL = (0.0, -0.0, 1.0, -1.0, 0.5, -0.5, 2.5, -2.75, 0.1, -0.1, 1e16, -1e16,
         math.inf, -math.inf, math.nan)


def cases_math():
    """`math`, in full — every function the dialect takes, and the
    inputs where CPython's answer is an error rather than a number.

    Four are deliberately absent, refused by name in the dialect
    rather than answered here: `frexp` and `modf` return tuples,
    `prod` and `sumprod` answer an int or a float depending on what
    is in the list, and `gamma`, `lgamma`, `erf` and `erfc` are
    computed by CPython itself rather than by the platform, so a twin
    for them is a port and not a call.
    """
    # One-argument, whole domain.
    for fn in ("fabs", "sin", "cos", "tan", "sinh", "tanh", "asinh", "atan",
               "cbrt", "degrees", "radians", "ulp", "isnan", "isinf", "isfinite"):
        yield from ((fn, (x,)) for x in FLOATS)
        yield from ((fn, (x,)) for x in (math.inf, -math.inf, math.nan))
    # One-argument, with a domain CPython names.
    yield from (("sqrt", (x,)) for x in NONNEG)
    yield from (("sqrt", (x,)) for x in (-1.0, -0.5, -1e300, math.inf, math.nan))
    for fn in ("acos", "asin", "atanh"):
        yield from ((fn, (x,)) for x in (-1.0, -0.5, 0.0, -0.0, 0.5, 1.0, 0.1))
        yield from ((fn, (x,)) for x in (2.0, -2.0, 1.5, math.nan))
    yield from (("acosh", (x,)) for x in (1.0, 1.5, 2.0, 10.0, 1e300, 0.5, -1.0, math.nan))
    for fn in ("log", "log2", "log10"):
        yield from ((fn, (x,)) for x in (1.0, 2.0, 0.5, 10.0, 1e300, 5e-324, 0.1))
        yield from ((fn, (x,)) for x in (0.0, -0.0, -1.0, math.inf, math.nan))
    yield from (("log1p", (x,)) for x in (0.0, -0.0, 1.0, -0.5, 1e-16, 1e300, -1.0, -2.0, math.nan))
    yield from (("log", (x, b)) for x in (8.0, 100.0, 1.0) for b in (2.0, 10.0, math.e))
    # One-argument, with a range CPython names.
    for fn in ("exp", "exp2", "expm1", "cosh"):
        yield from ((fn, (x,)) for x in (0.0, -0.0, 1.0, -1.0, 0.5, 709.0, -745.0, 710.0,
                                         1000.0, math.inf, -math.inf, math.nan))
    yield from (("sinh", (x,)) for x in (710.0, 1000.0, -1000.0))
    # Whole doubles as ints, and the two refusals.
    for fn in ("floor", "ceil", "trunc"):
        yield from ((fn, (x,)) for x in SMALL)
    # Two-argument.
    pairs = ((0.0, 0.0), (1.0, 2.0), (-1.0, 2.0), (1.0, -2.0), (2.5, 0.5), (-7.5, 2.0),
             (1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (math.inf, 1.0), (1.0, math.inf),
             (math.nan, 1.0), (1.0, math.nan), (5.0, 3.0), (-5.0, 3.0), (1e300, 1e-300))
    for fn in ("atan2", "copysign", "fmod", "remainder", "hypot", "nextafter", "isclose"):
        yield from ((fn, p) for p in pairs)
    yield from (("pow", (a, b)) for a in (0.0, 1.0, 2.0, -2.0, 0.5, 10.0)
                for b in (0.0, 1.0, 2.0, -1.0, 0.5, 3.0, -0.5))
    yield from (("pow", args) for args in (
        (10.0, 400.0), (10.0, -400.0), (math.inf, 2.0), (-math.inf, 3.0),
        (math.inf, -1.0), (2.0, math.inf), (0.5, math.inf), (math.nan, 0.0),
        (1.0, math.nan), (math.nan, 2.0), (-1.0, math.inf),
    ))
    yield from (("ldexp", (x, n)) for x in (1.0, -1.0, 0.0, 0.5, 1e300)
                for n in (0, 1, -1, 52, 1023, 1024, -1074, -1075, 100000, -100000))
    yield from (("fma", (a, b, c)) for a, b, c in (
        (2.0, 3.0, 4.0), (0.1, 0.2, 0.3), (1e300, 1e300, -1e300), (-1.0, 1.0, 1.0),
        (math.inf, 0.0, 1.0), (0.0, math.inf, 1.0), (math.nan, 1.0, 1.0),
    ))
    # Integers.
    yield from (("factorial", (n,)) for n in (0, 1, 2, 5, 10, 20, -1))
    yield from (("isqrt", (n,)) for n in (0, 1, 2, 3, 4, 8, 9, 10, 99, 100, 10**12, -1))
    yield from (("comb", (n, k)) for n in (0, 1, 5, 10, 60, -1)
                for k in (0, 1, 2, 5, 11, -1))
    yield from (("perm", (n, k)) for n in (0, 1, 5, 10, -1) for k in (0, 1, 2, 5, -1))
    yield from (("gcd", (a, b)) for a in INTS for b in (0, 1, 6, 15, -6))
    yield from (("lcm", (a, b)) for a in (0, 1, 4, 6, 21, -6) for b in (0, 1, 6, 15, -6))
    # Lists.
    yield from (("fsum", (xs,)) for xs in (
        [], [1.0], [0.1, 0.2, 0.3], [1e100, 1.0, -1e100], [1.0] * 10,
        [0.1] * 10, [1e308, 1e308, -1e308], [1.0, 1e-16, 1e-16],
        [math.inf, 1.0], [math.nan, 1.0], [math.inf, -math.inf],
    ))
    yield from (("dist", (p, q)) for p, q in (
        ([0.0], [0.0]), ([3.0, 0.0], [0.0, 4.0]), ([1.0, 2.0, 3.0], [4.0, 6.0, 3.0]),
        ([1e300, 1e300], [0.0, 0.0]), ([0.1, 0.2], [0.3, 0.4]),
    ))
    # Values.
    for name in ("pi", "e", "tau", "inf", "nan"):
        yield (name, ())


def cases_random():
    """`random`, which is stateful: the rows run in order and the
    twin's generator carries between them, exactly as CPython's
    module-level one does. Every sequence starts from a `seed`,
    because an unseeded generator is not something a table can
    describe — nor something the gate could hold two runs to.

    The seeds are chosen to exercise the word-splitting in
    `init_by_array`: one word, two words, and the negative that CPython
    takes the absolute value of.
    """
    for seed in (0, 1, 42, 2**31, 2**32 + 7, 2**62, -1, -123456789):
        yield ("seed", (seed,))
        yield from (("random", ()) for _ in range(4))
        yield from (("randint", (1, 6)) for _ in range(6))
        yield from (("getrandbits", (k,)) for k in (1, 8, 31, 32, 33, 52, 63))
        yield ("randrange", (10,))
        yield ("randrange", (5, 15))
        yield ("randrange", (0, 100, 7))
        yield ("randrange", (100, 0, -7))
        yield from (("uniform", (0.0, 1.0)) for _ in range(2))
        yield from (("uniform", (-2.5, 7.5)) for _ in range(2))
        # gauss keeps a spare between calls, so an odd count matters.
        yield from (("gauss", (0.0, 1.0)) for _ in range(5))
        yield from (("gauss", (10.0, 0.5)) for _ in range(2))
        yield ("choice", (["a", "b", "c", "d"],))
        yield ("choice", ([1, 2, 3],))
        yield ("choice", ([0.5, 1.5],))
        # Both of sample's strategies: a small population walks a
        # pool, a large one draws until it finds an unused index.
        yield ("sample", ([1, 2, 3, 4, 5, 6, 7, 8], 3))
        yield ("sample", (list(range(200)), 4))
        yield ("sample", (["x", "y", "z"], 3))
    # The refusals.
    yield ("randrange", (0,))
    yield ("randrange", (5, 5))
    yield ("getrandbits", (0,))
    yield ("getrandbits", (-1,))
    yield ("choice", ([],))
    yield ("sample", ([1, 2], 5))


def cases_statistics():
    """`statistics` over `list[float]`, which is the only shape the
    dialect takes: CPython answers an int for `mean([1, 2, 3])` and a
    float for `mean([1, 2, 4])`, so an int list has no static type.

    The sets are chosen to separate an exact answer from a nearly
    exact one — `[0.1, 0.2, 0.3]` is the classic, where a sum in
    floating point and a sum in rationals differ in the last bits.
    """
    sets = (
        [1.0],
        [1.0, 2.0],
        [0.1, 0.2, 0.3],
        [1.0, 2.0, 3.0, 4.0],
        [1.5, 2.5, 2.5, 2.75, 3.25, 4.75],
        [-2.0, -1.0, 0.0, 1.0, 2.0],
        [1e300, 1e300, -1e300],
        [1e-300, 2e-300, 3e-300],
        [0.1] * 10,
        [1.0, 1.0, 2.0, 3.0, 3.0, 3.0],
        [2.0, 2.0, 1.0, 1.0],
        [5e-324, 1.0],
        [1.7976931348623157e308, 1.0],
        [],
        # Non-finite data, which CPython keeps apart from the exact
        # sum: the total is the infinities and NaNs alone.
        [math.inf, 1.0],
        [math.nan, 1.0],
        [math.inf, -math.inf],
        [math.inf, 1.0, 2.0],
    )
    for fn in ("mean", "fmean", "median", "mode", "variance", "pvariance", "stdev", "pstdev"):
        yield from ((fn, (xs,)) for xs in sets)


def cases_json():
    """`json.dumps` with CPython's defaults, which is the only shape
    the dialect takes: `ensure_ascii=True`, `", "` and `": "` between
    the parts, keys in the order they went in.

    The strings are chosen for the escaping: the two characters JSON
    names, the control characters with short escapes, one without,
    DEL, Latin-1, a CJK character, and one past the basic plane, which
    CPython writes as the surrogate pair its UTF-16 encoding is.
    """
    texts = (
        "", "plain", 'a"b', "back\\slash", "tab\there", "nl\n", "cr\r",
        "\b\f", "bell\x07", "del\x7f", "\u00fcn\u00efcode", "\u65e5\u672c\u8a9e",
        "\U0001f600", "\u2028", "  spaced  ",
    )
    yield from (("dumps", (t,)) for t in texts)
    yield from (("dumps", (v,)) for v in (0, 1, -1, 2**62, -(2**62)))
    yield from (("dumps", (v,)) for v in (0.0, -0.0, 1.0, 0.1, 1e300, 1e16, 2.5,
                                          math.inf, -math.inf, math.nan))
    yield from (("dumps", (v,)) for v in (True, False, None))
    # Lists of one scalar type, the shape a held `list[...]` takes.
    yield from (("dumps", (xs,)) for xs in (
        [], [1, 2, 3], [1.5, 2.5], ["a", 'q"q'], [True, False],
        [0.1, 0.2, 0.3], ["\u65e5"],
    ))
    # Objects, in the order the keys went in — not sorted.
    yield from (("dumps", (d,)) for d in (
        {}, {"b": 1, "a": 2}, {"z": "last", "a": "first"},
        {"k": 1.5}, {"t": True}, {"q\"q": 1},
    ))
    # Nested, which a literal reaches to any depth.
    yield from (("dumps", (v,)) for v in (
        {"user": {"name": "momo", "tags": ["a", "b"], "n": 3},
         "ok": True, "score": 1.5, "extra": None},
        [[1, 2], [3]],
        [{"a": [1]}, {}],
        {"deep": {"deeper": {"deepest": [1, {"x": None}]}}},
    ))


MODULES = {
    "math": (math, cases_math),
    "random": (random, cases_random),
    "statistics": (statistics, cases_statistics),
    "json": (json, cases_json),
}


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
    if isinstance(v, (list, tuple)):
        return "[" + ",".join(enc(x) for x in v) + "]"
    if isinstance(v, dict):
        return "{" + ",".join(f"{enc(k)}={enc(x)}" for k, x in v.items()) + "}"
    if v is None:
        return "u:"
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
