# /// script
# requires-python = ">=3.14"
# ///
"""`yokan.fs` from the standard library: the interpreted and the
compiled app call the SAME implementation, so the gate arbitrates a
single truth (write 25 bytes, read them back). The rest of a file
app is here too — make a directory, append to a file, list what is
in it, remove one — and `fs.app_dir(name)` answers the directory
this app may keep its own files in, created if it is not there yet.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import button, column, list_view, row, run, State, text  # noqa: E402
from yokan import fs  # noqa: E402

DIR = "demo/.gate/fs_demo"
NOTE = "demo/.gate/fs_demo/note.txt"

content: State[str] = State("(not loaded)")
wrote: State[int] = State(0)
names: State[list[str]] = State([])
ready: State[bool] = State(False)


def save():
    fs.make_dir(DIR)
    wrote.set(fs.write_text(NOTE, "hello from one rust crate"))


def add_line():
    fs.append_text(NOTE, " (and again)")


def load():
    content.set(fs.read_text(NOTE))


def listing():
    names.set(fs.list_dir(DIR))


def clean():
    fs.remove(NOTE)
    names.set(fs.list_dir(DIR))


def data_dir():
    # the app's own directory, made on the way out
    ready.set(fs.exists(fs.app_dir("yokan-files-demo")))


def entry(i):
    return text(names()[i])


def view():
    with column(spacing=8, padding=12):
        text(f"content: {content()}")
        text(f"wrote: {wrote()} bytes")
        text(f"in {DIR}: {len(names())} file(s)")
        list_view(len(names()), entry, item_height=20.0, height=44.0)
        text(f"data dir ready: {ready()}")
        with row(spacing=6):
            button("save", on_click=save)
            button("append", on_click=add_line)
            button("load", on_click=load)
            button("list", on_click=listing)
        with row(spacing=6):
            button("remove", on_click=clean)
            button("data dir", on_click=data_dir)


if __name__ == "__main__":
    run(view, title="files")
