// The keyboard as a set of chords, and the same handlers in the
// application's menu bar. A script presses one with `key:cmd+s` and
// picks one with `menu:Save`.
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Keys struct {
	count  int
	saved  int
	last   string
	pasted string
}

func (k *Keys) save() {
	k.saved = k.count
}

func (k *Keys) clear() {
	k.count = 0
	k.saved = 0
}

func (k *Keys) copyCount() {
	ClipboardSetText(fmt.Sprintf("count=%d", k.count))
}

func (k *Keys) paste() {
	k.pasted = ClipboardGetText()
}

func (k *Keys) typed(chord string) {
	k.last = chord
}

func (k *Keys) View() Element {
	return Column(
		Text(fmt.Sprintf("count: %d  saved: %d", k.count, k.saved)),
		Text("last key: "+k.last),
		Text("pasted: "+k.pasted),
		Row(
			Button("+1").OnClick(func() { k.count += 1 }),
			Button("save").OnClick(func() { k.save() }),
			Button("copy").OnClick(func() { k.copyCount() }),
			Button("paste").OnClick(func() { k.paste() }),
		).Spacing(6),
	).Spacing(8).Padding(12)
}

func main() {
	app := &Keys{last: "-", pasted: "(nothing)"}

	MenuItem("Count", "Save", func() { app.save() })
	MenuItem("Count", "Clear", func() { app.clear() })

	Shortcut("cmd+s", func() { app.save() })
	Shortcut("cmd+shift+r", func() { app.clear() })
	Shortcut("cmd+shift+c", func() { app.copyCount() })
	Shortcut("cmd+shift+v", func() { app.paste() })
	OnKey(func(chord string) { app.typed(chord) })

	Run(app, Title("keys"))
}
