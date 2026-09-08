// A calculator: one accumulator, one pending operation, and a display
// the app builds as a string. The look of a key is a function that
// sets the same properties on every button handed to it.
package main

import (
	"strconv"
	"strings"

	. "github.com/i2y/yokan/gomamochi"
)

// The calculator's look.
func key(b *ButtonEl) *ButtonEl {
	return b.Grow(1).Size(20).Background("panel").HoverBackground("#45475a").ActiveBackground("#585b70")
}

func fun(b *ButtonEl) *ButtonEl { return key(b).Background("#313244").Color("#a6adc8") }

func op(b *ButtonEl) *ButtonEl {
	return key(b).Background("#fab387").Color("#1e1e2e").HoverBackground("#f8c49b").ActiveBackground("#f5e0dc")
}

func wide(b *ButtonEl) *ButtonEl { return key(b).Grow(2).Basis(8) }

func keys(r *RowEl) *RowEl { return r.Spacing(8).Grow(1) }

type Calc struct {
	display string
	acc     float64
	op      string
	fresh   bool
	hasDot  bool
}

// The readout reads the way the original's does: a whole number still
// shows its ".0".
func num(v float64) string {
	s := strconv.FormatFloat(v, 'f', -1, 64)
	if !strings.ContainsAny(s, ".e") {
		s += ".0"
	}
	return s
}

// A display that is not a number is worth 0, as it is in the original,
// where a trailing dot is not a number either.
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

func (c *Calc) press(d string) {
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

func (c *Calc) dot() {
	if c.fresh {
		c.display = "0."
		c.fresh = false
		c.hasDot = true
	} else if !c.hasDot {
		c.display = c.display + "."
		c.hasDot = true
	}
}

func (c *Calc) negate() {
	v := toFloatOrZero(c.display)
	if v == 0 {
		return
	}
	c.display = num(0 - v)
	c.fresh = false
}

func (c *Calc) percent() {
	c.display = num(toFloatOrZero(c.display) / 100)
	c.fresh = true
	c.hasDot = false
}

func (c *Calc) apply(next string) {
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

func (c *Calc) clear() {
	c.display = "0"
	c.acc = 0
	c.op = ""
	c.fresh = true
	c.hasDot = false
}

func (c *Calc) digit(d string) Element {
	return key(Button(d)).OnClick(func() { c.press(d) })
}

func (c *Calc) opKey(name string) Element {
	return op(Button(name)).OnClick(func() { c.apply(name) })
}

func (c *Calc) View() Element {
	return Column(
		Text(c.display).Size(40).Color("text").Align("right").Grow(1.4),
		keys(Row(
			fun(Button("C")).OnClick(func() { c.clear() }),
			fun(Button("±")).OnClick(func() { c.negate() }),
			fun(Button("%")).OnClick(func() { c.percent() }),
			c.opKey("÷"),
		)),
		keys(Row(c.digit("7"), c.digit("8"), c.digit("9"), c.opKey("×"))),
		keys(Row(c.digit("4"), c.digit("5"), c.digit("6"), c.opKey("-"))),
		keys(Row(c.digit("1"), c.digit("2"), c.digit("3"), c.opKey("+"))),
		keys(Row(
			wide(Button("0")).OnClick(func() { c.press("0") }),
			key(Button(".")).OnClick(func() { c.dot() }),
			op(Button("=")).OnClick(func() { c.apply("") }),
		)),
	).Spacing(8).Padding(16).Grow(1)
}

func main() {
	Run(&Calc{display: "0", fresh: true}, Title("calc"))
}
