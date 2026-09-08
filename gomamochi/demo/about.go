// Links that open a page, and the system clipboard.
package main

import . "github.com/i2y/yokan/gomamochi"

type About struct {
	status string
}

func (a *About) View() Element {
	return Column(
		Text("Gomamochi").Size(28),
		Text("version 0.1.0"),
		Link("Website", "https://i2y.github.io/yokan/"),
		Link("Source", "https://github.com/i2y/yokan"),
		Link("Docs", "https://i2y.github.io/yokan/tour/"),
		Button("copy link").OnClick(func() {
			ClipboardSetText("https://github.com/i2y/yokan")
			a.status = "copied"
		}),
		Text("status: "+a.status),
	).Spacing(8).Padding(14)
}

func main() {
	Run(&About{}, Title("about"))
}
