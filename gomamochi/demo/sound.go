// Sound. A WAV file is played and the call answers at once, so a
// handler that starts one carries on.
//
// A run under a script is silent: a gate must not need a machine with
// speakers, and both runs read that one flag through the same library,
// so neither is louder than the other. That is why this demo can be
// gated at all — the screen is what the two runs compare.
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

const dir = "demo/assets/sound"

type Sound struct {
	played int
	last   string
	volume float64
}

func (s *Sound) play(name string) {
	AudioPlay(dir+"/"+name+".wav", s.volume)
	s.played += 1
	s.last = name
}

func (s *Sound) hush() {
	AudioStop()
	s.last = "stopped"
}

func (s *Sound) View() Element {
	return Column(
		Text("sound").Size(18).Bold(true),
		Text(fmt.Sprintf("played: %d   last: %s", s.played, s.last)),
		Row(
			Button("jump").OnClick(func() { s.play("jump") }),
			Button("pickup").OnClick(func() { s.play("pickup") }),
			Button("blast").OnClick(func() { s.play("blast") }),
		).Spacing(6),
		Row(
			Button("shoot").OnClick(func() { s.play("shoot") }),
			Button("over").OnClick(func() { s.play("over") }),
			Button("stop").OnClick(func() { s.hush() }),
		).Spacing(6),
		Slider().Value(s.volume).Min(0).Max(1).Step(0.1).OnChange(func(v float64) { s.volume = v }),
		Text(fmt.Sprintf("volume %.1f", s.volume)).Size(12).Color("#8a8f98"),
	).Spacing(10).Padding(14)
}

func main() {
	Run(&Sound{last: "-", volume: 0.6}, Title("sound"))
}
