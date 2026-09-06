# A timer: declared before the app runs, told every second. Both runs
# tick off the same clock — a frame in a window, an `advance:` in a
# script — so the same number of ticks lands in both.
#
# The step is arithmetic rather than a random number: the two runs draw
# on different generators, and a dashboard that cannot be compared is
# not worth gating.
require "wakakusa"

SLOTS = 12

class Dashboard
  attr_reader :ticks

  def initialize
    @hist = Array.new(SLOTS, 0.0)
    @at = 0
    @ticks = 0
    @cur = 0.25
  end

  def tick
    @ticks += 1
    step = ((@ticks * 37 % 41).to_f / 100.0) - 0.2
    v = @cur + step
    v = 0.0 if v < 0.0
    v = 1.0 if v > 1.0
    @cur = v
    @hist[@at] = v
    @at = (@at + 1) % SLOTS
  end

  def view
    column(
      row(
        text("load, sampled every second", size: 13.0, color: "#8a8f98", grow: 1.0),
        spinner(size: 16.0),
        spacing: 8.0
      ),
      text(format("%.2f", @cur), size: 40.0),
      progress(@cur),
      line_chart(@hist, height: 120.0),
      text("#{@ticks} ticks · #{SLOTS} slots", size: 12.0, color: "#8a8f98"),
      spacing: 12.0,
      padding: 16.0
    )
  end
end

app = Dashboard.new
every(1.0) { app.tick }
run(app, title: "dashboard")
