# Pyxel Shooter, ported to Wakakusa.
#
# The original is `10_platformer.py`'s sibling `shooter.py` from
# Pyxel's examples (Takashi Kitao, MIT,
# https://github.com/kitao/pyxel), and `assets/shooter.png` is that
# example's own image bank written out with Pyxel's palette.
#
# What is different, and why. The effects are WAV files written by
# `tools/gen_sounds.rb` rather than the original's chiptune, since the
# engine plays files; a run under a script is silent, so the gate still
# compares two silent runs. And the numbers come from a generator
# written here rather than from `rand`, because a seeded `rand` gives
# the two runs different sequences and a game whose enemies arrive in
# different places is not one the gate can compare.
#
# Arrows move, space fires, enter starts and restarts, q closes.
require "wakakusa"

WIDTH = 120
HEIGHT = 160

SND_SHOOT = "demo/assets/sound/shoot.wav"
SND_BLAST = "demo/assets/sound/blast.wav"
SND_OVER = "demo/assets/sound/over.wav"

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

# Pyxel's own sixteen colors, which is what makes the numbers in this
# file mean what they mean in the original.
PALETTE = [
  "#000000", "#2b335f", "#7e2072", "#19959c",
  "#8b4852", "#395c98", "#a9c1ff", "#eeeeee",
  "#d4186c", "#d38441", "#e9c35b", "#70c6a9",
  "#7696de", "#a3a3a3", "#ff9798", "#edc7b0"
].freeze

# Whole numbers from arithmetic alone, so both runs play one game.
class Roll
  def initialize(seed)
    @s = seed
  end

  def int(lo, hi)
    @s = (@s * 1103515245 + 12345) % 2147483648
    lo + ((@s >> 8) % (hi - lo + 1))
  end
end

class Star
  # `y` is what the canvas draws; `y10` is where the star really is.
  attr_reader :x, :y, :y10, :speed10, :col

  def initialize(x, y, y10, speed10, col)
    @x = x
    @y = y
    @y10 = y10
    @speed10 = speed10
    @col = col
  end
end

class Bullet
  attr_reader :x, :y

  def initialize(x, y)
    @x = x
    @y = y
  end
end

class Enemy
  attr_reader :x, :y, :x10, :y10, :flip, :offset

  def initialize(x, y, x10, y10, flip, offset)
    @x = x
    @y = y
    @x10 = x10
    @y10 = y10
    @flip = flip
    @offset = offset
  end
end

class Blast
  attr_reader :x, :y, :radius

  def initialize(x, y, radius)
    @x = x
    @y = y
    @radius = radius
  end
end

