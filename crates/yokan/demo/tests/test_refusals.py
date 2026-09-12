"""`check` reports every refusal it can find, not the first one.

The reader is often an agent, for whom the round trip costs more than
the tenth of a second the check takes, so a file with four refusals
has to answer four times in one run. `refusals_fixture.py` beside this
file holds four, of four kinds, in four places.
"""
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
GATE = os.path.join(HERE, "..", "..", "yokan_gate.py")
FIXTURE = os.path.join(HERE, "refusals_fixture.py")


def check(path: str):
    r = subprocess.run(
        [sys.executable, GATE, "check", path],
        capture_output=True, text=True, encoding="utf-8",
    )
    return r.returncode, (r.stdout or "") + (r.stderr or "")


def test_four_refusals_come_back_together():
    code, out = check(FIXTURE)
    assert code != 0
    assert out.rstrip().endswith("4 refusals")


def test_each_one_names_its_own_line():
    _code, out = check(FIXTURE)
    # The path is printed relative to where the command ran, so the
    # file's own name is what to look for.
    stem = os.path.basename(FIXTURE)
    lines = [ln for ln in out.splitlines() if stem in ln.split(": ", 1)[0]]
    assert len(lines) == 4
    # In the file's order, which is the order a reader works in.
    at = [int(ln.split(":")[1]) for ln in lines]
    assert at == sorted(at)


def test_a_clean_app_says_nothing():
    code, out = check(os.path.join(HERE, "..", "counter.py"))
    assert code == 0
    assert out.strip() == ""
