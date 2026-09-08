<!-- Written by website/tools/tourpages from the tour. Edit the tour. -->
# The first app

Gomamochi builds desktop apps in Go on pixie's engine. `gomamochi run`
reads your file and runs it interpreted, with no build step: save, and
the window takes the new code and keeps the values it had. `gomamochi
build` compiles the same file with the Go compiler into a native
binary. Both reach the same drawing engine (**gpui**, the engine
behind the Zed editor) through pixie's C face, and whether the two are
the same program is something you check rather than hope: `gomamochi
gate` drives both with one interaction script and compares the screens
they drew, byte for byte. The functions that build the screen — `Text`,
`Button`, `Column` and thirty more — come with the package; the app
itself is a plain Go struct. How much of Go the interpreted run takes
is the dialect this tour describes, and what falls outside it is named
at [What Gomamochi refuses](tour-ship.md#what-gomamochi-refuses).

An app is a struct, its state is its fields, and a handler is a closure over them.

## The smallest app

<!-- script: click:+1,dump,input:Momo -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Counter struct {
	count int
	name  string
}

func (c *Counter) View() Element {
	return Column(
		Text(fmt.Sprintf("count: %d", c.count)).Size(34),
		Row(
			Button("+1").OnClick(func() { c.count += 1 }),
			Button("reset").OnClick(func() { c.count = 0 }),
		).Spacing(8),
		TextField(c.name).Placeholder("your name").OnChange(func(s string) { c.name = s }),
		Text(fmt.Sprintf("hello, %s", c.name)),
	).Spacing(12).Padding(16)
}

func main() {
	Run(&Counter{}, Title("counter"))
}
```

An app is a `package main` with a struct in it. The struct's fields are
the state, `View` is a method that answers one element, and a handler
is a closure over the struct through its pointer. `Run` is handed the
app and opens the window. There is nothing to inherit from, nothing to
register, and nothing to mark as observable: after every handler the
view is built again from the fields as they are.

The package is dot-imported, so the elements read as they do in the
other languages on this engine: `Column`, `Text`, `Button`. That is a
matter of taste, not a rule. Imported under a name, every call carries
the prefix — `gm.Column(gm.Text(…))` — and the app may then use any
name it likes for its own types, `App` included, which the dot import
reserves for the package. Both runs take either form, and so does
`check`.

```console
$ ./bin/gomamochi run app.go
```

That opens a window and watches the file. There is no build step: the
file is read and run by the interpreter compiled into the command,
and a save takes effect in place.


## Holding state

State is the fields of the struct, and nothing else. A handler writes
them; the view reads them. Give the app its starting values in the
literal you hand to `Run`, or in a function that makes one:

<!-- script: click:+1,click:mute,dump,input:live set,dump -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Mixer struct {
	volume int
	title  string
	muted  bool
}

func newMixer() *Mixer {
	return &Mixer{volume: 5, title: "untitled"}
}

func (m *Mixer) View() Element {
	cells := []Element{
		Text(fmt.Sprintf("%s — vol %d", m.title, m.volume)).Size(16),
		Row(
			Button("+1").OnClick(func() { m.volume += 1 }),
			Button("mute").OnClick(func() { m.muted = true }),
			Button("unmute").OnClick(func() { m.muted = false }),
		).Spacing(8),
	}
	if m.muted {
		cells = append(cells, Text("(muted)").Size(12).Color("#8a8f98"))
	}
	cells = append(cells, TextField(m.title).Placeholder("title").OnChange(func(t string) { m.title = t }))
	return Column(cells...).Spacing(10).Padding(14)
}

func main() {
	app := newMixer()
	Run(app, Title("mixer"))
}
```

One shape is refused: the app handed to `Run` straight from a call.
`Run(newMixer(), …)` fails in the interpreted run, which hands a
call's result over without the wrapper that lets an interpreted type
stand in for the package's `App` interface. `app := newMixer()` and
then `Run(app, …)` is what to write, and `check` says so with the line.

Types are Go's own, and there is nothing to annotate: `count int`,
`title string`, `volume float64`, a slice, a map, a struct of your
own. The compiled run is typed by the Go compiler; the interpreted run
reads the same declarations. A string and a number never meet without
`strconv` between them, which is Go, not a rule of ours.

