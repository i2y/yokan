# A canvas: a grid of virtual pixels, painted command by command.
#
# `canvas(width, height, scale:, background:, palette:)` opens the grid
# and its block paints — pixel, line, rect, rect_outline, circle,
# circle_outline, triangle, triangle_outline, sprite and pixel_text.
# `scale` says how many logical pixels one virtual pixel takes, so a
# 64x40 canvas at six is 384x240 on screen.
#
# Every color is a NUMBER: the index of a color in `palette`. That is
# how tools for pixel art work, so drawing code written for one moves
# here with its numbers unchanged.
#
# The commands are not elements. Nothing here can be clicked, themed,
# sized or animated, and a loop inside the canvas is the ordinary loop:
# what its body paints joins the frame where it stands.
require "wakakusa"

HEADING = { size: 18.0, color: "accent" }.freeze
FAINT = { size: 12.0, color: "#8a8f98" }.freeze

# Five colors are enough to show that the index IS the color.
PALETTE = ["#11111b", "#89b4fa", "#f38ba8", "#eeeeee", "#a6e3a1"].freeze

class Blip
  attr_reader :x, :y, :c

  def initialize(x, y, c)
    @x = x
    @y = y
    @c = c
  end
end

class Sky
  def initialize
    @frame = 0
    @ball_x = 30
    @ball_y = 18
    @dx = 1
    @dy = 1
    @blips = []
    seed
  end

  def seed
    @blips = [Blip.new(6, 4, 1), Blip.new(20, 9, 2), Blip.new(50, 6, 3), Blip.new(58, 30, 4)]
  end

  def tick
    @frame += 1
    # The keyboard is read here, in the tick, never in a view.
    # `key_down` is "held right now", so holding an arrow steers.
    @dx = -1 if key_down("left")
    @dx = 1 if key_down("right")
    @dy = -@dy if key_pressed("space")
    x = @ball_x + @dx
    y = @ball_y + @dy
    if x < 4
      x = 4
      @dx = 1
    end
    if x > 59
      x = 59
      @dx = -1
    end
    if y < 4
      y = 4
      @dy = 1
    end
    if y > 35
      y = 35
      @dy = -1
    end
    @ball_x = x
    @ball_y = y
  end

  def blip(b)
    pixel(b.x, b.y, b.c)
  end

  def view
    column(spacing: 12.0, padding: 16.0) {
      text "Canvas", **HEADING
      text "a grid of virtual pixels; every color is an index", **FAINT
      canvas(64, 40, scale: 6, background: 0, palette: PALETTE) {
        rect(2, 2, 12, 6, 1)
        rect_outline(16, 2, 12, 6, 2)
        circle_outline(34, 5, 4, 3)
        line(2, 11, 61, 11, 2)
        triangle(3, 37, 8, 28, 13, 37, 4)
        @blips.each { |b| blip(b) }
        circle(@ball_x, @ball_y, 3, 3)
        pixel_text(2, 14, "FRAME #{@frame}", 3)
      }
      row(spacing: 8.0) {
        button("seed") { seed }
      }
    }
  end
end

app = Sky.new
every(0.05) { app.tick }
run(app, title: "canvas")
