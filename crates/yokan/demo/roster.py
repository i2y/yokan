# /// script
# requires-python = ">=3.14"
# ///
"""A roster of 200 members in a `table`: the columns are tracks the
cells line up on, a click on a row selects it, and a click on a header
sorts. The widget only reports which column was clicked; the store
owns the order and re-sorts its own parallel lists, so the selection
is tracked by member and follows its row through a sort. A second
table lists the selected member's teammates, with a selection of its
own.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import column, row, run, store, table, text  # noqa: E402


@store
class Roster:
    team_names: list[str] = ["red", "blue", "green", "gold"]
    names: list[str] = []
    ids: list[int] = []
    teams: list[str] = []
    team_ix: list[int] = []
    scores: list[int] = []
    keys: list[int] = []
    sel: int = -1
    sel_id: int = -1
    sel_line: str = ""
    sort_col: int = -1
    desc: bool = False
    mates: list[str] = []
    mate_scores: list[int] = []
    mate_sel: int = -1
    mate_name: str = ""

    def seed(self) -> None:
        for i in range(200):
            self.names = self.names + [f"member {i}"]
            self.ids = self.ids + [i]
            self.team_ix = self.team_ix + [i % 4]
            self.teams = self.teams + [self.team_names[i % 4]]
            self.scores = self.scores + [(i * 37 + 11) % 100]

    def pick(self, i: int) -> None:
        self.sel = i
        self.sel_id = self.ids[i]
        self.sel_line = f"{self.names[i]} ({self.teams[i]}, {self.scores[i]})"
        self.mates = []
        self.mate_scores = []
        for k in range(len(self.names)):
            if self.team_ix[k] == self.team_ix[i] and k != i:
                self.mates = self.mates + [self.names[k]]
                self.mate_scores = self.mate_scores + [self.scores[k]]
        self.mate_sel = -1

    def pick_mate(self, i: int) -> None:
        self.mate_sel = i
        self.mate_name = self.mates[i]

    def swap(self, a: int, b: int) -> None:
        n = self.names[a]
        self.names[a] = self.names[b]
        self.names[b] = n
        d = self.ids[a]
        self.ids[a] = self.ids[b]
        self.ids[b] = d
        t = self.teams[a]
        self.teams[a] = self.teams[b]
        self.teams[b] = t
        x = self.team_ix[a]
        self.team_ix[a] = self.team_ix[b]
        self.team_ix[b] = x
        s = self.scores[a]
        self.scores[a] = self.scores[b]
        self.scores[b] = s
        k = self.keys[a]
        self.keys[a] = self.keys[b]
        self.keys[b] = k

    # The sort key is an int per column: the member number for the
    # name column (the names are seeded in order), the team's index,
    # the score. An insertion sort swaps every parallel list in step,
    # then the selection finds its member again.
    def sort_by(self, j: int) -> None:
        if j == self.sort_col:
            self.desc = not self.desc
        else:
            self.sort_col = j
            self.desc = False
        if j == 0:
            self.keys = [k for k in self.ids]
        elif j == 1:
            self.keys = [k for k in self.team_ix]
        else:
            self.keys = [k for k in self.scores]
        i = 1
        while i < len(self.names):
            k = i
            while k > 0:
                a = self.keys[k]
                b = self.keys[k - 1]
                if (self.desc and a > b) or (not self.desc and a < b):
                    Roster.swap(k, k - 1)
                    k = k - 1
                else:
                    break
            i = i + 1
        if self.sel_id >= 0:
            for k in range(len(self.names)):
                if self.ids[k] == self.sel_id:
                    self.sel = k


def cells(i: int):
    return row(text(Roster.names[i]), text(Roster.teams[i]), text(f"{Roster.scores[i]}"))


def mate_cells(i: int):
    return row(text(Roster.mates[i]), text(f"{Roster.mate_scores[i]}"))


def view():
    with column(spacing=8, padding=12, grow=1.0):
        text("roster: click a header to sort, a row to select", size=13)
        table(
            ["member", "team", "score"],
            len(Roster.names),
            cells,
            widths=[2.0, 1.0, 1.0],
            selected=Roster.sel,
            sort=Roster.sort_col,
            descending=Roster.desc,
            on_select=Roster.pick,
            on_sort=Roster.sort_by,
            grow=1.0,
        )
        if Roster.sel >= 0:
            text(f"selected: {Roster.sel_line}")
        else:
            text("selected: nobody")
        text("teammates", size=13)
        table(
            ["teammate", "score"],
            len(Roster.mates),
            mate_cells,
            widths=[2.0, 1.0],
            selected=Roster.mate_sel,
            on_select=Roster.pick_mate,
            height=120.0,
        )
        if Roster.mate_sel >= 0:
            text(f"teammate: {Roster.mate_name}")


if __name__ == "__main__":
    run(view, title="roster", width=560, height=600, on_start=Roster.seed)
