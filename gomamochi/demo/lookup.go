// A map on the app: reading with a fallback, asking whether a key is
// there, and adding one while the window is open. A map is never
// ranged over in a view — its order changes from run to run — but a
// key is read as freely as a field.
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Lookup struct {
	prices map[string]int
	picked int
	label  string
}

// The price of a fruit, or `fallback` when it is not on the list.
func (l *Lookup) price(name string, fallback int) int {
	if v, ok := l.prices[name]; ok {
		return v
	}
	return fallback
}

func (l *Lookup) pickApple() {
	l.picked = l.price("apple", -1)
	if _, ok := l.prices["cherry"]; ok {
		l.label = "cherry known"
	} else {
		l.label = "no cherry"
	}
}

func (l *Lookup) addCherry() {
	l.prices["cherry"] = 200
	l.picked = l.price("cherry", -1)
	if _, ok := l.prices["cherry"]; ok {
		l.label = "cherry known"
	}
}

func (l *Lookup) View() Element {
	return Column(
		Text(fmt.Sprintf("picked=%d n=%d %s", l.picked, len(l.prices), l.label)),
		Text(fmt.Sprintf("apple costs %d right now", l.price("apple", -1))).Size(12),
		Row(
			Button("apple").OnClick(func() { l.pickApple() }),
			Button("cherry").OnClick(func() { l.addCherry() }),
			Button("miss").OnClick(func() { l.picked = l.price("durian", -7) }),
		).Spacing(6),
	).Spacing(8).Padding(12)
}

func main() {
	Run(&Lookup{prices: map[string]int{"apple": 120, "banana": 80}, label: "none"}, Title("lookup"))
}
