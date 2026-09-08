<!-- Written by website/tools/tourpages from the tour. Edit the tour. -->
# The canvas and the keyboard

A grid of virtual pixels, and keys read as a device rather than waited for.

## The canvas

A canvas is a grid of virtual pixels, painted command by command.
`Canvas(width, height)` opens the grid; `Scale` says how many logical
pixels one virtual pixel takes, so a 64x40 canvas at six is 384x240 on
screen; `Palette` names the colors; and `Paint` is handed a painter
with the commands.

<!-- script: advance:50,dump,keydown:left,advance:50,dump -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

var palette = []string{"#11111b", "#89b4fa", "#f38ba8", "#eeeeee", "#a6e3a1"}

type Sky struct {
	frame int
	x, dx int
}

func (s *Sky) tick() {
	s.frame += 1
	// The keyboard is read here, in the tick, never in a view.
	if KeyDown("left") {
		s.dx = -1
	}
	if KeyDown("right") {
		s.dx = 1
	}
	s.x += s.dx
	if s.x < 4 || s.x > 59 {
		s.dx = -s.dx
		s.x += s.dx
	}
}

func (s *Sky) View() Element {
	return Column(
		Canvas(64, 40).Scale(6).Background(0).Palette(palette...).Paint(func(p *Painter) {
			p.Rect(2, 2, 12, 6, 1)
			p.CircleOutline(34, 5, 4, 3)
			p.Line(2, 11, 61, 11, 2)
			p.Circle(s.x, 24, 3, 3)
			p.PixelText(2, 14, fmt.Sprintf("FRAME %d", s.frame), 3)
		}),
	).Spacing(12).Padding(16)
}

func main() {
	app := &Sky{x: 30, dx: 1}
	Every(0.05, func() { app.tick() })
	Run(app, Title("canvas"))
}
```

Every color is a number: the index of a color in the palette. That is
how tools for pixel art work, so drawing code written for one moves
here with its numbers unchanged. The commands on the painter are
`Pixel`, `Line`, `Rect`, `RectOutline`, `Circle`, `CircleOutline`,
`Triangle`, `TriangleOutline`, `Sprite` and `PixelText`; every argument
is a whole virtual pixel, and every argument is written — `Sprite`
takes the sheet, the rectangle to cut, a color key (`-1` copies every
pixel) and two flips, in that order. A loop inside the paint closure
is the ordinary loop: what its body paints joins the frame where it
stands.

The commands are not elements. Nothing painted can be clicked, themed,
sized or animated, and a command means nothing outside the canvas it
was written in. A frame can be had without a window: `PIXIE_FRAMES=<dir>`
writes a PNG of the canvas after every step of a script, drawn by the
same rasterizer the window uses, which is how the two games' recordings
in the gallery were made.


## The keyboard

Two different questions, answered two different ways.

*What is being held right now* is for an app that draws frames.
`KeyDown("left")` is true while the key is held, `KeyPressed` once when
it goes down, `KeyReleased` once when it comes up; the name is the key
alone, never a chord. Read them in a timer, never in a view — a view
that read the keyboard would draw one thing in a window and another
under a script, and the gate would be comparing two different apps.

*What was just pressed* is a chord, and reaches the app as a handler
declared before it runs:

<!-- script: click:+1,key:cmd+s,dump,key:x,menu:Clear,dump -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Keys struct {
	count, saved int
	last         string
}

func (k *Keys) View() Element {
	return Column(
		Text(fmt.Sprintf("count: %d  saved: %d  last key: %s", k.count, k.saved, k.last)),
		Button("+1").OnClick(func() { k.count += 1 }),
	).Spacing(8).Padding(12)
}

func main() {
	app := &Keys{last: "-"}
	MenuItem("Count", "Save", func() { app.saved = app.count })
	MenuItem("Count", "Clear", func() { app.count, app.saved = 0, 0 })
	Shortcut("cmd+s", func() { app.saved = app.count })
	OnKey(func(chord string) { app.last = chord })
	Run(app, Title("keys"))
}
```

`Shortcut` takes a chord spelled the way the platform spells it
(`cmd+s`, `cmd+shift+r`), `MenuItem` puts the same handler in the
application's menu bar in the order declared, `OnKey` is told every
key as the chord it was, and `OnFileDrop` is told the path of a file
dragged onto the window. A script presses one with `key:cmd+s`, picks
one with `menu:Clear`, and drops a file with `drop:<path>`.

