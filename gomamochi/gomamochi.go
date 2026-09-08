// Package gomamochi builds desktop apps in Go on pixie's engine. An app
// is a struct with a View method; its state is the struct's fields, and
// a handler is a closure over them. `gomamochi run` interprets the file
// while it is being written, `gomamochi build` compiles it with gc, and
// `gomamochi gate` drives both with one script and compares the two
// byte for byte.
//
// Write an app with a dot import, so the elements read as they do in
// the other languages on the engine:
//
//	import . "github.com/i2y/yokan/gomamochi"
//
//	type Counter struct{ count int }
//
//	func (c *Counter) View() Element {
//		return Column(
//			Text(fmt.Sprint("count: ", c.count)).Size(34),
//			Button("+1").OnClick(func() { c.count++ }),
//		).Spacing(12).Padding(16)
//	}
//
//	func main() { Run(&Counter{}, Title("counter")) }
package gomamochi

import "github.com/i2y/yokan/gomamochi/internal/door"

// Element is one node of the tree a View answers. Elements are values;
// the engine is written to when it asks for the tree, so a tree can be
// built and inspected without a window.
type Element interface{ emit() int64 }

// App is what Run is handed: anything with a View.
type App interface{ View() Element }

// Option is a keyword of Run.
type Option func(*door.Config)

// Title is the window's title.
func Title(s string) Option { return func(c *door.Config) { c.Title = s } }

// Size is the window's initial width and height, in points.
func Size(w, h float64) Option {
	return func(c *door.Config) { c.Width, c.Height = w, h }
}

// Padding is the space between the window's edge and the tree.
func Padding(p float64) Option { return func(c *door.Config) { c.Padding = p } }

// Run opens the window and hands it the app. Under PIXIE_SCRIPT there is
// no window: the engine builds the tree, prints it, replays the script
// and returns, which is what the gate compares.
func Run(app App, opts ...Option) {
	c := door.Config{
		Title:   "gomamochi",
		Padding: -1,
		App:     app,
		Build:   func() int64 { return app.View().emit() },
	}
	for _, o := range opts {
		o(&c)
	}
	door.Run(c)
}

// Task runs `work` off the window's thread. When it is done, `done` is
// called on the window's thread with what the work answered. Nothing
// inside the work may touch the app's fields or the screen; the handler
// is where that belongs.
func Task(work func() any, done func(any)) { door.Task(work, done) }

// Every asks to be told every `seconds`, before Run. Both runs tick off
// the same clock: a frame in a window, an `advance:` in a script.
func Every(seconds float64, tick func()) { door.Every(seconds, tick) }

// Shortcut runs f on a chord, spelled the way the platform spells it
// ("cmd+s"). Declared before Run.
func Shortcut(chord string, f func()) { door.Shortcut(chord, f) }

// MenuItem is one item in the application's menu bar; declaration order
// is menu order.
func MenuItem(menu, item string, f func()) { door.MenuItem(menu, item, f) }

// OnKey is told every key, as the chord it was.
func OnKey(f func(chord string)) { door.OnKey(func() { f(door.EventText()) }) }

// OnFileDrop is told the path of a file dragged onto the window.
func OnFileDrop(f func(path string)) { door.OnFileDrop(func() { f(door.EventText()) }) }

// KeyDown, KeyPressed and KeyReleased are what the hands are doing, for
// an app that draws frames: the key alone ("left", "space"), read in a
// timer, never in a view.
func KeyDown(name string) bool     { return door.KeyDown(name) }
func KeyPressed(name string) bool  { return door.KeyPressed(name) }
func KeyReleased(name string) bool { return door.KeyReleased(name) }

// Quit closes the window on the engine's next frame. A headless run
// never takes it, so a script runs to its end.
func Quit() { door.Quit() }

