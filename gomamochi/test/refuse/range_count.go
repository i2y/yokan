package main

import . "github.com/i2y/yokan/gomamochi"

type Demo struct{ n int }

func (a *Demo) View() Element {
	var kids []Element
	for i := range a.n {
		kids = append(kids, Text(string(rune('a'+i))))
	}
	return Column(kids...)
}

func main() { Run(&Demo{n: 3}, Title("x")) }
