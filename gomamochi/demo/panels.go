// The elements that arrange or cover: tracks, layers, panes that
// scroll, and a panel over the rest of the window.
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Panels struct {
	open  bool
	pane  int
	panes []string
}

func (p *Panels) tracks() Element {
	return Grid(
		Text("one"), Text("two"),
		GridCell(Text("across both").Align("center").Background("#313244").
			Padding(4).BorderRadius(6)).ColSpan(2),
		Text("three"), Text("four"),
	).Columns(2).Spacing(6)
}

func (p *Panels) layers() Element {
	return Stack(
		Image("demo/assets/postcard.png").Width(180).Height(90),
		Text("over the picture").Size(14).Color("#11111b").Background("#f9e2af").Padding(4),
	)
}

func (p *Panels) scrolls() Element {
	var lines []Element
	for n := 1; n <= 12; n++ {
		lines = append(lines, Text(fmt.Sprintf("line %d", n)))
	}
	var cols []Element
	for n := 1; n <= 10; n++ {
		cols = append(cols, Text(fmt.Sprintf("col %d", n)).Width(70))
	}
	return Column(
		ScrollView(Column(lines...).Spacing(2)).Height(90),
		HScrollView(Row(cols...).Spacing(6)),
	).Spacing(8)
}

func (p *Panels) panel() Element {
	if p.pane == 0 {
		return p.tracks()
	}
	if p.pane == 1 {
		return p.layers()
	}
	return p.scrolls()
}

func (p *Panels) View() Element {
	return Stack(
		Column(
			Row(
				Text("Panels").Size(18),
				Spacer(),
				Link("pixie", "https://example.invalid").Size(12),
				Spinner().Size(14),
			).Spacing(8),
			Segmented().Options(p.panes...).Selected(p.pane).OnChange(func(i int) { p.pane = i }),
			p.panel(),
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
	Run(&Panels{panes: []string{"grid", "stack", "scrolls"}}, Title("panels"))
}
