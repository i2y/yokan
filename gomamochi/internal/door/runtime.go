package door

import (
	"fmt"
	"os"
	"reflect"
	"runtime"
	"sync"
	"unsafe"

	"github.com/ebitengine/purego"
)

// What the door holds between builds. There is one registry of work to
// do, filled as the tree is written and started over at every build, so
// a handler number means something only inside the build that handed
// it out. Every entry is a closure of no arguments: the one the app
// wrote, wrapped in one that fetches what the event carried and passes
// it on. A Go closure knows its own type, so one registry serves every
// kind of handler.
var (
	handlers []func()
	rows     []func(int64) int64
	// Declared before the app runs, and living as long as it does.
	timers   []func()
	bindings []func()
	// Work the app started, and what it answered. The lock is the one
	// place a worker and the window's thread meet.
	taskMu  sync.Mutex
	answers = map[int64]any{}
	dones   = map[int64]func(any){}

	// The app the window is showing, how to build its tree, and
	// whether the window is already up: a reload re-reads the whole
	// file, and the second Run it reaches must not open a second
	// window or throw away the values a person has been building up.
	app     any
	build   func() int64
	running bool
	// The next Run is a re-read of the app's file: its app is the new
	// shape, and the old one's values are carried into it. While the
	// re-read declares its timers and bindings again, they take the
	// places of the ones already installed, in order.
	reloading          bool
	reTimer, reBinding int

	watchPath string
	reloadFn  func() bool
)

// Config is what Run is handed: the window, the app, and how to build
// its tree.
type Config struct {
	Title                  string
	Width, Height, Padding float64
	App                    any
	Build                  func() int64
}

// The callbacks the engine reaches the app through. Made once; each is
// a C function pointer for the life of the process.
var (
	cbBuild = purego.NewCallback(func() int64 {
		defer guard()
		handlers = handlers[:0]
		rows = rows[:0]
		return build()
	})
	cbEvent = purego.NewCallback(func(id, kind int64) {
		defer guard()
		if int(id) < len(handlers) {
			handlers[id]()
		}
	})
	cbRow = purego.NewCallback(func(h, i int64) int64 {
		defer guard()
		if int(h) < len(rows) {
			return rows[h](i)
		}
		return 0
	})
	cbTimer = purego.NewCallback(func(id int64) {
		defer guard()
		if int(id) < len(timers) {
			timers[id]()
		}
	})
	// The engine says a piece of work is finished, on the window's
	// thread; the app's closure is called with what the work answered.
	cbTask = purego.NewCallback(func(id int64) {
		defer guard()
		taskMu.Lock()
		done, answer := dones[id], answers[id]
		delete(dones, id)
		delete(answers, id)
		taskMu.Unlock()
		if done != nil {
			done(answer)
		}
	})
	// The engine is waiting for work and hands the app a turn. Go's
	// scheduler runs the app's goroutines on threads of its own, so
	// the turn is a courtesy here rather than a necessity.
	cbPump    = purego.NewCallback(func() { runtime.Gosched() })
	cbBinding = purego.NewCallback(func(id int64) {
		defer guard()
		if int(id) < len(bindings) {
			bindings[id]()
		}
	})
	// The app's file changed. The host reads it again; if that took,
	// the engine rebuilds the view.
	cbReload = purego.NewCallback(func() int32 {
		defer guard()
		if reloadFn == nil {
			return 0
		}
		reloading, reTimer, reBinding = true, 0, 0
		ok := reloadFn()
		reloading = false
		if ok {
			return 1
		}
		return 0
	})
)

// A panic inside a callback cannot unwind through the engine's frames,
// so it is said here and the process stops where it stands — which is
// what a refusal does on the other side of the face too.
func guard() {
	if e := recover(); e != nil {
		fmt.Fprintf(os.Stderr, "gomamochi: %v\n", e)
		os.Exit(1)
	}
}

// Run opens the window and hands it the app. Under PIXIE_SCRIPT there
// is no window: the engine builds the tree, prints it, replays the
// script and returns.
func Run(c Config) {
	if running {
		if reloading {
			carry(c.App, app)
			app, build = c.App, c.Build
		}
		return
	}
	ensure()
	running = true
	app, build = c.App, c.Build
	pixieSetEventHandler(cbEvent)
	pixieSetRowBuilder(cbRow)
	pixieSetTimerHandler(cbTimer)
	pixieSetTaskHandler(cbTask)
	pixieSetPumpHandler(cbPump)
	pixieSetBindingHandler(cbBinding)
	// Only an interpreted run watches: a compiled app is what it is.
	if watchPath != "" {
		pixieWatch(watchPath, cbReload)
	}
	pixieRun(c.Title, c.Width, c.Height, c.Padding, cbBuild)
}

