// The reference: everything in this file is what Gomamochi takes. The
// app is a struct, its state is its fields, `View` is a method, and a
// handler is a closure over the fields.
//
//	gomamochi run  demo/counter.go
//	gomamochi gate demo/counter.go --script "click:+1,input:Momo"
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Counter struct {
	count int
	name  string
}

func (c *Counter) View() Element {
	return Column(
		Text(fmt.Sprintf("count: %d", c.count)).Size(34),
		Row(
			Button("+1").OnClick(func() { c.count += 1 }),
			Button("+10").OnClick(func() { c.count += 10 }),
			Button("reset").OnClick(func() { c.count = 0 }),
		).Spacing(8),
		TextField(c.name).Placeholder("your name").OnChange(func(s string) { c.name = s }),
		Text(fmt.Sprintf("hello, %s", c.name)),
	).Spacing(12).Padding(16)
}

func main() {
	Run(&Counter{}, Title("counter"))
}
