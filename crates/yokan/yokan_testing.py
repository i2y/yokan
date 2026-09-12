"""Testing a Yokan app: the headless run as a test's subject.

An app is a Python module, so its tests are Python tests. What this
module adds is the three things a UI test needs and a plain assert
does not give: a value that carries the run in pieces, a snapshot of
a screen that a review can read, and a way to say "and the compiled
run agrees" without leaving the test suite.

Two ways in, and they are the same machinery:

    from yokan_testing import run_script

    def test_counts():
        t = run_script(app.view, "click:+1,click:+1")
        assert "count: 2" in t

    def test_counts(app, run):          # the fixtures, no import
        assert "count: 2" in run(app, "click:+1,click:+1")

The fixtures come from the pytest plugin this module registers, and
they bring two flags with them: `--yokan-update` rewrites snapshots,
and `--yokan-gate` makes every `run` build the app and compare the
two runs as well, which is `yokan gate` inside the test suite.
"""

import difflib
import importlib.util
import os
import subprocess
import sys

__all__ = ["Step", "Transcript", "run_script", "import_app", "reset_app"]


class Step:
    """One step of a script, and what it printed.

    A step that prints nothing carries an empty text, so the steps are
    the script's own, in order: `t.steps[2]` is the third step a test
    wrote, whatever it did.
    """

    __slots__ = ("step", "text")

    def __init__(self, step: str, text: str) -> None:
        self.step = step
        self.text = text

    @property
    def kind(self) -> str:
        """The verb alone: `click` for `click@2:ok`, `a11y` for `a11y`."""
        return self.step.split(":", 1)[0].split("@", 1)[0]

    def __str__(self) -> str:
        return self.text

    def __contains__(self, needle: str) -> bool:
        return needle in self.text

    def __repr__(self) -> str:
        return f"Step({self.step!r}, {self.text!r})"

    def __eq__(self, other) -> bool:
        if isinstance(other, Step):
            return (self.step, self.text) == (other.step, other.text)
        return self.text == other

    def __hash__(self) -> int:
        return hash((self.step, self.text))


class Transcript(str):
    """What a headless run answers.

    It IS the string it always was — `in`, `==`, `str()`, `splitlines`
    and everything else `str` does, unchanged — with the run's pieces
    on it: the dump before the steps, the dump after, and each step's
    own output.
    """

    def __new__(cls, text: str, before: str = "", after: str = "", steps=()):
        self = super().__new__(cls, text)
        self._before = before
        self._after = after
        self._steps = tuple(Step(s, o) for s, o in steps)
        return self

    @property
    def before(self) -> str:
        """The screen as the app opened, before any step ran."""
        return self._before

    @property
    def after(self) -> str:
        """The screen after the last step."""
        return self._after

    @property
    def steps(self) -> tuple:
        """Every step of the script, in order, beside what it printed."""
        return self._steps

    @property
    def dumps(self) -> tuple:
        """The screens a `dump` step asked for, in order. The opening
        and closing screens are `before` and `after`, not these."""
        return tuple(s.text.rstrip("\n") for s in self._steps if s.kind == "dump")

    @property
    def a11y(self) -> str:
        """The accessibility tree the last `a11y` step printed, or ""
        when the script never asked for one."""
        got = [s.text.rstrip("\n") for s in self._steps if s.kind == "a11y"]
        return got[-1] if got else ""

    @property
    def mem(self):
        """How many objects the last `mem` step counted, or None."""
        got = [s.text for s in self._steps if s.kind == "mem"]
        if not got:
            return None
        return int(got[-1].split(":", 1)[1])

    def __repr__(self) -> str:
        return f"Transcript({str.__repr__(self)}, {len(self._steps)} steps)"


def import_app(path: str):
    """The app module, imported without running it.

    An app's `run(...)` sits under `if __name__ == "__main__"`, so a
    plain import loads the view and the state and opens no window —
    the same import the gate's interpreted run does.
    """
    path = os.path.abspath(path)
    if not os.path.isfile(path):
        raise FileNotFoundError(path)
    here = os.path.dirname(path)
    if here not in sys.path:
        sys.path.insert(0, here)
    name = "yokan_app_" + os.path.splitext(os.path.basename(path))[0]
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def reset_app(app) -> None:
    """Run the app's module body again, in place.

    An app's state is module-level, so two runs in one test would
    continue from each other unless something puts it back. Running
    the body again rebinds every `State` and every `@store` in the
    module the test is holding, so each run starts where the app
    starts and the test's own `app` is still the live one.
    """
    spec = getattr(app, "__spec__", None)
    if spec is not None and spec.loader is not None:
        spec.loader.exec_module(app)


def _view_of(app):
    """The view a caller named: a module's `view`, or the function
    itself. Both spellings read well at a call site, so both are
    taken."""
    view = getattr(app, "view", app)
    if not callable(view):
        raise TypeError(
            "run takes the app module or its view function — "
            f"`{app!r}` is neither"
        )
    return view


def run_script(app, script: str = "", state=None, on_start=None) -> Transcript:
    """Run the app against a script with no window and answer the
    screen as text. `app` is the module or its view function."""
    import yokan  # noqa: PLC0415 — the extension, imported where it is used

    return yokan.headless(_view_of(app), state, script, on_start)


