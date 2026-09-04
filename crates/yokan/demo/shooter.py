# /// script
# requires-python = ">=3.14"
# ///
"""Pyxel Shooter, ported to Yokan.

The original is `09_shooter.py` from Pyxel's examples (Takashi Kitao,
MIT, https://github.com/kitao/pyxel), and the sprite sheet in
`assets/shooter.png` is that example's own two 8x8 sprites, written out
with Pyxel's palette. The port follows it line by line: `pyxel.rect`
becomes `rect`, `pyxel.btn` becomes `keys.down`, `pyxel.blt` becomes
`sprite`, and the color numbers are the same numbers, because inside a
canvas a color is an index into the palette this file declares.

What is different, and why. Speeds that were fractional (1.5 px a
frame) are carried in tenths of a pixel and drawn whole, because a
pixel grid has no half pixels. The music and the sound effects are
gone: there is no audio here yet. So are the gamepad and `pyxel.quit`.
Everything else — three scenes, a hundred parallax stars, the enemy
that sways as it falls, rectangle collisions, expanding blasts — is
the game.

Arrow keys move, space fires, enter starts and restarts.
"""
import os
import random
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import (  # noqa: E402
    canvas,
    circle,
    circle_outline,
    column,
    every,
    keys,
    pixel,
    pixel_text,
    rect,
    run,
    sprite,
    store,
    value,
)

WIDTH = 120
HEIGHT = 160

SCENE_TITLE = 0
SCENE_PLAY = 1
SCENE_GAMEOVER = 2

NUM_STARS = 100
STAR_COLOR_HIGH = 12
STAR_COLOR_LOW = 5

PLAYER_WIDTH = 8
PLAYER_HEIGHT = 8
PLAYER_SPEED = 2

BULLET_WIDTH = 2
BULLET_HEIGHT = 8
BULLET_COLOR = 11
BULLET_SPEED = 4

ENEMY_WIDTH = 8
ENEMY_HEIGHT = 8
# Pyxel's 1.5 px a frame, in tenths.
ENEMY_SPEED = 15

BLAST_START_RADIUS = 1
BLAST_END_RADIUS = 8
BLAST_COLOR_IN = 7
BLAST_COLOR_OUT = 10

SHEET = "demo/assets/shooter.png"


@value
class Star:
    x: int
    # `y` is what the canvas draws; `y10` is where the star really is.
    y: int
    y10: int
    speed10: int
    col: int


@value
class Bullet:
    x: int
    y: int


@value
class Enemy:
    x: int
    y: int
    x10: int
    y10: int
    flip: bool
    offset: int


@value
class Blast:
    x: int
    y: int
    radius: int


