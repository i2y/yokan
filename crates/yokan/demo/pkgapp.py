# /// script
# requires-python = ">=3.14"
# ///
"""An app built from a package written in the dialect.

`yokanui` is an ordinary installed Python package that carries a
`py.yokan` marker beside its `__init__.py`. The translator reads its
modules the way it reads the app's own files and compiles them into
the same binary, so there is no library at run time and nothing to
ship beside the app.

Its names are emitted under names derived from its modules, so the
package and the app can both have a `badge` and neither has to know
about the other. Here they do.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import State, button, column, row, run, text  # noqa: E402
from yokanui import badge, panel, pill, plain  # noqa: E402

seen: State[int] = State(0)
folded: State[str] = State("")


def badge_of(n: int) -> str:
    """The app's own `badge`-ish name, which the package's `badge`
    does not disturb."""
    return "many" if n > 2 else "a few"


def look() -> None:
    seen.set(seen() + 1)


def fold() -> None:
    # The package declares the Rust crate this calls, and the app
    # never mentions it: `[tool.yokan.crates]` beside the marker
    # merges into the app's.
    folded.set(plain("ようかん"))


def view() -> None:
    with column(spacing=10, padding=14):
        panel("from a package", f"looked {seen()} times")
        with row(spacing=8):
            badge(badge_of(seen()))
            pill("loud" if seen() > 2 else "quiet", seen() > 2)
        text(f"folded: {folded()}")
        with row(spacing=8):
            button("look", on_click=look)
            button("fold", on_click=fold)


if __name__ == "__main__":
    run(view, title="pkgapp")
