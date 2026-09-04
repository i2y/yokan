#!/usr/bin/env python3
"""What the standard library covers, module by module, against the
CPython running this script.

    uv run tools/stdlib_coverage.py                # every module
    uv run tools/stdlib_coverage.py math random    # named ones
    uv run tools/stdlib_coverage.py -o cov.md      # to a file
    uv run tools/stdlib_coverage.py --lang ja      # the Japanese page

The tour's standard-library section and the "what does not work yet"
list are written FROM this output rather than kept in step with it by
hand — the manifest in `yokan_gate.py` is the one table, and this
reads it.

Two kinds of module appear. One shares a name with Python's, and the
report says how far it reaches into it. The other is Yokan's own,
with nothing to be measured against, so the report just lists it.

Three of Python's modules — `datetime`, `collections`, `itertools` —
are carried by the translator's own tables rather than the manifest,
and are read from those tables here for the same reason: a report
kept in step by hand is a report that falls behind.

The builtins are neither. Nothing declares them, so they are probed:
each one is put in a handler and run past the translator, and what
comes back is either nothing (it is in the dialect) or the refusal,
which is the sentence that says what to write instead.
"""

import argparse
import importlib
import inspect
import os
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, ".."))

from yokan_gate import Translator, Untranslatable, translate_file  # noqa: E402


def members(mod):
    """The public surface of a CPython module: what it offers and
    whether the offer is a call or a value.

    A module's own surface, not what it happens to import. `statistics`
    pulls in `sin`, `Fraction` and `math` itself to do its work, and
    counting those against the dialect would be counting the wrong
    thing."""
    out = {}
    for name, val in inspect.getmembers(mod):
        if name.startswith("_") or inspect.ismodule(val):
            continue
        owner = getattr(val, "__module__", None)
        # A module's C accelerator is the module: `bisect_left` says
        # `_bisect`, and dropping it read as "Python has nothing here".
        if owner is not None and owner not in (mod.__name__, "_" + mod.__name__):
            continue
        out[name] = "call" if callable(val) else "value"
    return out


# Python's modules the translator carries in its own tables instead
# of the manifest. Read from those tables, not typed out again.
def carried():
    T = Translator
    return {
        "datetime": {n: ("call", True) for n in T.DT_TYPES},
        "collections": {"Counter": ("call", True)},
        "itertools": {n: ("call", True) for n in T.ITERTOOLS},
    }


# One expression per builtin, in the position an app would write it:
# a statement inside a handler. Some that are refused there work in a
# `for` instead, and the refusal is the thing that says so — which is
# why the message is printed rather than a verdict.
BUILTIN_PROBES = {
    # Each builtin in the position an app would write it: a statement
    # in a handler, and a `for` for the ones that walk a list. What
    # comes back is nothing, or the refusal — which is the sentence
    # that says what to write instead.
    "abs": "self.n = abs(-2)",
    "all": "self.ok = all(bits())",
    "any": "self.ok = any(bits())",
    "bin": "self.note = bin(5)",
    "bool": "self.ok = bool(1)",
    "callable": "self.ok = callable(view)",
    "chr": "self.note = chr(65)",
    "dict": "d = dict()",
    "divmod": "q, r = divmod(7, 2)",
    "enumerate": "for i, v in enumerate(xs()):\n            self.n = i + v",
    "filter": "ys = filter(view, xs())",
    "float": 'self.x = float("1.5")',
    "format": 'self.note = format(1, "d")',
    "getattr": 'self.note = getattr(S, "note")',
    "hasattr": 'self.ok = hasattr(S, "note")',
    "hash": 'self.n = hash("a")',
    "hex": "self.note = hex(255)",
    "id": "self.n = id(xs)",
    "input": "self.note = input()",
    "int": 'self.n = int("3")',
    "isinstance": "self.ok = isinstance(1, int)",
    "iter": "it = iter(xs())",
    "len": "self.n = len(xs())",
    "list": "ys = list(xs())",
    "map": "ys = map(view, xs())",
    "max": "self.n = max(xs())",
    "min": "self.n = min(xs())",
    "next": "v = next(iter(xs()))",
    "oct": "self.note = oct(8)",
    "open": 'f = open("a.txt")',
    "ord": 'self.n = ord("a")',
    "pow": "self.n = pow(2, 3)",
    "print": 'print("hi")',
    "range": "for i in range(3):\n            self.n = i",
    "repr": 'self.note = repr("a")',
    "reversed": "for v in reversed(xs()):\n            self.n = v",
    "round": "self.n = round(1.5)",
    "set": "ys = set()",
    "setattr": 'setattr(S, "note", "x")',
    "sorted": "self.ys = sorted(xs())",
    "str": "self.note = str(1)",
    "sum": "self.n = sum(xs())",
    "tuple": "t = tuple(xs())",
    "type": "self.note = type(1)",
    "zip": "for a, b in zip(xs(), xs()):\n            self.n = a + b",
}

