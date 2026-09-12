"""The package's Rust half: a crate it declares and calls itself.

An app that imports `plain` gets `deunicode` in its build without
having declared it.
"""
from yokan import crates


def plain(s: str) -> str:
    """`s` with its accents and kana folded to ASCII."""
    return crates.deunicode.deunicode(s)
