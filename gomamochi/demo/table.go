// DataTable draws the table itself: the first row inside it is the
// header, every later row is a data row shaded in alternation, and the
// frame comes with the element. Columns line up because the cells of
// one column carry the same `Grow` share.
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Fleet struct {
	latency map[string]int
	polls   int
}

func (f *Fleet) refresh() {
	f.polls += 1
	f.latency["api"] = (f.latency["api"]*3 + 29) % 140
	f.latency["db"] = (f.latency["db"]*5 + 11) % 140
	f.latency["cache"] = (f.latency["cache"]*7 + 3) % 140
	f.latency["edge"] = (f.latency["edge"]*2 + 47) % 140
}

func health(ms int) string {
	label := "ok"
	if ms > 60 {
		label = "watch"
	}
	if ms > 100 {
		label = "slow"
	}
	return label
}

func (f *Fleet) serviceRow(name string) Element {
	return Row(
		Text(name).Grow(2),
		Text(fmt.Sprintf("%d ms", f.latency[name])).Grow(1).Align("right"),
		Text(health(f.latency[name])).Grow(1).Align("center"),
	).Spacing(8)
}

func (f *Fleet) View() Element {
	return Column(
		Text(fmt.Sprintf("fleet latency — %d polls", f.polls)).Size(16),
		DataTable(
			Row(
				Text("service").Grow(2),
				Text("latency").Grow(1).Align("right"),
				Text("health").Grow(1).Align("center"),
			).Spacing(8),
			f.serviceRow("api"),
			f.serviceRow("db"),
			f.serviceRow("cache"),
			f.serviceRow("edge"),
		),
		Button("refresh").OnClick(func() { f.refresh() }),
	).Spacing(10).Padding(14)
}

func main() {
	Run(&Fleet{latency: map[string]int{"api": 42, "db": 17, "cache": 8, "edge": 95}}, Title("table"))
}
