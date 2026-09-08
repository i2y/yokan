// The table that builds its rows on demand: the closure builds row i as
// a row of one cell per column, and the header and the rows sit on
// tracks whose shares are `Widths`. Picking a row and sorting a column
// are the app's own methods.
package main

import (
	"fmt"
	"sort"
	"strconv"

	. "github.com/i2y/yokan/gomamochi"
)

type Roster struct {
	teams   []string
	names   []string
	teamOf  []string
	scores  []int
	sel     int
	line    string
	sortCol int
	desc    bool
}

func newRoster() *Roster {
	r := &Roster{teams: []string{"red", "blue", "green", "gold"}, sel: -1, sortCol: -1}
	for i := 0; i < 24; i++ {
		r.names = append(r.names, fmt.Sprintf("member %d", i))
		r.teamOf = append(r.teamOf, r.teams[i%4])
		r.scores = append(r.scores, (i*37+11)%100)
	}
	return r
}

func (r *Roster) pick(i int) {
	r.sel = i
	r.line = fmt.Sprintf("%s (%s, %d)", r.names[i], r.teamOf[i], r.scores[i])
}

func (r *Roster) sortBy(col int) {
	if col == r.sortCol {
		r.desc = !r.desc
	} else {
		r.desc = false
	}
	r.sortCol = col
	order := make([]int, len(r.names))
	for i := range order {
		order[i] = i
	}
	sort.Slice(order, func(a, b int) bool {
		if col == 2 {
			return r.scores[order[a]] < r.scores[order[b]]
		}
		return r.names[order[a]] < r.names[order[b]]
	})
	if r.desc {
		for i, j := 0, len(order)-1; i < j; i, j = i+1, j-1 {
			order[i], order[j] = order[j], order[i]
		}
	}
	names, teams, scores := make([]string, 0, len(order)), make([]string, 0, len(order)), make([]int, 0, len(order))
	for _, i := range order {
		names = append(names, r.names[i])
		teams = append(teams, r.teamOf[i])
		scores = append(scores, r.scores[i])
	}
	r.names, r.teamOf, r.scores = names, teams, scores
	r.sel = -1
	r.line = ""
}

func (r *Roster) row(i int) Element {
	return Row(
		Text(r.names[i]).Grow(2),
		Text(r.teamOf[i]).Grow(1),
		Text(strconv.Itoa(r.scores[i])).Grow(1).Align("right"),
	)
}

func (r *Roster) View() Element {
	line := r.line
	if line == "" {
		line = "nobody picked"
	}
	return Column(
		Text(fmt.Sprintf("Roster — %d people", len(r.names))).Size(16),
		Table([]string{"name", "team", "score"}, len(r.names), func(i int) Element { return r.row(i) }).
			Widths(2, 1, 1).Height(220).
			Selected(r.sel).Sort(r.sortCol).Descending(r.desc).
			OnSelect(func(i int) { r.pick(i) }).
			OnSort(func(i int) { r.sortBy(i) }),
		Text(line).Size(12),
	).Spacing(10).Padding(14)
}

func main() {
	app := newRoster()
	Run(app, Title("roster"))
}
