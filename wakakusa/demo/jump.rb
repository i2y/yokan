# Pyxel Jump, ported to Wakakusa.
#
# The original is `02_jump_game.py` from Pyxel's examples (Takashi
# Kitao, MIT, https://github.com/kitao/pyxel), and `assets/jump.png` is
# that example's own image bank written out with Pyxel's palette. The
# port follows it line by line: `pyxel.blt` becomes `sprite`,
# `pyxel.btn` becomes `key_down`, `pyxel.cls(12)` becomes the canvas
# background, and 12 still means the same color, because inside a
# canvas a color is an index into the palette this file declares.
#
# What is different, and why. There is no sound: the engine has no
# audio verb yet, so the three effects the original plays are gone. And
# the numbers come from a generator written here rather than from
# `rand`: a seeded `rand` gives the two runs different sequences, and a
# game whose floors land in different places is not one the gate can
# compare. Everything else is the game.
#
# Left and right move; the rest is gravity.
require "wakakusa"

WIDTH = 160
HEIGHT = 120
SKY = 12
SHEET = "demo/assets/jump.png"

PALETTE = [
  "#000000", "#2b335f", "#7e2072", "#19959c",
  "#8b4852", "#395c98", "#a9c1ff", "#eeeeee",
  "#d4186c", "#d38441", "#e9c35b", "#70c6a9",
  "#7696de", "#a3a3a3", "#ff9798", "#edc7b0"
].freeze

# Whole numbers from arithmetic alone, so both runs draw one game.
class Roll
  def initialize(seed)
    @s = seed
  end

  def int(lo, hi)
    @s = (@s * 1103515245 + 12345) % 2147483648
    lo + ((@s >> 8) % (hi - lo + 1))
  end
end

class Cloud
  attr_reader :x, :y

  def initialize(x, y)
    @x = x
    @y = y
  end
end

class Floor
  attr_reader :x, :y, :alive

  def initialize(x, y, alive)
    @x = x
    @y = y
    @alive = alive
  end
end

class Fruit
  attr_reader :x, :y, :kind, :alive

  def initialize(x, y, kind, alive)
    @x = x
    @y = y
    @kind = kind
    @alive = alive
  end
end

class Game
  def initialize
    @score = 0
    @px = 72
    @py = -16
    @dy = 0
    @alive = true
    @frame = 0
    # What the view needs whole: the parallax offsets and which of the
    # two player sprites to cut out.
    @tree_off = 0
    @far_off = 0
    @near_off = 0
    @player_u = 0
    @roll = Roll.new(11)
    @far = [Cloud.new(-10, 75), Cloud.new(40, 65), Cloud.new(90, 60)]
    @near = [Cloud.new(10, 25), Cloud.new(70, 35), Cloud.new(120, 15)]
    @floors = []
    @fruits = []
    4.times { |i| boot_one(i) }
  end

  def boot_one(i)
    @floors.push(Floor.new(i * 60, @roll.int(8, 104), true))
    @fruits.push(Fruit.new(i * 60, @roll.int(0, 104), @roll.int(0, 2), true))
  end

  def tick
    @frame += 1
    @tree_off = @frame % 160
    @far_off = (@frame / 16) % 160
    @near_off = (@frame / 8) % 160
    update_player
    update_floors
    update_fruits
  end

  def update_player
    @px = [@px - 2, 0].max if key_down("left")
    @px = [@px + 2, WIDTH - 16].min if key_down("right")
    @py += @dy
    @dy = [@dy + 1, 8].min
    @player_u = 0
    @player_u = 16 if @dy > 0
    return if @py <= HEIGHT

    @alive = false
    return if @py <= 600

    @score = 0
    @px = 72
    @py = -16
    @dy = 0
    @alive = true
  end

  # A floor the player lands on drops away and bounces them. The
  # original edits the tuple in the list; these are built fresh
  # instead, and `dy` is carried in a local because the bounce it
  # writes is what the floors after this one see.
  def update_floors
    out = []
    @floors.each { |f| out.push(next_floor(f)) }
    @floors = out
  end

  def next_floor(f)
    x = f.x
    y = f.y
    alive = f.alive
    if alive
      if @px + 16 >= x && @px <= x + 40 && @py + 16 >= y && @py <= y + 8 && @dy > 0
        alive = false
        @score += 10
        @dy = -12
      end
    else
      y += 6
    end
    x -= 4
    if x < -40
      x += 240
      y = @roll.int(8, 104)
      alive = true
    end
    Floor.new(x, y, alive)
  end

  def update_fruits
    out = []
    @fruits.each { |f| out.push(next_fruit(f)) }
    @fruits = out
  end

  def next_fruit(f)
    x = f.x
    y = f.y
    kind = f.kind
    alive = f.alive
    if alive && (x - @px).abs < 12 && (y - @py).abs < 12
      alive = false
      @score += (kind + 1) * 100
      @dy = [@dy, -8].min
    end
    x -= 2
    if x < -40
      x += 240
      y = @roll.int(0, 104)
      kind = @roll.int(0, 2)
      alive = true
    end
    Fruit.new(x, y, kind, alive)
  end

  def tree(i)
    sprite(i * 160 - @tree_off, 104, SHEET, 0, 48, 160, 16, colkey: SKY)
  end

  def far_cloud(c, i)
    sprite(c.x + i * 160 - @far_off, c.y, SHEET, 64, 32, 32, 8, colkey: SKY)
  end

  def near_cloud(c, i)
    sprite(c.x + i * 160 - @near_off, c.y, SHEET, 0, 32, 56, 8, colkey: SKY)
  end

  def far_strip(i)
    @far.each { |c| far_cloud(c, i) }
  end

  def near_strip(i)
    @near.each { |c| near_cloud(c, i) }
  end

  def floor_of(f)
    sprite(f.x, f.y, SHEET, 0, 16, 40, 8, colkey: SKY)
  end

  def fruit_of(f)
    sprite(f.x, f.y, SHEET, 32 + f.kind * 16, 0, 16, 16, colkey: SKY) if f.alive
  end

  def view
    column(spacing: 0.0, padding: 0.0) {
      canvas(WIDTH, HEIGHT, scale: 4, background: SKY, palette: PALETTE) {
        # sky, mountain, and the trees that scroll fastest
        sprite(0, 88, SHEET, 0, 88, 160, 32)
        sprite(0, 88, SHEET, 0, 64, 160, 24, colkey: SKY)
        2.times { |i| tree(i) }
        # two layers of cloud, each strip drawn twice so it wraps
        2.times { |i| far_strip(i) }
        2.times { |i| near_strip(i) }
        @floors.each { |f| floor_of(f) }
        @fruits.each { |f| fruit_of(f) }
        sprite(@px, @py, SHEET, @player_u, 0, 16, 16, colkey: SKY)
        pixel_text(5, 4, format("SCORE %4d", @score), 1)
        pixel_text(4, 4, format("SCORE %4d", @score), 7)
      }
    }
  end
end

app = Game.new
every(0.033) { app.tick }
run(app, title: "Pyxel Jump", width: 640.0, height: 480.0, padding: 0.0)
