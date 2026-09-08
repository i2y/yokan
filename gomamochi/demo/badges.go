// Text as a pill: a background with padding and a radius. And the rest
// of what a run of text can be — monospace, underlined, italic, clipped
// with an ellipsis, or wrapped and then clamped.
package main

import . "github.com/i2y/yokan/gomamochi"

type Badges struct {
	tint string
	hot  bool
}

func (b *Badges) flip() {
	b.hot = !b.hot
	if b.hot {
		b.tint = "#f38ba8"
	} else {
		b.tint = "#45475a"
	}
}

// A pill is a piece of text with a background; the rest is the same
// for every one of them.
func pill(label, background string) Element {
	return Text(label).Size(11).Color("#11111b").Padding(4).BorderRadius(10).Background(background)
}

func (b *Badges) View() Element {
	return Column(
		Text("Badges").Size(20).Bold(true),
		Row(
			pill("● OK", "#2fa84f"),
			pill("● WARN", "#fab387"),
			pill("● CRIT", "#f38ba8"),
			Text("● BUILD").Size(11).Color("#cdd6f4").Background(b.tint).
				Padding(4).BorderRadius(10).BorderWidth(1).BorderColor("#585b70"),
		).Spacing(6),
		Button("flip").OnClick(func() { b.flip() }),
		Text("commit 9f2c1ab8e04d").Mono(true).Size(12),
		Text("an underlined note").Underline(true),
		Text("in italics, for contrast").Italic(true),
		// An ellipsis needs a bounded box to clip against.
		Text("a single line far too long for the box it was given, so it ends in an ellipsis").
			Wrap("ellipsis").Width(260),
		// The clamp is the other half: this one wraps, then stops.
		Text("a paragraph that wraps at the window's width and then stops after two lines, "+
			"because a clamped label is what a card summary wants").
			MaxLines(2).Width(260),
	).Spacing(8).Padding(12)
}

func main() {
	Run(&Badges{tint: "#45475a"}, Title("badges"))
}
