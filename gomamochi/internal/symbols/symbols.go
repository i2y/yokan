// Package symbols is the interpreter's view of the gomamochi package:
// every exported name as a reflect value, in the table yaegi's `extract`
// writes, plus a wrapper for the one interface an interpreted type
// implements. The generator writes this file from the table in phase 1;
// until then it is kept by hand beside elements.go.
package symbols

import (
	"reflect"

	gm "github.com/i2y/yokan/gomamochi"
)

// Symbols is what the host hands to interp.Use.
var Symbols = map[string]map[string]reflect.Value{}

func init() {
	Symbols["github.com/i2y/yokan/gomamochi/gomamochi"] = map[string]reflect.Value{
		// the app and the window
		"App":     reflect.ValueOf((*gm.App)(nil)),
		"_App":    reflect.ValueOf((*_gomamochi_App)(nil)),
		"Element": reflect.ValueOf((*gm.Element)(nil)),
		"Option":  reflect.ValueOf((*gm.Option)(nil)),
		"Run":     reflect.ValueOf(gm.Run),
		"Title":   reflect.ValueOf(gm.Title),
		"Size":    reflect.ValueOf(gm.Size),
		"Padding": reflect.ValueOf(gm.Padding),
		"Quit":    reflect.ValueOf(gm.Quit),
		// work, time and the keyboard
		"Task":        reflect.ValueOf(gm.Task),
		"Every":       reflect.ValueOf(gm.Every),
		"Shortcut":    reflect.ValueOf(gm.Shortcut),
		"MenuItem":    reflect.ValueOf(gm.MenuItem),
		"OnKey":       reflect.ValueOf(gm.OnKey),
		"OnFileDrop":  reflect.ValueOf(gm.OnFileDrop),
		"KeyDown":     reflect.ValueOf(gm.KeyDown),
		"KeyPressed":  reflect.ValueOf(gm.KeyPressed),
		"KeyReleased": reflect.ValueOf(gm.KeyReleased),
		// the elements
		"Text":        reflect.ValueOf(gm.Text),
		"Button":      reflect.ValueOf(gm.Button),
		"TextField":   reflect.ValueOf(gm.TextField),
		"Column":      reflect.ValueOf(gm.Column),
		"Row":         reflect.ValueOf(gm.Row),
		"ListView":    reflect.ValueOf(gm.ListView),
		"TextEl":      reflect.ValueOf((*gm.TextEl)(nil)),
		"ButtonEl":    reflect.ValueOf((*gm.ButtonEl)(nil)),
		"TextFieldEl": reflect.ValueOf((*gm.TextFieldEl)(nil)),
		"BoxEl":       reflect.ValueOf((*gm.BoxEl)(nil)),
		"ListViewEl":  reflect.ValueOf((*gm.ListViewEl)(nil)),
	}
}

// _gomamochi_App is an interface wrapper for App: an interpreted type
// with a View reaches the compiled Run through it.
type _gomamochi_App struct {
	IValue interface{}
	WView  func() gm.Element
}

func (W _gomamochi_App) View() gm.Element { return W.WView() }
