package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Demo struct{ n int }

func (a *Demo) View() Element { return Text(fmt.Sprintf("%T", a)) }

func main() { Run(&Demo{}, Title("x")) }
