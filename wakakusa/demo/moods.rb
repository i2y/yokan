# Values that are one of a few named things, and a value that may be
# nothing at all. Ruby writes the first as symbols and the second as
# nil, and `case` tells them apart.
require "wakakusa"

class Tracker
  attr_reader :last, :trend

  def initialize
    @last = nil
    @trend = :happy
  end

  def note(v)
    @last = v
    @trend = case @trend
             when :happy then :sad
             else :happy
             end
  end

  def wipe
    @last = nil
  end
end

class Moods
  def initialize
    @mood = :happy
    @sel = nil
    @note = "-"
    @tracker = Tracker.new
  end

  def flip
    @mood = case @mood
            when :happy then :sad
            else :happy
            end
  end

  def describe
    @note = @sel.nil? ? "nothing chosen" : "chose #{@sel}"
  end

  def mood_line
    return text("mood: up", size: 18.0, color: "accent", animate: 120.0, easing: "out") if @mood == :happy

    text("mood: down", size: 18.0, color: "#f38ba8", animate: 120.0, easing: "out")
  end

  def view
    column(spacing: 8.0, padding: 12.0) {
      mood_line
      if @sel.nil?
        text "(no selection)"
      else
        text "selection: #{@sel}"
      end
      text "note: #{@note}"
      if @tracker.last.nil?
        text "(nothing tracked)", size: 12.0
      else
        text "tracked: #{@tracker.last}", size: 12.0
      end
      row(spacing: 6.0) {
        button("flip") { flip }
        button("pick") { @sel = 7 }
        button("clear") { @sel = nil }
        button("describe") { describe }
        button("track", animate: 100.0, easing: "inOut") { @tracker.note(9) }
        button("wipe") { @tracker.wipe }
      }
    }
  end
end

run(Moods.new, title: "moods")