class Game
  def initialize
    @scene = SCENE_TITLE
    @score = 0
    @frame = 0
    @title_col = 0
    @px = 56
    @py = 140
    @stars = []
    @bullets = []
    @enemies = []
    @blasts = []
    @live_enemies = []
    @hit_bullets = []
    @player_struck = false
    @enemy_struck = false
    @roll = Roll.new(7)
    NUM_STARS.times { boot_star }
  end

  def boot_star
    x = @roll.int(0, WIDTH - 1)
    y = @roll.int(0, HEIGHT - 1)
    speed10 = @roll.int(10, 25)
    col = speed10 > 18 ? STAR_COLOR_HIGH : STAR_COLOR_LOW
    @stars.push(Star.new(x, y, y * 10, speed10, col))
  end

  def tick
    quit if key_pressed("q")
    @frame += 1
    @title_col = @frame % 16
    move_stars
    if @scene == SCENE_TITLE
      @scene = SCENE_PLAY if key_pressed("enter")
    elsif @scene == SCENE_PLAY
      play
    else
      over
    end
  end

  def move_stars
    out = []
    @stars.each { |s| out.push(next_star(s)) }
    @stars = out
  end

  def next_star(s)
    y10 = s.y10 + s.speed10
    y10 -= HEIGHT * 10 if y10 >= HEIGHT * 10
    Star.new(s.x, y10 / 10, y10, s.speed10, s.col)
  end

  def play
    if (@frame % 6).zero?
      x = @roll.int(0, WIDTH - ENEMY_WIDTH)
      @enemies.push(Enemy.new(x, 0, x * 10, 0, false, @roll.int(0, 59)))
    end
    collide
    move_player
    move_bullets
    move_enemies
    move_blasts
  end

  def over
    move_bullets
    move_enemies
    move_blasts
    return unless key_pressed("enter")

    @scene = SCENE_PLAY
    @px = 56
    @py = 140
    @score = 0
    @enemies = []
    @bullets = []
    @blasts = []
  end

  def move_player
    x = @px
    y = @py
    x -= PLAYER_SPEED if key_down("left")
    x += PLAYER_SPEED if key_down("right")
    y -= PLAYER_SPEED if key_down("up")
    y += PLAYER_SPEED if key_down("down")
    @px = [[x, 0].max, WIDTH - PLAYER_WIDTH].min
    @py = [[y, 0].max, HEIGHT - PLAYER_HEIGHT].min
    return unless key_pressed("space")

    @bullets.push(Bullet.new(@px + 3, @py - 4))
    audio_play(SND_SHOOT, 0.35)
  end

  def move_bullets
    out = []
    @bullets.each { |b| keep_bullet(out, b) }
    @bullets = out
  end

  def keep_bullet(out, b)
    y = b.y - BULLET_SPEED
    out.push(Bullet.new(b.x, y)) if y + BULLET_HEIGHT - 1 >= 0
  end

  def move_enemies
    out = []
    @enemies.each { |e| keep_enemy(out, e) }
    @enemies = out
  end

  def keep_enemy(out, e)
    x10 = e.x10
    flip = true
    if (@frame + e.offset) % 60 < 30
      x10 += ENEMY_SPEED
      flip = false
    else
      x10 -= ENEMY_SPEED
    end
    y10 = e.y10 + ENEMY_SPEED
    out.push(Enemy.new(x10 / 10, y10 / 10, x10, y10, flip, e.offset)) if y10 <= (HEIGHT - 1) * 10
  end

  def move_blasts
    out = []
    @blasts.each { |b| keep_blast(out, b) }
    @blasts = out
  end

  def keep_blast(out, b)
    r = b.radius + 1
    out.push(Blast.new(b.x, b.y, r)) if r <= BLAST_END_RADIUS
  end

  # The two rectangle tests, resolved into new lists. Where the
  # original sets `is_alive = False` and filters afterwards, this keeps
  # the ones that live.
  #
  # The three names below are a frame's working room rather than state
  # the game has: a block cannot write the local outside it, so what
  # the loop collects is collected on the object.
  def collide
    @live_enemies = []
    @hit_bullets = []
    @player_struck = false
    @enemies.each { |e| resolve(e) }
    live = []
    @bullets.each_with_index { |b, i| live.push(b) unless @hit_bullets.include?(i) }
    @enemies = @live_enemies
    @bullets = live
    @scene = SCENE_GAMEOVER if @player_struck
  end

  def resolve(e)
    @enemy_struck = false
    @bullets.each_with_index { |b, i| note_hit(e, b, i) }
    if @enemy_struck
      @blasts.push(Blast.new(e.x + 4, e.y + 4, BLAST_START_RADIUS))
      @score += 10
      audio_play(SND_BLAST, 0.5)
    elsif rammed?(e)
      @blasts.push(Blast.new(@px + 4, @py + 4, BLAST_START_RADIUS))
      @player_struck = true
      audio_play(SND_OVER, 0.6)
    else
      @live_enemies.push(e)
    end
  end

  def note_hit(e, b, i)
    return unless shot?(e, b)

    @enemy_struck = true
    @hit_bullets.push(i)
  end

  def shot?(e, b)
    e.x + ENEMY_WIDTH > b.x && b.x + BULLET_WIDTH > e.x &&
      e.y + ENEMY_HEIGHT > b.y && b.y + BULLET_HEIGHT > e.y
  end

  def rammed?(e)
    @px + PLAYER_WIDTH > e.x && e.x + ENEMY_WIDTH > @px &&
      @py + PLAYER_HEIGHT > e.y && e.y + ENEMY_HEIGHT > @py
  end

  def star(s)
    pixel(s.x, s.y, s.col)
  end

  def bullet(b)
    rect(b.x, b.y, BULLET_WIDTH, BULLET_HEIGHT, BULLET_COLOR)
  end

  def enemy(e)
    sprite(e.x, e.y, SHEET, 8, 0, ENEMY_WIDTH, ENEMY_HEIGHT, colkey: 0, flip_x: e.flip)
  end

  def blast(b)
    circle(b.x, b.y, b.radius, BLAST_COLOR_IN)
    circle_outline(b.x, b.y, b.radius, BLAST_COLOR_OUT)
  end

  def view
    column(spacing: 0.0, padding: 0.0) {
      canvas(WIDTH, HEIGHT, scale: 4, background: 0, palette: PALETTE) {
        @stars.each { |s| star(s) }
        if @scene == SCENE_TITLE
          pixel_text(35, 66, "Pyxel Shooter", @title_col)
          pixel_text(31, 126, "- PRESS ENTER -", 13)
        elsif @scene == SCENE_PLAY
          sprite(@px, @py, SHEET, 0, 0, PLAYER_WIDTH, PLAYER_HEIGHT, colkey: 0)
        else
          pixel_text(43, 66, "GAME OVER", 8)
          pixel_text(31, 126, "- PRESS ENTER -", 13)
        end
        @bullets.each { |b| bullet(b) }
        @enemies.each { |e| enemy(e) }
        @blasts.each { |b| blast(b) }
        pixel_text(39, 4, format("SCORE %5d", @score), 7)
      }
    }
  end
end

app = Game.new
every(0.033) { app.tick }
# `padding: 0.0`: the canvas IS the app, so it paints to the window's
# edge rather than sitting inside the engine's ring.
run(app, title: "Pyxel Shooter", width: 480.0, height: 640.0, padding: 0.0)
