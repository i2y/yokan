# spacer and divider: a filler and a rule. The header row's spacer
# pushes "ping" to the far edge; the footer's does the same for the
# count. divider draws the rules, the second one heavier and colored.
require "wakakusa"

class Layout
  def initialize
    @pings = 0
  end

  def view
    column(
      row(
        text("Layout", size: 18.0),
        spacer,
        button("ping") { @pings += 1 }
      ),
      divider,
      column(
        text("Section one", size: 14.0),
        text("spacer takes the slack a row leaves behind."),
        divider(thickness: 2.0, color: "accent"),
        text("Section two", size: 14.0),
        text("divider draws a rule across its parent."),
        spacing: 6.0
      ),
      row(
        spacer,
        text("pings: #{@pings}")
      ),
      spacing: 12.0,
      padding: 16.0
    )
  end
end

run(Layout.new, title: "layout")
