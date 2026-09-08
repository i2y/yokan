// Pyxel Jump, ported to Gomamochi.
//
// The original is `02_jump_game.py` from Pyxel's examples (Takashi
// Kitao, MIT, https://github.com/kitao/pyxel), and `assets/jump.png` is
// that example's own image bank written out with Pyxel's palette. The
// port follows it line by line: `pyxel.blt` becomes `Sprite`,
// `pyxel.btn` becomes `KeyDown`, `pyxel.cls(12)` becomes the canvas
// background, and 12 still means the same color, because inside a
// canvas a color is an index into the palette this file declares.
//
// What is different, and why. The three effects are WAV files rather
// than the original's chiptune, since the engine plays files; a run
// under a script is silent, so the gate still compares two silent
// runs. And the numbers come from a generator written here rather than
// from a random source, so that a reader can follow every floor to its
// place. Everything else is the game.
//
// Left and right move; the rest is gravity.
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

const (
	width     = 160
	height    = 120
	sky       = 12
	sheet     = "demo/assets/jump.png"
	sndBounce = "demo/assets/sound/jump.wav"
	sndFruit  = "demo/assets/sound/pickup.wav"
	sndOver   = "demo/assets/sound/over.wav"
)

var palette = []string{
	"#000000", "#2b335f", "#7e2072", "#19959c",
	"#8b4852", "#395c98", "#a9c1ff", "#eeeeee",
	"#d4186c", "#d38441", "#e9c35b", "#70c6a9",
	"#7696de", "#a3a3a3", "#ff9798", "#edc7b0",
}

// Whole numbers from arithmetic alone, so both runs draw one game.
type Roll struct{ s int }

func (r *Roll) next(lo, hi int) int {
	r.s = (r.s*1103515245 + 12345) % 2147483648
	return lo + ((r.s >> 8) % (hi - lo + 1))
}

type Cloud struct{ x, y int }

type Floor struct {
	x, y  int
	alive bool
}

type Fruit struct {
	x, y, kind int
	alive      bool
}

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

func absInt(a int) int {
	if a < 0 {
		return -a
	}
	return a
}

type Game struct {
	score int
	px    int
	py    int
	dy    int
	alive bool
	frame int
	// What the view needs whole: the parallax offsets and which of the
	// two player sprites to cut out.
	treeOff int
	farOff  int
	nearOff int
	playerU int
	roll    *Roll
	far     []Cloud
	near    []Cloud
	floors  []Floor
	fruits  []Fruit
}

func newGame() *Game {
	g := &Game{
		px: 72, py: -16, alive: true,
		roll: &Roll{s: 11},
		far:  []Cloud{{-10, 75}, {40, 65}, {90, 60}},
		near: []Cloud{{10, 25}, {70, 35}, {120, 15}},
	}
	for i := 0; i < 4; i++ {
		g.bootOne(i)
	}
	return g
}

func (g *Game) bootOne(i int) {
	g.floors = append(g.floors, Floor{i * 60, g.roll.next(8, 104), true})
	g.fruits = append(g.fruits, Fruit{i * 60, g.roll.next(0, 104), g.roll.next(0, 2), true})
}

func (g *Game) tick() {
	g.frame += 1
	g.treeOff = g.frame % 160
	g.farOff = (g.frame / 16) % 160
	g.nearOff = (g.frame / 8) % 160
	g.updatePlayer()
	g.updateFloors()
	g.updateFruits()
}

func (g *Game) updatePlayer() {
	if KeyDown("left") {
		g.px = maxInt(g.px-2, 0)
	}
	if KeyDown("right") {
		g.px = minInt(g.px+2, width-16)
	}
	g.py += g.dy
	g.dy = minInt(g.dy+1, 8)
	g.playerU = 0
	if g.dy > 0 {
		g.playerU = 16
	}
	if g.py <= height {
		return
	}
	if g.alive {
		AudioPlay(sndOver, 0.5)
	}
	g.alive = false
	if g.py <= 600 {
		return
	}
	g.score = 0
	g.px = 72
	g.py = -16
	g.dy = 0
	g.alive = true
}

