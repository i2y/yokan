# A chooser that changes what a list shows. The rows are built on
# demand, so the list is asked only for the ones in view.
require "wakakusa"

class Alerts
  def initialize
    @levels = ["all", "crit", "warn"]
    @level = 0
    @crit = [
      "crit  09:02  payments p95 breach — circuit breaker armed",
      "crit  09:11  db failover triggered",
      "crit  09:20  worker pool exhausted",
    ]
    @warn = [
      "warn  09:05  error budget burn 2x on web",
      "warn  09:14  cache hit rate below 80%",
      "warn  09:24  edge latency above SLO",
    ]
    @visible = @crit + @warn
  end

  def pick(i)
    @level = i
    @visible = case i
               when 1 then @crit
               when 2 then @warn
               else @crit + @warn
               end
  end

  def alert_row(i)
    text(@visible[i], size: 12.0, mono: true)
  end

  def view
    column(
      text("alert filter", size: 16.0),
      segmented(options: @levels, selected: @level) { |i| pick(i) },
      text("#{@visible.length} shown", size: 12.0, color: "textDim"),
      list_view(@visible.length, item_height: 22.0, height: 150.0) { |i| alert_row(i) },
      spacing: 10.0,
      padding: 14.0
    )
  end
end

run(Alerts.new, title: "filter")
