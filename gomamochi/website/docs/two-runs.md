# The two runs

An app is one file. It is run two ways, and the whole point of
Gomamochi is that the two are the same program. This page is what each
run is made of, what they share, and the few places they are not the
same.

**Both runs are Go, and only one of them is the Go compiler.** When you
ship, `go build` compiles your file into the binary you hand out. While
you write, the same file is read by an interpreter, embedded in the
`gomamochi` command. That asymmetry is the reverse of the one the other
languages on this engine live with: there the interpreter is the
reference and the compiled run is the one that could differ; here the
compiled run is the reference, and the interpreted run is the one that
could. Every difference between the two comes from that, and so does
the reason `check` refuses a handful of shapes before anything runs.

## One language, one file

Gomamochi defines no language of its own and translates nothing. What
you write is Go, given a library:

| | What it takes |
|---|---|
| **`go build`** | all of Go 1.25. It is what your app ships as, and `check` type-checks your file with the same `go/types` the compiler uses, so an error there is Go's own, in Go's own words. |
| **yaegi** | the Go of 1.22, as an interpreter: generics, closures, goroutines, channels, and the standard library's packages compiled into the command. Not `min` and `max`, not `range` over a number or a function, not `%T` on the app's own types, not `reflect`, `unsafe`, cgo or `//go:embed`, and not a module outside the standard library yet. `gomamochi check` refuses those before anything is built: [What Gomamochi refuses](refusals.md). |

On top of that, Gomamochi adds one thing, and it is a library rather
than a language: the elements that build the screen, and `Run`,
`Every`, `Task`, `SqliteExec` and their neighbours. Your structs, your
methods, your goroutines and Go's own packages are Go's.

## What runs while you are writing

```console
$ ./bin/gomamochi run app.go
```

That is the whole of it. The command is a Go program with
[yaegi](https://github.com/traefik/yaegi) (pinned at 0.16.1), the
standard library's symbols and the door compiled into it, where the
door is the package that opens the engine's library through purego. It
reads your file, hands it to a fresh interpreter, and the interpreter
calls the same compiled package your binary will. Nothing above the
door knows which run this is.

One rewrite happens before the interpreter reads the file. Since Go
1.22 a loop variable is one per iteration, so a closure that captures
it sees its own; yaegi keeps the older rule. So every loop variable a
closure captures gets a copy of its own at the top of the loop's body,
on the same line as the brace, so that line numbers stay where they
were and the interpreter sees what the compiler would have given the
closure. The one shape the copy would hide, a three-clause loop whose
body writes the variable, is refused instead.

A save while the window is open reads the file again. A new interpreter
takes the new file, the `Run` it reaches finds the window already up,
and the app the window was holding is carried field by field, by name
and type, into the new one. The values survive; a field that changed
its type starts over.

## What runs when you ship

```console
$ ./bin/gomamochi build app.go --release
```

`go build`, with `CGO_ENABLED=0`, on the file as you wrote it: no
translation, and no rewrite either, because the loop rule is the
compiler's own. The binary links nothing but the system's own libraries
and opens the engine's library beside it when it starts; `--app` puts
the two in one bundle.

## What both of them drive

One engine, behind a C face: `crates/pixie-capi`, which is pixie's
kernel with gpui drawing, built as one shared library. Both runs open
it through purego, without cgo. This is the one place Gomamochi's shape
differs from its siblings': both runs cross the C face, and neither
links the engine statically.

The face is deliberately narrow. An element is opened, written into by
number, and closed; a handler is a number the door hands out; what the
engine calls back with is integers only. **The engine never holds a Go
value**, which is why one implementation can serve an interpreter and a
compiled binary without knowing which it is talking to.

Those numbers are not written by hand either. `elements.toml` is the
one table: every element, every keyword it takes, its type and its
default. `go run ./tools/gen` writes `elements.go` (one type per
element, one method per keyword) and the interpreter's view of the
package from it, and the sweep fails when either is behind the table.
An element cannot come to mean one thing in Go and another where it is
drawn, and the interpreter cannot see a different package from the one
the binary links. The other three languages on this engine read the
same table.

## Where the two are not the same

The compiler is the specification: where the two differ, the compiled
run is right and the interpreted run has a bug. What follows from that
is what a gate tells you. A green gate says the window you were writing
in was showing you what ships. The differences come in three kinds, and
it is worth knowing which kind you are looking at, because they are
caught by three different things.

### 1. Refused before it runs

Most of the gap is closed by subtraction. A shape the interpreter stops
on, or runs and quietly answers differently, is refused with the line
and the rewrite, before anything is built. `check` reads the file with
`go/parser`, which needs no toolchain, and, when `go` is on the path,
type-checks it with `go/types` against the toolchain's own export data,
which is what tells a `range` over a map, a number or a function apart.
[What Gomamochi refuses](refusals.md) is the whole list, quoted from
the fixtures that hold the wording.

Three of those deserve naming here, because they are Go a Go programmer
writes without thinking. `min` and `max` are Go 1.21's, and the
interpreter does not know them: write the comparison out, or a small
function of your own. `for i := range n` is Go 1.22's, and the
interpreter stops on it: write the three-clause loop. And `for k := range m`
over a map is refused in a view, because a map walks in a different
order every run; keep a sorted list of the keys on the app, made in a
handler, which may range over the map freely.

### 2. Answers differently

Go's own standard library is the same compiled code in both runs. The
interpreter calls the `strconv`, `strings`, `math` and `sort` compiled
into the command, and the binary links the same packages, so there are
no twins to keep honest and no table of what Go prints: a number
formats the same, an integer overflows the same, a string upper-cases
the same. What the interpreter sees, though, is the standard library's
table as of Go 1.22; what the library gained after that is not there.

Two things do answer differently. `%T` prints the interpreter's name
for an app's own type, not Go's, so it is refused. And a runtime panic
is worded differently by the interpreter; an app that recovers one and
draws its text is a difference the gate catches, and nothing refuses
yet.

The framework's own standard library is one implementation. A database,
the clipboard, the platform's dialogs, sound and notifications are the
engine's, reached through the C face by both runs, so what the gate
compares there is one library answering twice. Files, the network, JSON
and the clock are Go's own packages, `os`, `net/http`, `encoding/json`
and `time`, the same code in both runs.

### 3. The same program, at different speeds

The compiled run is faster. The arithmetic of a frame of the canvas
games ran about a hundred times slower interpreted, and still took
under a tenth of a millisecond, well inside a frame; a recursive
`fib(25)` was two hundred times slower. A long computation that merely
feels slow while you are writing is not a difference in behaviour, and
a handler that blocks freezes the window in both, which is what `Task`
is for.

## The clock

Both runs tick off one clock. A timer declared with `Every` fires on a
frame in a window and on an `advance:<ms>` step under a script, so the
same number of ticks lands in both. A view may not read the wall clock,
and `check` refuses `time.Now` there; a handler or a timer reads it and
keeps what it read on the app, and both runs then draw what the app
holds.

## What that leaves you to check

The gate. One script, both runs, byte for byte:

```console
$ ./bin/gomamochi gate demo/counter.go --script "click:+1,dump"
GATE OK — 3 dump lines identical in both runs
```

A green gate says the two runs agree on everything the script touched.
It says nothing about what a script never reached, and nothing at all
about the framework's own library, where one implementation answers
both runs and a mistake in it is a mistake in both. That is what the
sweep is for: every demo through the gate, the refusals against the
fixtures that hold their wording, and every complete example in the
tour. Together they are the claim: the compiled run is the Go
compiler's, and the window you were writing in agreed with it.
