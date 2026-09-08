package main

import (
	"reflect"

	. "github.com/i2y/yokan/gomamochi"
)

type Demo struct{ n int }

func (a *Demo) View() Element { return Text(reflect.TypeOf(a).String()) }

func main() { Run(&Demo{}, Title("x")) }
