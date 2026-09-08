// The controls a person changes: a box, a switch, a track, and the four
// choosers. Each hands its new value to the closure.
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Forms struct {
	dark   bool
	wifi   bool
	volume float64
	fruits []string
	fruit  int
	sizes  []string
	size   int
	tabs   []string
	tab    int
	note   string
}

func (f *Forms) panel() Element {
	if f.tab == 0 {
		return Text("general panel").Size(12)
	}
	if f.tab == 1 {
		return Text("details panel").Size(12)
	}
	return Text("about panel").Size(12)
}

func (f *Forms) View() Element {
	return Column(
		Checkbox("Dark mode").Checked(f.dark).Tooltip("the whole window follows this").
			OnChange(func(on bool) { f.dark = on }),
		Switch("Wi-Fi").Checked(f.wifi).OnChange(func(on bool) { f.wifi = on }),
		Slider().Value(f.volume).Min(0).Max(10).Step(1).Tooltip("0 to 10, in whole steps").
			OnChange(func(v float64) { f.volume = v }),
		Select().Options(f.fruits...).Selected(f.fruit).OnChange(func(i int) { f.fruit = i }),
		RadioGroup().Options(f.sizes...).Selected(f.size).OnChange(func(i int) { f.size = i }),
		TabBar().Labels(f.tabs...).Active(f.tab).OnChange(func(i int) { f.tab = i }),
		f.panel(),
		TextField(f.note).Placeholder("notes (enter writes a newline)").Multiline(true).Rows(3).
			OnChange(func(t string) { f.note = t }),
		Text(fmt.Sprintf("dark=%v  wifi=%v  vol=%.1f", f.dark, f.wifi, f.volume)),
		Text(fmt.Sprintf("fruit#%d  size#%d  tab#%d", f.fruit, f.size, f.tab)),
	).Spacing(10).Padding(14)
}

func main() {
	Run(&Forms{
		wifi:   true,
		volume: 5,
		fruits: []string{"apple", "banana", "cherry"},
		sizes:  []string{"small", "medium", "large"},
		size:   1,
		tabs:   []string{"General", "Details", "About"},
	}, Title("forms"), Size(460, 420))
}
