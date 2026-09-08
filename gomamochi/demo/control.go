// Ordinary Go inside a view: `if`, a loop, and a method that answers
// part of the screen. Nothing here is a special form — the view builds
// a list of elements the way any Go builds a slice, and hands it to
// the column.
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

func (c *Control) hint() Element {
	return Text("pick one").Size(12).Color("#8a8f98")
}

func (c *Control) line(name string, i int) Element {
	label := "  " + name
	if i == c.picked {
		label = "▸ " + name
	}
	return Row(
		Text(label),
		Button(fmt.Sprintf("pick %d", i)).OnClick(func() { c.picked = i }),
	).Spacing(8)
}

func (c *Control) tabButton(n int) Element {
	return Button(fmt.Sprintf("tab %d", n)).OnClick(func() { c.tab = n })
}

func (c *Control) View() Element {
	kids := []Element{Text("control flow").Size(18).Bold(true)}
	if c.showHint {
		kids = append(kids, c.hint())
	} else {
		kids = append(kids, Text("hidden").Size(12))
	}
	for i, name := range c.items {
		kids = append(kids, c.line(name, i))
	}
	if c.picked >= 0 {
		color := "#f38ba8"
		if c.picked%2 == 0 {
			color = "accent"
		}
		kids = append(kids, Text(fmt.Sprintf("picked %s", c.items[c.picked])).Color(color))
	}
	tabs := []Element{Button("hint").OnClick(func() { c.showHint = !c.showHint })}
	for n := 0; n < 3; n++ {
		tabs = append(tabs, c.tabButton(n))
	}
	kids = append(kids, Row(tabs...).Spacing(6), Text(fmt.Sprintf("tab %d", c.tab)))
	return Column(kids...).Spacing(10).Padding(14)
}

func main() {
	Run(&Control{items: []string{"milk", "eggs", "rice"}, picked: -1, showHint: true}, Title("control"))
}
