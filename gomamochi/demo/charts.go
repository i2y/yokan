// Charts that can say what they mean: a profit-and-loss bar chart whose
// losing months hang below the zero line, and a two-series line chart.
// `Min`/`Max` both zero take the range from the data; `Axis` draws the
// tick labels and a faint gridline at each; `Series` takes one list per
// line, `Colors` one color each.
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Book struct {
	months   []string
	profit   []float64
	requests []float64
	errors   []float64
	n        int
}

func (b *Book) nextMonth() {
	b.n += 1
	// A deterministic next month, so both runs read the same numbers
	// and the gate can compare them.
	b.profit = append(b.profit, float64(b.n*7%41)-18)
	b.months = append(b.months, fmt.Sprintf("M%d", b.n))
	b.requests = append(b.requests, float64(b.n*13%50)+30)
	b.errors = append(b.errors, float64(b.n*5%14))
}

func (b *Book) View() Element {
	return Column(
		Text("Profit and loss").Size(18).Color("accent"),
		Text("negative months hang below the zero line").Size(12).Color("#8a8f98"),
		BarChart(b.profit).Labels(b.months...).Axis(true).Height(150),
		Text("Traffic").Size(18).Color("accent"),
		Text("requests and errors, one color each").Size(12).Color("#8a8f98"),
		LineChart(nil).Series([][]float64{b.requests, b.errors}).Labels(b.months...).
			Colors("accent", "#f38ba8").Axis(true).Max(90).Height(150),
		Row(Button("next month").OnClick(func() { b.nextMonth() })).Spacing(8),
	).Spacing(12).Padding(16)
}

func main() {
	Run(&Book{
		months:   []string{"Jan", "Feb", "Mar", "Apr", "May", "Jun"},
		profit:   []float64{12, -8, 4, -3, 15, -6},
		requests: []float64{40, 55, 48, 62, 70, 58},
		errors:   []float64{3, 9, 5, 12, 6, 4},
		n:        6,
	}, Title("charts"))
}
