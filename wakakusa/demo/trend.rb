# One list of numbers, drawn twice. A chart takes its data as its first
# argument, so a view that computes the numbers reads in order.
require "wakakusa"

class Trend
  def initialize
    @values = [3.0, 5.0, 2.0]
    @limit = 4.5
  end

  def bump
    values = @values.dup
    values.push(8.0)
    @values = values
  end

  def view
    column(
      text("points: #{@values.length}", size: 14.0),
      line_chart(@values, height: 120.0),
      bar_chart(@values, height: 90.0),
      text("limit: #{format("%.1f", @limit)}", size: 12.0, color: "#8a8f98"),
      row(
        button("add point") { bump },
        button("raise limit") { @limit += 0.5 },
        spacing: 8.0
      ),
      spacing: 10.0,
      padding: 14.0
    )
  end
end

run(Trend.new, title: "trend")
