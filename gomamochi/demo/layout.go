// Spacer and Divider: a filler and a rule. The header row's spacer
// pushes "ping" to the far edge; the footer's does the same for the
// count. Divider draws the rules, the second one heavier and colored.
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Layout struct {
	pings int
}

func (l *Layout) View() Element {
	return Column(
		Row(
			Text("Layout").Size(18),
			Spacer(),
			Button("ping").OnClick(func() { l.pings += 1 }),
		),
		Divider(),
		Column(
			Text("Section one").Size(14),
			Text("spacer takes the slack a row leaves behind."),
			Divider().Thickness(2).Color("accent"),
			Text("Section two").Size(14),
			Text("divider draws a rule across its parent."),
		).Spacing(6),
		Row(
			Spacer(),
			Text(fmt.Sprintf("pings: %d", l.pings)),
		),
	).Spacing(12).Padding(16)
}

func main() {
	Run(&Layout{}, Title("layout"))
}
