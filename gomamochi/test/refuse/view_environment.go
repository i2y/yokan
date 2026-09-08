package main

import (
	"os"

	. "github.com/i2y/yokan/gomamochi"
)

type Demo struct{ n int }

func (a *Demo) View() Element { return Text(os.Getenv("HOME")) }

func main() { Run(&Demo{}, Title("x")) }
