# Charts that can say what they mean: a profit-and-loss bar chart whose
# losing months hang below the zero line, and a two-series line chart.
# `min`/`max` both zero take the range from the data; `axis` draws the
# tick labels and a faint gridline at each; `series` takes one list per
# line, `colors` one color each.
require "wakakusa"

HEADING = { size: 18.0, color: "accent" }.freeze
FAINT = { size: 12.0, color: "#8a8f98" }.freeze

class Book
  def initialize
    @months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun"]
    @profit = [12.0, -8.0, 4.0, -3.0, 15.0, -6.0]
    @requests = [40.0, 55.0, 48.0, 62.0, 70.0, 58.0]
    @errors = [3.0, 9.0, 5.0, 12.0, 6.0, 4.0]
    @traffic = [@requests, @errors]
    @n = 6
  end

  def next_month
    @n += 1
    # A deterministic next month, so both runs read the same numbers
    # and the gate can compare them.
    @profit = grown(@profit, (@n * 7 % 41).to_f - 18.0)
    @months = grown(@months, "M#{@n}")
    @requests = grown(@requests, (@n * 13 % 50).to_f + 30.0)
    @errors = grown(@errors, (@n * 5 % 14).to_f)
    @traffic = [@requests, @errors]
  end

  # `list + [one]` on a field takes a different shape in the two runs;
  # a copy that is pushed to takes the same one in both.
  def grown(list, item)
    out = list.dup
    out.push(item)
    out
  end

  def view
    column(
      text("Profit and loss", **HEADING),
      text("negative months hang below the zero line", **FAINT),
      bar_chart(@profit, labels: @months, axis: true, height: 150.0),
      text("Traffic", **HEADING),
      text("requests and errors, one color each", **FAINT),
      line_chart(series: @traffic, labels: @months, colors: ["accent", "#f38ba8"],
                 axis: true, max: 90.0, height: 150.0),
      row(button("next month") { next_month }, spacing: 8.0),
      spacing: 12.0,
      padding: 16.0
    )
  end
end

run(Book.new, title: "charts")
