// The counter again, written the other way: the package imported under
// a name rather than dot-imported. Every call reads `gm.` in front, the
// app may use any name it likes for its own types, and both runs take
// it the same. Which way to write an app is a matter of taste; the
// tour uses the bare form because the other languages on the engine
// read that way.
package main

import (
	"fmt"

	gm "github.com/i2y/yokan/gomamochi"
)

type Counter struct {
	count int
	name  string
}

func (c *Counter) View() gm.Element {
	return gm.Column(
		gm.Text(fmt.Sprintf("count: %d", c.count)).Size(34),
		gm.Row(
			gm.Button("+1").OnClick(func() { c.count += 1 }),
			gm.Button("+10").OnClick(func() { c.count += 10 }),
			gm.Button("reset").OnClick(func() { c.count = 0 }),
		).Spacing(8),
		gm.TextField(c.name).Placeholder("your name").OnChange(func(s string) { c.name = s }),
		gm.Text(fmt.Sprintf("hello, %s", c.name)),
	).Spacing(12).Padding(16)
}

func main() {
	gm.Run(&Counter{}, gm.Title("prefixed"))
}
