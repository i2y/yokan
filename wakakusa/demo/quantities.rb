# The two fields that hold a number rather than text: enter or leaving
# them commits, text that is not a number is dropped and the shown
# value returns to what the app holds.
require "wakakusa"

class Order
  def initialize
    @qty = 1
    @price = 0.0
  end

  def reset
    @qty = 1
    @price = 0.0
  end

  def view
    column(
      text("Order line", size: 18.0),
      row(
        text("quantity"),
        int_field(@qty, min: 1, max: 99, placeholder: "qty") { |n| @qty = n },
        spacing: 8.0
      ),
      row(
        text("unit price"),
        number_field(@price, min: 0.0, max: 1000.0, step: 0.5,
                     placeholder: "price") { |p| @price = p },
        spacing: 8.0
      ),
      text("total  #{@qty * @price}"),
      button("reset") { reset },
      spacing: 10.0,
      padding: 14.0
    )
  end
end

run(Order.new, title: "quantities")
