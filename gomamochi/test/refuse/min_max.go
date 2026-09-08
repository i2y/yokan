package main

import . "github.com/i2y/yokan/gomamochi"

type Demo struct{ n int }

func (a *Demo) View() Element { return Text("x") }

func (a *Demo) clamp() { a.n = min(a.n, 10) }

func main() { Run(&Demo{}, Title("x")) }
