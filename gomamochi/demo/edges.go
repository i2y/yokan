// The edges: an index past the end of a list, and a number far past
// what a machine word holds that keeps growing. Both runs have to
// answer the same, and this is the demo that says so.
//
// Go answers the first by refusing: an index past the end stops the
// program in both runs, so the app asks the length first. The second
// is `math/big`, the same package in both runs.
package main

import (
	"fmt"
	"math/big"

	. "github.com/i2y/yokan/gomamochi"
)

type Edges struct {
	xs     []int
	picked int
	big    *big.Int
	steps  int
}

// The element at i, or -1 when there is none.
func (e *Edges) at(i int) int {
	if i < 0 || i >= len(e.xs) {
		return -1
	}
	return e.xs[i]
}

func (e *Edges) View() Element {
	return Column(
		Text(fmt.Sprintf("picked=%d steps=%d", e.picked, e.steps)),
		Text("big="+e.big.String()),
		Button("oob").OnClick(func() { e.picked = e.at(5) }),
		Button("grow").OnClick(func() { e.big = new(big.Int).Mul(e.big, big.NewInt(4)) }),
		Button("partial").OnClick(func() {
			e.steps += 1
			e.picked = e.at(9)
		}),
	).Spacing(8).Padding(12)
}

func main() {
	start, _ := new(big.Int).SetString("18446744073709551616", 10)
	Run(&Edges{xs: []int{7}, big: start}, Title("edges"))
}
