# /// script
# requires-python = ">=3.14"
# ///
"""`bytes` in the dialect, and the two modules that wait on it.

A `bytes` value is what a file, a response or a digest is made of.
The literal is Python's (`b"..."` with its escapes), and so are the
operations: `len`, indexing to a number, slicing, `+`, `.hex()`,
`bytes.fromhex`, `str.encode()` and `bytes.decode()`. A hole renders
one the way Python's `str(b)` does, `b'...'` and all.

`hashlib` and `base64` are Python's own modules over those bytes:
`hashlib.sha256(b).hexdigest()` and `base64.b64encode(b)`. A digest
is a function of its input alone, so the compiled run answers it
without CPython, and a table CPython printed holds it to CPython.

`fs.read_bytes` and `fs.write_bytes` are Yokan's own: one
implementation, both runs, for the file the text pair cannot carry.
"""
import base64
import hashlib
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import State, button, column, row, run, store, text, text_field  # noqa: E402
from yokan import fs  # noqa: E402

# The eight bytes every PNG starts with, written as a literal.
PNG = b"\x89PNG\r\n\x1a\n"
FILE = "demo/.gate/bytes_demo/mark.bin"

phrase: State[str] = State("yokan")
said: State[str] = State("")


@store
class Note:
    # A bytes field starts empty and is filled while the app runs.
    body: bytes = b""
    digest: str = ""
    size: int = 0

    def take(self, text: str) -> None:
        self.body = text.encode()
        self.size = len(self.body)
        self.digest = hashlib.sha256(self.body).hexdigest()

    def save(self) -> None:
        Note.take(phrase())
        n = fs.write_bytes(FILE, PNG + self.body)
        said.set(f"wrote {n} bytes")

    def load(self) -> None:
        try:
            raw = fs.read_bytes(FILE)
        except RuntimeError:
            said.set("nothing saved yet")
            return
        head = raw[0:8]
        rest = raw[8:len(raw)]
        self.body = rest
        self.size = len(rest)
        self.digest = hashlib.sha256(rest).hexdigest()
        if head == PNG:
            said.set(f"read {len(raw)} bytes, mark {head} is the one")
        else:
            said.set(f"read {len(raw)} bytes, mark {head} is not the one")


def encode() -> None:
    Note.take(phrase())
    said.set(f"{base64.b64encode(Note.body)}")


def decode() -> None:
    # Back the way it came: base64 answers bytes, and bytes decode to
    # the text they were made from.
    packed = base64.b64encode(phrase().encode())
    said.set(base64.b64decode(packed).decode())


def view() -> None:
    with column(spacing=10, padding=14):
        text("bytes, hashlib, base64", size=20)
        text_field(phrase(), placeholder="text to encode", on_change=phrase.set)
        with row(spacing=6):
            button("encode", on_click=encode)
            button("decode", on_click=decode)
            button("save", on_click=Note.save)
            button("load", on_click=Note.load)
        text(f"said: {said()}")
        text(f"bytes: {Note.body}")
        text(f"length: {Note.size}, first byte: {Note.body[0] if Note.size > 0 else 0}")
        text(f"hex: {Note.body.hex()}")
        text(f"sha256: {Note.digest}")
        text(f"png mark: {PNG}, as hex {PNG.hex()}")


if __name__ == "__main__":
    run(view, title="bytes", on_start=encode)
