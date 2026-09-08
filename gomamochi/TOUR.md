# Gomamochi language tour

Gomamochi builds desktop apps in Go on pixie's engine. `gomamochi run`
reads your file and runs it interpreted, with no build step: save, and
the window takes the new code and keeps the values it had. `gomamochi
build` compiles the same file with the Go compiler into a native
binary. Both reach the same drawing engine (**gpui**, the engine
behind the Zed editor) through pixie's C face, and whether the two are
the same program is something you check rather than hope: `gomamochi
gate` drives both with one interaction script and compares the screens
they drew, byte for byte. The functions that build the screen — `Text`,
`Button`, `Column` and thirty more — come with the package; the app
itself is a plain Go struct. How much of Go the interpreted run takes
is the dialect this tour describes, and what falls outside it is named
at [What Gomamochi refuses](#what-gomamochi-refuses).

This page is the language, in the order you meet it. Everything in it
runs: `go run ./tools/tourcheck` pulls every complete example out of
this file and puts it through the same command a demo goes through, so
a rename in the vocabulary breaks this page before a reader meets it.
日本語版は [TOUR.ja.md](TOUR.ja.md).

## Table of contents

- [The smallest app](#the-smallest-app)
- [Holding state](#holding-state)
- [Writing views](#writing-views)
- [Control flow in a view](#control-flow-in-a-view)
- [Form controls](#form-controls)
- [Handlers](#handlers)
- [Lists, charts, and rows built on demand](#lists-charts-and-rows-built-on-demand)
- [Maps](#maps)
- [Value structs](#value-structs)
- [The canvas](#the-canvas)
- [The keyboard](#the-keyboard)
- [The keywords every element takes](#the-keywords-every-element-takes)
- [Themes and animation](#themes-and-animation)
- [The window](#the-window)
- [Go's own standard library](#gos-own-standard-library)
- [The framework's standard library](#the-frameworks-standard-library)
- [Timers and work off the window's thread](#timers-and-work-off-the-windows-thread)
- [While you are writing it](#while-you-are-writing-it)
- [Headless runs and the gate](#headless-runs-and-the-gate)
- [What Gomamochi refuses](#what-gomamochi-refuses)
- [Shipping](#shipping)
- [What does not work yet](#what-does-not-work-yet)

## The smallest app

<!-- script: click:+1,dump,input:Momo -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Counter struct {
	count int
	name  string
}

func (c *Counter) View() Element {
	return Column(
		Text(fmt.Sprintf("count: %d", c.count)).Size(34),
		Row(
			Button("+1").OnClick(func() { c.count += 1 }),
			Button("reset").OnClick(func() { c.count = 0 }),
		).Spacing(8),
		TextField(c.name).Placeholder("your name").OnChange(func(s string) { c.name = s }),
		Text(fmt.Sprintf("hello, %s", c.name)),
	).Spacing(12).Padding(16)
}

func main() {
	Run(&Counter{}, Title("counter"))
}
```

An app is a `package main` with a struct in it. The struct's fields are
the state, `View` is a method that answers one element, and a handler
is a closure over the struct through its pointer. `Run` is handed the
app and opens the window. There is nothing to inherit from, nothing to
register, and nothing to mark as observable: after every handler the
view is built again from the fields as they are.

The package is dot-imported, so the elements read as they do in the
other languages on this engine: `Column`, `Text`, `Button`. That is a
matter of taste, not a rule. Imported under a name, every call carries
the prefix — `gm.Column(gm.Text(…))` — and the app may then use any
name it likes for its own types, `App` included, which the dot import
reserves for the package. Both runs take either form, and so does
`check`.

```console
$ ./bin/gomamochi run app.go
```

That opens a window and watches the file. There is no build step: the
file is read and run by the interpreter compiled into the command,
and a save takes effect in place.

## Holding state

State is the fields of the struct, and nothing else. A handler writes
them; the view reads them. Give the app its starting values in the
literal you hand to `Run`, or in a function that makes one:

<!-- script: click:+1,click:mute,dump,input:live set,dump -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Mixer struct {
	volume int
	title  string
	muted  bool
}

func newMixer() *Mixer {
	return &Mixer{volume: 5, title: "untitled"}
}

func (m *Mixer) View() Element {
	cells := []Element{
		Text(fmt.Sprintf("%s — vol %d", m.title, m.volume)).Size(16),
		Row(
			Button("+1").OnClick(func() { m.volume += 1 }),
			Button("mute").OnClick(func() { m.muted = true }),
			Button("unmute").OnClick(func() { m.muted = false }),
		).Spacing(8),
	}
	if m.muted {
		cells = append(cells, Text("(muted)").Size(12).Color("#8a8f98"))
	}
	cells = append(cells, TextField(m.title).Placeholder("title").OnChange(func(t string) { m.title = t }))
	return Column(cells...).Spacing(10).Padding(14)
}

func main() {
	app := newMixer()
	Run(app, Title("mixer"))
}
```

One shape is refused: the app handed to `Run` straight from a call.
`Run(newMixer(), …)` fails in the interpreted run, which hands a
call's result over without the wrapper that lets an interpreted type
stand in for the package's `App` interface. `app := newMixer()` and
then `Run(app, …)` is what to write, and `check` says so with the line.

Types are Go's own, and there is nothing to annotate: `count int`,
`title string`, `volume float64`, a slice, a map, a struct of your
own. The compiled run is typed by the Go compiler; the interpreted run
reads the same declarations. A string and a number never meet without
`strconv` between them, which is Go, not a rule of ours.

## Writing views

`View` answers one element. An element is a value: the constructor
takes what the element cannot do without — the text of a `Text`, the
label of a `Button`, the children of a `Column` — and every other
property is a method on it, so a chain reads the way a keyword list
would:

```go
Text("Badges").Size(20).Bold(true)
Button("save").Width(120).Background("#313244")
Column(a, b, c).Spacing(12).Padding(16)
```

Nothing is written to the engine until it asks for the tree, so a
view can build pieces in any order and hand them on. A piece of the
screen with a name is a method that answers an element, and one that
wraps other elements takes them as arguments:

<!-- script: click:+1,click:+10,dump -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Cards struct {
	a, b int
}

func (c *Cards) card(title string, kids ...Element) Element {
	cells := []Element{Text(title).Size(18)}
	cells = append(cells, kids...)
	return Column(cells...).Spacing(4).Padding(8).
		BorderWidth(1).BorderColor("accent").BorderRadius(8)
}

func (c *Cards) View() Element {
	return Column(
		c.card("counters",
			Row(Text(fmt.Sprintf("a: %d", c.a)), Button("+1").OnClick(func() { c.a += 1 })).Spacing(6),
			Row(Text(fmt.Sprintf("b: %d", c.b)), Button("+10").OnClick(func() { c.b += 10 })).Spacing(6)),
		Text("outside the card").Size(12),
	).Spacing(10).Padding(16)
}

func main() {
	Run(&Cards{}, Title("cards"))
}
```

A view only reads. It is built again from the same state whenever
anything changes, so it may not write a field, start a goroutine, read
the clock, the environment, a file, the keyboard or a random number,
or start work: `check` names each of those with the line, and a
handler or a timer is where they belong. A function that answers an
`Element`, and a closure handed a `*Painter`, are views too; the
closures on buttons and fields are handlers, and are where writing
happens.

## Control flow in a view

A view is ordinary Go: `if`, `switch`, a loop that appends to a slice
of elements, and a method that answers part of the screen.

<!-- script: click:pick 1,dump,click:hint,click:tab 2,dump -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Control struct {
	items    []string
	picked   int
	showHint bool
	tab      int
}

func (c *Control) line(name string, i int) Element {
	label := "  " + name
	if i == c.picked {
		label = "▸ " + name
	}
	return Row(Text(label), Button(fmt.Sprintf("pick %d", i)).OnClick(func() { c.picked = i })).Spacing(8)
}

func (c *Control) View() Element {
	kids := []Element{Text("control flow").Size(18).Bold(true)}
	if c.showHint {
		kids = append(kids, Text("pick one").Size(12).Color("#8a8f98"))
	} else {
		kids = append(kids, Text("hidden").Size(12))
	}
	for i, name := range c.items {
		kids = append(kids, c.line(name, i))
	}
	if c.picked >= 0 {
		kids = append(kids, Text(fmt.Sprintf("picked %s", c.items[c.picked])).Color("accent"))
	}
	tabs := []Element{Button("hint").OnClick(func() { c.showHint = !c.showHint })}
	for n := 0; n < 3; n++ {
		tab := n
		tabs = append(tabs, Button(fmt.Sprintf("tab %d", n)).OnClick(func() { c.tab = tab }))
	}
	kids = append(kids, Row(tabs...).Spacing(6), Text(fmt.Sprintf("tab %d", c.tab)))
	return Column(kids...).Spacing(10).Padding(14)
}

func main() {
	Run(&Control{items: []string{"milk", "eggs", "rice"}, picked: -1, showHint: true}, Title("control"))
}
```

The `tab := n` inside that loop is not needed in Go 1.22 or newer,
where every iteration has a variable of its own, and Gomamochi keeps
that rule in both runs: the interpreter still has the older one, so
before it reads a file, every loop variable a closure captures is given
a copy of its own at the top of the body, on the same line. Write the
loop the way you would for the compiler. The one shape refused is a
three-clause loop that assigns to its own variable inside the body
while a closure captures it, because the copy would hide the
assignment from the loop.

## Form controls

The controls a person changes, and what each hands its handler:

| Element | Handler | What it carries |
|---|---|---|
| `Checkbox(label)`, `Switch(label)` | `OnChange(func(bool))` | the new state |
| `Slider()`, `NumberField(value)` | `OnChange(func(float64))` | the new number |
| `IntField(value)` | `OnChange(func(int))` | the new whole number |
| `Select()`, `RadioGroup()`, `Segmented()`, `TabBar()` | `OnChange(func(int))` | the chosen index |
| `TextField(value)` | `OnChange(func(string))`, `OnSubmit(func(string))` | the text |
| `Button(label)` | `OnClick(func())` | nothing |
| `Table(…)` | `OnSelect(func(int))`, `OnSort(func(int))` | the row, the column |

<!-- script: click:Dark mode,slide:7,select:banana -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Forms struct {
	dark   bool
	volume float64
	fruits []string
	fruit  int
	note   string
}

func (f *Forms) View() Element {
	return Column(
		Checkbox("Dark mode").Checked(f.dark).OnChange(func(on bool) { f.dark = on }),
		Slider().Value(f.volume).Min(0).Max(10).Step(1).OnChange(func(v float64) { f.volume = v }),
		Select().Options(f.fruits...).Selected(f.fruit).OnChange(func(i int) { f.fruit = i }),
		TextField(f.note).Placeholder("notes").Multiline(true).Rows(3).OnChange(func(t string) { f.note = t }),
		Text(fmt.Sprintf("dark=%v  vol=%.1f  fruit#%d", f.dark, f.volume, f.fruit)),
	).Spacing(10).Padding(14)
}

func main() {
	Run(&Forms{volume: 5, fruits: []string{"apple", "banana", "cherry"}}, Title("forms"))
}
```

A field shows what the app holds and nothing else. Type into a
`TextField` whose `OnChange` does not write the field back, and the
next build shows the old value again — which is the point: the app's
fields are the one truth, and a control is a view of them.

## Handlers

A handler is a Go closure, typed by what the event carries: `func()`
for a click, `func(string)` for text, `func(bool)`, `func(int)` or
`func(float64)` for the rest. It closes over the app through the
pointer receiver, so `c.count += 1` inside one is the whole of the
update, and the view is built again afterwards. A method value does
the same thing with a name:

```go
func (t *Todo) add(s string) { t.items = append(t.items, s) }

TextField(t.draft).OnSubmit(t.add)
```

Handlers run on the window's thread, one at a time, between builds. A
handler that would take a while — a fetch, a query over a large table,
a dialog that waits for a person — belongs in a `Task`, which is what
[Timers and work off the window's thread](#timers-and-work-off-the-windows-thread)
is about.

## Lists, charts, and rows built on demand

A `ListView` is asked for the rows in view, not for all of them: the
count, and a function from a row number to its element.

<!-- script: input:eggs,submit,dump,click:done,dump -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Todo struct {
	items []string
	draft string
	done  int
}

func (t *Todo) add(s string) {
	t.items = append(t.items, s)
	t.draft = ""
}

func (t *Todo) line(i int) Element {
	cells := []Element{Text(fmt.Sprintf("%d. %s", i+1, t.items[i]))}
	if i == t.done {
		cells = append(cells, Text("done").Color("accent"))
	}
	cells = append(cells, Button("done").OnClick(func() { t.done = i }))
	return Row(cells...).Spacing(8)
}

func (t *Todo) View() Element {
	return Column(
		Text(fmt.Sprintf("todo — %d items", len(t.items))).Size(16),
		TextField(t.draft).Placeholder("add and press enter").
			OnChange(func(s string) { t.draft = s }).OnSubmit(t.add),
		ListView(len(t.items), t.line).ItemHeight(26).Height(280),
		Button("clear").OnClick(func() { t.items = nil }),
	).Spacing(10).Padding(14)
}

func main() {
	Run(&Todo{items: []string{"milk"}, done: -1}, Title("todo"))
}
```

`Table(columns, count, row)` is the same idea with a header, tracks
whose shares are `Widths`, a `Selected` row, and `Sort` and
`Descending` the app keeps, with `OnSelect` and `OnSort` telling it
which row and which column a person chose. `DataTable` is the simpler
one: its first child is the header, the rest are rows, and the frame
comes with it.

Charts take their numbers as their first argument. `BarChart(data)`
and `LineChart(data)` share `Labels`, `Axis`, `Min` and `Max` (both
zero takes the range from the data), and `Series` draws several lines
or groups with one color each from `Colors`:

```go
BarChart(profit).Labels(months...).Axis(true).Height(150)
LineChart(nil).Series([][]float64{requests, errors}).Colors("accent", "#f38ba8").Max(90).Height(150)
```

## Maps

A map is a field like any other: read a key with the second result,
count it, add to it in a handler.

<!-- script: click:apple,dump,click:cherry,dump -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Lookup struct {
	prices map[string]int
	picked int
	label  string
}

func (l *Lookup) price(name string, fallback int) int {
	if v, ok := l.prices[name]; ok {
		return v
	}
	return fallback
}

func (l *Lookup) View() Element {
	return Column(
		Text(fmt.Sprintf("picked=%d n=%d %s", l.picked, len(l.prices), l.label)),
		Row(
			Button("apple").OnClick(func() { l.picked = l.price("apple", -1) }),
			Button("cherry").OnClick(func() {
				l.prices["cherry"] = 200
				l.picked = l.price("cherry", -1)
				l.label = "cherry known"
			}),
		).Spacing(6),
	).Spacing(8).Padding(12)
}

func main() {
	Run(&Lookup{prices: map[string]int{"apple": 120}, label: "none"}, Title("lookup"))
}
```

What a view may not do is `range` over one. Go walks a map in a
different order every run, so a view that did would draw a different
screen in the two runs — and the gate would say so, on some runs.
`check` refuses it with the line. A handler may range over a map,
which is how it collects the keys to sort them; what it then keeps on
the app, a sorted slice, is what the view reads.

## Value structs

A struct of your own is a value the app holds, and a slice of them is
a list of values. The games rebuild theirs every tick and hand the new
slice to the field; nothing needs to be marked, shared or observed.

<!-- script: click:right,click:measure,dump,click:swap,dump -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Point struct {
	x, y int
}

type Points struct {
	sel  Point
	dist int
}

func (p *Points) View() Element {
	return Column(
		Text(fmt.Sprintf("p=(%d, %d) d2=%d", p.sel.x, p.sel.y, p.dist)),
		Row(
			Button("right").OnClick(func() { p.sel = Point{p.sel.x + 5, p.sel.y} }),
			Button("swap").OnClick(func() { p.sel = Point{p.sel.y, p.sel.x} }),
			Button("measure").OnClick(func() { p.dist = p.sel.x*p.sel.x + p.sel.y*p.sel.y }),
		).Spacing(6),
	).Spacing(8).Padding(12)
}

func main() {
	Run(&Points{sel: Point{3, 4}}, Title("points"))
}
```

A value that may be nothing at all is a pointer that may be nil, and
objects that point at one another are pointers both ways: Go's
collector takes a cycle in its stride, so a tree is written the way it
reads.

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

## The keywords every element takes

Fifteen properties mean the same on every element, and are methods on
every one:

| Method | Type | What it does |
|---|---|---|
| `Width(v)`, `Height(v)` | float64 | a fixed size; written even when 0 |
| `MinWidth(v)`, `MaxWidth(v)` | float64 | bounds |
| `Disabled(v)` | bool | inert, and drawn so |
| `Theme(v)` | string | the palette its subtree resolves colors in: `"dark"` or `"light"` |
| `Animate(ms)`, `Easing(v)` | float64, string | a tween on change, and its curve (`"out"`, `"inOut"`) |
| `Enter(v)`, `Exit(v)` | bool | animate its arrival, its departure |
| `ColSpan(v)`, `RowSpan(v)` | int | how many of a grid's tracks it covers |
| `Role(v)`, `A11yLabel(v)` | string | what a screen reader is told |
| `Tooltip(v)` | string | what the pointer shows |

An element whose own keyword has one of these names keeps its own
meaning: a `Button`'s `Width` is the button's, sized as the button
wants, where a `Column`'s is a box around it. The elements whose own
label is already the name a screen reader reads — `Checkbox`, `Switch`,
`Progress` — refuse `A11yLabel`, since there is no second name to give.

## Themes and animation

Colors are the engine's tokens — `"accent"`, `"panel"`, `"text"`,
`"textDim"` — or a hex value. A token is resolved in the palette of
the nearest `Theme` above it, so one method flips a whole panel:

<!-- script: click:+1,dump,click:flip,dump -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

func key(b *ButtonEl) *ButtonEl { return b.Background("#313244").HoverBackground("#45475a") }

type Styled struct {
	mode string
	n    int
}

func (s *Styled) flip() {
	if s.mode == "dark" {
		s.mode = "light"
	} else {
		s.mode = "dark"
	}
}

func (s *Styled) View() Element {
	return Column(
		Text(fmt.Sprintf("n=%d", s.n)).Size(18).Color("accent"),
		Row(
			key(Button("+1")).OnClick(func() { s.n += 1 }),
			key(Button("flip")).Background("#fab387").OnClick(func() { s.flip() }),
		).Spacing(6),
	).Spacing(8).Padding(12).Background("panel").Theme(s.mode)
}

func main() {
	Run(&Styled{mode: "dark"}, Title("styled"))
}
```

A look kept in one place is a function that sets the same properties
on every element handed to it, like `key` above: the element's type
comes back, so the chain goes on. `Animate(120).Easing("out")` on an
element tweens the change to it over 120 milliseconds; `Enter` and
`Exit` do the same for its arrival and departure.

## The window

`Run` takes the app and its options: `Title`, `Size(width, height)`,
and `Padding`, the space between the window's edge and the tree.
`Quit()` closes the window on the engine's next frame; a headless run
never takes it, so a script runs to its end.

The elements that arrange or cover: `Grid` lays its children on
`Columns` tracks, and `GridCell(child).ColSpan(2)` spans them; `Stack`
puts its children on top of one another; `ScrollView` and
`HScrollView` are panes that scroll; `Modal` is a panel over the rest
of the window while `Open`. `Spacer` takes the space its parent has
left over, `Divider` draws a rule, `Link` opens a page, `Spinner` and
`Progress` show work, `Image` and `Svg` show a file.

<!-- script: click:about,dump,click:close,dump -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Panels struct {
	open bool
}

func (p *Panels) View() Element {
	var lines []Element
	for n := 1; n <= 12; n++ {
		lines = append(lines, Text(fmt.Sprintf("line %d", n)))
	}
	return Stack(
		Column(
			Row(Text("Panels").Size(18), Spacer(), Spinner().Size(14)).Spacing(8),
			Grid(Text("one"), Text("two"), GridCell(Text("across both").Align("center")).ColSpan(2)).Columns(2).Spacing(6),
			ScrollView(Column(lines...).Spacing(2)).Height(90),
			Button("about").OnClick(func() { p.open = true }),
		).Spacing(10).Padding(14),
		Modal(
			Column(
				Text("A panel over the rest of it.").Size(14),
				Button("close").OnClick(func() { p.open = false }),
			).Spacing(8).Padding(12).Background("panel"),
		).Open(p.open),
	)
}

func main() {
	Run(&Panels{}, Title("panels"), Size(420, 360))
}
```

## Go's own standard library

`fmt`, `strings`, `strconv`, `sort`, `math`, `time`, `encoding/json`,
`encoding/csv`, `regexp`, `os`, `net/http`: the language's own, and
both runs call the same compiled packages. The interpreter does not
reimplement them — it calls the ones compiled into the command — so
what `fmt.Sprintf("%.2f", x)` answers is the same bytes in both runs,
and the gate never has to compare two implementations of a library.

<!-- script: click:stats,click:parse,click:scan,dump -->
```go
package main

import (
	"encoding/json"
	"fmt"
	"regexp"
	"sort"
	"strconv"

	. "github.com/i2y/yokan/gomamochi"
)

type Stdlib struct {
	scores []int
	spread string
	doc    string
	sum    int
}

func (s *Stdlib) stats() {
	sorted := append([]int{}, s.scores...)
	sort.Ints(sorted)
	s.spread = fmt.Sprintf("median %d min %d max %d", sorted[len(sorted)/2], sorted[0], sorted[len(sorted)-1])
}

func (s *Stdlib) parse() {
	var doc map[string]any
	json.Unmarshal([]byte(`{"name": "gomamochi", "ok": true}`), &doc)
	s.doc = fmt.Sprintf("%v %v", doc["name"], doc["ok"])
}

func (s *Stdlib) scan() {
	s.sum = 0
	for _, m := range regexp.MustCompile(`\d+`).FindAllString("a1b22c333", -1) {
		v, _ := strconv.Atoi(m)
		s.sum += v
	}
}

func (s *Stdlib) View() Element {
	return Column(
		Text("spread: "+s.spread),
		Text("json: "+s.doc),
		Text(fmt.Sprintf("scan: %d", s.sum)),
		Row(
			Button("stats").OnClick(func() { s.stats() }),
			Button("parse").OnClick(func() { s.parse() }),
			Button("scan").OnClick(func() { s.scan() }),
		).Spacing(6),
	).Spacing(6).Padding(14)
}

func main() {
	Run(&Stdlib{scores: []int{3, 5, 8, 13, 21}, spread: "-", doc: "-"}, Title("stdlib"))
}
```

What the interpreter knows of the library is Go 1.22's: the packages
and functions of that release. Of the language it knows less: `min`
and `max` (1.21), `range` over a number (1.22) and over a function
(1.23) are not there, nor is anything the library gained in 1.23 or
later, and `check` says so by name for the first three. Files are `os`, the
network is `net/http`, a big number is `math/big`; the demos read a
file, serve a page to themselves and fetch it, and parse CSV, with
nothing but the standard library.

## The framework's standard library

What the engine mediates comes from the package, and there one
implementation answers both runs, through the same C face: a scripted
run keeps a dialog answered by `file:<path>` and a sound silent in
both.

| Function | What it does |
|---|---|
| `SqliteExec(db, sql, params...)` | run a statement; answers the rows changed |
| `SqliteQueryText`, `SqliteQueryInt`, `SqliteQueryRows` | a query's first column, first cell, or whole rows |
| `SqliteQueryTextOr`, `SqliteQueryIntOr`, `SqliteQueryRowsOr` | the same, answering nothing rather than stopping when the query fails |
| `ClipboardSetText(s)`, `ClipboardGetText()` | the system clipboard |
| `OpenDialog(title)`, `SaveDialog(name)` | the platform's own panels; they wait, so call them inside a `Task` |
| `AudioPlay(path, volume)`, `AudioStop()` | a WAV file, played and forgotten |
| `NotifySend(title, body)` | a notification |

Write `?` in a statement and put the values after it, and text a
person typed can never become part of the statement:

<!-- script: click:setup,click:load,dump -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

const db = "demo/.gate/tour-notes.db"

type Notes struct {
	changed int
	rows    []string
}

func (nt *Notes) setup() {
	SqliteExec(db, "CREATE TABLE IF NOT EXISTS notes(t TEXT)")
	SqliteExec(db, "DELETE FROM notes")
	nt.changed = SqliteExec(db, "INSERT INTO notes VALUES (?), (?)", "alpha", "beta")
}

func (nt *Notes) View() Element {
	return Column(
		Text(fmt.Sprintf("inserted=%d rows=%d", nt.changed, len(nt.rows))),
		Row(
			Button("setup").OnClick(func() { nt.setup() }),
			Button("load").OnClick(func() { nt.rows = SqliteQueryText(db, "SELECT t FROM notes ORDER BY t") }),
		).Spacing(6),
		ListView(len(nt.rows), func(i int) Element { return Text(nt.rows[i]) }).ItemHeight(22).Height(80),
	).Spacing(8).Padding(12)
}

func main() {
	Run(&Notes{}, Title("notes"))
}
```

A query that may run before its table exists — the first load of a
ledger does — is the `…Or` twin, which answers nothing; the plain one
stops the app, as the library's own would.

## Timers and work off the window's thread

`Every(seconds, tick)` asks to be told every so often, declared before
`Run`. Both runs tick off the same clock: a frame in a window, and
`advance:<ms>` in a script, so the same number of ticks lands in both.
A tick is where a game moves and where the keyboard is read.

`Task(work, done)` runs `work` on a goroutine of its own and, when it
is done, calls `done` on the window's thread with what the work
answered. Nothing inside the work touches the app's fields or the
screen; that is the whole rule, and the reason the answer comes back
as an argument rather than the worker writing it anywhere.

<!-- script: click:start,dump -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Jobs struct {
	status string
	answer int
}

func (j *Jobs) start() {
	j.status = "working"
	Task(func() any {
		total := 0
		for i := 0; i < 300000; i++ {
			total += i % 7
		}
		return total
	}, func(v any) {
		j.answer = v.(int)
		j.status = "done"
	})
}

func (j *Jobs) View() Element {
	return Column(
		Text(fmt.Sprintf("%s: %d", j.status, j.answer)),
		Button("start").OnClick(func() { j.start() }),
	).Spacing(10).Padding(14)
}

func main() {
	Run(&Jobs{status: "idle"}, Title("tasks"))
}
```

A script settles the work before its next step, so `dump` after
`click:start` shows the answer in both runs. The work may call the
framework's library — a dialog, a query — from its goroutine, or from
any goroutine the app starts itself: each call keeps to one thread for
its own duration, and nothing in the app has to know.

## While you are writing it

`gomamochi run` watches the app's file. Save, and the window picks the
edit up: the file is read again by a fresh interpreter, and the app the
window is holding hands its values to the new one — every field with
the same name and type keeps what it had, a field the new file adds
starts at its zero value, and a field whose type changed starts over.
Timers and shortcuts the file declares are bound again to the new app.
A file that does not compile leaves the window on what it had and says
so in the terminal, and the next save that does compile takes.

A save is a fresh read of the whole file, and `main` runs again. The
starting values it gives the app do not replace the ones the window
has, since those are carried over — which is the point.

## Headless runs and the gate

`PIXIE_SCRIPT` replaces the person. The engine builds the tree, drives
it with the steps, and prints the dumps:

```
click:<label>      press a button by the label it shows
input:<text>       type into a field   submit    press enter in it
slide / select     move a slider, pick an option
click@1:<label>    the second button with that label (n counts from 0, in tree order)
                   and the same for input@n:, submit@n, slide@n:, select@n:
key:<chord>        a keystroke bound to a shortcut
keydown:<key> / keyup:<key>    hold a key down, let it up
menu:<item>        pick a menu item    file:<path>   answer a dialog
drop:<path>        a file dragged onto the window
advance:<ms>       move the clock      theme:dark|light
dump               print the tree      a11y   print what a reader reads
```

`gomamochi gate` runs the app twice with one script — the file under
the interpreter, and the binary the Go compiler built from it — and
compares the two transcripts byte for byte.

```console
$ ./bin/gomamochi gate demo/counter.go --script "click:+1,dump,input:Momo"
GATE OK — 3 dump lines identical in both runs
```

The gate is the promise. Here it says that the interpreter, which is
what you were looking at while writing, agrees with the compiler,
which is what ships: everything else in this page is a way of writing
something the gate can keep. `--fresh <path>` deletes a path before
each run, so an app that keeps a file or a database starts both runs
from the same nothing.

## What Gomamochi refuses

`gomamochi check` reads the app and names what it cannot take, with the
line, a caret, and the rewrite. It runs before every run, build and
gate, and prints nothing when there is nothing to say.

```console
$ ./bin/gomamochi check demo/broken.go
demo/broken.go:8:2: Gomamochi cannot take this — a view only reads. Move the write into a handler — the closure on a button, or a method the app calls from one
    	a.n += 1
    	^
```

Two layers. The first reads the file alone, and runs everywhere. The
second is Go's own type checker, fed the toolchain's export data, which
speaks Go's type errors in Go's words and adds what only a type can
decide; it runs when `go` is on the path, which `build` and `gate`
need anyway.

What the interpreted run cannot run as the compiled one does:

- `min` and `max` (Go 1.21), and `range` over a number (1.22) or a
  function (1.23), which the interpreter does not have. Write the
  comparison out, or `for i := 0; i < n; i++`.
- The app handed to `Run` straight from a call. Give it a name first.
- `%T`, and `reflect`: the interpreter names the app's own types
  differently.
- `unsafe`, cgo, `//go:embed`, and a module outside the standard
  library.
- A three-clause loop that writes its own variable while a closure
  captures it.

What a view may not do, because it is built again from the same state
whenever anything changes:

- Write a field of the app, or start a goroutine.
- Read the clock, the environment, a file, a stream, the network, a
  random number, or the keyboard; start work or a timer; play a sound.
- `range` over a map.

Each of those has a fixture under `test/refuse/` holding the message
it prints, so a refusal cannot quietly change its wording.

## Shipping

```console
$ ./bin/gomamochi build demo/todo.go --release --app
built: demo/.gate/todo/todo (1.9 MB)
bundle: demo/dist/todo.app (22.3 MB)
```

`build` compiles the file with the Go compiler, with cgo off, into a
binary that links nothing but the system's own library; the engine is
a shared library that rides beside it, and the door looks there first.
`--release` drops the symbol table. `--app` wraps the two in a macOS
application bundle, ad-hoc signed, with `<stem>.png` or `<stem>.icns`
beside the app as its icon; the bundle is the whole program, and opens
on a machine with neither Go nor the toolchain installed. On Linux
`--app` writes an AppDir instead, `--appimage` packs it into one file,
and `--carry-libs` makes it carry the desktop's libraries too, for a
machine that may not have them.

## What does not work yet

- The interpreter's library is Go 1.22's, and its language is short of
  that: `min` and `max` (1.21), `range` over a number (1.22) or a
  function (1.23), and what the library gained in 1.23 and later are
  not there; `check` names the first three, and the interpreter's own
  error names the rest.
- A module outside the standard library cannot be read by the
  interpreted run yet, and an app is one file.
- Under the dot import, an app cannot declare a name the package
  exports — `App`, `Element`, `Text` and the rest. Import the package
  under a name and it can.
- A recovered panic's message is worded differently in the two runs,
  and `%T` prints a different name; neither is a thing to show a
  person.
- The interpreted run is slower than the compiled one by two orders of
  magnitude on tight loops, which is what an interpreter costs; a
  frame of either game is still under a millisecond of it.
- macOS and Linux. The bundle and the AppDir carry the engine, so even
  a small app weighs about 22 MB; the Linux packaging is written the
  way Yokan's is and has not yet been run on a Linux machine.
