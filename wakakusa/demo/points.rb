# A small class of values, carried on the app's own state.
require "wakakusa"

class Point
  attr_reader :x, :y

  def initialize(x, y = 0)
    @x = x
    @y = y
  end
end

class Points
  def initialize
    @sel = Point.new(3, 4)
    @dist = 0
  end

  def view
    column(
      text("p=(#{@sel.x}, #{@sel.y}) d2=#{@dist}"),
      row(
        button("right") { @sel = Point.new(@sel.x + 5, @sel.y) },
        button("swap") { @sel = Point.new(@sel.y, @sel.x) },
        button("measure") { @dist = @sel.x * @sel.x + @sel.y * @sel.y },
        spacing: 6.0
      ),
      spacing: 8.0,
      padding: 12.0
    )
  end
end

run(Points.new, title: "points")
