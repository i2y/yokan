// One list of numbers, drawn twice. A chart takes its data as its first
// argument, so a view that computes the numbers reads in order.
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Trend struct {
	values []float64
	limit  float64
}

func (t *Trend) bump() {
	t.values = append(t.values, 8)
}

func (t *Trend) View() Element {
	return Column(
		Text(fmt.Sprintf("points: %d", len(t.values))).Size(14),
		LineChart(t.values).Height(120),
		BarChart(t.values).Height(90),
		Text(fmt.Sprintf("limit: %.1f", t.limit)).Size(12).Color("#8a8f98"),
		Row(
			Button("add point").OnClick(func() { t.bump() }),
			Button("raise limit").OnClick(func() { t.limit += 0.5 }),
		).Spacing(8),
	).Spacing(10).Padding(14)
}

func main() {
	Run(&Trend{values: []float64{3, 5, 2}, limit: 4.5}, Title("trend"))
}
