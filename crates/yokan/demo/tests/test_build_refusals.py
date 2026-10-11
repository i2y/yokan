"""What `check` refuses so that the compiled build never stops on it.

Each case is a shape `check` once passed and the native build then
failed on; what the compiled side can now take is gated in
`build_edges.py`, and these are the edges beside it, each refused at
`check` with its reason.
"""
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
GATE = os.path.join(HERE, "..", "..", "yokan_gate.py")

APP = '''from yokan import State, column, run, text

level: State[float] = State(2.5)
count: State[int] = State(3)


def g(x: int | None) -> int:
    if x is None:
        return 0
    return x


{top}


def go() -> None:
    x = level()
    n = count()
    {body}


def view():
    with column():
        {view}


if __name__ == "__main__":
    run(view, on_start=go)
'''

CASES = {
    "a call in a view's condition": (
        "", "print(n)", 'if abs(level()) > 1.0:\n            text("big")',
        "calls something, and a view's `if` reads states, fields and literals only",
    ),
    "a float against an int state in a view's condition": (
        "", "print(n)", 'if level() > count():\n            text("over")',
        "compare in a handler and keep the answer in a State",
    ),
    "a helper that reads a State": (
        "def doubled() -> int:\n    return count() * 2", "print(doubled())", 'text("x")',
        "a helper reads only what it is handed",
    ),
    "an int literal no float equals": (
        "", "print(x < 9007199254740993)", 'text("x")',
        "no float equals `9007199254740993`",
    ),
    "a bool against a number": (
        "", "print(True < x)", 'text("x")',
        "a bool compared with a number counts as 1 or 0 in Python",
    ),
    "max of an int and a float": (
        "", "print(max(x, n))", 'text("x")',
        "`max` of an int and a float answers the one it picks as it is",
    ),
    "None for an optional parameter where no line fits": (
        "def h(n: int) -> int:\n    return g(None) + n", "print(h(n))", 'text("x")',
        "hold the value in a local first",
    ),
}


def check(tmp_path, top: str, body: str, view: str) -> str:
    app = tmp_path / "app.py"
    app.write_text(APP.replace("{top}", top).replace("{body}", body).replace("{view}", view), encoding="utf-8")
    r = subprocess.run(
        [sys.executable, GATE, "check", str(app)],
        capture_output=True, text=True, encoding="utf-8",
    )
    assert r.returncode != 0, f"accepted, and should not have been: {body} / {view}"
    return (r.stdout or "") + (r.stderr or "")


def test_each_build_edge_is_refused_at_check(tmp_path):
    wrong = []
    for name, (top, body, view, said) in CASES.items():
        case = tmp_path / name.replace(" ", "_").replace("'", "")
        case.mkdir()
        out = check(case, top, body, view)
        if said not in out:
            wrong.append(f"{name}: expected {said!r} in\n{out}")
    assert not wrong, "\n\n".join(wrong)
