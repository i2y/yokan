// Files, with Go's own `os`. Nothing here is Gomamochi's: both runs
// call the same package, and the gate is what says they answer the
// same.
package main

import (
	"fmt"
	"os"

	. "github.com/i2y/yokan/gomamochi"
)

const (
	dir  = "demo/.gate/fs_demo"
	note = "demo/.gate/fs_demo/note.txt"
)

type Files struct {
	content string
	wrote   int
	names   []string
	ready   bool
}

func (f *Files) save() {
	os.MkdirAll(dir, 0o755)
	text := "hello from one standard library"
	if os.WriteFile(note, []byte(text), 0o644) == nil {
		f.wrote = len(text)
	}
}

func (f *Files) addLine() {
	out, err := os.OpenFile(note, os.O_APPEND|os.O_WRONLY, 0o644)
	if err != nil {
		return
	}
	out.WriteString(" (and again)")
	out.Close()
}

func (f *Files) load() {
	text, err := os.ReadFile(note)
	if err != nil {
		f.content = "(unreadable)"
		return
	}
	f.content = string(text)
}

func (f *Files) listing() {
	f.names = nil
	entries, _ := os.ReadDir(dir)
	for _, e := range entries {
		f.names = append(f.names, e.Name())
	}
}

func (f *Files) clean() {
	os.Remove(note)
	f.listing()
}

// A place of the app's own, made on the way out. A demo has no
// business in someone's home directory, so this one keeps to the
// directory the gate already writes in — and beside the one it lists,
// not inside it, or the listing would depend on the order.
func (f *Files) dataDir() {
	path := "demo/.gate/fs_demo_app"
	os.MkdirAll(path, 0o755)
	st, err := os.Stat(path)
	f.ready = err == nil && st.IsDir()
}

func (f *Files) entry(i int) Element {
	return Text(f.names[i])
}

func (f *Files) View() Element {
	return Column(
		Text("content: "+f.content),
		Text(fmt.Sprintf("wrote: %d bytes", f.wrote)),
		Text(fmt.Sprintf("in %s: %d file(s)", dir, len(f.names))),
		ListView(len(f.names), func(i int) Element { return f.entry(i) }).ItemHeight(20).Height(44),
		Text(fmt.Sprintf("data dir ready: %v", f.ready)),
		Row(
			Button("save").OnClick(func() { f.save() }),
			Button("append").OnClick(func() { f.addLine() }),
			Button("load").OnClick(func() { f.load() }),
			Button("list").OnClick(func() { f.listing() }),
			Button("data dir").OnClick(func() { f.dataDir() }),
			Button("remove").OnClick(func() { f.clean() }),
		).Spacing(6),
	).Spacing(8).Padding(12)
}

func main() {
	Run(&Files{content: "(not loaded)"}, Title("files"))
}
