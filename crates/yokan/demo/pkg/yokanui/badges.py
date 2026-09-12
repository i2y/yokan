"""Small labelled things."""
from yokan import row, text

# The package's own palette. A module-level constant, folded where it
# is read, like any other.
QUIET = "#7aa2f7"
LOUD = "#f7768e"


def badge(label: str):
    """A short label in the package's quiet colour."""
    return text(label, size=12, color=QUIET)


def pill(label: str, loud: bool):
    """A label that can raise its voice."""
    with row(spacing=4):
        text(label, size=12, color=LOUD if loud else QUIET)
