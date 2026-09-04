# /// script
# requires-python = ">=3.14"
# ///
"""A canvas: a grid of virtual pixels you paint command by command.

`canvas(width, height, scale=…, background=…, palette=…)` opens the
grid, and inside it the commands paint — `pixel`, `line`, `rect`,
`rect_outline`, `circle`, `circle_outline`, `triangle`,
`triangle_outline`, `sprite` and `pixel_text`. `scale` says how many
logical pixels each virtual one takes, so a 64x40 canvas at six is
384x240 on screen.

Every color is a NUMBER: the index of a color in `palette`. That is
how tools for pixel art work, so drawing code written for one moves
here with its numbers unchanged.

The commands are not elements. Nothing here can be clicked, themed,
sized or animated, and a `for` inside the canvas is the ordinary
loop — what its body paints joins the frame where it stands.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import (  # noqa: E402
    button,
    canvas,
    circle,
    circle_outline,
    column,
    every,
    keys,
    line,
    pixel,
    pixel_text,
    rect,
    rect_outline,
    row,
    run,
    store,
    style,
    text,
    triangle,
    value,
)

heading = style(size=18, color="accent")
faint = style(size=12, color="#8a8f98")


@value
class Blip:
    x: int
    y: int
    c: int


@store
class Sky:
    frame: int = 0
    ball_x: int = 30
    ball_y: int = 18
    dx: int = 1
    dy: int = 1
    blips: list[Blip] = []
    # Five colors are enough to show that the index IS the color.
    palette: list[str] = ["#11111b", "#89b4fa", "#f38ba8", "#eeeeee", "#a6e3a1"]

    def seed(self) -> None:
        self.blips = [Blip(6, 4, 1), Blip(20, 9, 2), Blip(50, 6, 3), Blip(58, 30, 4)]

    def tick(self) -> None:
        self.frame = self.frame + 1
        # The keyboard is read here, in the tick — never in a view.
        # `down` is "held right now", so holding an arrow steers.
        if keys.down("left"):
            self.dx = -1
        if keys.down("right"):
            self.dx = 1
        if keys.pressed("space"):
            self.dy = -self.dy
        x = self.ball_x + self.dx
        y = self.ball_y + self.dy
        if x < 4:
            x = 4
            self.dx = 1
        if x > 59:
            x = 59
            self.dx = -1
        if y < 4:
            y = 4
            self.dy = 1
        if y > 35:
            y = 35
            self.dy = -1
        self.ball_x = x
        self.ball_y = y


every(0.05, Sky.tick)


def seed():
    Sky.seed()


def view():
    with column(spacing=12, padding=16):
        text("Canvas", **heading)
        text("a grid of virtual pixels; every color is an index", **faint)
        with canvas(64, 40, scale=6, background=0, palette=Sky.palette):
            rect(2, 2, 12, 6, 1)
            rect_outline(16, 2, 12, 6, 2)
            circle_outline(34, 5, 4, 3)
            line(2, 11, 61, 11, 2)
            triangle(3, 37, 8, 28, 13, 37, 4)
            for b in Sky.blips:
                pixel(b.x, b.y, b.c)
            circle(Sky.ball_x, Sky.ball_y, 3, 3)
            pixel_text(2, 14, f"FRAME {Sky.frame}", 3)
        with row(spacing=8):
            button("seed", on_click=seed)


if __name__ == "__main__":
    run(view, title="canvas", on_start=seed)
