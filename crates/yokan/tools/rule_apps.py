#!/usr/bin/env python3
"""Write the app that puts a generated rule module through the dialect.

    uv run tools/rule_apps.py demo/tests/rules/<module> ...   write app.py
    uv run tools/rule_apps.py --check demo/tests/rules/*      fail when one is stale

A directory under `demo/tests/rules/` holds what a code generator wrote
for one business rule, as it wrote it:

    <module>.py              the Python module (rulec gen, python/)
    <module>.api.json        its interface (rulec api --format json)
    vectors/                 the inputs and the answers (rulec gen, vectors/):
      <module>.expected.jsonl          one record per vector
      <module>.traces.jsonl            a state machine's call sequences
      <module>.traces.expected.jsonl   and their answers

This writes `app.py` beside them: at startup it calls the rule's function
and its traced twin with every vector's input, and prints what came out
and the rows that matched, one line each. A state machine's sequences are
walked too, each call starting from the state the call before it
answered. `expected_lines` renders what the app has to print from the
vectors' own answers, so a test can hold CPython's run to them and the
gate can hold the compiled run to CPython's.

To bring in another rule:

    rulec gen <rule>.rule --out <tmp>
    rulec api <rule>.rule --format json > demo/tests/rules/<module>/<module>.api.json
    copy <tmp>/python/<module>.py and the vectors files above into the directory
    uv run tools/rule_apps.py demo/tests/rules/<module>
"""
import ast
import datetime
import json
import os
import sys


def _load(path: str) -> list:
    with open(path, encoding="utf-8") as f:
        return [json.loads(line) for line in f if line.strip()]


