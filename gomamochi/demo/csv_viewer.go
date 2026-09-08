// A hundred thousand rows, filtered as you type. The list is
// virtualized: only the rows in the window are ever built, so the
// filter is the only thing that touches all of them.
//
// The numbers are arithmetic rather than random, so that a reader can
// follow every row to its value.
package main

import (
	"fmt"
	"strings"

	. "github.com/i2y/yokan/gomamochi"
)

const n = 100000

var (
	cats  = []string{"alpha", "beta", "gamma", "delta", "epsilon"}
	stems = []string{"kuro", "shiro", "aka", "ao", "momo", "yuki", "hana", "sora"}
	tails = []string{"maru", "suke", "chan", "gou", "ta", "emon"}
)

type Viewer struct {
	names  []string
	cats   []string
	values []float64
	q      string
	idx    []int
}

func newViewer() *Viewer {
	v := &Viewer{names: make([]string, n), cats: make([]string, n), values: make([]float64, n), idx: make([]int, n)}
	for i := 0; i < n; i++ {
		v.names[i] = fmt.Sprintf("%s%s-%06d", stems[i%8], tails[(i/8)%6], i)
		v.cats[i] = cats[i%5]
		v.values[i] = float64(i*37%4000)/100 + 30
		v.idx[i] = i
	}
	return v
}

func (v *Viewer) filter(q string) {
	v.q = q
	if q == "" {
		v.idx = make([]int, n)
		for i := range v.idx {
			v.idx[i] = i
		}
		return
	}
	low := strings.ToLower(q)
	idx := make([]int, 0, 1024)
	for i := 0; i < n; i++ {
		if strings.Contains(v.names[i], low) || strings.Contains(v.cats[i], low) {
			idx = append(idx, i)
		}
	}
	v.idx = idx
}

func (v *Viewer) line(k int) Element {
	i := v.idx[k]
	return Row(
		Text(fmt.Sprintf("%06d", i)).Size(12).Color("#8a8f98"),
		Text(v.names[i]).Grow(1),
		Text(v.cats[i]).Size(12).Color("#7aa2f7"),
		Text(fmt.Sprintf("%.2f", v.values[i])).Align("right"),
	).Spacing(12)
}

func (v *Viewer) View() Element {
	return Column(
		Text(fmt.Sprintf("csv viewer — %d rows, virtualized", n)).Size(13).Color("#8a8f98"),
		TextField(v.q).Placeholder("filter…").OnChange(func(t string) { v.filter(t) }),
		Text(fmt.Sprintf("%d / %d rows match", len(v.idx), n)).Size(12),
		ListView(len(v.idx), func(k int) Element { return v.line(k) }).ItemHeight(26).Height(430),
	).Spacing(10).Padding(14)
}

func main() {
	app := newViewer()
	Run(app, Title("csv_viewer"))
}
