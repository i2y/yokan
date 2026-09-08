# Installation

Gomamochi lives in its repository today: there is no distribution to
install and nothing to add to a `go.mod` of yours. You clone the
checkout, and `./bin/gomamochi` is the whole command line.

## What you need

- **macOS on Apple silicon, or Linux.** The engine draws through the
  platform's own GPU stack: Metal there, Vulkan here, on Wayland or
  X11.
- **Go 1.25 or newer.** It builds the command itself, once, and it is
  what `build` and `gate` compile your app with. `run` needs no Go
  toolchain once the command is built: the interpreter is inside it.
- **Rust**, via [rustup](https://rustup.rs). The exact compiler is
  pinned by the repository and fetched on the first build.
- **On macOS, Xcode's Metal toolchain**, because the engine compiles
  its shaders at build time. **On Linux, a C compiler and the
  development packages the engine links**: alsa, fontconfig, freetype,
  xkbcommon and its x11 half, xcb, and the Vulkan loader with a driver.

The interpreter ([yaegi](https://github.com/traefik/yaegi)) and what
opens the engine's library from Go without cgo
([purego](https://github.com/ebitengine/purego)) are Go modules, and
`go` fetches them the first time the command is built. Nothing else is
needed.

## Once per machine

```console
$ export CARGO_TARGET_DIR=$HOME/.cache/pixie/target
```

`CARGO_TARGET_DIR` is shared by every crate in the checkout, which is
what keeps later builds fast. Set it in your shell profile and forget
it.

Nothing else is fetched behind your back. The engine is a crate in the
same checkout, built by `cargo` the first time you run any command
(every command builds it first, so neither run can be a version
behind), and the command is a Go program that `bin/gomamochi` builds
into `~/.cache/gomamochi/bin/` and runs.

## The commands

Run these from `gomamochi/`.

```console
$ ./bin/gomamochi run   demo/counter.go                   # a window; a save reloads it
$ ./bin/gomamochi check demo/counter.go                   # what it cannot take
$ ./bin/gomamochi gate  demo/counter.go --script "click:+1,dump"
$ ./bin/gomamochi build demo/counter.go --release --app   # the binary, and a bundle
```

There is no `translate`, because nothing is translated: the compiled
run is the Go compiler's, from the file as you wrote it.

`run` opens a window and watches the file, so a save takes effect in
place, with the app's values kept. `check` builds nothing. `gate` is
the one that matters: both runs, one script, compared byte for byte.

Four flags belong to `build`. `--release` drops the symbol table;
`--app` wraps the binary and the engine's library as an application: a
macOS bundle under `dist/`, double-clickable, with `<stem>.icns` or
`<stem>.png` beside the app as its icon, or on Linux an AppDir under
`dist/`, which `--appimage` packs into one file; `--carry-libs` makes a
Linux package carry the desktop libraries the engine links as well, for
a machine that may not have them. One flag belongs to `gate`:
`--fresh <path>` deletes a path before each run, so an app that keeps a
file or a database starts both runs from the same nothing.

## Your first file

```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Counter struct {
	count int
}

func (c *Counter) View() Element {
	return Column(
		Text(fmt.Sprintf("count: %d", c.count)).Size(34),
		Button("+1").OnClick(func() { c.count += 1 }),
	).Spacing(12).Padding(16)
}

func main() {
	Run(&Counter{}, Title("counter"))
}
```

```console
$ ./bin/gomamochi run app.go
```

Edit and save while that window is open: a fresh interpreter reads the
file again, and the struct the window is holding carries every value it
had into the new code.

## What a build produces

Measured on macOS/arm64, with the engine built and the caches warm.

| What | Value |
|---|---|
| the binary `gate` builds | 2.9 MB |
| the shipped binary (`--release`) | 1.9 MB |
| the engine's library beside it | 20.4 MB |
| the application bundle (`--app`), holding both | 22.2 MB |
| `check`, through `bin/gomamochi` | 0.6 s, of which the check itself is about a tenth; the rest is `go build` confirming the command is current |
| a headless run under `PIXIE_SCRIPT` | 1.0 s |
| one gate round | about 2 s |
| the whole sweep: 44 demos, the refusals, the tour in both languages and on the site | about three minutes |

The binary is the Go compiler's own, built without cgo, and links
nothing but the system's own libraries; the engine rides beside it as
one shared library. The person receiving it needs neither Go nor the
toolchain.

## Checking the whole thing works

```console
$ just gomamochi-sweep
```

The generated vocabulary against its table, Go's own `vet`, the
refusals against their fixtures, every demo through both runs, and
every complete example in the tour. It is what a change to the
vocabulary, the door or the engine has to pass, and it is the fastest
way to find out that your setup is right.

## Where next

- [The first app](tour.md) — the language, in the order you meet it.
- [Demos](demos.md) — forty-four apps, each with its screenshot and its
  source.
