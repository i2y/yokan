# /// script
# requires-python = ">=3.14"
# ///
"""The About panel: the app's identity, and three Links out to the
project. A Link is a line of text that opens its `url` in the
browser when clicked (accent-colored, underlined, a pointer cursor);
a headless run accepts the click and does nothing, since opening a
browser is not app state — `dump` never moves because of one. The
button copies the source URL to the clipboard with `clipboard.set_text`
and flips the status line to confirm it.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import button, column, link, run, store, text  # noqa: E402
from yokan import clipboard  # noqa: E402


@store
class About:
    status: str = ""

    def copy_link(self) -> None:
        clipboard.set_text("https://github.com/i2y/yokan")
        self.status = "copied"


def view():
    with column(spacing=8, padding=14):
        text("Yokan", size=28)
        text("version 0.1.2")
        link("Website", "https://i2y.github.io/yokan/")
        link("Source", "https://github.com/i2y/yokan")
        link("Docs", "https://i2y.github.io/yokan/tour/")
        button("copy link", on_click=About.copy_link)
        text(f"status: {About.status}")


if __name__ == "__main__":
    run(view, title="about")
