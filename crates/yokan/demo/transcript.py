# /// script
# requires-python = ">=3.14"
# ///
"""A transcript, read top to bottom: the turns of a conversation with
a coding agent, prose and code, in a `scroll_view` over a `column`.
The rows are not one height — a turn is a paragraph or a block of
code — so this is the shape for them, not the virtualized
`list_view`, whose rows share one `item_height`. A session is
hundreds of turns, and a column of hundreds of texts scrolls fine.

Each turn is a value class; the view walks the list and picks the
shape by its kind: a person's turn as a pill, the agent's prose
wrapped, a code block in `mono` on a panel, and a tool call as one
dim line. `copy` puts a block on the clipboard, which a script
checks with a paste into the field.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import (  # noqa: E402
    State,
    button,
    clipboard,
    column,
    row,
    run,
    scroll_view,
    store,
    style,
    text,
    text_field,
    value,
)

dim = style(size=12, color="#8a8f98")


@value
class Turn:
    kind: str  # "user", "agent", "code" or "tool"
    body: str


@store
class Session:
    turns: list[Turn] = [
        Turn("user", "Make the counter demo save its count between runs."),
        Turn(
            "agent",
            "The count lives in a State, so the place to write it is the "
            "handler that changes it, and the place to read it back is "
            "on_start. I will keep it in the app's own directory.",
        ),
        Turn("tool", "read  demo/counter.py"),
        Turn(
            "code",
            "def boot():\n"
            "    count.set(strings.to_int(fs.read_text_or(PATH, \"0\"), 0))\n"
            "\n"
            "def bump():\n"
            "    count.set(count() + 1)\n"
            "    fs.write_text(PATH, f\"{count()}\")",
        ),
        Turn("tool", "write demo/counter.py"),
        Turn("tool", "run   yokan gate demo/counter.py --script click:+1,dump"),
        Turn("agent", "The gate is green: the count is written on every click and read back at start."),
        Turn("user", "Where does the file go?"),
        Turn(
            "agent",
            "fs.app_dir names the directory the platform keeps for an app, "
            "so it is Application Support on macOS and XDG_DATA_HOME on Linux.",
        ),
        Turn("code", "PATH = fs.app_dir(\"counter\") + \"/count.txt\""),
    ]
    extra: int = 0

    def more(self) -> None:
        # A longer session: the same turns again, numbered, so a
        # script can scroll a screen that is taller than the window.
        self.extra = self.extra + 1
        self.turns = self.turns + [
            Turn("user", f"And one more thing ({self.extra})."),
            Turn("agent", f"Done, as turn {self.extra}: the change is in, and the gate still passes."),
        ]


pasted: State[str] = State("")


def copy_code():
    for t in Session.turns:
        if t.kind == "code":
            clipboard.set_text(t.body)


def paste():
    pasted.set(clipboard.get_text())


def view():
    with column(spacing=8, padding=12):
        text(f"transcript: {len(Session.turns)} turns", size=16, bold=True)
        with scroll_view(height=360.0):
            with column(spacing=8):
                for t in Session.turns:
                    if t.kind == "user":
                        text(t.body, background="#313244", padding=8.0, border_radius=8.0)
                    if t.kind == "agent":
                        text(t.body)
                    if t.kind == "code":
                        text(t.body, mono=True, size=12, background="#181825", padding=8.0, border_radius=6.0)
                    if t.kind == "tool":
                        text(t.body, **dim)
        with row(spacing=6):
            button("more", on_click=Session.more)
            button("copy code", on_click=copy_code)
            button("paste", on_click=paste)
            text_field(pasted(), placeholder="pasted here", on_change=pasted.set)


if __name__ == "__main__":
    run(view, title="transcript", width=560, height=520)
