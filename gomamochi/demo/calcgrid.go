// The same calculator as demo/calc.go, on a grid instead of five rows.
// `ColSpan` is what makes the zero key twice as wide.
package main

import (
	"strconv"
	"strings"

	. "github.com/i2y/yokan/gomamochi"
)

func key(b *ButtonEl) *ButtonEl {
	return b.Grow(1).Size(20).Background("panel").HoverBackground("#45475a").ActiveBackground("#585b70")
}

func fun(b *ButtonEl) *ButtonEl { return key(b).Background("#313244").Color("#a6adc8") }

func op(b *ButtonEl) *ButtonEl {
	return key(b).Background("#fab387").Color("#1e1e2e").HoverBackground("#f8c49b").ActiveBackground("#f5e0dc")
}

type CalcGrid struct {
	display string
	acc     float64
	op      string
	fresh   bool
	hasDot  bool
}

func num(v float64) string {
	s := strconv.FormatFloat(v, 'f', -1, 64)
	if !strings.ContainsAny(s, ".e") {
		s += ".0"
	}
	return s
}

func toFloatOrZero(s string) float64 {
	if strings.HasSuffix(s, ".") {
		return 0
	}
	v, err := strconv.ParseFloat(s, 64)
	if err != nil {
		return 0
	}
	return v
}

func (c *CalcGrid) press(d string) {
	if c.fresh {
		c.display = d
		c.fresh = false
		c.hasDot = false
	} else if c.display == "0" {
		c.display = d
	} else {
		c.display = c.display + d
	}
}

func (c *CalcGrid) dot() {
	if c.fresh {
		c.display = "0."
		c.fresh = false
		c.hasDot = true
	} else if !c.hasDot {
		c.display = c.display + "."
		c.hasDot = true
	}
}

func (c *CalcGrid) negate() {
	v := toFloatOrZero(c.display)
	if v == 0 {
		return
	}
	c.display = num(0 - v)
	c.fresh = false
}

func (c *CalcGrid) percent() {
	c.display = num(toFloatOrZero(c.display) / 100)
	c.fresh = true
	c.hasDot = false
}

func (c *CalcGrid) apply(next string) {
	if c.fresh && c.op != "" {
		c.op = next
		return
	}
	cur := toFloatOrZero(c.display)
	switch c.op {
	case "":
		c.acc = cur
	case "+":
		c.acc += cur
	case "-":
		c.acc -= cur
	case "×":
		c.acc *= cur
	case "÷":
		if cur == 0 {
			c.display = "Error"
			c.acc = 0
			c.op = ""
			c.fresh = true
			return
		}
		c.acc /= cur
	}
	c.display = num(c.acc)
	c.op = next
	c.fresh = true
}

func (c *CalcGrid) clear() {
	c.display = "0"
	c.acc = 0
	c.op = ""
	c.fresh = true
	c.hasDot = false
}

func (c *CalcGrid) digit(d string) Element {
	return key(Button(d)).OnClick(func() { c.press(d) })
}

func (c *CalcGrid) opKey(name string) Element {
	return op(Button(name)).OnClick(func() { c.apply(name) })
}

func (c *CalcGrid) View() Element {
	return Column(
		Text(c.display).Size(40).Color("text").Align("right").Grow(1.4),
		Grid(
			fun(Button("C")).OnClick(func() { c.clear() }),
			fun(Button("±")).OnClick(func() { c.negate() }),
			fun(Button("%")).OnClick(func() { c.percent() }),
			c.opKey("÷"),
			c.digit("7"), c.digit("8"), c.digit("9"), c.opKey("×"),
			c.digit("4"), c.digit("5"), c.digit("6"), c.opKey("-"),
			c.digit("1"), c.digit("2"), c.digit("3"), c.opKey("+"),
			key(Button("0")).ColSpan(2).OnClick(func() { c.press("0") }),
			key(Button(".")).OnClick(func() { c.dot() }),
			op(Button("=")).OnClick(func() { c.apply("") }),
		).Columns(4).Rows(5).Spacing(8).Grow(5),
	).Spacing(8).Padding(16).Grow(1)
}

func main() {
	Run(&CalcGrid{display: "0", fresh: true}, Title("calcgrid"))
}
