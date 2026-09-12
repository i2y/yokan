"""The counter demo, tested the way an app's own suite tests it.

Three shapes, and all three are what a Yokan app's tests look like: a
script asserted on, a screen recorded as a snapshot, and the gate run
from inside the suite. `yokan init` writes the first of them beside a
new app.

The `app` and `run` fixtures come from the plugin on the wheel. `app`
imports `counter.py` without running it — its `run(...)` is under
`__main__` — and `run` drives it with no window.
"""


def test_clicking_counts(app, run):
    t = run(app, "click:+1,click:+1,click:+10")
    assert "count: 12" in t
    assert "count: 0" in t.before
    assert "count: 12" in t.after


def test_typing_greets(app, run):
    assert "hello, Momo" in run(app, "input:Momo")


def test_reset_goes_back(app, run):
    t = run(app, "click:+10,dump,click:reset")
    assert "count: 10" in t.dumps[0]
    assert "count: 0" in t.after


def test_a_screen_reader_hears_the_count(app, run):
    # The accessibility tree is a kernel output, so a test can assert
    # on what a platform adapter would be handed.
    t = run(app, "click:+1,a11y")
    assert 'label "count: 1"' in t.a11y
    assert 'button "+1"' in t.a11y


def test_the_screen_is_what_it_was(app, run, snapshot):
    # The whole transcript, recorded. A diff in a review reads as the
    # screen changing; `--yokan-update` records a new one.
    snapshot(run(app, "click:+1,click:+10"))


def test_the_compiled_run_agrees(gate):
    # The gate, inside the suite: the same script through the
    # interpreted and the compiled run, byte-compared. It compiles, so
    # a suite keeps one of these rather than one per test.
    gate("click:+1,click:+10,click:reset")
