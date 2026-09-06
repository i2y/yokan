# Text as a pill: a background with padding and a radius. And the rest
# of what a run of text can be — monospace, underlined, italic, clipped
# with an ellipsis, or wrapped and then clamped.
require "wakakusa"

PILL = { size: 11.0, color: "#11111b", padding: 4.0, border_radius: 10.0 }.freeze
PILL_OK = PILL.merge({ background: "#2fa84f" }).freeze
PILL_WARN = PILL.merge({ background: "#fab387" }).freeze
PILL_CRIT = PILL.merge({ background: "#f38ba8" }).freeze

class Badges
  def initialize
    @tint = "#45475a"
    @hot = false
  end

  def flip
    @hot = !@hot
    @tint = @hot ? "#f38ba8" : "#45475a"
  end

  def view
    column(
      text("Badges", size: 20.0, bold: true),
      row(
        text("● OK", **PILL_OK),
        text("● WARN", **PILL_WARN),
        text("● CRIT", **PILL_CRIT),
        text("● BUILD", size: 11.0, color: "#cdd6f4", background: @tint,
             padding: 4.0, border_radius: 10.0, border_width: 1.0,
             border_color: "#585b70"),
        spacing: 6.0
      ),
      button("flip") { flip },
      text("commit 9f2c1ab8e04d", mono: true, size: 12.0),
      text("an underlined note", underline: true),
      text("in italics, for contrast", italic: true),
      # An ellipsis needs a bounded box to clip against.
      text("a single line far too long for the box it was given, so it ends in an ellipsis",
           wrap: "ellipsis", width: 260.0),
      # The clamp is the other half: this one wraps, then stops.
      text("a paragraph that wraps at the window's width and then stops after two lines, " \
           "because a clamped label is what a card summary wants",
           max_lines: 2, width: 260.0),
      spacing: 8.0,
      padding: 12.0
    )
  end
end

run(Badges.new, title: "badges")
