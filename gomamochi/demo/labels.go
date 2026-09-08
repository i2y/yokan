// What a screen reader is told, and what the pointer shows. `Role`
// takes a value, so the summary line is a heading until there is a
// result under it and then it is not.
package main

import . "github.com/i2y/yokan/gomamochi"

type Labels struct {
	title       string
	query       string
	summaryRole string
}

func (l *Labels) View() Element {
	return Column(
		Text(l.title).Size(22).Role("heading"),
		Row(
			Svg("demo/assets/yokan.svg").Width(20).Height(20).A11yLabel("Yokan"),
			Svg("demo/assets/search.svg").Width(20).Height(20).A11yLabel("Search"),
			// The one element carrying a tooltip, a role, a name and a
			// tween at once, which is what pins the order they wrap in.
			Button("save").Animate(150).Easing("out").Role("button").
				A11yLabel("Save the report").Tooltip("Save this report").
				OnClick(func() { l.summaryRole = "label" }),
		).Spacing(6).Role("group").A11yLabel("toolbar"),
		TextField(l.query).Placeholder("search").A11yLabel("search").
			OnChange(func(q string) { l.query = q }),
		Text("1 of 4 saved").Role(l.summaryRole),
		Progress(0.4),
	).Spacing(8).Padding(12)
}

func main() {
	Run(&Labels{title: "Reports", summaryRole: "heading"}, Title("labels"), Size(420, 320))
}