# ---------------------------------------------------------------- #
# The pytest plugin. Guarded, because this module is importable on
# its own and a plain script should not need pytest to use it.
# ---------------------------------------------------------------- #

try:
    import pytest
except ModuleNotFoundError:  # pragma: no cover - the import-only path
    pytest = None


if pytest is not None:

    def pytest_addoption(parser):
        group = parser.getgroup("yokan")
        group.addoption(
            "--yokan-update",
            action="store_true",
            default=False,
            help="rewrite the snapshot files this run compares against",
        )
        group.addoption(
            "--yokan-gate",
            action="store_true",
            default=False,
            help="also build the app and compare the two runs for every script "
                 "a test runs (slow; this is `yokan gate` inside the suite)",
        )
        parser.addini(
            "yokan_app",
            "the app a test's `app` fixture imports, relative to the rootdir",
            default="",
        )

    def _app_path(request) -> str:
        """Where the app is. The ini key says it outright; otherwise a
        test named `test_<stem>.py` names `<stem>.py`, looked for
        beside the test and one directory up — which is the layout
        `yokan init` writes."""
        named = request.config.getini("yokan_app")
        if named:
            return os.path.join(str(request.config.rootpath), named)
        here = os.path.dirname(str(request.path))
        stem = os.path.splitext(os.path.basename(str(request.path)))[0]
        if not stem.startswith("test_"):
            raise pytest.UsageError(
                f"`{stem}.py` does not name an app — name the test "
                "`test_<app>.py`, or set `yokan_app` in the pytest ini"
            )
        want = stem[len("test_"):] + ".py"
        for d in (here, os.path.dirname(here), str(request.config.rootpath)):
            p = os.path.join(d, want)
            if os.path.isfile(p):
                return p
        raise pytest.UsageError(
            f"no `{want}` beside `{stem}.py` or above it — "
            "set `yokan_app` in the pytest ini to say where the app is"
        )

    @pytest.fixture
    def app(request):
        """The app module, imported without running it."""
        return import_app(_app_path(request))

    @pytest.fixture
    def run(request):
        """Run a script against the app and answer the `Transcript`.

        Under `--yokan-gate` the same script also goes through the
        gate, so a suite written with this fixture becomes the proof
        that the compiled run agrees.
        """

        def _run(app, script: str = "", state=None, on_start=None) -> Transcript:
            # Every run starts where the app starts, so a test can
            # drive it more than once and read the same numbers a
            # person would.
            reset_app(app)
            out = run_script(app, script, state, on_start)
            if request.config.getoption("--yokan-gate"):
                _gate(_app_path(request), script)
            return out

        return _run

    @pytest.fixture
    def gate(request):
        """Build the app and compare the two runs for one script.

        This is what `yokan gate` does, failing the test on a
        difference and printing what diverged. It compiles, so a suite
        keeps one or two of these rather than one per test.
        """

        def _fn(script: str = "") -> None:
            _gate(_app_path(request), script)

        return _fn

    @pytest.fixture
    def snapshot(request):
        """Compare a transcript against the screen this test recorded.

        The file is `__snapshots__/<test>.dump` beside the test, and
        it holds the transcript verbatim — so a diff in a review reads
        as the screen changing. `--yokan-update` writes it.
        """

        def _fn(value, name: str | None = None) -> None:
            here = os.path.dirname(str(request.path))
            stem = name or request.node.name
            path = os.path.join(here, "__snapshots__", _safe(stem) + ".dump")
            text = str(value)
            if not text.endswith("\n"):
                text += "\n"
            if request.config.getoption("--yokan-update"):
                os.makedirs(os.path.dirname(path), exist_ok=True)
                with open(path, "w", encoding="utf-8") as f:
                    f.write(text)
                return
            if not os.path.isfile(path):
                pytest.fail(
                    f"no snapshot at {os.path.relpath(path)} yet — run the suite "
                    "once with --yokan-update to record this screen",
                    pytrace=False,
                )
            with open(path, encoding="utf-8") as f:
                want = f.read()
            if want != text:
                diff = "".join(
                    difflib.unified_diff(
                        want.splitlines(keepends=True),
                        text.splitlines(keepends=True),
                        fromfile=os.path.relpath(path),
                        tofile="this run",
                    )
                )
                pytest.fail(
                    f"the screen changed:\n{diff}\n"
                    "--yokan-update records the new one, if it is the one you meant",
                    pytrace=False,
                )

        return _fn


def _safe(name: str) -> str:
    """A test's name as a file name. Parametrised tests carry their
    ids in brackets, which are fine on every platform but read badly
    in a directory listing."""
    out = "".join(c if c.isalnum() or c in "._-" else "_" for c in name)
    return out.strip("_") or "snapshot"


def _gate(app_path: str, script: str) -> None:
    """`yokan gate` as a test: the same command, in its own process,
    failing the test with what it printed."""
    import yokan_gate  # noqa: PLC0415 — the command, beside this module

    cmd = [sys.executable, yokan_gate.__file__, "gate", app_path]
    if script:
        cmd += ["--script", script]
    r = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        out = (r.stdout or "") + (r.stderr or "")
        if pytest is not None:
            pytest.fail(f"the two runs do not agree:\n{out}", pytrace=False)
        raise AssertionError(f"the two runs do not agree:\n{out}")
