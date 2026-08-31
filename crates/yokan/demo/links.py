# /// script
# requires-python = ">=3.14"
# ///
"""Models referencing models, gated: `Node | None` fields wire an
ownership chain in handlers, `Weak[Node]` is the not-owning back
pointer (it breaks the cycle, so dropping the root frees the chain
in both tiers at the same statement), and views read through
walrus-narrowed bindings.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import button, column, model, row, run, State, store, text, Weak  # noqa: E402


@model
class Node:
    label: str = "n"
    kid: Node | None = None
    parent: Weak[Node] = None


@store
class Tree:
    root: Node | None = None
    keep: Node | None = None
    note: str = "-"

    def build(self) -> None:
        a = Node()
        a.label = "alpha"
        b = Node()
        b.label = "beta"
        a.kid = b
        b.parent = a
        self.root = a
        self.keep = b

    def drop_root(self) -> None:
        self.root = None

    def peek(self) -> None:
        if (r := Tree.root) is not None:
            if (k := r.kid) is not None:
                if (p := k.parent) is not None:
                    self.note = f"kid={k.label} parent={p.label}"
                else:
                    self.note = f"kid={k.label} parent=gone"
            else:
                self.note = "no kid"
        elif (k := Tree.keep) is not None:
            if (p := k.parent) is not None:
                self.note = f"kept {k.label}, parent={p.label}"
            else:
                self.note = f"kept {k.label}, parent=gone"
        else:
            self.note = "no root"


def view():
    with column(spacing=8, padding=12):
        text(f"note: {Tree.note}")
        if (r := Tree.root) is not None:
            text(f"root: {r.label}")
        else:
            text("root: (none)")
        with row(spacing=6):
            button("build", on_click=Tree.build)
            button("peek", on_click=Tree.peek)
            button("drop", on_click=Tree.drop_root)


if __name__ == "__main__":
    run(view, title="links")
