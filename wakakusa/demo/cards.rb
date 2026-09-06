# A piece of screen with a name is a method that answers an element,
# and one that wraps other elements takes them as arguments.
require "wakakusa"

class Cards
  def initialize
    @a = 0
    @b = 0
  end

  def card(title, *kids)
    column(
      text(title, size: 18.0),
      *kids,
      spacing: 4.0, padding: 8.0,
      border_width: 1.0, border_color: "accent", border_radius: 8.0
    )
  end

  def view
    column(
      card("counters",
           row(text("a: #{@a}"), button("+1") { @a += 1 }, spacing: 6.0),
           row(text("b: #{@b}"), button("+10") { @b += 10 }, spacing: 6.0)),
      text("outside the card", size: 12.0),
      spacing: 10.0,
      padding: 16.0
    )
  end
end

run(Cards.new, title: "cards")
