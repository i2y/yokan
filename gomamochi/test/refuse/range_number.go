package main

import . "github.com/i2y/yokan/gomamochi"

type Demo struct{ n int }

func (a *Demo) View() Element { return Text("x") }

func (a *Demo) count() {
	for i := range 3 {
		a.n += i
	}
}

func main() { Run(&Demo{}, Title("x")) }
