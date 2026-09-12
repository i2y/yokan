"""A titled block, built from the badges beside it."""
from yokan import column, text

from .badges import badge


def panel(title: str, note: str):
    """A heading, a note under it, and a badge naming where it came
    from."""
    with column(spacing=4):
        text(title, size=18)
        text(note)
        badge("yokanui")
