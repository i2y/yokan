// Pyxel Shooter, ported to Gomamochi.
//
// The original is `shooter.py` from Pyxel's examples (Takashi Kitao,
// MIT, https://github.com/kitao/pyxel), and `assets/shooter.png` is that
// example's own image bank written out with Pyxel's palette.
//
// What is different, and why. The effects are WAV files rather than
// the original's chiptune, since the engine plays files; a run under a
// script is silent, so the gate still compares two silent runs. And the
// numbers come from a generator written here rather than from a random
// source, so that a reader can follow every enemy to its place.
//
// Arrows move, space fires, enter starts and restarts, q closes.
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

const (
	width  = 120
	height = 160

	sndShoot = "demo/assets/sound/shoot.wav"
	sndBlast = "demo/assets/sound/blast.wav"
	sndOver  = "demo/assets/sound/over.wav"

	sceneTitle    = 0
	scenePlay     = 1
	sceneGameOver = 2

	numStars      = 100
	starColorHigh = 12
	starColorLow  = 5

	playerWidth  = 8
	playerHeight = 8
	playerSpeed  = 2

	bulletWidth  = 2
	bulletHeight = 8
	bulletColor  = 11
	bulletSpeed  = 4

	enemyWidth  = 8
	enemyHeight = 8
	// Pyxel's 1.5 px a frame, in tenths.
	enemySpeed = 15

	blastStartRadius = 1
	blastEndRadius   = 8
	blastColorIn     = 7
	blastColorOut    = 10

	sheet = "demo/assets/shooter.png"
)

// Pyxel's own sixteen colors, which is what makes the numbers in this
// file mean what they mean in the original.
var palette = []string{
	"#000000", "#2b335f", "#7e2072", "#19959c",
	"#8b4852", "#395c98", "#a9c1ff", "#eeeeee",
	"#d4186c", "#d38441", "#e9c35b", "#70c6a9",
	"#7696de", "#a3a3a3", "#ff9798", "#edc7b0",
}

// Whole numbers from arithmetic alone, so both runs play one game.
type Roll struct{ s int }

func (r *Roll) next(lo, hi int) int {
	r.s = (r.s*1103515245 + 12345) % 2147483648
	return lo + ((r.s >> 8) % (hi - lo + 1))
}

// Star: `y` is what the canvas draws; `y10` is where the star really is.
type Star struct{ x, y, y10, speed10, col int }

type Bullet struct{ x, y int }

type Enemy struct {
	x, y, x10, y10 int
	flip           bool
	offset         int
}

type Blast struct{ x, y, radius int }

func maxInt(a, b int) int {
	if a > b {
		return a
	}
	return b
}

func minInt(a, b int) int {
	if a < b {
		return a
	}
	return b
}

// The original's `//` rounds toward minus infinity, and an enemy
// sweeping off the left edge has a negative tenth.
func div10(v int) int {
	if v < 0 && v%10 != 0 {
		return v/10 - 1
	}
	return v / 10
}

type Game struct {
	scene    int
	score    int
	frame    int
	titleCol int
	px, py   int
	stars    []Star
	bullets  []Bullet
	enemies  []Enemy
	blasts   []Blast
	roll     *Roll
}

func newGame() *Game {
	g := &Game{px: 56, py: 140, roll: &Roll{s: 7}}
	for i := 0; i < numStars; i++ {
		g.bootStar()
	}
	return g
}

func (g *Game) bootStar() {
	x := g.roll.next(0, width-1)
	y := g.roll.next(0, height-1)
	speed10 := g.roll.next(10, 25)
	col := starColorLow
	if speed10 > 18 {
		col = starColorHigh
	}
	g.stars = append(g.stars, Star{x, y, y * 10, speed10, col})
}

func (g *Game) tick() {
	if KeyPressed("q") {
		Quit()
	}
	g.frame += 1
	g.titleCol = g.frame % 16
	g.moveStars()
	switch g.scene {
	case sceneTitle:
		if KeyPressed("enter") {
			g.scene = scenePlay
		}
	case scenePlay:
		g.play()
	default:
		g.over()
	}
}

func (g *Game) moveStars() {
	out := make([]Star, 0, len(g.stars))
	for _, s := range g.stars {
		y10 := s.y10 + s.speed10
		if y10 >= height*10 {
			y10 -= height * 10
		}
		out = append(out, Star{s.x, y10 / 10, y10, s.speed10, s.col})
	}
	g.stars = out
}

func (g *Game) play() {
	if g.frame%6 == 0 {
		x := g.roll.next(0, width-enemyWidth)
		g.enemies = append(g.enemies, Enemy{x, 0, x * 10, 0, false, g.roll.next(0, 59)})
	}
	g.collide()
	g.movePlayer()
	g.moveBullets()
	g.moveEnemies()
	g.moveBlasts()
}

func (g *Game) over() {
	g.moveBullets()
	g.moveEnemies()
	g.moveBlasts()
	if !KeyPressed("enter") {
		return
	}
	g.scene = scenePlay
	g.px = 56
	g.py = 140
	g.score = 0
	g.enemies = nil
	g.bullets = nil
	g.blasts = nil
}

