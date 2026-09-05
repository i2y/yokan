# Bars and lines. `min`/`max` both zero take the range from the data,
# so a negative month hangs below the zero line; `series` draws several
# lines at once, one color each.
require "wakakusa"

class Charts
  def initialize
    @months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun"]
    @profit = [12.0, -4.0, 9.0, 21.0, -2.0, 15.0]
    @traffic = [[30.0, 45.0, 38.0, 62.0, 55.0, 71.0], [4.0, 9.0, 6.0, 12.0, 8.0, 5.0]]
    @month = 6
  end

  def advance
    @month += 1
    profit = @profit.dup
    profit.push((@month * 7 % 40).to_f - 12.0)
    @profit = profit
    months = @months.dup
    months.push("M#{@month}")
    @months = months
    hits = @traffic[0].dup
    hits.push((@month * 11 % 60).to_f + 20.0)
    errs = @traffic[1].dup
    errs.push((@month * 5 % 14).to_f + 2.0)
    @traffic = [hits, errs]
  end

  def view
    column(
      text("Profit and loss", size: 18.0, bold: true),
      text("negative months hang below the zero line", size: 12.0, color: "#8a8f98"),
      bar_chart(@profit, labels: @months, axis: true, height: 150.0),
      text("Traffic", size: 18.0, bold: true),
      text("requests and errors, one color each", size: 12.0, color: "#8a8f98"),
      line_chart(series: @traffic, labels: @months, colors: ["accent", "#f38ba8"],
                 axis: true, max: 90.0, height: 150.0),
      row(button("next month") { advance }, spacing: 8.0),
      spacing: 12.0,
      padding: 16.0
    )
  end
end

run(Charts.new, title: "charts")
