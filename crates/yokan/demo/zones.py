# /// script
# requires-python = ">=3.14"
# ///
"""`zoneinfo` in the dialect: one meeting, read in four places.

A zone is named where it is written — `ZoneInfo("Asia/Tokyo")` — and
both runs read the same zone files off the machine, so they cannot
disagree about an offset. `datetime(..., tzinfo=TOKYO)` is the wall
clock in that zone; `astimezone` moves it to another; `isoformat`,
`tzname`, `utcoffset` and `strftime`'s `%z` / `%Z` say where it is,
and a subtraction between two of them is the difference between the
instants, not between the clocks.

The zone rides in the type rather than in the value, so a key is a
literal: the compiled side reads it while it translates. What a State
or a field holds is the naive `datetime` it always held.
"""
import os
import sys
from datetime import datetime, timedelta
from zoneinfo import ZoneInfo

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import State, button, column, row, run, text  # noqa: E402

TOKYO = ZoneInfo("Asia/Tokyo")
NEW_YORK = ZoneInfo("America/New_York")
LONDON = ZoneInfo("Europe/London")
KOLKATA = ZoneInfo("Asia/Kolkata")

# The meeting, in the clock of the room it is booked in.
lines: State[list[str]] = State([])
gap: State[str] = State("")
zone_now: State[str] = State("")


def elsewhere() -> None:
    here = datetime(2026, 7, 14, 9, 30, tzinfo=TOKYO)
    there = here.astimezone(NEW_YORK)
    over = here.astimezone(LONDON)
    east = here.astimezone(KOLKATA)
    lines.set(
        [
            f"Tokyo     {here.strftime('%a %d %b %H:%M %Z %z')}",
            f"New York  {there.strftime('%a %d %b %H:%M %Z %z')}",
            f"London    {over.strftime('%a %d %b %H:%M %Z %z')}",
            f"Kolkata   {east.strftime('%a %d %b %H:%M %Z %z')}",
        ]
    )


def winter() -> None:
    # The same hour six months later: New York is on standard time,
    # so the difference from Tokyo is an hour wider.
    here = datetime(2026, 1, 14, 9, 30, tzinfo=TOKYO)
    there = here.astimezone(NEW_YORK)
    lines.set(
        [
            f"Tokyo     {here.isoformat()}",
            f"New York  {there.isoformat()}",
            f"New York offset {there.strftime('%z')}",
            f"the name it goes by {there.tzname()}",
        ]
    )


def difference() -> None:
    # Two clocks, one instant: the difference is zero. Move one of
    # them and the difference is what moved.
    start = datetime(2026, 7, 14, 9, 30, tzinfo=TOKYO)
    end = start.astimezone(NEW_YORK) + timedelta(hours=2)
    gap.set(f"{(end - start).total_seconds() / 3600.0} hours apart")


def right_now() -> None:
    # The clock itself is not the same in two runs, so what the demo
    # shows is what the zone says about now: its name and its offset.
    n = datetime.now(TOKYO)
    zone_now.set(f"Tokyo is {n.tzname()} at {n.strftime('%z')}")


def view() -> None:
    with column(spacing=10, padding=14):
        text("one meeting, four clocks", size=20)
        with row(spacing=6):
            button("summer", on_click=elsewhere)
            button("winter", on_click=winter)
            button("difference", on_click=difference)
            button("now", on_click=right_now)
        for line in lines():
            text(line)
        text(f"{gap()}")
        text(f"{zone_now()}")


if __name__ == "__main__":
    run(view, title="zones", on_start=elsewhere)
