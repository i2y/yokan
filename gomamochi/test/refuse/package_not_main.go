package app

import . "github.com/i2y/yokan/gomamochi"

type Demo struct{ n int }

func (a *Demo) View() Element { return Text("x") }
