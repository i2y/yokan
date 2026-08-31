# /// script
# requires-python = ">=3.14"
# ///
"""Enum, Optional and animation. `match` IS pixie's `case` (exhaustiveness checked
natively), the walrus IS `if let some(v)` (Python's own spelling of
narrowing), and `animate=`/`easing=` ride the kernel's animation clock, so
`advance:` frames dump identically in both tiers.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from enum import Enum, auto  # noqa: E402

from yokan import button, column, row, run, State, store, text  # noqa: E402


class Mood(Enum):
    HAPPY = auto()
    SAD = auto()


@store
class Tracker:
    last: int | None = None
    trend: Mood = Mood.HAPPY

    def note(self, v: int) -> None:
        self.last = v
        match self.trend:
            case Mood.HAPPY:
                self.trend = Mood.SAD
            case Mood.SAD:
                self.trend = Mood.HAPPY

    def wipe(self) -> None:
        self.last = None


mood: State[Mood] = State(Mood.HAPPY)
sel: State[int | None] = State(None)
note: State[str] = State("-")


def flip():
    match mood():
        case Mood.HAPPY:
            mood.set(Mood.SAD)
        case Mood.SAD:
            mood.set(Mood.HAPPY)


def describe():
    if (v := sel()) is not None:
        note.set(f"picked {v}")
    else:
        note.set("nothing picked")


def view():
    with column(spacing=8, padding=12):
        match mood():
            case Mood.HAPPY:
                text("mood: up", size=18, color="accent", animate=120, easing="out")
            case Mood.SAD:
                text("mood: down", size=18, color="#f38ba8", animate=120, easing="out")
        if (v := sel()) is not None:
            text(f"selection: {v}")
        else:
            text("(no selection)")
        text(f"note: {note()}")
        if (t := Tracker.last) is not None:
            text(f"tracked: {t}", size=12)
        else:
            text("(nothing tracked)", size=12)
        with row(spacing=6):
            button("flip", on_click=flip)
            button("pick", on_click=lambda: sel.set(7))
            button("clear", on_click=lambda: sel.set(None))
            button("describe", on_click=describe)
            button("track", on_click=lambda: Tracker.note(9), animate=100, easing="inOut")
            button("wipe", on_click=Tracker.wipe)


if __name__ == "__main__":
    run(view, title="moods")
