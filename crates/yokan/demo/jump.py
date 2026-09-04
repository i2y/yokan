# /// script
# requires-python = ">=3.14"
# ///
"""Pyxel Jump, ported to Yokan.

The original is `02_jump_game.py` from Pyxel's examples (Takashi
Kitao, MIT, https://github.com/kitao/pyxel), and `assets/jump.png` is
that example's own image bank (`jump_game.pyxres`) written out with
Pyxel's palette. The port follows it line by line: `pyxel.blt` becomes
`sprite`, `pyxel.btn` becomes `keys.down`, `pyxel.cls(12)` becomes the
canvas background, and 12 still means the same color, because inside a
canvas a color is an index into the palette this file declares.

What is different, and why. The music and the sound effects are gone:
there is no audio here yet. So is the gamepad. Everything else — the falling player, the floors
that drop away when you land on them, the fruit, the scrolling
mountain, trees and two layers of cloud — is the game.

Left and right move; the rest is gravity.
"""
import os
import random
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import (  # noqa: E402
    canvas,
    column,
    every,
    keys,
    pixel_text,
    run,
    sprite,
    store,
    value,
)

WIDTH = 160
HEIGHT = 120
SKY = 12
SHEET = "demo/assets/jump.png"


@value
class Cloud:
    x: int
    y: int


@value
class Floor:
    x: int
    y: int
    alive: bool


@value
class Fruit:
    x: int
    y: int
    kind: int
    alive: bool


@store
class Game:
    score: int = 0
    px: int = 72
    py: int = -16
    dy: int = 0
    alive: bool = True
    frame: int = 0
    far: list[Cloud] = []
    near: list[Cloud] = []
    floors: list[Floor] = []
    fruits: list[Fruit] = []
    # What the view needs whole: the parallax offsets and which of the
    # two player sprites to cut out.
    tree_off: int = 0
    far_off: int = 0
    near_off: int = 0
    player_u: int = 0
    palette: list[str] = [
        "#000000", "#2b335f", "#7e2072", "#19959c",
        "#8b4852", "#395c98", "#a9c1ff", "#eeeeee",
        "#d4186c", "#d38441", "#e9c35b", "#70c6a9",
        "#7696de", "#a3a3a3", "#ff9798", "#edc7b0",
    ]

    def boot(self) -> None:
        random.seed(11)
        self.far = [Cloud(-10, 75), Cloud(40, 65), Cloud(90, 60)]
        self.near = [Cloud(10, 25), Cloud(70, 35), Cloud(120, 15)]
        floors_: list[Floor] = []
        fruits_: list[Fruit] = []
        for i in range(4):
            floors_ = floors_ + [Floor(i * 60, random.randint(8, 104), True)]
            fruits_ = fruits_ + [
                Fruit(i * 60, random.randint(0, 104), random.randint(0, 2), True)
            ]
        self.floors = floors_
        self.fruits = fruits_

    def tick(self) -> None:
        self.frame = self.frame + 1
        self.tree_off = self.frame % 160
        self.far_off = (self.frame // 16) % 160
        self.near_off = (self.frame // 8) % 160
        Game.update_player()
        Game.update_floors()
        Game.update_fruits()

    def update_player(self) -> None:
        if keys.down("left"):
            self.px = max(self.px - 2, 0)
        if keys.down("right"):
            self.px = min(self.px + 2, WIDTH - 16)
        self.py = self.py + self.dy
        self.dy = min(self.dy + 1, 8)
        self.player_u = 0
        if self.dy > 0:
            self.player_u = 16
        if self.py > HEIGHT:
            self.alive = False
            if self.py > 600:
                self.score = 0
                self.px = 72
                self.py = -16
                self.dy = 0
                self.alive = True

    def update_floors(self) -> None:
        """A floor the player lands on drops away and bounces them.

        The original edits the tuple in the list; a value is not edited
        in place, so this builds the next list — and `dy` is carried in
        a local because the bounce it writes is what the floors after
        this one see."""
        out: list[Floor] = []
        score_ = self.score
        dy_ = self.dy
        for f in self.floors:
            x = f.x
            y = f.y
            alive_ = f.alive
            if alive_:
                if (
                    self.px + 16 >= x
                    and self.px <= x + 40
                    and self.py + 16 >= y
                    and self.py <= y + 8
                    and dy_ > 0
                ):
                    alive_ = False
                    score_ = score_ + 10
                    dy_ = -12
            else:
                y = y + 6
            x = x - 4
            if x < -40:
                x = x + 240
                y = random.randint(8, 104)
                alive_ = True
            out = out + [Floor(x, y, alive_)]
        self.floors = out
        self.score = score_
        self.dy = dy_

    def update_fruits(self) -> None:
        out: list[Fruit] = []
        score_ = self.score
        dy_ = self.dy
        for f in self.fruits:
            x = f.x
            y = f.y
            kind = f.kind
            alive_ = f.alive
            if alive_ and abs(x - self.px) < 12 and abs(y - self.py) < 12:
                alive_ = False
                score_ = score_ + (kind + 1) * 100
                dy_ = min(dy_, -8)
            x = x - 2
            if x < -40:
                x = x + 240
                y = random.randint(0, 104)
                kind = random.randint(0, 2)
                alive_ = True
            out = out + [Fruit(x, y, kind, alive_)]
        self.fruits = out
        self.score = score_
        self.dy = dy_


every(0.033, Game.tick)


def view():
    with column(spacing=0, padding=0):
        with canvas(WIDTH, HEIGHT, scale=4, background=SKY, palette=Game.palette):
            # sky, mountain, and the trees that scroll fastest
            sprite(0, 88, SHEET, 0, 88, 160, 32)
            sprite(0, 88, SHEET, 0, 64, 160, 24, colkey=SKY)
            for i in range(2):
                sprite(i * 160 - Game.tree_off, 104, SHEET, 0, 48, 160, 16, colkey=SKY)
            # two layers of cloud, each strip drawn twice so it wraps
            for i in range(2):
                for c in Game.far:
                    sprite(c.x + i * 160 - Game.far_off, c.y, SHEET, 64, 32, 32, 8, colkey=SKY)
            for i in range(2):
                for c in Game.near:
                    sprite(c.x + i * 160 - Game.near_off, c.y, SHEET, 0, 32, 56, 8, colkey=SKY)
            for f in Game.floors:
                sprite(f.x, f.y, SHEET, 0, 16, 40, 8, colkey=SKY)
            for fr in Game.fruits:
                if fr.alive:
                    sprite(fr.x, fr.y, SHEET, 32 + fr.kind * 16, 0, 16, 16, colkey=SKY)
            sprite(Game.px, Game.py, SHEET, Game.player_u, 0, 16, 16, colkey=SKY)
            pixel_text(5, 4, f"SCORE {Game.score:>4}", 1)
            pixel_text(4, 4, f"SCORE {Game.score:>4}", 7)


if __name__ == "__main__":
    run(
        view,
        title="Pyxel Jump",
        width=640.0,
        height=480.0,
        padding=0.0,
        on_start=Game.boot,
    )
