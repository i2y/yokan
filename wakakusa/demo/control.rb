# Ordinary Ruby inside a view: `if`, `unless`, a ternary, a loop, and a
# method that answers part of the screen. Nothing here is a special
# form — the block is Ruby, and each element joins the container that
# is open.
#
# The one shape that is refused is a block written inside a loop's
# block. `line` below is what to write instead, and `wakakusa check`
# says so with the line.
require "wakakusa"

class Control
  def initialize
    @items = ["milk", "eggs", "rice"]
    @picked = -1
    @show_hint = true
    @tab = 0
  end

  def hint
    text "pick one", size: 12.0, color: "#8a8f98"
  end

  def line(name, i)
    row(spacing: 8.0) {
      text(i == @picked ? "▸ #{name}" : "  #{name}")
      button("pick #{i}") { @picked = i }
    }
  end

  def tab_button(n)
    button("tab #{n}") { @tab = n }
  end

  def view
    column(spacing: 10.0, padding: 14.0) {
      text "control flow", size: 18.0, bold: true

      if @show_hint
        hint
      else
        text "hidden", size: 12.0
      end

      @items.each_with_index { |name, i| line(name, i) }

      unless @picked < 0
        text "picked #{@items[@picked]}", color: @picked.even? ? "accent" : "#f38ba8"
      end

      row(spacing: 6.0) {
        button("hint") { @show_hint = !@show_hint }
        3.times { |n| tab_button(n) }
      }
      text "tab #{@tab}"
    }
  end
end

run(Control.new, title: "control")
