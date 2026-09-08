package main

import . "github.com/i2y/yokan/gomamochi"

type Demo struct{ n int }

func (a *Demo) View() Element {
	if KeyDown("space") {
		return Text("space")
	}
	return Text("x")
}

func main() { Run(&Demo{}, Title("x")) }
