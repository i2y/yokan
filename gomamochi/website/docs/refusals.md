<!-- Written by website/tools/refusalspage from test/refuse/. Edit the fixtures. -->
# What Gomamochi refuses

There is no translator in Gomamochi, so there is no second language to
learn: an app is Go. What `gomamochi check` refuses is the Go the
interpreted run cannot run the way the compiled one does, and the few
rules a view has to keep. It reads the app and names what it cannot
take, with the file, the line and the column, the line itself, and what
to write instead; with `go` on the path it also type-checks the file the
way the compiler will, and Go's own errors come back in Go's own words.
`check` runs before every build and every gate, builds nothing and opens
no window, and prints nothing at all when there is nothing to say.

Each of the 21 below has a file under `test/refuse/` that triggers it and
the message it must print, word for word. The sweep runs them, so a
refusal cannot quietly change its wording, and this page is quoted from
those same files.

## The app's shape

An app is a program: `package main`, with a `main` that hands the app to `Run`.

```console
test/refuse/package_not_main.go:1:9: Gomamochi cannot take this — an app is a `package main` whose `main` hands the app to `Run`
    package app
            ^
```

Without a `main` there is nothing to run, and nothing to hand to `Run`.

```console
test/refuse/no_main.go:1:1: Gomamochi cannot take this — there is no `main`. Write `func main() { Run(&App{}, Title("…")) }`
    package main
    ^
```

The interpreter hands a call's result over without the wrapper that lets an interpreted type satisfy a compiled interface, and `Run` then cannot take it; a value with a name gets the wrapper.

```console
test/refuse/run_from_call.go:11:19: Gomamochi cannot take this — the app is handed to `Run` straight from a call, and the interpreted run cannot take it that way. Give it a name first: `app := newApp()`, then `Run(app, …)`
    func main() { Run(newApp(), Title("x")) }
                      ^
```

## What the interpreted run cannot run the way the compiler does

gc knows `min` and `max`; the interpreter, whose Go is 1.22's, does not, so the app would run one way and not the other.

```console
test/refuse/min_max.go:9:32: Gomamochi cannot take this — `min` is Go 1.21's, and the interpreted run does not know it. Write the comparison out (`if a < b { … }`), or a small function of your own
    func (a *Demo) clamp() { a.n = min(a.n, 10) }
                                   ^
```

`range` over a number is Go 1.22's; the interpreter stops on it rather than answering something else, so it is refused before it runs.

```console
test/refuse/range_number.go:10:17: Gomamochi cannot take this — `range` over a number is Go 1.22's, and the interpreted run stops on it. Write `for i := 0; i < n; i++`
    	for i := range 3 {
    	               ^
```

The same shape with a variable rather than a literal, which only the type checker can see; it is refused when `go` is on the path.

```console
test/refuse/range_count.go:9:17: Gomamochi cannot take this — `range` over a number is Go 1.22's, and the interpreted run stops on it. Write `for i := 0; i < n; i++`
    	for i := range a.n {
    	               ^
```

`range` over a function is Go 1.23's, and the interpreter stops on it too.

```console
test/refuse/range_function.go:13:17: Gomamochi cannot take this — `range` over a function is Go 1.23's, and the interpreted run stops on it. Call the function and range over what it answers
    	for i := range pair {
    	               ^
```

`%T` prints the interpreter's name for an app's type, not Go's, so the two runs would print different text.

```console
test/refuse/percent_t.go:11:57: Gomamochi cannot take this — `%T` names the app's own types differently in the interpreted run. Print the value (`%v`), or give the type a `String` method
    func (a *Demo) View() Element { return Text(fmt.Sprintf("%T", a)) }
                                                            ^
```

`reflect` sees the same difference: the interpreter names the app's own types differently from the compiled run.

```console
test/refuse/import_reflect.go:4:2: Gomamochi cannot take this — `reflect`: the interpreted run names the app's own types differently from the compiled one, so what it answers would differ. Write the check as a type switch or a method
    	"reflect"
    	^
```

