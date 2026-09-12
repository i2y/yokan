<!-- Written by website/tools/tourpages from the tour. Edit the tour. -->
# Views and control flow

How a screen is built, broken into pieces, and driven by what the app holds.

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
[Timers and work off the window's thread](tour-lib.md#timers-and-work-off-the-windows-thread)
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

Moving the pointer over a chart shows the value under it: the bar's
or the sample's label and one number per series. A script puts the
pointer there with `hover:<i>` and takes it away with `hover:`, and
the dump carries the readout, so what a person sees on hover is
checked the way a click is.


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

