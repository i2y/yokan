// Work that takes a while, done off the window's thread. `Task` runs
// the work on a goroutine of its own; when it answers, the second
// closure is called on the window's thread with the answer.
//
// Nothing inside the work touches the app's fields or the screen. That
// is the whole rule, and it is why the answer comes back as an argument
// rather than the worker writing it anywhere.
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Jobs struct {
	status string
	answer int
	done   int
}

func (j *Jobs) start() {
	j.status = "working"
	Task(func() any {
		// deliberately slow, and deliberately arithmetic: both runs
		// have to agree about what it answers.
		total := 0
		for i := 0; i < 300000; i++ {
			total += i % 7
		}
		return total
	}, func(v any) {
		j.answer = v.(int)
		j.done += 1
		j.status = "done"
	})
}

func (j *Jobs) View() Element {
	return Column(
		Text("background work").Size(18).Bold(true),
		Text("status: "+j.status),
		Text(fmt.Sprintf("answer: %d  (%d finished)", j.answer, j.done)),
		Button("start slow work").OnClick(func() { j.start() }),
	).Spacing(10).Padding(14)
}

func main() {
	Run(&Jobs{status: "idle"}, Title("tasks"))
}
