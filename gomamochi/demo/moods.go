// Values that are one of a few named things, and a value that may be
// nothing at all. Go writes the first as constants and the second as a
// pointer that may be nil.
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

const (
	happy = "happy"
	sad   = "sad"
)

type Tracker struct {
	last  *int
	trend string
}

func (t *Tracker) note(v int) {
	t.last = &v
	if t.trend == happy {
		t.trend = sad
	} else {
		t.trend = happy
	}
}

func (t *Tracker) wipe() {
	t.last = nil
}

type Moods struct {
	mood    string
	sel     *int
	note    string
	tracker *Tracker
}

func (m *Moods) flip() {
	if m.mood == happy {
		m.mood = sad
	} else {
		m.mood = happy
	}
}

func (m *Moods) describe() {
	if m.sel == nil {
		m.note = "nothing chosen"
	} else {
		m.note = fmt.Sprintf("chose %d", *m.sel)
	}
}

func (m *Moods) moodLine() Element {
	if m.mood == happy {
		return Text("mood: up").Size(18).Color("accent").Animate(120).Easing("out")
	}
	return Text("mood: down").Size(18).Color("#f38ba8").Animate(120).Easing("out")
}

func (m *Moods) View() Element {
	selection := Text("(no selection)")
	if m.sel != nil {
		selection = Text(fmt.Sprintf("selection: %d", *m.sel))
	}
	tracked := Text("(nothing tracked)").Size(12)
	if m.tracker.last != nil {
		tracked = Text(fmt.Sprintf("tracked: %d", *m.tracker.last)).Size(12)
	}
	return Column(
		m.moodLine(),
		selection,
		Text("note: "+m.note),
		tracked,
		Row(
			Button("flip").OnClick(func() { m.flip() }),
			Button("pick").OnClick(func() { seven := 7; m.sel = &seven }),
			Button("clear").OnClick(func() { m.sel = nil }),
			Button("describe").OnClick(func() { m.describe() }),
			Button("track").Animate(100).Easing("inOut").OnClick(func() { m.tracker.note(9) }),
			Button("wipe").OnClick(func() { m.tracker.wipe() }),
		).Spacing(6),
	).Spacing(8).Padding(12)
}

func main() {
	Run(&Moods{mood: happy, note: "-", tracker: &Tracker{trend: happy}}, Title("moods"))
}
