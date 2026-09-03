# /// script
# requires-python = ">=3.14"
# ///
"""An order line with typed numeric inputs: int_field for the
quantity (1..99) and number_field for the unit price (0..1000, in
half-yen steps). Both commit on `enter` or when the field loses
focus — in a script, `input:` commits — so text that is not a number
never reaches the store, and the total is computed in the view.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import button, column, int_field, number_field, row, run, store, text


@store
class Order:
    qty: int = 1
    price: float = 0.0

    def set_qty(self, n: int) -> None:
        self.qty = n

    def set_price(self, p: float) -> None:
        self.price = p

    def reset(self) -> None:
        self.qty = 1
        self.price = 0.0


def view():
    with column(spacing=10, padding=14):
        text("Order line", size=18)
        with row(spacing=8):
            text("quantity")
            int_field(Order.qty, min=1, max=99, placeholder="qty", on_change=Order.set_qty)
        with row(spacing=8):
            text("unit price")
            number_field(
                Order.price,
                min=0.0,
                max=1000.0,
                step=0.5,
                placeholder="price",
                on_change=Order.set_price,
            )
        text(f"total  {Order.qty * Order.price}")
        button("reset", on_click=Order.reset)


if __name__ == "__main__":
    run(view, title="quantities", width=420, height=260)
