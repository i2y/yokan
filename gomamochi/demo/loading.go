// The bar that fills, in its three forms: with a caption above it, at a
// size the app chose, and sweeping for work with no known length.
package main

import (
	"strconv"
	"strings"

	. "github.com/i2y/yokan/gomamochi"
)

type Loading struct {
	ratio float64
	busy  bool
}

func (l *Loading) step() {
	if l.ratio >= 1 {
		l.ratio = 0
	} else {
		l.ratio += 0.25
	}
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

func (l *Loading) View() Element {
	return Column(
		Text("ratio: "+num(l.ratio)),
		Progress(l.ratio).Label("Uploading"),
		Progress(l.ratio).Width(240).Height(6),
		Progress(l.ratio).Indeterminate(l.busy),
		Row(
			Button("step").OnClick(func() { l.step() }),
			Button("busy").OnClick(func() { l.busy = !l.busy }),
		).Spacing(8),
	).Spacing(12).Padding(16)
}

func main() {
	Run(&Loading{ratio: 0.25}, Title("loading"))
}