// A floor the player lands on drops away and bounces them. The
// original edits the tuple in the list; these are built fresh instead,
// and the bounce it writes to `dy` is what the floors after this one
// see.
func (g *Game) updateFloors() {
	out := make([]Floor, 0, len(g.floors))
	for _, f := range g.floors {
		out = append(out, g.nextFloor(f))
	}
	g.floors = out
}

func (g *Game) nextFloor(f Floor) Floor {
	x, y, alive := f.x, f.y, f.alive
	if alive {
		if g.px+16 >= x && g.px <= x+40 && g.py+16 >= y && g.py <= y+8 && g.dy > 0 {
			alive = false
			g.score += 10
			g.dy = -12
			AudioPlay(sndBounce, 0.5)
		}
	} else {
		y += 6
	}
	x -= 4
	if x < -40 {
		x += 240
		y = g.roll.next(8, 104)
		alive = true
	}
	return Floor{x, y, alive}
}

func (g *Game) updateFruits() {
	out := make([]Fruit, 0, len(g.fruits))
	for _, f := range g.fruits {
		out = append(out, g.nextFruit(f))
	}
	g.fruits = out
}

func (g *Game) nextFruit(f Fruit) Fruit {
	x, y, kind, alive := f.x, f.y, f.kind, f.alive
	if alive && absInt(x-g.px) < 12 && absInt(y-g.py) < 12 {
		alive = false
		g.score += (kind + 1) * 100
		g.dy = minInt(g.dy, -8)
		AudioPlay(sndFruit, 0.5)
	}
	x -= 2
	if x < -40 {
		x += 240
		y = g.roll.next(0, 104)
		kind = g.roll.next(0, 2)
		alive = true
	}
	return Fruit{x, y, kind, alive}
}

func (g *Game) tree(p *Painter, i int) {
	p.Sprite(i*160-g.treeOff, 104, sheet, 0, 48, 160, 16, sky, false, false)
}

func (g *Game) farCloud(p *Painter, c Cloud, i int) {
	p.Sprite(c.x+i*160-g.farOff, c.y, sheet, 64, 32, 32, 8, sky, false, false)
}

func (g *Game) nearCloud(p *Painter, c Cloud, i int) {
	p.Sprite(c.x+i*160-g.nearOff, c.y, sheet, 0, 32, 56, 8, sky, false, false)
}

func (g *Game) floorOf(p *Painter, f Floor) {
	p.Sprite(f.x, f.y, sheet, 0, 16, 40, 8, sky, false, false)
}

func (g *Game) fruitOf(p *Painter, f Fruit) {
	if f.alive {
		p.Sprite(f.x, f.y, sheet, 32+f.kind*16, 0, 16, 16, sky, false, false)
	}
}

func (g *Game) View() Element {
	return Column(
		Canvas(width, height).Scale(4).Background(sky).Palette(palette...).Paint(func(p *Painter) {
			// sky, mountain, and the trees that scroll fastest
			p.Sprite(0, 88, sheet, 0, 88, 160, 32, -1, false, false)
			p.Sprite(0, 88, sheet, 0, 64, 160, 24, sky, false, false)
			for i := 0; i < 2; i++ {
				g.tree(p, i)
			}
			// two layers of cloud, each strip drawn twice so it wraps
			for i := 0; i < 2; i++ {
				for _, c := range g.far {
					g.farCloud(p, c, i)
				}
			}
			for i := 0; i < 2; i++ {
				for _, c := range g.near {
					g.nearCloud(p, c, i)
				}
			}
			for _, f := range g.floors {
				g.floorOf(p, f)
			}
			for _, f := range g.fruits {
				g.fruitOf(p, f)
			}
			p.Sprite(g.px, g.py, sheet, g.playerU, 0, 16, 16, sky, false, false)
			p.PixelText(5, 4, fmt.Sprintf("SCORE %4d", g.score), 1)
			p.PixelText(4, 4, fmt.Sprintf("SCORE %4d", g.score), 7)
		}),
	).Spacing(0).Padding(0)
}

func main() {
	app := newGame()
	Every(0.033, func() { app.tick() })
	Run(app, Title("Pyxel Jump"), Size(640, 480), Padding(0))
}
