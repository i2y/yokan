# /// script
# requires-python = ">=3.14"
# ///
"""Named stores: `@store` is a process-lifetime singleton with
fields AND methods — the decorator returns the instance, so the
class name IS the store. Stores
call each other's methods; views read their fields reactively.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import button, column, row, run, store, text  # noqa: E402


@store
class Settings:
    factor: int = 2

    def double(self) -> None:
        self.factor *= 2


@store
class Cart:
    items: list[str] = []
    total: int = 0

    def add(self, name: str, price: int) -> None:
        self.items = self.items + [name]
        self.total += price * Settings.factor
        Settings.double()

    def clear(self) -> None:
        self.items = []
        self.total = 0


def view():
    with column(spacing=8, padding=12):
        text(f"n={len(Cart.items)} total={Cart.total} f={Settings.factor}")
        with row(spacing=6):
            button("add", on_click=lambda: Cart.add("apple", 10))
            button("clear", on_click=Cart.clear)


if __name__ == "__main__":
    run(view, title="stores")
