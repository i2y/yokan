"""A format spec the two runs would answer differently is refused.

A hole's spec is CPython's mini-language in the interpreted run and a
Rust formatter written to answer the same in the compiled one. Where
Python itself rejects a spec, `check` says so with Python's reason;
where the formatter does not answer as CPython does (a format code it
does not have, a precision with no type on a float, a `0` on a str),
`check` refuses it rather than let the two runs print two things.
"""
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
GATE = os.path.join(HERE, "..", "..", "yokan_gate.py")

APP = '''from yokan import State, column, run, text

name: State[str] = State("ada")


def go() -> None:
    n = 255
    x = 2.5
    s = "ab"
    {body}


def view():
    with column():
        text({hole})


if __name__ == "__main__":
    run(view, on_start=go)
'''

CASES = {
    "a hex int": ('print(f"{n:04x}")', "the `x` format code is not in the dialect yet"),
    "an int with a precision": ('print(f"{n:.2}")', "an int's spec takes no precision"),
    "a float as `d`": ('print(f"{x:d}")', "`d` is not a format code for a float"),
    "a float with a precision and no type": ('print(f"{x:.3}")', "the way `g` does"),
    "a float grouped with no type": ('print(f"{x:,}")', "groups the digits of a float's repr"),
    "a str with a sign": ('print(f"{s:+5}")', "a str's spec takes no sign"),
    "a str aligned after its sign": ('print(f"{s:=5}")', "`=` aligns after a sign"),
    "a str padded with `0`": ('print(f"{s:05}")', "pads it on the right in Python"),
    "a fill and a `0`": ('print(f"{n:x<05}")', "names a fill and also `0`"),
    "the alternate form": ('print(f"{n:#d}")', "`#` in a format spec is not in the dialect yet"),
    "`_` between digits": ('print(f"{n:_}")', "`_` as a digit separator is not in the dialect yet"),
    "a spec Python cannot read": ('print(f"{n:5.}")', "is not a format spec the dialect reads"),
}


def check(tmp_path, body: str, hole: str = '"x"') -> str:
    app = tmp_path / "app.py"
    app.write_text(APP.replace("{body}", body).replace("{hole}", hole), encoding="utf-8")
    r = subprocess.run(
        [sys.executable, GATE, "check", str(app)],
        capture_output=True, text=True, encoding="utf-8",
    )
    assert r.returncode != 0, f"accepted, and should not have been: {body} / {hole}"
    return (r.stdout or "") + (r.stderr or "")


def test_each_spec_edge_is_refused_with_its_reason(tmp_path):
    wrong = []
    for name, (body, said) in CASES.items():
        case = tmp_path / name.replace(" ", "_").replace("`", "")
        case.mkdir()
        out = check(case, body)
        if said not in out:
            wrong.append(f"{name}: expected {said!r} in\n{out}")
    assert not wrong, "\n\n".join(wrong)


def test_a_view_hole_is_held_to_the_same_rule(tmp_path):
    # `.2f` in a view goes to the engine's own fixed-decimal formatting,
    # and is held to the same spec rule on the way.
    out = check(tmp_path, "print(n)", 'f"{name():.2f}"')
    assert "`f` is not a format code for a str" in out
