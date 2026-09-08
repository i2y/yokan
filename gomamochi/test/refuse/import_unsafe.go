package main

import (
	"unsafe"

	. "github.com/i2y/yokan/gomamochi"
)

type Demo struct{ n int }

func (a *Demo) View() Element { return Text("x") }

func main() {
	_ = unsafe.Sizeof(0)
	Run(&Demo{}, Title("x"))
}