class Rule:
    """One rule directory: the module, its interface and its vectors."""

    def __init__(self, root: str):
        self.root = root
        names = [f for f in os.listdir(root) if f.endswith(".api.json")]
        if len(names) != 1:
            raise SystemExit(f"{root}: expected one <module>.api.json, found {len(names)}")
        with open(os.path.join(root, names[0]), encoding="utf-8") as f:
            self.api = json.load(f)
        p = self.api["python"]
        self.module = p["module"]
        self.function = p["function"]
        self.traced = p["traced"]
        self.params = p["params"]
        self.outputs = p["outputs"]
        self.returns = p["returns"]
        self.enums = {e["alias"]: e for e in (p.get("enums") or [])}
        # the rule's own name for an enum value -> the member's name
        self.member = {
            alias: {v["name"]: v["alias"] for v in e["values"]} for alias, e in self.enums.items()
        }
        self.element = next(
            (prm["type"][len("list["):-1] for prm in self.params if prm.get("elements")), None
        )
        self.vectors = os.path.join(root, "vectors")
        with open(os.path.join(root, f"{self.module}.py"), encoding="utf-8") as f:
            top = ast.parse(f.read()).body
        own = set()
        for st in top:
            if isinstance(st, (ast.ClassDef, ast.FunctionDef)):
                own.add(st.name)
            elif isinstance(st, ast.Assign):
                own |= {t.id for t in st.targets if isinstance(t, ast.Name)}
        # The module's own names come first: when it declares one the
        # app would import from yokan (`State`, say), the app reaches
        # yokan through `ui.` instead.
        self.through_ui = bool(own & {"State", "column", "run", "text"})

    # ---- types and values -----------------------------------------------
    def plain(self, t: str) -> bool:
        return t in self.enums or t in ("bool", "str", "int") or t.endswith(" | None") or t.startswith("list[")

    def held_as(self, t: str) -> str:
        """The annotation the app holds an input as: a unit is its int."""
        if t.endswith(" | None"):
            return f"{self.held_as(t[:-7])} | None"
        return t if self.plain(t) else "int"

    def literal(self, t: str, v, fields=None) -> str:
        if t.endswith(" | None"):
            return "None" if v is None else self.literal(t[:-7], v)
        if t in self.enums:
            return f"{t}.{self.member[t][v]}"
        if t == "bool":
            return "True" if v else "False"
        if t == "str":
            return json.dumps(v, ensure_ascii=False)
        if t.startswith("list["):
            el = t[5:-1]
            return "[" + ", ".join(
                f"{el}({', '.join(self.field(f, x[f['name']]) for f in fields)})" for x in v
            ) + "]"
        if isinstance(v, str):
            # A calendar date travels in its civil spelling; the module
            # takes the day it is, counted from 1970-01-01.
            return str((datetime.date.fromisoformat(v) - datetime.date(1970, 1, 1)).days)
        return str(v)

    def field(self, f, v) -> str:
        """An element's field as the element's class declares it: a unit
        is made from the int."""
        lit = self.literal(f["type"], v)
        return lit if self.plain(f["type"]) else f"{f['type']}({lit})"

    def handed(self, prm, var: str) -> str:
        t = prm["type"]
        return var if self.plain(t) else f"{t}({var})"

    # ---- what is printed --------------------------------------------------
    def shown(self, var: str) -> list:
        def one(o, e):
            return f"{e}.value" if o["type"] in self.enums else e

        if self.returns == "Output":
            return [one(o, f"{var}.{o['alias']}") for o in self.outputs]
        (o,) = self.outputs
        return [one(o, var)]

    def text(self, o, v) -> str:
        if o["type"] == "bool":
            return "True" if v else "False"
        return str(v)

    def record_lines(self, rec) -> list:
        vals = [self.text(o, rec["observed"][o["name"]]) for o in self.outputs]
        out = [" ".join(vals), " ".join(vals + [str(len(rec["trace"]))])]
        for r in rec["trace"]:
            out.append(" ".join([r["table"], str(r["row"])] + ([r["label"]] if r.get("label") else [])))
        return out

    def calls(self, names: list, pad: str) -> list:
        args = ", ".join(self.handed(prm, n) for prm, n in zip(self.params, names))
        return [
            f"{pad}out = {self.function}({args})",
            f"{pad}print({', '.join(self.shown('out'))})",
            f"{pad}got, rows = {self.traced}({args})",
            f"{pad}print({', '.join(self.shown('got') + ['len(rows)'])})",
            f"{pad}for r in rows:",
            f'{pad}    if r.label != "":',
            f"{pad}        print(r.table, r.row, r.label)",
            f"{pad}    else:",
            f"{pad}        print(r.table, r.row)",
        ]

    # ---- the app ----------------------------------------------------------
    def records(self) -> list:
        return _load(os.path.join(self.vectors, f"{self.module}.expected.jsonl"))

    def machine(self):
        """(the call sequences, their answers) for a state machine, else None."""
        path = os.path.join(self.vectors, f"{self.module}.traces.jsonl")
        if not os.path.exists(path):
            return None
        return _load(path), _load(os.path.join(self.vectors, f"{self.module}.traces.expected.jsonl"))

    def app(self) -> str:
        ui = (lambda n: f"ui.{n}") if self.through_ui else (lambda n: n)
        recs = self.records()
        names = [f"a{i}" for i in range(len(self.params))]
        imports = {self.function, self.traced, *self.enums}
        if self.element:
            imports.add(self.element)
        imports |= {prm["type"] for prm in self.params if not self.plain(prm["type"])}
        imports |= {
            f["type"] for prm in self.params for f in (prm.get("elements") or [])
            if not self.plain(f["type"])
        }
        seq = self.machine()
        if seq is not None:
            imports |= {"INITIAL", "is_final"}
        anns = [self.held_as(prm["type"]) for prm in self.params]
        case_ty = anns[0] if len(anns) == 1 else f"tuple[{', '.join(anns)}]"
        out = [
            f'"""Every vector rulec wrote for `{self.api["rule"]}`, through `{self.function}` and',
            f"`{self.traced}`: each input is called both ways, and what came out and the",
            "rows that matched are printed, one line each.",
            "",
            "Written by tools/rule_apps.py from the vectors beside it.",
            '"""',
            "import yokan as ui" if self.through_ui else "from yokan import State, column, run, text",
            "",
            f"from {self.module} import {', '.join(sorted(imports, key=str.lower))}",
            "",
            "",
            f"cases: {ui('State')}[list[{case_ty}]] = {ui('State')}([",
        ]
        for rec in recs:
            vals = [self.literal(prm["type"], rec["in"].get(prm["name"]), prm.get("elements"))
                    for prm in self.params]
            out.append(f"    {vals[0] if len(vals) == 1 else '(' + ', '.join(vals) + ')'},")
        out.append("])")
        if seq is not None:
            calls, _answers = seq
            carry = calls[0]["carry"]
            cin = next(prm for prm in self.params if prm["name"] == carry["in"])
            cout = next(o for o in self.outputs if o["name"] == carry["out"])
            sty = cin["type"]
            rest = [prm for prm in self.params if prm["name"] != carry["in"]]
            first = f"{sty}.{self.enums[sty]['values'][0]['alias']}"
            out += [
                "",
                "# A state machine's call sequences: whether a call starts one, the state it",
                "# starts in (a placeholder where it carries on), then the other inputs.",
                f"steps: {ui('State')}[list[tuple[{', '.join(['bool', sty] + [self.held_as(p['type']) for p in rest])}]]] = {ui('State')}([",
            ]
            for rec in calls[1:]:
                start = rec["step"] == "start"
                vals = ["True" if start else "False", self.literal(sty, rec["state"]) if start else first]
                vals += [self.literal(p["type"], rec["in"].get(p["name"]), p.get("elements")) for p in rest]
                out.append(f"    ({', '.join(vals)}),")
            out.append("])")
        out += ["", "", "def check() -> None:"]
        out.append(f"    for {', '.join(names)} in cases():")
        out += self.calls(names, " " * 8)
        if seq is not None:
            walked, k = [], 0
            for prm in self.params:
                if prm["name"] == carry["in"]:
                    walked.append("st")
                else:
                    walked.append(f"b{k}")
                    k += 1
            out += [
                "    print(INITIAL.value)",
                f"    for s in {sty}:",
                "        if is_final(s):",
                "            print(s.value)",
                "    st = INITIAL",
                f"    for {', '.join(['start', 's0'] + [f'b{i}' for i in range(k)])} in steps():",
                "        if start:",
                "            st = s0",
            ]
            out += self.calls(walked, " " * 8)
            out.append(f"        st = out.{cout['alias']}")
        out += [
            "",
            "",
            "def view():",
            f"    with {ui('column')}():",
            f'        {ui("text")}("{self.module}: {len(recs)} vectors")',
            "",
            "",
            'if __name__ == "__main__":',
            f"    {ui('run')}(view, on_start=check)",
            "",
        ]
        return "\n".join(out)

    def expected_lines(self) -> list:
        """What the app has to print, from the vectors' own answers."""
        lines = []
        for rec in self.records():
            lines += self.record_lines(rec)
        seq = self.machine()
        if seq is not None:
            calls, answers = seq
            carry = calls[0]["carry"]
            sty = next(prm for prm in self.params if prm["name"] == carry["in"])["type"]
            head = answers[0]
            lines.append(head["initial"])
            lines += [v["name"] for v in self.enums[sty]["values"] if v["name"] in head["final"]]
            for rec in answers[1:]:
                lines += self.record_lines(rec)
        return lines


def main(argv) -> int:
    check = "--check" in argv
    stale = 0
    for root in [a for a in argv if a != "--check"]:
        rule = Rule(root)
        want = rule.app()
        path = os.path.join(root, "app.py")
        have = open(path, encoding="utf-8").read() if os.path.exists(path) else None
        if check:
            if have != want:
                print(f"stale: {path}")
                stale += 1
            continue
        if have != want:
            with open(path, "w", encoding="utf-8") as f:
                f.write(want)
            print(f"wrote {path} ({len(rule.records())} vectors)")
    return 1 if stale else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