PROBE_APP = """from yokan import State, column, run, store, text

xs: State[list[int]] = State([1, 2, 3])
bits: State[list[bool]] = State([True])


@store
class S:
    note: str = "-"
    n: int = 0
    x: float = 0.0
    ok: bool = False
    ys: list[int] = []

    def go(self) -> None:
        %s


def view():
    with column():
        text(f"{S.note}")


if __name__ == "__main__":
    run(view, title="probe")
"""


def probe_builtins():
    """Each builtin past the translator: `None` when it is taken, the
    refusal's own sentence when it is not."""
    out = {}
    with tempfile.TemporaryDirectory() as d:
        app = os.path.join(d, "probe.py")
        for name, expr in BUILTIN_PROBES.items():
            open(app, "w").write(PROBE_APP % expr)
            try:
                translate_file(app)
                out[name] = None
            except Untranslatable as e:
                out[name] = e.msg
            except (ValueError, SyntaxError) as e:
                out[name] = str(e)
    return out


PHRASES = {
    "en": {
        "title": "Coverage against CPython {v}",
        "made": "Written by `tools/stdlib_coverage.py` from the manifest in "
                "`yokan_gate.py` and the translator's own tables. Do not edit by hand.",
        "yes": "✓",
        "no": "—",
        "cols": "| name | Yokan | note |",
        "own_head": "## `{m}` — Yokan's own, {n} functions",
        "own_note": "Python has no module of this name, so there is nothing to "
                    "measure against — Yokan has all of them.",
        "cmp_head": "## `{m}` — {n} of Python's {t}",
        "borrowed": "Python's name, Yokan's own meaning",
        "shape": "a {theirs} in Python, a {ours} here",
        "extra": "not in Python's module",
        "bi_head": "## The builtins — {n} of the {t} probed",
        "bi_note": "Nothing declares these, so each one was written into a handler the "
                   "way an app would write it and run past the translator. The note is "
                   "the refusal's own words. Some that are refused as a value are taken "
                   "in a `for`, and the note says so.",
    },
    "ja": {
        "title": "CPython {v} に対する対応状況",
        "made": "`tools/stdlib_coverage.py` が `yokan_gate.py` のマニフェストと"
                "翻訳器のテーブルから生成しています。手で編集しないでください。",
        "yes": "✓",
        "no": "—",
        "cols": "| 名前 | Yokan | 備考 |",
        "own_head": "## `{m}` — Yokan 独自、{n} 個",
        "own_note": "Python に同じ名前のモジュールはないので、比べる相手がありません。"
                    "以下はすべて Yokan にあります。",
        "cmp_head": "## `{m}` — Python の {t} 個のうち {n} 個",
        "borrowed": "Python の名前で、意味は Yokan 独自",
        "shape": "Python では{theirs}、ここでは{ours}",
        "extra": "Python のモジュールにはない",
        "bi_head": "## 組み込み関数 — 調べた {t} 個のうち {n} 個",
        "bi_note": "組み込み関数はどこにも宣言されていないので、アプリが書くとおりに"
                   "ハンドラへ書いて翻訳器に通しました。備考は拒否の文言そのまま（英語）です。"
                   "値としては断られても `for` でなら通るものがあり、それも備考に出ます。",
    },
}

