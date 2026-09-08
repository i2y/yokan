# Gomamochi

**Gomamochi builds desktop apps in Go on pixie's engine, and
`gomamochi gate` is how you check that what you saw while writing is
what you ship.**

An app is a Go struct with a `View` method. `gomamochi run` reads the
file and runs it interpreted, with no build step: save the file and the
window takes the new code and keeps the values it had. `gomamochi
build` compiles the same file with the Go compiler into a native
binary. Both reach the same drawing engine — **gpui**, the engine
behind the Zed editor — through pixie's C face, opened without cgo. The
gate drives both with one interaction script and compares what they
drew, byte for byte — so "it worked while I was writing it" and "it
works as shipped" are one claim, not two.

Gomamochi is the fourth language on this engine, after Yokan (Python),
Wakakusa (Ruby) and Rakugan (Perl). It shares the substrate with them
and nothing else. There is no translator in it: the compiled run is
the Go compiler's own, and the interpreted run is
[yaegi](https://github.com/traefik/yaegi), compiled into the command
together with the door and the standard library.

## What an app looks like

The app is a struct. Its state is its fields, `View` is a method that
answers one element, and a handler is a closure over the fields. The
package is dot-imported, so the elements read as they do in the other
languages on the engine.

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
			Button("+10").OnClick(func() { c.count += 10 }),
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

Go has no keyword arguments, so an element's keywords are methods on
it, and a chain reads like the keyword list would: `Text("…").Size(34)`,
`Column(…).Spacing(12).Padding(16)`. A handler's type follows what the
event carries: `OnClick(func())`, `OnChange(func(string))`.

## The commands

```console
$ ./bin/gomamochi check demo/counter.go
$ ./bin/gomamochi run   demo/counter.go
$ ./bin/gomamochi build demo/counter.go --release
$ ./bin/gomamochi gate  demo/counter.go --script "click:+1,dump,input:Momo"
GATE OK — 3 dump lines identical in both runs
```

`check` names what the app writes that Gomamochi cannot take, with the
line and what to write instead, and says nothing when there is nothing
to say. `run` opens a window and watches the file. `build` writes the
native binary under `demo/.gate/<name>/`, with the engine's library
beside it. `gate` runs the app both ways headless and compares the two
transcripts; `tools/gate_all.sh` does that for every demo.

The command is a Go program: `bin/gomamochi` builds it into
`~/.cache/gomamochi/bin/` and runs it. It needs Go 1.25 or newer and
the engine, which it builds with cargo into the shared target dir
before every run.

## Where the vocabulary comes from

Every element, its keywords, their types and defaults, and what a
handler receives are one table, `crates/pixie-capi/elements.toml`,
which the engine and the other languages read too. `go run ./tools/gen`
writes `elements.go` (one type per element, one method per keyword, the
canvas's drawing commands) and the interpreter's view of the package
from it; the sweep fails when either is behind the table. Adding an
element is a row there and an arm in the engine, and nothing in Go.

## What is here now

The door over the whole C face, the whole vocabulary, `Run`, `Task`,
`Every`, the keyboard, and eight demos. Timers, the canvas, a window's
reload with values kept, and the framework's own standard library are
written but not yet driven by a demo.
