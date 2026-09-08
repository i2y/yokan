<!-- Written by website/tools/tourpages from the tour. Edit the tour. -->
# Verify and ship

The gate, what Gomamochi refuses, the bundle, and what does not work yet.

## Headless runs and the gate

`PIXIE_SCRIPT` replaces the person. The engine builds the tree, drives
it with the steps, and prints the dumps:

```
click:<label>      press a button by the label it shows
input:<text>       type into a field   submit    press enter in it
slide / select     move a slider, pick an option
click@1:<label>    the second button with that label (n counts from 0, in tree order)
                   and the same for input@n:, submit@n, slide@n:, select@n:
key:<chord>        a keystroke bound to a shortcut
keydown:<key> / keyup:<key>    hold a key down, let it up
menu:<item>        pick a menu item    file:<path>   answer a dialog
drop:<path>        a file dragged onto the window
advance:<ms>       move the clock      theme:dark|light
dump               print the tree      a11y   print what a reader reads
```

`gomamochi gate` runs the app twice with one script — the file under
the interpreter, and the binary the Go compiler built from it — and
compares the two transcripts byte for byte.

```console
$ ./bin/gomamochi gate demo/counter.go --script "click:+1,dump,input:Momo"
GATE OK — 3 dump lines identical in both runs
```

The gate is the promise. Here it says that the interpreter, which is
what you were looking at while writing, agrees with the compiler,
which is what ships: everything else in this page is a way of writing
something the gate can keep. `--fresh <path>` deletes a path before
each run, so an app that keeps a file or a database starts both runs
from the same nothing.


## What Gomamochi refuses

`gomamochi check` reads the app and names what it cannot take, with the
line, a caret, and the rewrite. It runs before every run, build and
gate, and prints nothing when there is nothing to say.

```console
$ ./bin/gomamochi check demo/broken.go
demo/broken.go:8:2: Gomamochi cannot take this — a view only reads. Move the write into a handler — the closure on a button, or a method the app calls from one
    	a.n += 1
    	^
```

Two layers. The first reads the file alone, and runs everywhere. The
second is Go's own type checker, fed the toolchain's export data, which
speaks Go's type errors in Go's words and adds what only a type can
decide; it runs when `go` is on the path, which `build` and `gate`
need anyway.

What the interpreted run cannot run as the compiled one does:

- `min` and `max` (Go 1.21), and `range` over a number (1.22) or a
  function (1.23), which the interpreter does not have. Write the
  comparison out, or `for i := 0; i < n; i++`.
- The app handed to `Run` straight from a call. Give it a name first.
- `%T`, and `reflect`: the interpreter names the app's own types
  differently.
- `unsafe`, cgo, `//go:embed`, and a module outside the standard
  library.
- A three-clause loop that writes its own variable while a closure
  captures it.

What a view may not do, because it is built again from the same state
whenever anything changes:

- Write a field of the app, or start a goroutine.
- Read the clock, the environment, a file, a stream, the network, a
  random number, or the keyboard; start work or a timer; play a sound.
- `range` over a map.

Each of those has a fixture under `test/refuse/` holding the message
it prints, so a refusal cannot quietly change its wording.


## Shipping

```console
$ ./bin/gomamochi build demo/todo.go --release --app
built: demo/.gate/todo/todo (1.9 MB)
bundle: demo/dist/todo.app (22.3 MB)
```

`build` compiles the file with the Go compiler, with cgo off, into a
binary that links nothing but the system's own library; the engine is
a shared library that rides beside it, and the door looks there first.
`--release` drops the symbol table. `--app` wraps the two in a macOS
application bundle, ad-hoc signed, with `<stem>.png` or `<stem>.icns`
beside the app as its icon; the bundle is the whole program, and opens
on a machine with neither Go nor the toolchain installed. On Linux
`--app` writes an AppDir instead, `--appimage` packs it into one file,
and `--carry-libs` makes it carry the desktop's libraries too, for a
machine that may not have them.


## What does not work yet

- The interpreter's library is Go 1.22's, and its language is short of
  that: `min` and `max` (1.21), `range` over a number (1.22) or a
  function (1.23), and what the library gained in 1.23 and later are
  not there; `check` names the first three, and the interpreter's own
  error names the rest.
- A module outside the standard library cannot be read by the
  interpreted run yet, and an app is one file.
- Under the dot import, an app cannot declare a name the package
  exports — `App`, `Element`, `Text` and the rest. Import the package
  under a name and it can.
- A recovered panic's message is worded differently in the two runs,
  and `%T` prints a different name; neither is a thing to show a
  person.
- The interpreted run is slower than the compiled one by two orders of
  magnitude on tight loops, which is what an interpreter costs; a
  frame of either game is still under a millisecond of it.
- macOS and Linux. The bundle and the AppDir carry the engine, so even
  a small app weighs about 22 MB; the Linux packaging is written the
  way Yokan's is and has not yet been run on a Linux machine.