KIND = {"en": {"call": "call", "value": "value"},
        "ja": {"call": "関数", "value": "値"}}


def builtins_section(t, lang) -> list:
    """One row per builtin: taken, or the refusal's own sentence. A
    sentence many of them share is carried once, under the table,
    rather than down the whole note column."""
    import re

    probed = probe_builtins()
    ok = [n for n, msg in probed.items() if msg is None]
    why = {}
    for n, msg in probed.items():
        if msg is None:
            continue
        m = re.match(r"^`[^`]*`\s*(.*)$", msg, re.S)
        why[n] = (m.group(1) if m else msg).strip().removeprefix("is ")
    # only a long sentence many rows share is worth a footnote;
    # a short one reads better where it belongs
    shared = sorted({w for w in why.values()
                     if list(why.values()).count(w) > 3 and len(w) > 60})
    marks = {w: f"[{i + 1}]" for i, w in enumerate(shared)}
    lines = [
        t["bi_head"].format(n=len(ok), t=len(probed)),
        "",
        t["bi_note"],
        "",
        t["cols"],
        "|---|---|---|",
    ]
    for n in sorted(probed):
        if n not in why:
            lines.append(f"| `{n}` | {t['yes']} | |")
        else:
            lines.append(f"| `{n}` | {t['no']} | {marks.get(why[n], why[n])} |")
    lines.append("")
    for w, mark in marks.items():
        lines += [f"{mark} {w}", ""]
    return lines


def report(names, lang="en") -> str:
    t = PHRASES[lang]
    kind = KIND[lang]
    have = {}
    for _cls, mod, _layer, rows in Translator.STDLIB:
        if mod is None:
            continue
        have[mod] = {
            py: ("value" if "const" in flags else "call", "cpython" in flags)
            for py, _fn, _params, _r, _ru, *flags in rows
            if py
        }
    have.update(carried())
    body = []
    for mod in names:
        ours = have[mod]
        try:
            theirs = members(importlib.import_module(mod))
        except ImportError:
            theirs = None
        if theirs is None:
            body += [
                t["own_head"].format(m=mod, n=len(ours)),
                "",
                t["own_note"],
                "",
                "- " + ", ".join(f"`{n}`" for n in sorted(ours)),
                "",
            ]
            continue
        # A name in common is not the same function: the manifest's
        # `cpython` flag is what says a row answers what Python's
        # member of that name answers.
        covered = {n for n in ours if n in theirs and ours[n][1]}
        body += [
            t["cmp_head"].format(m=mod, n=len(covered), t=len(theirs)),
            "",
            t["cols"],
            "|---|---|---|",
        ]
        for n in sorted(set(theirs) | set(ours)):
            if n not in ours:
                body.append(f"| `{n}` | {t['no']} | |")
                continue
            note = ""
            if n not in theirs:
                note = t["extra"]
            elif n not in covered:
                note = t["borrowed"]
            elif theirs[n] != ours[n][0]:
                note = t["shape"].format(theirs=kind[theirs[n]], ours=kind[ours[n][0]])
            body.append(f"| `{n}` | {t['yes']} | {note} |")
        body.append("")
    lines = [
        "# " + t["title"].format(v=sys.version.split()[0]),
        "",
        t["made"],
        "",
    ]
    return "\n".join(lines + builtins_section(t, lang) + body) + "\n"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("modules", nargs="*", help="modules to report on (default: all)")
    ap.add_argument("-o", "--out", help="write here instead of stdout")
    ap.add_argument("--lang", default="en", choices=sorted(PHRASES),
                    help="the language to write in (default: en)")
    args = ap.parse_args()
    known = [m for _c, m, _l, _r in Translator.STDLIB if m] + list(carried())
    names = args.modules or known
    for n in names:
        if n not in known:
            sys.exit(f"`{n}` is not in the manifest — have {', '.join(known)}")
    text = report(names, args.lang)
    if args.out:
        open(args.out, "w").write(text)
        print(f"{args.out}: {len(names)} modules")
    else:
        sys.stdout.write(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