func (g *Game) movePlayer() {
	x, y := g.px, g.py
	if KeyDown("left") {
		x -= playerSpeed
	}
	if KeyDown("right") {
		x += playerSpeed
	}
	if KeyDown("up") {
		y -= playerSpeed
	}
	if KeyDown("down") {
		y += playerSpeed
	}
	g.px = minInt(maxInt(x, 0), width-playerWidth)
	g.py = minInt(maxInt(y, 0), height-playerHeight)
	if !KeyPressed("space") {
		return
	}
	g.bullets = append(g.bullets, Bullet{g.px + 3, g.py - 4})
	AudioPlay(sndShoot, 0.35)
}

func (g *Game) moveBullets() {
	out := make([]Bullet, 0, len(g.bullets))
	for _, b := range g.bullets {
		y := b.y - bulletSpeed
		if y+bulletHeight-1 >= 0 {
			out = append(out, Bullet{b.x, y})
		}
	}
	g.bullets = out
}

func (g *Game) moveEnemies() {
	out := make([]Enemy, 0, len(g.enemies))
	for _, e := range g.enemies {
		x10 := e.x10
		flip := true
		if (g.frame+e.offset)%60 < 30 {
			x10 += enemySpeed
			flip = false
		} else {
			x10 -= enemySpeed
		}
		y10 := e.y10 + enemySpeed
		if y10 <= (height-1)*10 {
			out = append(out, Enemy{div10(x10), y10 / 10, x10, y10, flip, e.offset})
		}
	}
	g.enemies = out
}

func (g *Game) moveBlasts() {
	out := make([]Blast, 0, len(g.blasts))
	for _, b := range g.blasts {
		r := b.radius + 1
		if r <= blastEndRadius {
			out = append(out, Blast{b.x, b.y, r})
		}
	}
	g.blasts = out
}

// The two rectangle tests, resolved into new lists. Where the original
// sets `is_alive = False` and filters afterwards, this keeps the ones
// that live.
func (g *Game) collide() {
	live := make([]Enemy, 0, len(g.enemies))
	hit := make([]bool, len(g.bullets))
	struck := false
	for _, e := range g.enemies {
		enemyStruck := false
		for i, b := range g.bullets {
			if shot(e, b) {
				enemyStruck = true
				hit[i] = true
			}
		}
		if enemyStruck {
			g.blasts = append(g.blasts, Blast{e.x + 4, e.y + 4, blastStartRadius})
			g.score += 10
			AudioPlay(sndBlast, 0.5)
		} else if g.rammed(e) {
			g.blasts = append(g.blasts, Blast{g.px + 4, g.py + 4, blastStartRadius})
			struck = true
			AudioPlay(sndOver, 0.6)
		} else {
			live = append(live, e)
		}
	}
	kept := make([]Bullet, 0, len(g.bullets))
	for i, b := range g.bullets {
		if !hit[i] {
			kept = append(kept, b)
		}
	}
	g.enemies = live
	g.bullets = kept
	if struck {
		g.scene = sceneGameOver
	}
}

func shot(e Enemy, b Bullet) bool {
	return e.x+enemyWidth > b.x && b.x+bulletWidth > e.x &&
		e.y+enemyHeight > b.y && b.y+bulletHeight > e.y
}

func (g *Game) rammed(e Enemy) bool {
	return g.px+playerWidth > e.x && e.x+enemyWidth > g.px &&
		g.py+playerHeight > e.y && e.y+enemyHeight > g.py
}

func (g *Game) View() Element {
	return Column(
		Canvas(width, height).Scale(4).Background(0).Palette(palette...).Paint(func(p *Painter) {
			for _, s := range g.stars {
				p.Pixel(s.x, s.y, s.col)
			}
			switch g.scene {
			case sceneTitle:
				p.PixelText(35, 66, "Pyxel Shooter", g.titleCol)
				p.PixelText(31, 126, "- PRESS ENTER -", 13)
			case scenePlay:
				p.Sprite(g.px, g.py, sheet, 0, 0, playerWidth, playerHeight, 0, false, false)
			default:
				p.PixelText(43, 66, "GAME OVER", 8)
				p.PixelText(31, 126, "- PRESS ENTER -", 13)
			}
			for _, b := range g.bullets {
				p.Rect(b.x, b.y, bulletWidth, bulletHeight, bulletColor)
			}
			for _, e := range g.enemies {
				p.Sprite(e.x, e.y, sheet, 8, 0, enemyWidth, enemyHeight, 0, e.flip, false)
			}
			for _, b := range g.blasts {
				p.Circle(b.x, b.y, b.radius, blastColorIn)
				p.CircleOutline(b.x, b.y, b.radius, blastColorOut)
			}
			p.PixelText(39, 4, fmt.Sprintf("SCORE %5d", g.score), 7)
		}),
	).Spacing(0).Padding(0)
}

func main() {
	app := newGame()
	Every(0.033, func() { app.tick() })
	// Padding 0: the canvas IS the app, so it paints to the window's
	// edge rather than sitting inside the engine's ring.
	Run(app, Title("Pyxel Shooter"), Size(480, 640), Padding(0))
}
