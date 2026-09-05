# A look kept in one place: a Hash of properties, merged and handed to
# an element with `**`. `theme:` swaps the palette its subtree resolves
# colors in, so one keyword flips the whole panel.
require "wakakusa"

CHIP = { size: 18.0, color: "accent" }.freeze
KEY = { background: "#313244", hover_background: "#45475a" }.freeze
HOT = { background: "#fab387" }.freeze
KEY_HOT = KEY.merge(HOT).freeze

class Styled
  def initialize
    @mode = "dark"
    @n = 0
  end

  def flip
    @mode = @mode == "dark" ? "light" : "dark"
  end

  def view
    column(
      text("n=#{@n}", **CHIP),
      row(
        button("+1", **KEY) { @n += 1 },
        button("flip", **KEY_HOT) { flip },
        spacing: 6.0
      ),
      spacing: 8.0, padding: 12.0, background: "panel", theme: @mode
    )
  end
end

run(Styled.new, title: "styled")
