package main

import . "github.com/i2y/yokan/gomamochi"

type Demo struct{ n int }

func pair(yield func(int) bool) {
	_ = yield(1) && yield(2)
}

func (a *Demo) View() Element {
	var kids []Element
	for i := range pair {
		kids = append(kids, Text(string(rune('a'+i))))
	}
	return Column(kids...)
}

func main() { Run(&Demo{}, Title("x")) }