// Watch asks Run to have the engine watch the app's file, and to call
// `reload` on the window's thread when it changes. `reload` reads the
// file again and answers whether that took; the Run it reaches then
// carries the app's values into the new shape.
func Watch(path string, reload func() bool) {
	watchPath, reloadFn = path, reload
}

// Handler files a closure under the next number and tells the engine
// that number for this key of this element.
func Handler(el int64, key int32, f func()) {
	handlers = append(handlers, f)
	pixieOn(el, key, int64(len(handlers)-1))
}

// RowBuilder is the same for a list that builds its rows on demand:
// the engine asks for row i and is answered with that row's handle.
func RowBuilder(el int64, key int32, f func(int64) int64) {
	rows = append(rows, f)
	pixieRows(el, key, int64(len(rows)-1))
}

// Every asks to be told every `seconds`. Declared before Run; both runs
// tick off the same clock, a frame in a window and an `advance:` in a
// script.
func Every(seconds float64, f func()) {
	if running {
		if reloading && reTimer < len(timers) {
			timers[reTimer] = f
		}
		reTimer++
		return
	}
	ensure()
	timers = append(timers, f)
	pixieEvery(seconds, int64(len(timers)-1))
}

// A shortcut, a menu item, a key or a dropped file: declared before the
// app runs, outliving every build, numbered in a list of their own.
func bind(f func()) (int64, bool) {
	if running {
		if reloading && reBinding < len(bindings) {
			bindings[reBinding] = f
		}
		reBinding++
		return 0, false
	}
	ensure()
	bindings = append(bindings, f)
	return int64(len(bindings) - 1), true
}

func Shortcut(chord string, f func()) {
	if id, fresh := bind(f); fresh {
		pixieShortcut(chord, id)
	}
}

func MenuItem(menu, item string, f func()) {
	if id, fresh := bind(f); fresh {
		pixieMenuItem(menu, item, id)
	}
}

func OnKey(f func()) {
	if id, fresh := bind(f); fresh {
		pixieOnKey(id)
	}
}

func OnFileDrop(f func()) {
	if id, fresh := bind(f); fresh {
		pixieOnFileDrop(id)
	}
}

// Task runs `work` on a goroutine of its own. When it is done, `done`
// is called on the window's thread with what the work answered.
// Nothing inside the work may touch the app or the screen: the answer
// comes back this way so that the handler is where the writing
// happens.
func Task(work func() any, done func(any)) {
	ensure()
	id := pixieTask()
	taskMu.Lock()
	dones[id] = done
	taskMu.Unlock()
	go func() {
		// A failing library call panics; off the window's thread the
		// same guard says it, or the runtime would print a trace.
		defer guard()
		v := work()
		taskMu.Lock()
		answers[id] = v
		taskMu.Unlock()
		pixieTaskDone(id)
	}()
}

// carry copies every field of the app the window had into the app the
// re-read file made, where the new one has a field of the same name and
// type; a field that changed its type starts over. The apps arrive as
// whatever the door was handed: a pointer to a struct, or an
// interpreter's wrapper around one.
func carry(newApp, oldApp any) {
	n := unwrap(reflect.ValueOf(newApp))
	o := unwrap(reflect.ValueOf(oldApp))
	if n.Kind() != reflect.Struct || o.Kind() != reflect.Struct {
		return
	}
	for i := 0; i < n.NumField(); i++ {
		f := n.Type().Field(i)
		of := o.FieldByName(f.Name)
		if !of.IsValid() || of.Type() != f.Type || !n.Field(i).CanAddr() || !of.CanAddr() {
			continue
		}
		dst := reflect.NewAt(f.Type, unsafe.Pointer(n.Field(i).UnsafeAddr())).Elem()
		src := reflect.NewAt(f.Type, unsafe.Pointer(of.UnsafeAddr())).Elem()
		dst.Set(src)
	}
}

func unwrap(v reflect.Value) reflect.Value {
	for {
		switch v.Kind() {
		case reflect.Interface, reflect.Pointer:
			if v.IsNil() {
				return v
			}
			v = v.Elem()
			continue
		case reflect.Struct:
			if v.NumField() >= 1 && v.Type().Field(0).Name == "IValue" {
				v = v.Field(0)
				continue
			}
		}
		return v
	}
}
