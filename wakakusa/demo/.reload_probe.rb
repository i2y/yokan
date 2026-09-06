require "wakakusa"

class Probe
  def initialize
    @ticks = 0
  end

  def tick
    @ticks += 1
    warn "tick #{@ticks} — version B"
  end

  def view
    column(spacing: 10.0, padding: 14.0) {
      text "version B: #{@ticks}", size: 20.0
    }
  end
end

app = Probe.new
every(1.0) { app.tick }
run(app, title: "reloadprobe")