@store
class Game:
    scene: int = 0
    score: int = 0
    frame: int = 0
    title_col: int = 0
    px: int = 56
    py: int = 140
    stars: list[Star] = []
    bullets: list[Bullet] = []
    enemies: list[Enemy] = []
    blasts: list[Blast] = []
    # Pyxel's own sixteen colors, which is what makes the numbers in
    # this file mean what they mean in the original.
    palette: list[str] = [
        "#000000", "#2b335f", "#7e2072", "#19959c",
        "#8b4852", "#395c98", "#a9c1ff", "#eeeeee",
        "#d4186c", "#d38441", "#e9c35b", "#70c6a9",
        "#7696de", "#a3a3a3", "#ff9798", "#edc7b0",
    ]

    def boot(self) -> None:
        random.seed(7)
        out: list[Star] = []
        for i in range(NUM_STARS):
            x = random.randint(0, WIDTH - 1)
            y = random.randint(0, HEIGHT - 1)
            speed10 = random.randint(10, 25)
            col = STAR_COLOR_LOW
            if speed10 > 18:
                col = STAR_COLOR_HIGH
            out = out + [Star(x, y, y * 10, speed10, col)]
        self.stars = out

    def tick(self) -> None:
        self.frame = self.frame + 1
        self.title_col = self.frame % 16
        Game.move_stars()
        if self.scene == SCENE_TITLE:
            if keys.pressed("enter"):
                self.scene = SCENE_PLAY
        elif self.scene == SCENE_PLAY:
            Game.play()
        else:
            Game.over()

    def move_stars(self) -> None:
        out: list[Star] = []
        for s in self.stars:
            y10 = s.y10 + s.speed10
            if y10 >= HEIGHT * 10:
                y10 = y10 - HEIGHT * 10
            out = out + [Star(s.x, y10 // 10, y10, s.speed10, s.col)]
        self.stars = out

    def play(self) -> None:
        if self.frame % 6 == 0:
            x = random.randint(0, WIDTH - ENEMY_WIDTH)
            self.enemies = self.enemies + [
                Enemy(x, 0, x * 10, 0, False, random.randint(0, 59))
            ]
        Game.collide()
        Game.move_player()
        Game.move_bullets()
        Game.move_enemies()
        Game.move_blasts()

    def over(self) -> None:
        Game.move_bullets()
        Game.move_enemies()
        Game.move_blasts()
        if keys.pressed("enter"):
            self.scene = SCENE_PLAY
            self.px = 56
            self.py = 140
            self.score = 0
            self.enemies = []
            self.bullets = []
            self.blasts = []

    def move_player(self) -> None:
        x = self.px
        y = self.py
        if keys.down("left"):
            x = x - PLAYER_SPEED
        if keys.down("right"):
            x = x + PLAYER_SPEED
        if keys.down("up"):
            y = y - PLAYER_SPEED
        if keys.down("down"):
            y = y + PLAYER_SPEED
        self.px = min(max(x, 0), WIDTH - PLAYER_WIDTH)
        self.py = min(max(y, 0), HEIGHT - PLAYER_HEIGHT)
        if keys.pressed("space"):
            self.bullets = self.bullets + [
                Bullet(self.px + 3, self.py - 4)
            ]

    def move_bullets(self) -> None:
        out: list[Bullet] = []
        for b in self.bullets:
            y = b.y - BULLET_SPEED
            if y + BULLET_HEIGHT - 1 >= 0:
                out = out + [Bullet(b.x, y)]
        self.bullets = out

    def move_enemies(self) -> None:
        out: list[Enemy] = []
        for e in self.enemies:
            x10 = e.x10
            flip = True
            if (self.frame + e.offset) % 60 < 30:
                x10 = x10 + ENEMY_SPEED
                flip = False
            else:
                x10 = x10 - ENEMY_SPEED
            y10 = e.y10 + ENEMY_SPEED
            if y10 <= (HEIGHT - 1) * 10:
                out = out + [
                    Enemy(x10 // 10, y10 // 10, x10, y10, flip, e.offset)
                ]
        self.enemies = out

    def move_blasts(self) -> None:
        out: list[Blast] = []
        for b in self.blasts:
            r = b.radius + 1
            if r <= BLAST_END_RADIUS:
                out = out + [Blast(b.x, b.y, r)]
        self.blasts = out

    def collide(self) -> None:
        """The two rectangle tests, resolved into new lists.

        A value is not edited in place, so where the original sets
        `is_alive = False` and filters afterwards, this keeps the ones
        that live."""
        live_enemies: list[Enemy] = []
        hit: list[int] = []
        blasts_ = self.blasts
        score_ = self.score
        struck_player = False
        for e in self.enemies:
            struck = False
            for i in range(len(self.bullets)):
                b = self.bullets[i]
                if (
                    e.x + ENEMY_WIDTH > b.x
                    and b.x + BULLET_WIDTH > e.x
                    and e.y + ENEMY_HEIGHT > b.y
                    and b.y + BULLET_HEIGHT > e.y
                ):
                    struck = True
                    hit = hit + [i]
            if struck:
                blasts_ = blasts_ + [
                    Blast(e.x + 4, e.y + 4, BLAST_START_RADIUS)
                ]
                score_ = score_ + 10
            elif (
                self.px + PLAYER_WIDTH > e.x
                and e.x + ENEMY_WIDTH > self.px
                and self.py + PLAYER_HEIGHT > e.y
                and e.y + ENEMY_HEIGHT > self.py
            ):
                blasts_ = blasts_ + [
                    Blast(self.px + 4, self.py + 4, BLAST_START_RADIUS)
                ]
                struck_player = True
            else:
                live_enemies = live_enemies + [e]
        live_bullets: list[Bullet] = []
        for i in range(len(self.bullets)):
            if i in hit:
                continue
            live_bullets = live_bullets + [self.bullets[i]]
        self.enemies = live_enemies
        self.bullets = live_bullets
        self.blasts = blasts_
        self.score = score_
        if struck_player:
            self.scene = SCENE_GAMEOVER


every(0.033, Game.tick)


def view():
    with column(spacing=0, padding=0):
        with canvas(WIDTH, HEIGHT, scale=4, background=0, palette=Game.palette):
            for s in Game.stars:
                pixel(s.x, s.y, s.col)
            if Game.scene == SCENE_TITLE:
                pixel_text(35, 66, "Pyxel Shooter", Game.title_col)
                pixel_text(31, 126, "- PRESS ENTER -", 13)
            elif Game.scene == SCENE_PLAY:
                sprite(Game.px, Game.py, SHEET, 0, 0, PLAYER_WIDTH, PLAYER_HEIGHT, colkey=0)
            else:
                pixel_text(43, 66, "GAME OVER", 8)
                pixel_text(31, 126, "- PRESS ENTER -", 13)
            for b in Game.bullets:
                rect(b.x, b.y, BULLET_WIDTH, BULLET_HEIGHT, BULLET_COLOR)
            for e in Game.enemies:
                sprite(e.x, e.y, SHEET, 8, 0, ENEMY_WIDTH, ENEMY_HEIGHT, colkey=0, flip_x=e.flip)
            for bl in Game.blasts:
                circle(bl.x, bl.y, bl.radius, BLAST_COLOR_IN)
                circle_outline(bl.x, bl.y, bl.radius, BLAST_COLOR_OUT)
            pixel_text(39, 4, f"SCORE {Game.score:5}", 7)


if __name__ == "__main__":
    run(view, title="Pyxel Shooter", width=480.0, height=640.0, on_start=Game.boot)
