// A chooser that changes what a list shows. The rows are built on
// demand, so the list is asked only for the ones in view.
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Alerts struct {
	levels  []string
	level   int
	crit    []string
	warn    []string
	visible []string
}

func (a *Alerts) all() []string {
	out := append([]string{}, a.crit...)
	return append(out, a.warn...)
}

func (a *Alerts) pick(i int) {
	a.level = i
	switch i {
	case 1:
		a.visible = a.crit
	case 2:
		a.visible = a.warn
	default:
		a.visible = a.all()
	}
}

func (a *Alerts) alertRow(i int) Element {
	return Text(a.visible[i]).Size(12)
}

func (a *Alerts) View() Element {
	return Column(
		Text("alert filter").Size(16),
		Segmented().Options(a.levels...).Selected(a.level).OnChange(func(i int) { a.pick(i) }),
		Text(fmt.Sprintf("%d shown", len(a.visible))).Size(12).Color("textDim"),
		ListView(len(a.visible), func(i int) Element { return a.alertRow(i) }).ItemHeight(22).Height(150),
	).Spacing(10).Padding(14)
}

func main() {
	app := &Alerts{
		levels: []string{"all", "crit", "warn"},
		crit: []string{
			"crit  09:02  payments p95 breach — circuit breaker armed",
			"crit  09:11  db failover triggered",
			"crit  09:20  worker pool exhausted",
		},
		warn: []string{
			"warn  09:05  error budget burn 2x on web",
			"warn  09:14  cache hit rate below 80%",
			"warn  09:24  edge latency above SLO",
		},
	}
	app.visible = app.all()
	Run(app, Title("filter"))
}
