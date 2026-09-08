package main

import . "github.com/i2y/yokan/gomamochi"

type Demo struct{ picked int }

func (a *Demo) View() Element {
	var kids []Element
	for i := 0; i < 3; i++ {
		if i == 1 {
			i = 2
		}
		kids = append(kids, Button("pick").OnClick(func() { a.picked = i }))
	}
	return Column(kids...)
}

func main() { Run(&Demo{}, Title("x")) }
