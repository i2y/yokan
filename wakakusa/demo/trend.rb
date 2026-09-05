# One list of numbers, drawn twice. A chart takes its data as its first
# argument, so a view that computes the numbers reads in order.
require "wakakusa"

class Trend
  def initialize
    @seed = 3
    @steps = 0
  end

  def values
    out = []
    n = @seed
    8.times do
      n = (n * 7 + 13) % 50
      out.push(n.to_f)
    end
    out
  end

  def stir
    @seed = (@seed * 5 + 1) % 97
    @steps += 1
  end

  def view
    column(
      text("Trend", size: 18.0, bold: true),
      text("seed #{@seed}, #{@steps} stirs", size: 12.0, color: "#8a8f98"),
      line_chart(values, height: 120.0),
      bar_chart(values, height: 90.0),
      button("stir") { stir },
      spacing: 10.0,
      padding: 14.0
    )
  end
end

run(Trend.new, title: "trend")
