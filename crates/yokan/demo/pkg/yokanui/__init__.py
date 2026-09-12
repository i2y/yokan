"""A few view helpers, written in the dialect and compiled into the
app that imports them.

The package says so with `py.yokan` beside this file. What it offers
is re-exported here, so an app writes `from yokanui import panel`.
"""
from .badges import badge, pill
from .panels import panel
from .plain import plain

__all__ = ["badge", "panel", "pill", "plain"]
