# The properties every element takes, on elements that have nothing
# else in common: a theme scope on a spacer, a box around a column, a
# tween on a chooser, a tooltip on a rule, and the lock that makes a
# field and a button inert.
require "wakakusa"

class Locks
  def initialize
    @locked = false
    @saves = 0
    # The palette the spacer's subtree resolves its tokens in — a
    # property takes a value, not just a literal, so the lock switches it.
    @mode = "dark"
    @tab = 0
    @note = "draft"
  end

  def flip
    @locked = !@locked
    @mode = @locked ? "light" : "dark"
  end

  def view
    column(
      text("shared", size: 20.0, role: "heading"),
      row(
        text("mode: #{@mode}  saves: #{@saves}", size: 12.0),
        # A theme scope on a spacer: the property is the element's,
        # whichever element it is.
        spacer(grow: 1.0, theme: @mode),
        button("lock", tooltip: "flip the lock") { flip },
        spacing: 8.0
      ),
      segmented(options: ["read", "write"], selected: @tab,
                animate: 120.0, easing: "out") { |i| @tab = i },
      # A box around the section: 260 wide, never under 200.
      column(
        # The field takes two of the grid's three tracks, and goes inert
        # with the lock.
        grid(
          text("note", size: 12.0),
          text_field(@note, col_span: 2, disabled: @locked) { |t| @note = t },
          columns: 3, spacing: 8.0
        ),
        button("save", disabled: @locked, tooltip: "count a save") { @saves += 1 },
        width: 260.0, min_width: 200.0, spacing: 8.0, padding: 8.0, background: "panel"
      ),
      link("Docs", "https://i2y.github.io/yokan/", role: "button"),
      divider(tooltip: "the end of the shared properties"),
      spacing: 10.0,
      padding: 14.0
    )
  end
end

run(Locks.new, title: "shared")
