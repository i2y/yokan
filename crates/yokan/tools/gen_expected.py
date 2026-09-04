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

import datetime as _dt
import json
import math
import os
import random
import re
import re._compiler
import re._parser
import bisect
import heapq
import statistics
import string
import struct
import textwrap
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


# `datetime` values are carried as integers, so the table asks for
# the integers: a date is its ordinal, a datetime is microseconds from
# the same origin, a timedelta is microseconds. The generator does the
# same conversion CPython would, which is why the module here is a
# stand-in rather than `datetime` itself.
DAY_US = 86_400_000_000


def _ord(d):
    return d.toordinal()


def _us(t):
    return (t.toordinal() - 1) * DAY_US + (
        t.hour * 3_600_000_000 + t.minute * 60_000_000 + t.second * 1_000_000 + t.microsecond
    )


def _td(t):
    return t.days * DAY_US + t.seconds * 1_000_000 + t.microseconds


class _DatetimeTwin:
    """What the twins answer, in the integers they answer it in."""

    __name__ = "datetime"

    date_new = staticmethod(lambda y, m, d: _ord(_dt.date(y, m, d)))
    date_from_iso = staticmethod(lambda s: _ord(_dt.date.fromisoformat(s)))
    date_isoformat = staticmethod(lambda o: _dt.date.fromordinal(o).isoformat())
    date_str = staticmethod(lambda o: str(_dt.date.fromordinal(o)))
    date_year = staticmethod(lambda o: _dt.date.fromordinal(o).year)
    date_month = staticmethod(lambda o: _dt.date.fromordinal(o).month)
    date_day = staticmethod(lambda o: _dt.date.fromordinal(o).day)
    date_weekday = staticmethod(lambda o: _dt.date.fromordinal(o).weekday())
    date_isoweekday = staticmethod(lambda o: _dt.date.fromordinal(o).isoweekday())
    date_strftime = staticmethod(lambda o, f: _dt.date.fromordinal(o).strftime(f))
    date_add_delta = staticmethod(
        lambda o, us: _ord(_dt.date.fromordinal(o) + _dt.timedelta(microseconds=us))
    )
    date_sub_date = staticmethod(
        lambda a, b: _td(_dt.date.fromordinal(a) - _dt.date.fromordinal(b))
    )

    datetime_new = staticmethod(lambda y, m, d, h, mi, s, us: _us(_dt.datetime(y, m, d, h, mi, s, us)))
    datetime_from_iso = staticmethod(lambda s: _us(_dt.datetime.fromisoformat(s)))
    datetime_isoformat = staticmethod(lambda u: _dtof(u).isoformat())
    datetime_str = staticmethod(lambda u: str(_dtof(u)))
    datetime_date = staticmethod(lambda u: _ord(_dtof(u).date()))
    datetime_year = staticmethod(lambda u: _dtof(u).year)
    datetime_month = staticmethod(lambda u: _dtof(u).month)
    datetime_day = staticmethod(lambda u: _dtof(u).day)
    datetime_hour = staticmethod(lambda u: _dtof(u).hour)
    datetime_minute = staticmethod(lambda u: _dtof(u).minute)
    datetime_second = staticmethod(lambda u: _dtof(u).second)
    datetime_microsecond = staticmethod(lambda u: _dtof(u).microsecond)
    datetime_weekday = staticmethod(lambda u: _dtof(u).weekday())
    datetime_strftime = staticmethod(lambda u, f: _dtof(u).strftime(f))
    datetime_add_delta = staticmethod(lambda u, d: _us(_dtof(u) + _dt.timedelta(microseconds=d)))
    datetime_sub_datetime = staticmethod(lambda a, b: _td(_dtof(a) - _dtof(b)))
    datetime_of_date = staticmethod(lambda o: _us(_dt.datetime.combine(_dt.date.fromordinal(o), _dt.time())))

    delta_new = staticmethod(
        lambda d, s, us, ms, mi, h, w: _td(
            _dt.timedelta(days=d, seconds=s, microseconds=us, milliseconds=ms,
                          minutes=mi, hours=h, weeks=w)
        )
    )
    delta_days = staticmethod(lambda u: _dt.timedelta(microseconds=u).days)
    delta_seconds = staticmethod(lambda u: _dt.timedelta(microseconds=u).seconds)
    delta_microseconds = staticmethod(lambda u: _dt.timedelta(microseconds=u).microseconds)
    delta_total_seconds = staticmethod(lambda u: _dt.timedelta(microseconds=u).total_seconds())
    delta_str = staticmethod(lambda u: str(_dt.timedelta(microseconds=u)))


