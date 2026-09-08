// A timer: declared before the app runs, told every second. Both runs
// tick off the same clock — a frame in a window, an `advance:` in a
// script — so the same number of ticks lands in both.
//
// The step is arithmetic rather than a random number: a dashboard that
// cannot be compared is not worth gating.
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

const slots = 12

type Dashboard struct {
	hist  []float64
	at    int
	ticks int
	cur   float64
}

func (d *Dashboard) tick() {
	d.ticks += 1
	step := float64(d.ticks*37%41)/100 - 0.2
	v := d.cur + step
	if v < 0 {
		v = 0
	}
	if v > 1 {
		v = 1
	}
	d.cur = v
	d.hist[d.at] = v
	d.at = (d.at + 1) % slots
}

func (d *Dashboard) View() Element {
	return Column(
		Row(
			Text("load, sampled every second").Size(13).Color("#8a8f98").Grow(1),
			Spinner().Size(16),
		).Spacing(8),
		Text(fmt.Sprintf("%.2f", d.cur)).Size(40),
		Progress(d.cur),
		LineChart(d.hist).Height(120),
		Text(fmt.Sprintf("%d ticks · %d slots", d.ticks, slots)).Size(12).Color("#8a8f98"),
	).Spacing(12).Padding(16)
}

func main() {
	app := &Dashboard{hist: make([]float64, slots), cur: 0.25}
	Every(1.0, func() { app.tick() })
	Run(app, Title("dashboard"))
}
