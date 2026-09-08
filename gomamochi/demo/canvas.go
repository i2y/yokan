// A canvas: a grid of virtual pixels, painted command by command.
//
// `Canvas(width, height)` opens the grid; `Scale` says how many logical
// pixels one virtual pixel takes, so a 64x40 canvas at six is 384x240
// on screen; and `Paint` is handed a painter with the commands — Pixel,
// Line, Rect, RectOutline, Circle, CircleOutline, Triangle,
// TriangleOutline, Sprite and PixelText.
//
// Every color is a NUMBER: the index of a color in `Palette`. That is
// how tools for pixel art work, so drawing code written for one moves
// here with its numbers unchanged.
//
// The commands are not elements. Nothing here can be clicked, themed,
// sized or animated, and a loop inside the paint closure is the
// ordinary loop: what its body paints joins the frame where it stands.
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

// Five colors are enough to show that the index IS the color.
var palette = []string{"#11111b", "#89b4fa", "#f38ba8", "#eeeeee", "#a6e3a1"}

type Blip struct {
	x, y, c int
}

type Sky struct {
	frame  int
	ballX  int
	ballY  int
	dx, dy int
	blips  []Blip
}

func (s *Sky) seed() {
	s.blips = []Blip{{6, 4, 1}, {20, 9, 2}, {50, 6, 3}, {58, 30, 4}}
}

func (s *Sky) tick() {
	s.frame += 1
	// The keyboard is read here, in the tick, never in a view.
	// KeyDown is "held right now", so holding an arrow steers.
	if KeyDown("left") {
		s.dx = -1
	}
	if KeyDown("right") {
		s.dx = 1
	}
	if KeyPressed("space") {
		s.dy = -s.dy
	}
	x := s.ballX + s.dx
	y := s.ballY + s.dy
	if x < 4 {
		x = 4
		s.dx = 1
	}
	if x > 59 {
		x = 59
		s.dx = -1
	}
	if y < 4 {
		y = 4
		s.dy = 1
	}
	if y > 35 {
		y = 35
		s.dy = -1
	}
	s.ballX = x
	s.ballY = y
}

func (s *Sky) View() Element {
	return Column(
		Text("Canvas").Size(18).Color("accent"),
		Text("a grid of virtual pixels; every color is an index").Size(12).Color("#8a8f98"),
		Canvas(64, 40).Scale(6).Background(0).Palette(palette...).Paint(func(p *Painter) {
			p.Rect(2, 2, 12, 6, 1)
			p.RectOutline(16, 2, 12, 6, 2)
			p.CircleOutline(34, 5, 4, 3)
			p.Line(2, 11, 61, 11, 2)
			p.Triangle(3, 37, 8, 28, 13, 37, 4)
			for _, b := range s.blips {
				p.Pixel(b.x, b.y, b.c)
			}
			p.Circle(s.ballX, s.ballY, 3, 3)
			p.PixelText(2, 14, fmt.Sprintf("FRAME %d", s.frame), 3)
		}),
		Row(
			Button("seed").OnClick(func() { s.seed() }),
		).Spacing(8),
	).Spacing(12).Padding(16)
}

func main() {
	app := &Sky{ballX: 30, ballY: 18, dx: 1, dy: 1}
	app.seed()
	Every(0.05, func() { app.tick() })
	Run(app, Title("canvas"))
}
