"""Where typed Python stops being the dialect, and that it says why.

Each case is a module the typed demo's shapes almost take, and the
sentence `check` answers it with. What is accepted is gated by
`demo/typed.py` and the rule apps; these are the edges beside it, each
refused for a reason the two runs would otherwise disagree about.
"""
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
GATE = os.path.join(HERE, "..", "..", "yokan_gate.py")

HEAD = '''import enum
from typing import Callable, NamedTuple, NewType

Gram = NewType("Gram", int)


class Kind(enum.Enum):
    A = "a"


class Row(NamedTuple):
    name: str
    qty: int


class Plain(ValueError):
    """Python's own __init__."""


def g(x: int) -> int:
    return x + 1


def h(x: float) -> float:
    return x


'''

APP = '''from yokan import column, run, text
from mod import go


def view():
    with column():
        text("x")


if __name__ == "__main__":
    run(view, on_start=go)
'''

CASES = {
    "a bool into an int": ("print(g(True))", "a bool — Python hands a bool in as it is"),
    "an int into a float": ("print(h(1))", "Python keeps an int an int"),
    "isinstance on a float": (
        "x = h(1.5)\n    print(isinstance(x, float))",
        "a `float` slot can hold an int in Python",
    ),
    "isinstance against a NewType": (
        "x = Gram(3)\n    print(isinstance(x, Gram))",
        "`Gram` is a NewType, which has no class at run time",
    ),
    "isinstance on what a call answers": (
        "print(isinstance(g(1), int))",
        "`isinstance` here asks about a local",
    ),
    "a NewType holding a float": ("print(Gram(1.5))", "`Gram` is int, and `1.5` is float"),
    "an append after the list was handed on": (
        "xs: list[int] = []\n    ys = xs\n    xs.append(1)\n    print(len(ys))",
        "here it is handed on",
    ),
    "ordering two NamedTuples": (
        "b = Row('a', 1) < Row('b', 2)",
        "ordering two tuples compares their parts in turn",
    ),
    "the repr of a str": ("s = 'a'\n    print(repr(s))", "`repr` of a str is not in the dialect yet"),
    "walking a set": ("for k in {1, 2}:\n        print(k)", "a `set` iterates in an order"),
    "a keyword to an exception without an __init__": (
        "raise Plain('x', value='1')",
        "its `__init__` is Python's own",
    ),
    "a raise inside a try": (
        "try:\n        raise ValueError('x')\n    except ValueError:\n        print(1)",
        "a `raise` inside a `try` is not in the dialect",
    ),
}


def refusal(tmp_path, body: str) -> str:
    d = tmp_path
    (d / "mod.py").write_text(HEAD + "def go() -> None:\n    " + body + "\n", encoding="utf-8")
    (d / "app.py").write_text(APP, encoding="utf-8")
    r = subprocess.run(
        [sys.executable, GATE, "check", str(d / "app.py")],
        capture_output=True, text=True, encoding="utf-8",
    )
    assert r.returncode != 0, "accepted, and should not have been"
    return (r.stdout or "") + (r.stderr or "")


def test_each_edge_is_refused_with_its_reason(tmp_path):
    wrong = []
    for name, (body, said) in CASES.items():
        case = tmp_path / name.replace(" ", "_")
        case.mkdir()
        out = refusal(case, body)
        if said not in out:
            wrong.append(f"{name}: expected {said!r} in\n{out}")
    assert not wrong, "\n\n".join(wrong)