The interpreted run does not take `unsafe`.

```console
test/refuse/import_unsafe.go:4:2: Gomamochi cannot take this — `unsafe`: the interpreted run does not take it. Write the same thing in plain Go
    	"unsafe"
    	^
```

The interpreted run cannot call C, and the compiled run is built without cgo.

```console
test/refuse/import_cgo.go:3:8: Gomamochi cannot take this — cgo: the interpreted run cannot call C, and the compiled run is built without it. Reach the engine through the package, and anything else through Go
    import "C"
           ^
```

The interpreted run reads the file as source, and has nothing to embed.

```console
test/refuse/import_embed.go:4:2: Gomamochi cannot take this — `//go:embed`: the interpreted run cannot embed a file. Read it with `os.ReadFile` in a handler, or name it as an element's source
    	_ "embed"
    	^
```

The interpreted run cannot read a module outside the standard library yet; the standard library and this package are what an app imports.

```console
test/refuse/import_module.go:4:2: Gomamochi cannot take this — `github.com/example/charts` is a module outside the standard library, and the interpreted run cannot read it yet. The standard library and this package are what an app imports
    	"github.com/example/charts"
    	^
```

The interpreted run gets a per-iteration copy of a captured loop variable; a body that writes the variable would be writing something the copy hides.

```console
test/refuse/loop_variable_written.go:11:4: Gomamochi cannot take this — the loop's own variable is written inside its body and a closure captures it: the two runs would disagree about which iteration the closure sees. Copy it first (`j := i`) and let the closure use the copy
    			i = 2
    			^
```

## Views

Building a screen twice has to build the same screen, so building it only reads.

```console
test/refuse/view_write.go:8:2: Gomamochi cannot take this — a view only reads. Move the write into a handler — the closure on a button, or a method the app calls from one
    	a.n += 1
    	^
```

A view is built again whenever anything changes, and each build would start the work again; a handler starts it once, with `Task`.

```console
test/refuse/view_goroutine.go:8:2: Gomamochi cannot take this — a view starts a goroutine, and a view is built again from the same state whenever anything changes. Start the work from a handler with `Task`
    	go func() { a.n++ }()
    	^
```

The clock answers differently from one build to the next, and the two runs would draw different screens; a timer reads it and keeps the answer on the app.

```console
test/refuse/view_clock.go:11:45: Gomamochi cannot take this — `time.Now` reads the clock, and a view may only read the app: it is built again from the same state whenever anything changes. Read the clock in a timer and keep the answer on the app
    func (a *Demo) View() Element { return Text(time.Now().Format("15:04")) }
                                                ^
```

The environment, a file, a stream, the network and a random number are read in a handler, and what they answered is kept on the app.

```console
test/refuse/view_environment.go:11:45: Gomamochi cannot take this — `os.Getenv` reads the environment or a file, and a view may only read the app: it is built again from the same state whenever anything changes. Read it in a handler and keep the answer on the app
    func (a *Demo) View() Element { return Text(os.Getenv("HOME")) }
                                                ^
```

The keyboard is a device, read from a timer and never from a view.

```console
test/refuse/view_keyboard.go:8:5: Gomamochi cannot take this — `KeyDown` reads the keyboard, and a view is built again from the same state whenever anything changes. Call it from a handler or a timer, and keep what it answers on the app
    	if KeyDown("space") {
    	   ^
```

A map walks in a different order every run, and both runs would draw different screens; a handler may range over one to collect and sort its keys.

```console
test/refuse/range_map.go:9:20: Gomamochi cannot take this — `range` over a map walks it in a different order every run, and a view must draw the same screen from the same state. Keep a sorted list of the keys on the app, made in a handler, and range over that
    	for name := range a.prices {
    	                  ^
```

## Go's own verdict

When nothing here has anything to say, Go's own type checker speaks, in Go's own words.

```console
test/refuse/undefined_name.go:7:45: undefined: label
```
