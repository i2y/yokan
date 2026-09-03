#!/usr/bin/env python3
"""What the standard library covers, module by module, against the
CPython running this script.

    uv run tools/stdlib_coverage.py              # every module
    uv run tools/stdlib_coverage.py math random  # named ones
    uv run tools/stdlib_coverage.py -o cov.md    # to a file

The tour's standard-library section and the "what does not work yet"
list are written FROM this output rather than kept in step with it by
hand — the manifest in `yokan_gate.py` is the one table, and this
reads it.

Two kinds of module appear. One shares a name with Python's, and the
report says how far it reaches into it. The other is Yokan's own,
with nothing to be measured against, so the report just lists it.
"""

import argparse
import importlib
import inspect
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, ".."))

from yokan_gate import Translator  # noqa: E402


def members(mod):
    """The public surface of a CPython module: what it offers and
    whether the offer is a call or a value."""
    out = {}
    for name, val in inspect.getmembers(mod):
        if name.startswith("_"):
            continue
        out[name] = "call" if callable(val) else "value"
    return out


def report(names) -> str:
    have = {}
    for _cls, mod, rows in Translator.STDLIB:
        if mod is None:
            continue
        have[mod] = {
            py: ("call", "cpython" in flags)
            for py, _fn, _params, _r, _ru, *flags in rows
            if py
        }
    lines = [
        f"# The standard library against CPython {sys.version.split()[0]}",
        "",
        "Written by `tools/stdlib_coverage.py` from the manifest in "
        "`yokan_gate.py`. Do not edit by hand.",
        "",
    ]
    summary = ["| module | in the dialect | Python has |", "|---|---|---|"]
    body = []
    for mod in names:
        ours = have[mod]
        try:
            theirs = members(importlib.import_module(mod))
        except ImportError:
            theirs = None
        if theirs is None:
            summary.append(f"| `{mod}` | {len(ours)} | — (Yokan's own) |")
            body += [
                f"## `{mod}` — Yokan's own, {len(ours)} functions",
                "",
                "Python has no module of this name, so there is nothing to "
                "measure against.",
                "",
                "- " + ", ".join(f"`{n}`" for n in sorted(ours)),
                "",
            ]
            continue
        # A name in common is not the same function: the manifest's
        # `cpython` flag is what says a row answers what Python's
        # member of that name answers.
        covered = sorted(n for n in ours if n in theirs and ours[n][1])
        borrowed = sorted(n for n in ours if n in theirs and not ours[n][1])
        differs = sorted(n for n in covered if theirs[n] != ours[n][0])
        extra = sorted(n for n in ours if n not in theirs)
        missing = sorted(n for n in theirs if n not in ours)
        summary.append(f"| `{mod}` | {len(covered)} | {len(theirs)} |")
        body.append(f"## `{mod}` — {len(covered)} of Python's {len(theirs)}")
        body.append("")
        if covered:
            body += ["**In the dialect.** " + ", ".join(f"`{n}`" for n in covered), ""]
        if differs:
            body += [
                "**A different shape from Python's.** "
                + ", ".join(
                    f"`{n}` (a {theirs[n]} in Python, a {ours[n][0]} here)" for n in differs
                ),
                "",
            ]
        if borrowed:
            body += [
                "**Python's name, Yokan's own meaning.** "
                + ", ".join(f"`{n}`" for n in borrowed),
                "",
            ]
        if extra:
            body += [
                "**Not in Python's module at all.** " + ", ".join(f"`{n}`" for n in extra),
                "",
            ]
        if missing:
            body += ["**Not yet.** " + ", ".join(f"`{n}`" for n in missing), ""]
    return "\n".join(lines + summary + [""] + body) + "\n"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("modules", nargs="*", help="modules to report on (default: all)")
    ap.add_argument("-o", "--out", help="write here instead of stdout")
    args = ap.parse_args()
    known = [m for _c, m, _r in Translator.STDLIB if m]
    names = args.modules or known
    for n in names:
        if n not in known:
            sys.exit(f"`{n}` is not in the manifest — have {', '.join(known)}")
    text = report(names)
    if args.out:
        open(args.out, "w").write(text)
        print(f"{args.out}: {len(names)} modules")
    else:
        sys.stdout.write(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