def _dtof(us):
    return _dt.datetime.fromordinal(us // DAY_US + 1) + _dt.timedelta(microseconds=us % DAY_US)


def cases_datetime():
    """`date`, `datetime` and `timedelta` in the integers the dialect
    carries them as. Naive only: an aware datetime carries a zone, and
    a zone is what the plan keeps out until both runs read it from the
    same place."""
    dates = ((1, 1, 1), (1970, 1, 1), (2000, 2, 29), (2026, 9, 4), (9999, 12, 31),
             (2024, 2, 29), (1999, 12, 31), (2026, 1, 1))
    for y, m, d in dates:
        yield ("date_new", (y, m, d))
        o = _ord(_dt.date(y, m, d))
        for fn in ("date_isoformat", "date_str", "date_year", "date_month", "date_day",
                   "date_weekday", "date_isoweekday"):
            yield (fn, (o,))
        for f in ("%Y-%m-%d", "%y%m%d", "%a %A", "%b %B", "%j", "%w", "%U", "%W", "%%", "x%Yx"):
            yield ("date_strftime", (o, f))
        yield ("date_from_iso", (_dt.date(y, m, d).isoformat(),))
        yield ("datetime_of_date", (o,))
    yield from (("date_new", args) for args in ((2026, 13, 1), (2026, 2, 30), (0, 1, 1), (10000, 1, 1)))
    yield ("date_from_iso", ("nonsense",))

    times = ((2026, 9, 4, 13, 5, 6, 789012), (1, 1, 1, 0, 0, 0, 0),
             (2026, 1, 1, 0, 0, 0, 0), (2026, 6, 30, 23, 59, 59, 999999),
             (9999, 12, 31, 23, 59, 59, 999999), (2026, 9, 4, 0, 30, 0, 1))
    for args in times:
        yield ("datetime_new", args)
        u = _us(_dt.datetime(*args))
        for fn in ("datetime_isoformat", "datetime_str", "datetime_date", "datetime_year",
                   "datetime_month", "datetime_day", "datetime_hour", "datetime_minute",
                   "datetime_second", "datetime_microsecond", "datetime_weekday"):
            yield (fn, (u,))
        for f in ("%Y-%m-%dT%H:%M:%S", "%H:%M", "%f", "%p %I", "%A, %d %B %Y"):
            yield ("datetime_strftime", (u, f))
        yield ("datetime_from_iso", (_dt.datetime(*args).isoformat(),))
    yield ("datetime_new", (2026, 9, 4, 24, 0, 0, 0))
    yield ("datetime_new", (2026, 9, 4, 0, 0, 0, 1000000))
    yield from (("datetime_from_iso", (s,)) for s in
                ("2026-09-04", "2026-09-04 13:05:06", "2026-09-04T13:05", "not a date"))

    deltas = ((0, 0, 0, 0, 0, 0, 0), (1, 0, 0, 0, 0, 0, 0), (-1, 0, 0, 0, 0, 2, 0),
              (0, 1, 0, 0, 0, 0, 0), (0, 0, 1, 0, 0, 0, 0), (2, 0, 5, 0, 0, 0, 0),
              (0, 0, 0, 1500, 0, 0, 0), (0, 0, 0, 0, 90, 0, 0), (0, 0, 0, 0, 0, 0, 2),
              (-3, -30, -7, 0, 0, 0, 0))
    for args in deltas:
        yield ("delta_new", args)
        u = _td(_dt.timedelta(days=args[0], seconds=args[1], microseconds=args[2],
                              milliseconds=args[3], minutes=args[4], hours=args[5], weeks=args[6]))
        for fn in ("delta_days", "delta_seconds", "delta_microseconds",
                   "delta_total_seconds", "delta_str"):
            yield (fn, (u,))

    # Arithmetic across the three.
    o = _ord(_dt.date(2026, 9, 4))
    u = _us(_dt.datetime(2026, 9, 4, 13, 5, 6, 789012))
    for d in (0, DAY_US, -DAY_US, 25 * 3_600_000_000, 1, -1, 90 * 60_000_000):
        yield ("date_add_delta", (o, d))
        yield ("datetime_add_delta", (u, d))
    yield ("date_sub_date", (o, _ord(_dt.date(2026, 1, 1))))
    yield ("date_sub_date", (_ord(_dt.date(2026, 1, 1)), o))
    yield ("datetime_sub_datetime", (u, _us(_dt.datetime(2026, 9, 4, 0, 0, 0, 0))))
    yield ("date_add_delta", (_ord(_dt.date(9999, 12, 31)), DAY_US))


# `re` is the same arrangement as `datetime`: the twin is asked in the
# shape the dialect hands it, which is CPython's own compiled pattern
# as a list of ints. The generator compiles the pattern exactly as the
# translator does, so the table checks the ENGINE rather than a
# rewriting of it.
def _code(pattern):
    p = re._parser.parse(pattern)
    return [int(x) for x in re._compiler._code(p, 0)], p.state.groups - 1


def _template(repl, pattern):
    parts, lits = [], []
    for chunk in re._parser.parse_template(repl, re.compile(pattern)):
        if isinstance(chunk, int):
            parts.append(chunk)
            lits.append("")
        else:
            parts.append(-1)
            lits.append(chunk)
    return parts, lits


class _ReTwin:
    """What the twins answer, asked the way the dialect asks."""

    __name__ = "re"

    re_search = staticmethod(lambda c, s: re.search(_undo(c), s) is not None)
    re_match = staticmethod(lambda c, s: re.match(_undo(c), s) is not None)
    re_fullmatch = staticmethod(lambda c, s: re.fullmatch(_undo(c), s) is not None)
    re_findall = staticmethod(lambda c, s, g: re.findall(_undo(c), s))
    re_split = staticmethod(
        lambda c, s, n: re.split(_undo(c), s, maxsplit=n if n > 0 else 0)
    )
    re_escape = staticmethod(re.escape)

    @staticmethod
    def re_sub(c, parts, lits, s, count, ngroups):
        pattern, repl = _SUBS[(tuple(c), tuple(parts), tuple(lits))]
        return re.sub(pattern, repl, s, count=count if count > 0 else 0)


# The table records the compiled arrays, so the generator keeps the
# patterns they came from to ask CPython the same question.
_PATTERNS = {}
_SUBS = {}


def _undo(code):
    return _PATTERNS[tuple(code)]


def cases_re():
    """`re`, asked the way the dialect asks: the pattern arrives as
    the array CPython's own compiler produced. Only the calls whose
    answer is already a dialect type are here — a `Match` has no shape
    a typed subset can hold, so the translator refuses it by name."""
    tests = (
        (r"\d+", "a12b345"),
        (r"^a", "abc"),
        (r"^a", "bac"),
        (r"[A-Z]\w*", "hello World"),
        (r"(?i)ab", "AB xy"),
        (r"x*", "abc"),
        (r"\bfoo\b", "a foo b"),
        (r"\s*,\s*", "a , b,c"),
        (r"日本", "これは日本語"),
        (r"a(?=b)", "ab ac"),
    )
    for pat, subject in tests:
        code, n = _code(pat)
        _PATTERNS[tuple(code)] = pat
        for fn in ("re_search", "re_match", "re_fullmatch"):
            yield (fn, (code, subject))
        yield ("re_findall", (code, subject, 1 if n else 0))
        if not n:
            yield ("re_split", (code, subject, 0))
            yield ("re_split", (code, subject, 1))
    grouped = ((r"(\d)\d", "a12b345"), (r"(a)?b", "b ab"), (r"(\w+)@", "me@you he@"))
    for pat, subject in grouped:
        code, _n = _code(pat)
        _PATTERNS[tuple(code)] = pat
        yield ("re_findall", (code, subject, 1))
    subs = (
        (r"\d+", "N", "a12b345", 0),
        (r"(\w)(\d)", r"\2\1", "a1 b2", 0),
        (r"x*", "-", "abc", 0),
        (r"\d", "N", "123", 2),
        (r"\s+", " ", "a   b\tc", 0),
        (r"(a)", r"[\g<1>]", "abca", 0),
    )
    for pat, repl, subject, count in subs:
        code, n = _code(pat)
        _PATTERNS[tuple(code)] = pat
        parts, lits = _template(repl, pat)
        _SUBS[(tuple(code), tuple(parts), tuple(lits))] = (pat, repl)
        yield ("re_sub", (code, parts, lits, subject, count, n))
    yield from (("re_escape", (s,)) for s in
                ("plain", "a.b*c", "a b-c", "^$\\", "日本 語", "tab\there"))


class _SmallTwin:
    """`string`, `textwrap`, `bisect`, `heapq` and the rest of `str`,
    asked the way the dialect asks."""

    __name__ = "small"

    string_ascii_letters = staticmethod(lambda: string.ascii_letters)
    string_ascii_lowercase = staticmethod(lambda: string.ascii_lowercase)
    string_ascii_uppercase = staticmethod(lambda: string.ascii_uppercase)
    string_digits = staticmethod(lambda: string.digits)
    string_hexdigits = staticmethod(lambda: string.hexdigits)
    string_octdigits = staticmethod(lambda: string.octdigits)
    string_punctuation = staticmethod(lambda: string.punctuation)
    string_whitespace = staticmethod(lambda: string.whitespace)
    string_printable = staticmethod(lambda: string.printable)

    textwrap_dedent = staticmethod(textwrap.dedent)
    textwrap_indent = staticmethod(textwrap.indent)

    bisect_left = staticmethod(bisect.bisect_left)
    bisect_right = staticmethod(bisect.bisect_right)
    heapq_nsmallest = staticmethod(heapq.nsmallest)
    heapq_nlargest = staticmethod(heapq.nlargest)

    # The halves of the calls that answer a tuple: each is a static
    # of its own here, and the translator builds the tuple.
    math_frexp_m = staticmethod(lambda v: math.frexp(v)[0])
    math_frexp_e = staticmethod(lambda v: math.frexp(v)[1])
    math_modf_frac = staticmethod(lambda v: math.modf(v)[0])
    math_modf_int = staticmethod(lambda v: math.modf(v)[1])
    py_str_partition_before = staticmethod(lambda s, sep: s.partition(sep)[0])
    py_str_partition_sep = staticmethod(lambda s, sep: s.partition(sep)[1])
    py_str_partition_after = staticmethod(lambda s, sep: s.partition(sep)[2])
    py_str_rpartition_before = staticmethod(lambda s, sep: s.rpartition(sep)[0])
    py_str_rpartition_sep = staticmethod(lambda s, sep: s.rpartition(sep)[1])
    py_str_rpartition_after = staticmethod(lambda s, sep: s.rpartition(sep)[2])

    py_str_title = staticmethod(str.title)
    py_str_capitalize = staticmethod(str.capitalize)
    py_str_swapcase = staticmethod(str.swapcase)
    py_str_zfill = staticmethod(str.zfill)
    py_str_ljust = staticmethod(str.ljust)
    py_str_rjust = staticmethod(str.rjust)
    py_str_center = staticmethod(str.center)
    py_str_isupper = staticmethod(str.isupper)
    py_str_islower = staticmethod(str.islower)
    py_str_isalpha = staticmethod(str.isalpha)
    py_str_isdigit = staticmethod(str.isnumeric)
    py_str_isalnum = staticmethod(str.isalnum)
    py_str_isspace = staticmethod(str.isspace)
    py_str_isascii = staticmethod(str.isascii)
    py_str_removeprefix = staticmethod(str.removeprefix)
    py_str_removesuffix = staticmethod(str.removesuffix)
    py_str_rfind = staticmethod(str.rfind)
    py_str_index_of = staticmethod(str.index)
    py_str_rindex = staticmethod(str.rindex)
    py_str_splitlines = staticmethod(str.splitlines)
    py_str_expandtabs = staticmethod(str.expandtabs)
    py_str_strip_chars = staticmethod(str.strip)
    py_str_lstrip_chars = staticmethod(str.lstrip)
    py_str_rstrip_chars = staticmethod(str.rstrip)


def cases_small():
    """The pure small modules. `isdigit` is asked as `isnumeric`
    deliberately: Rust's `char::is_numeric` is Unicode's N* category,
    which is what `isnumeric` means, and the row says so rather than
    the twin quietly answering a different question."""
    for c in ("ascii_letters", "ascii_lowercase", "ascii_uppercase", "digits",
              "hexdigits", "octdigits", "punctuation", "whitespace", "printable"):
        yield (f"string_{c}", ())
    texts = ("", "  a\n  b\n", "\ta\n\tb", "  a\n    b\n", "a\n  b", "  \n  a\n",
             "  a\n\n  b\n", "line\n")
    yield from (("textwrap_dedent", (t,)) for t in texts)
    yield from (("textwrap_indent", (t, "> ")) for t in texts)
    ints = [1, 3, 3, 5, 8]
    yield from (("bisect_left", (ints, x)) for x in (0, 1, 3, 4, 8, 9))
    yield from (("bisect_right", (ints, x)) for x in (0, 1, 3, 4, 8, 9))
    yield from (("bisect_left", ([], 1)), ("bisect_right", ([], 1)))
    words = ["pear", "apple", "fig"]
    yield from (("bisect_left", (sorted(words), w)) for w in ("apple", "b", "zz"))
    for n in (0, 1, 3, 9):
        yield ("heapq_nsmallest", (n, [5, 1, 9, 3, 3]))
        yield ("heapq_nlargest", (n, [5, 1, 9, 3, 3]))
        yield ("heapq_nsmallest", (n, ["pear", "apple", "fig"]))
    strs = ("", "hello world", "don't", "HELLO", "hello", "MiXeD", "\u00fcnicode",
            "\u65e5\u672c go", "a1b2", "  padded  ", "-42", "+42", "42", "a-b_c",
            "\u00df", "\u0130")
    for fn in ("py_str_title", "py_str_capitalize", "py_str_swapcase",
               "py_str_isupper", "py_str_islower", "py_str_isalpha", "py_str_isdigit",
               "py_str_isalnum", "py_str_isspace", "py_str_isascii", "py_str_splitlines"):
        yield from ((fn, (t,)) for t in strs)
    yield from (("py_str_isspace", (t,)) for t in (" ", "\t\n", " x ", "\u00a0"))
    yield from (("py_str_isdigit", (t,)) for t in ("123", "\u0661\u0662", "\u2160", "1.5"))
    for w in (0, 1, 5, 8, 9):
        for t in ("ab", "abc", "-42", ""):
            yield ("py_str_zfill", (t, w))
            yield ("py_str_ljust", (t, w, "."))
            yield ("py_str_rjust", (t, w, "."))
            yield ("py_str_center", (t, w, "."))
    yield from (("py_str_removeprefix", ("prefix-body", p)) for p in ("prefix-", "x", ""))
    yield from (("py_str_removesuffix", ("body-suffix", p)) for p in ("-suffix", "x", ""))
    for t, p in (("abcabc", "b"), ("abcabc", "z"), ("abc", ""), ("\u65e5\u672c\u65e5", "\u65e5")):
        yield ("py_str_rfind", (t, p))
        yield ("py_str_index_of", (t, p))
        yield ("py_str_rindex", (t, p))
    yield from (("py_str_splitlines", (t,)) for t in ("a\nb", "a\r\nb", "a\n", "", "\n", "a\u2028b"))
    for t in ("a\tb", "\tx", "ab\tc\td", "a\nb\tc"):
        yield from (("py_str_expandtabs", (t, n)) for n in (0, 1, 4, 8))
    for v in (0.0, -0.0, 1.0, -1.0, 0.5, 2.5, 1e300, 5e-324, 123.456, -0.75,
              math.inf, -math.inf, math.nan):
        yield ("math_frexp_m", (v,))
        yield ("math_frexp_e", (v,))
        yield ("math_modf_frac", (v,))
        yield ("math_modf_int", (v,))
    for t, sep in (("key=value=more", "="), ("nosep", "="), ("=lead", "="),
                   ("trail=", "="), ("", "="), ("a--b", "--"), ("\u65e5=\u672c", "=")):
        for fn in ("py_str_partition_before", "py_str_partition_sep", "py_str_partition_after",
                   "py_str_rpartition_before", "py_str_rpartition_sep", "py_str_rpartition_after"):
            yield (fn, (t, sep))
    for t, cs in (("xxaxx", "x"), ("  a  ", " "), ("aabbaa", "ab"), ("abc", "z"), ("", "x")):
        yield ("py_str_strip_chars", (t, cs))
        yield ("py_str_lstrip_chars", (t, cs))
        yield ("py_str_rstrip_chars", (t, cs))


MODULES = {
    "math": (math, cases_math),
    "small": (_SmallTwin, cases_small),
    "re": (_ReTwin, cases_re),
    "datetime": (_DatetimeTwin, cases_datetime),
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
