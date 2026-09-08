// A panel over the rest of the window, opened and closed by the app.
package main

import . "github.com/i2y/yokan/gomamochi"

type Dialog struct {
	show   bool
	status string
}

func (d *Dialog) decide(answer string) {
	d.status = answer
	d.show = false
}

func (d *Dialog) View() Element {
	kids := []Element{
		Text("status: " + d.status).Size(16),
		Button("open dialog").OnClick(func() { d.show = true }),
	}
	if d.show {
		kids = append(kids, Modal(
			Text("accept the terms?").Size(18),
			Row(
				Button("accept").OnClick(func() { d.decide("accepted") }),
				Button("decline").OnClick(func() { d.decide("declined") }),
			).Spacing(8),
		))
	} else {
		kids = append(kids, Text("(dialog closed)").Size(12).Color("#8a8f98"))
	}
	return Column(kids...).Spacing(10).Padding(14)
}

func main() {
	Run(&Dialog{status: "undecided"}, Title("dialog"))
}
