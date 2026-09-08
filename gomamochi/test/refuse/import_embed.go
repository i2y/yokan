package main

import (
	_ "embed"

	. "github.com/i2y/yokan/gomamochi"
)

//go:embed import_embed.go
var self string

type Demo struct{ n int }

func (a *Demo) View() Element { return Text(self) }

func main() { Run(&Demo{}, Title("x")) }
