package main

import . "github.com/i2y/yokan/gomamochi"

type Demo struct{ n int }

func (a *Demo) View() Element {
	a.n += 1
	return Text("x")
}

func main() { Run(&Demo{}, Title("x")) }
