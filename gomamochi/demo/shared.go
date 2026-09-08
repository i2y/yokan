// The properties every element takes, on elements that have nothing
// else in common: a theme scope on a spacer, a box around a column, a
// tween on a chooser, a tooltip on a rule, and the lock that makes a
// field and a button inert.
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Locks struct {
	locked bool
	saves  int
	// The palette the spacer's subtree resolves its tokens in — a
	// property takes a value, not just a literal, so the lock switches it.
	mode string
	tab  int
	note string
}

func (l *Locks) flip() {
	l.locked = !l.locked
	if l.locked {
		l.mode = "light"
	} else {
		l.mode = "dark"
	}
}

func (l *Locks) View() Element {
	return Column(
		Text("shared").Size(20).Role("heading"),
		Row(
			Text(fmt.Sprintf("mode: %s  saves: %d", l.mode, l.saves)).Size(12),
			// A theme scope on a spacer: the property is the element's,
			// whichever element it is.
			Spacer().Grow(1).Theme(l.mode),
			Button("lock").Tooltip("flip the lock").OnClick(func() { l.flip() }),
		).Spacing(8),
		Segmented().Options("read", "write").Selected(l.tab).Animate(120).Easing("out").
			OnChange(func(i int) { l.tab = i }),
		// A box around the section: 260 wide, never under 200.
		Column(
			// The field takes two of the grid's three tracks, and goes
			// inert with the lock.
			Grid(
				Text("note").Size(12),
				TextField(l.note).ColSpan(2).Disabled(l.locked).OnChange(func(t string) { l.note = t }),
			).Columns(3).Spacing(8),
			Button("save").Disabled(l.locked).Tooltip("count a save").OnClick(func() { l.saves += 1 }),
		).Width(260).MinWidth(200).Spacing(8).Padding(8).Background("panel"),
		Link("Docs", "https://i2y.github.io/yokan/").Role("button"),
		Divider().Tooltip("the end of the shared properties"),
	).Spacing(10).Padding(14)
}

func main() {
	Run(&Locks{mode: "dark", note: "draft"}, Title("shared"))
}
