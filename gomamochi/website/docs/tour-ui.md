<!-- Written by website/tools/tourpages from the tour. Edit the tour. -->
# Look, and the window

The keywords every element takes, the palette, and what the window itself brings.

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

