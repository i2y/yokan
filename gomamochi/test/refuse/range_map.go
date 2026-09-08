package main

import . "github.com/i2y/yokan/gomamochi"

type Demo struct{ prices map[string]int }

func (a *Demo) View() Element {
	var kids []Element
	for name := range a.prices {
		kids = append(kids, Text(name))
	}
	return Column(kids...)
}

func main() { Run(&Demo{prices: map[string]int{"apple": 120}}, Title("x")) }
