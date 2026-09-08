// The platform's own panels, and a file dragged onto the window. A
// dialog waits for a person, so it is asked for off the window's
// thread; a script answers one with `file:<path>` and drops one with
// `drop:<path>`.
package main

import (
	"os"

	. "github.com/i2y/yokan/gomamochi"
)

type Picker struct {
	chosen string
	body   string
	saved  string
}

func (p *Picker) took(path string) {
	p.chosen = path
	if path != "" {
		p.body = readOr(path, "(unreadable)")
	}
}

func readOr(path, fallback string) string {
	text, err := os.ReadFile(path)
	if err != nil {
		return fallback
	}
	return string(text)
}

func (p *Picker) openOne() {
	Task(func() any { return OpenDialog("Choose a file") }, func(v any) { p.took(v.(string)) })
}

func (p *Picker) saveAs() {
	Task(func() any { return SaveDialog("notes.txt") }, func(v any) {
		path := v.(string)
		if path != "" {
			if os.WriteFile(path, []byte(p.body), 0o644) == nil {
				p.saved = path
			}
		}
	})
}

// The first forty characters of the text.
func head(s string) string {
	r := []rune(s)
	if len(r) > 40 {
		r = r[:40]
	}
	return string(r)
}

func (p *Picker) View() Element {
	return Column(
		Text("chosen: "+p.chosen),
		Text("first line: "+head(p.body)),
		Text("saved to: "+p.saved),
		Row(
			Button("open…").Tooltip("the platform's own panel").OnClick(func() { p.openOne() }),
			Button("save as…").OnClick(func() { p.saveAs() }),
		).Spacing(6),
	).Spacing(8).Padding(12)
}

func main() {
	app := &Picker{chosen: "(nothing yet)", saved: "(not saved)"}
	OnFileDrop(func(path string) { app.took(path) })
	Run(app, Title("picker"))
}
