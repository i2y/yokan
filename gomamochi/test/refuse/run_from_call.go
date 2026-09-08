package main

import . "github.com/i2y/yokan/gomamochi"

type Demo struct{ n int }

func (a *Demo) View() Element { return Text("x") }

func newApp() *Demo { return &Demo{} }

func main() { Run(newApp(), Title("x")) }
