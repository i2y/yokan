// The two fields that hold a number rather than text: enter or leaving
// them commits, text that is not a number is dropped and the shown
// value returns to what the app holds.
package main

import (
	"strconv"
	"strings"

	. "github.com/i2y/yokan/gomamochi"
)

type Order struct {
	qty   int
	price float64
}

func (o *Order) reset() {
	o.qty = 1
	o.price = 0
}

// A number the way the other languages print one: a whole number
// still shows its ".0".
func num(v float64) string {
	s := strconv.FormatFloat(v, 'f', -1, 64)
	if !strings.ContainsAny(s, ".e") {
		s += ".0"
	}
	return s
}

func (o *Order) View() Element {
	return Column(
		Text("Order line").Size(18),
		Row(
			Text("quantity"),
			IntField(o.qty).Min(1).Max(99).Placeholder("qty").OnChange(func(n int) { o.qty = n }),
		).Spacing(8),
		Row(
			Text("unit price"),
			NumberField(o.price).Min(0).Max(1000).Step(0.5).Placeholder("price").
				OnChange(func(p float64) { o.price = p }),
		).Spacing(8),
		Text("total  "+num(float64(o.qty)*o.price)),
		Button("reset").OnClick(func() { o.reset() }),
	).Spacing(10).Padding(14)
}

func main() {
	Run(&Order{qty: 1}, Title("quantities"))
}
