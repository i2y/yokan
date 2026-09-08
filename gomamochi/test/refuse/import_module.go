package main

import (
	"github.com/example/charts"

	. "github.com/i2y/yokan/gomamochi"
)

type Demo struct{ n int }

func (a *Demo) View() Element { return Text(charts.Title()) }

func main() { Run(&Demo{}, Title("x")) }
