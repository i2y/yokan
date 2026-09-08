package main

import (
	"time"

	. "github.com/i2y/yokan/gomamochi"
)

type Demo struct{ n int }

func (a *Demo) View() Element { return Text(time.Now().Format("15:04")) }

func main() { Run(&Demo{}, Title("x")) }
