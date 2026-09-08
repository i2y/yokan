// Control flow in the handlers: a loop that skips, a loop that stops, a
// `for` with a condition, and a method that wraps another one. Both
// runs do the same thing, which is what the gate compares.
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Flow struct {
	count  int
	total  int
	status string
}

func (f *Flow) double(v int) int {
	return v * 2
}

// A wrapper around a handler: it says what it is doing, runs the
// handler, and says it is done.
func (f *Flow) announced(work func()) {
	f.status = "working"
	work()
	f.status = "done"
}

func (f *Flow) step() {
	f.count += 1
	if f.count > 3 && f.count < 100 {
		f.status = "big"
	} else if f.count == 3 {
		f.status = "three"
	} else {
		f.status = "small"
	}
}

func (f *Flow) tally() {
	f.total = 0
	for i := 1; i < 6; i++ {
		if i == 3 {
			continue
		}
		f.total += f.double(i)
	}
}

func (f *Flow) bump3() {
	f.announced(func() {
		for f.count < 3 {
			f.count += 1
		}
	})
}

func (f *Flow) find() {
	for i := 0; i < 10; i++ {
		if i*i > 10 {
			f.count = i
			break
		}
	}
}

func (f *Flow) View() Element {
	return Column(
		Text(fmt.Sprintf("count=%d total=%d status=%s", f.count, f.total, f.status)),
		Row(
			Button("step").OnClick(func() { f.step() }),
			Button("tally").OnClick(func() { f.tally() }),
			Button("bump3").OnClick(func() { f.bump3() }),
			Button("find").OnClick(func() { f.find() }),
		).Spacing(6),
	).Spacing(8).Padding(12)
}

func main() {
	Run(&Flow{status: "start"}, Title("flow"))
}
