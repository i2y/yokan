# What a canvas is painted with.
#
# A command is not an element. It takes none of the keywords every
# element takes, nothing can click it, and it means nothing outside the
# canvas it was written in — so it has no handle of its own, and joins
# the canvas that is open where it stands.
#
# Every coordinate is a whole virtual pixel, and every color is a
# number: the index of a color in the canvas's palette. That is how
# tools for pixel art work, so a frame written for a pixel machine
# ports command for command, its numbers unchanged.

# The canvas being painted, or 0 between canvases. A canvas cannot hold
# another, so one place to keep it is enough.
$wakakusa_canvas = 0

# A canvas's block. What it paints belongs to this canvas until it ends.
def wakakusa_paint(el, &blk)
  return if blk.nil?

  outer = $wakakusa_canvas
  # `.to_i` is not decoration: a global takes its type from what it was
  # first given, and a method's parameter has none the compiler knows.
  $wakakusa_canvas = el.to_i
  blk.call
  $wakakusa_canvas = outer.to_i
end

def pixel(x, y, color)
  PixieC.pixie_op_pixel($wakakusa_canvas, x, y, color)
end

def line(x1, y1, x2, y2, color)
  PixieC.pixie_op_line($wakakusa_canvas, x1, y1, x2, y2, color)
end

def rect(x, y, w, h, color)
  PixieC.pixie_op_rect($wakakusa_canvas, x, y, w, h, color)
end

def rect_outline(x, y, w, h, color)
  PixieC.pixie_op_rect_outline($wakakusa_canvas, x, y, w, h, color)
end

def circle(x, y, r, color)
  PixieC.pixie_op_circle($wakakusa_canvas, x, y, r, color)
end

def circle_outline(x, y, r, color)
  PixieC.pixie_op_circle_outline($wakakusa_canvas, x, y, r, color)
end

def triangle(x1, y1, x2, y2, x3, y3, color)
  PixieC.pixie_op_triangle($wakakusa_canvas, x1, y1, x2, y2, x3, y3, color)
end

def triangle_outline(x1, y1, x2, y2, x3, y3, color)
  PixieC.pixie_op_triangle_outline($wakakusa_canvas, x1, y1, x2, y2, x3, y3, color)
end

# A rectangle of `source`, copied onto the canvas. The sheet and the
# canvas share one palette, which is what `colkey` indexes: that color
# is not copied, and -1 copies every pixel.
def sprite(x, y, source, u, v, w, h, colkey: -1, flip_x: false, flip_y: false)
  PixieC.pixie_op_sprite($wakakusa_canvas, x, y, source, u, v, w, h, colkey,
                         flip_x ? 1 : 0, flip_y ? 1 : 0)
end

# A line of text in the canvas's own 4x6 font, laid on the pixel grid
# rather than by the text system.
def pixel_text(x, y, text, color)
  PixieC.pixie_op_pixel_text($wakakusa_canvas, x, y, text, color)
end
