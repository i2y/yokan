// Package door is pixie's C face opened from Go: purego over the
// cdylib, no cgo, and the same library in both of an app's runs — which
// is what lets a gate over the two runs mean anything. This file finds
// the library and declares its calls; what the door holds between
// builds, and how the engine reaches back into the app, is in
// runtime.go.
//
// Everything crosses as numbers and NUL-terminated strings. The engine
// copies a string on entry, holds no Go value at all, and hands back
// the numbers it was given; a Go closure never crosses, only the index
// the door filed it under.
package door

import (
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"sync"

	"github.com/ebitengine/purego"
)

func init() {
	// gpui wants the window on the main thread. Locking at init is how
	// a Go program keeps its main goroutine there (ebitengine's rule);
	// Run is then called from main and stays put.
	runtime.LockOSThread()
}

// Library is what cargo names the C face on this platform.
func Library() string {
	if runtime.GOOS == "darwin" {
		return "libpixie_capi.dylib"
	}
	return "libpixie_capi.so"
}

// Where the library is: PIXIE_CAPI wins, then a copy beside the
// executable (what a shipped app carries), then the shared cargo target
// dir (what a checkout builds into).
func libraryPath() string {
	if p := os.Getenv("PIXIE_CAPI"); p != "" {
		return p
	}
	if exe, err := os.Executable(); err == nil {
		beside := filepath.Join(filepath.Dir(exe), Library())
		if _, err := os.Stat(beside); err == nil {
			return beside
		}
	}
	target := os.Getenv("CARGO_TARGET_DIR")
	if target == "" {
		home, _ := os.UserHomeDir()
		target = filepath.Join(home, ".cache", "pixie", "target")
	}
	return filepath.Join(target, "release", Library())
}

var (
	once    sync.Once
	openErr error

	pixieEl                func(kind int32) int64
	pixieStr               func(el int64, key int32, v string)
	pixieNum               func(el int64, key int32, v float64)
	pixieInt               func(el int64, key int32, v int64)
	pixieBool              func(el int64, key int32, v int32)
	pixiePushStr           func(el int64, key int32, v string)
	pixiePushNum           func(el int64, key int32, v float64)
	pixieListBreak         func(el int64, key int32)
	pixieOpPixel           func(el, x, y, color int64)
	pixieOpLine            func(el, x1, y1, x2, y2, color int64)
	pixieOpRect            func(el, x, y, w, h, color int64)
	pixieOpRectOutline     func(el, x, y, w, h, color int64)
	pixieOpCircle          func(el, x, y, r, color int64)
	pixieOpCircleOutline   func(el, x, y, r, color int64)
	pixieOpTriangle        func(el, x1, y1, x2, y2, x3, y3, color int64)
	pixieOpTriangleOutline func(el, x1, y1, x2, y2, x3, y3, color int64)
	pixieOpSprite          func(el, x, y int64, path string, sx, sy, sw, sh, scale int64, flipX, flipY int32)
	pixieOpPixelText       func(el, x, y int64, text string, color int64)
	pixieQuit              func()
	pixieKeyDown           func(name string) int32
	pixieKeyPressed        func(name string) int32
	pixieKeyReleased       func(name string) int32
	pixieOn                func(el int64, key int32, handler int64)
	pixieRows              func(el int64, key int32, handler int64)
	pixieChildren          func(el int64, ids *int64, n uintptr)
	pixieEnd               func(el int64) int64
	pixieEventInt          func() int64
	pixieEventNum          func() float64
	pixieEventTextLength   func() int64
	pixieEventTextChar     func(i int64) int64
	pixieSetEventHandler   func(f uintptr)
	pixieSetRowBuilder     func(f uintptr)
	pixieSetTimerHandler   func(f uintptr)
	pixieWatch             func(path string, f uintptr)
	pixieSetTaskHandler    func(f uintptr)
	pixieTask              func() int64
	pixieTaskDone          func(id int64)
	pixieSetPumpHandler    func(f uintptr)
	pixieSetBindingHandler func(f uintptr)
	pixieShortcut          func(chord string, handler int64)
	pixieOnKey             func(handler int64)
	pixieMenuItem          func(menu, item string, handler int64)
	pixieOnFileDrop        func(handler int64)
	pixieAnswerLength      func() int64
	pixieAnswerChar        func(i int64) int64
	pixieClipboardSet      func(text string)
	pixieClipboardGet      func()
	pixieDialog            func(save int32, label string)
	pixieAudioPlay         func(path string, volume float64) int64
	pixieAudioStop         func() int64
	pixieSqliteBind        func(value string)
	pixieSqliteExec        func(path, sql string) int64
	pixieSqliteQuery       func(path, sql string) int64
	pixieSqliteColumns     func() int64
	pixieSqliteCell        func(row, col int64)
	pixieEvery             func(seconds float64, handler int64)
	pixieRun               func(title string, w, h, pad float64, build uintptr) int32
	pixieStdReset          func()
	pixieStdArgStr         func(v string)
	pixieStdArgInt         func(v int64)
	pixieStdArgNum         func(v float64)
	pixieStdArgListBegin   func()
	pixieStdArgListEnd     func()
	pixieStdRows           func() int64
	pixieStdCells          func(row int64) int64
	pixieStdPick           func(row, col int64)
	pixieStdAnswerNum      func() float64
	pixieStdCallChecked    func(id int32) int64
	pixieStdFailed         func() int32
)

// ensure opens the library the first time anything needs it. Lazily,
// so that a command that only reads an app's file never asks for an
// engine.
func ensure() {
	once.Do(open)
	if openErr != nil {
		fmt.Fprintln(os.Stderr, "gomamochi:", openErr)
		os.Exit(1)
	}
}

func open() {
	path := libraryPath()
	lib, err := purego.Dlopen(path, purego.RTLD_NOW|purego.RTLD_GLOBAL)
	if err != nil {
		openErr = fmt.Errorf("the engine did not open: %s: %v", path, err)
		return
	}
	reg := func(fptr any, name string) { purego.RegisterLibFunc(fptr, lib, name) }
	reg(&pixieEl, "pixie_el")
	reg(&pixieStr, "pixie_str")
	reg(&pixieNum, "pixie_num")
	reg(&pixieInt, "pixie_int")
	reg(&pixieBool, "pixie_bool")
	reg(&pixiePushStr, "pixie_push_str")
	reg(&pixiePushNum, "pixie_push_num")
	reg(&pixieListBreak, "pixie_list_break")
	reg(&pixieOpPixel, "pixie_op_pixel")
	reg(&pixieOpLine, "pixie_op_line")
	reg(&pixieOpRect, "pixie_op_rect")
	reg(&pixieOpRectOutline, "pixie_op_rect_outline")
	reg(&pixieOpCircle, "pixie_op_circle")
	reg(&pixieOpCircleOutline, "pixie_op_circle_outline")
	reg(&pixieOpTriangle, "pixie_op_triangle")
	reg(&pixieOpTriangleOutline, "pixie_op_triangle_outline")
	reg(&pixieOpSprite, "pixie_op_sprite")
	reg(&pixieOpPixelText, "pixie_op_pixel_text")
	reg(&pixieQuit, "pixie_quit")
	reg(&pixieKeyDown, "pixie_key_down")
	reg(&pixieKeyPressed, "pixie_key_pressed")
	reg(&pixieKeyReleased, "pixie_key_released")
	reg(&pixieOn, "pixie_on")
	reg(&pixieRows, "pixie_rows")
	reg(&pixieChildren, "pixie_children")
	reg(&pixieEnd, "pixie_end")
	reg(&pixieEventInt, "pixie_event_int")
	reg(&pixieEventNum, "pixie_event_num")
	reg(&pixieEventTextLength, "pixie_event_text_length")
	reg(&pixieEventTextChar, "pixie_event_text_char")
	reg(&pixieSetEventHandler, "pixie_set_event_handler")
	reg(&pixieSetRowBuilder, "pixie_set_row_builder")
	reg(&pixieSetTimerHandler, "pixie_set_timer_handler")
	reg(&pixieWatch, "pixie_watch")
	reg(&pixieSetTaskHandler, "pixie_set_task_handler")
	reg(&pixieTask, "pixie_task")
	reg(&pixieTaskDone, "pixie_task_done")
	reg(&pixieSetPumpHandler, "pixie_set_pump_handler")
	reg(&pixieSetBindingHandler, "pixie_set_binding_handler")
	reg(&pixieShortcut, "pixie_shortcut")
	reg(&pixieOnKey, "pixie_on_key")
	reg(&pixieMenuItem, "pixie_menu_item")
	reg(&pixieOnFileDrop, "pixie_on_file_drop")
	reg(&pixieAnswerLength, "pixie_answer_length")
	reg(&pixieAnswerChar, "pixie_answer_char")
	reg(&pixieClipboardSet, "pixie_clipboard_set")
	reg(&pixieClipboardGet, "pixie_clipboard_get")
	reg(&pixieDialog, "pixie_dialog")
	reg(&pixieAudioPlay, "pixie_audio_play")
	reg(&pixieAudioStop, "pixie_audio_stop")
	reg(&pixieSqliteBind, "pixie_sqlite_bind")
	reg(&pixieSqliteExec, "pixie_sqlite_exec")
	reg(&pixieSqliteQuery, "pixie_sqlite_query")
	reg(&pixieSqliteColumns, "pixie_sqlite_columns")
	reg(&pixieSqliteCell, "pixie_sqlite_cell")
	reg(&pixieEvery, "pixie_every")
	reg(&pixieRun, "pixie_run")
	reg(&pixieStdReset, "pixie_std_reset")
	reg(&pixieStdArgStr, "pixie_std_arg_str")
	reg(&pixieStdArgInt, "pixie_std_arg_int")
	reg(&pixieStdArgNum, "pixie_std_arg_num")
	reg(&pixieStdArgListBegin, "pixie_std_arg_list_begin")
	reg(&pixieStdArgListEnd, "pixie_std_arg_list_end")
	reg(&pixieStdRows, "pixie_std_rows")
	reg(&pixieStdCells, "pixie_std_cells")
	reg(&pixieStdPick, "pixie_std_pick")
	reg(&pixieStdAnswerNum, "pixie_std_answer_num")
	reg(&pixieStdCallChecked, "pixie_std_call_checked")
	reg(&pixieStdFailed, "pixie_std_failed")
}

// --- writing an element -----------------------------------------------------

// El opens an element of `kind`; the calls below write into it, and End
// closes it and answers its handle for this build.
func El(kind int32) int64                    { ensure(); return pixieEl(kind) }
func Str(el int64, key int32, v string)      { pixieStr(el, key, v) }
func Num(el int64, key int32, v float64)     { pixieNum(el, key, v) }
func Int(el int64, key int32, v int64)       { pixieInt(el, key, v) }
func Bool(el int64, key int32, v bool)       { pixieBool(el, key, b2i(v)) }
func PushStr(el int64, key int32, v string)  { pixiePushStr(el, key, v) }
func PushNum(el int64, key int32, v float64) { pixiePushNum(el, key, v) }
func ListBreak(el int64, key int32)          { pixieListBreak(el, key) }
func End(el int64) int64                     { return pixieEnd(el) }

// Children hands a container the handles of its children, in order.
func Children(el int64, ids []int64) {
	if len(ids) == 0 {
		return
	}
	pixieChildren(el, &ids[0], uintptr(len(ids)))
}

// The drawing commands of a canvas.
func OpPixel(el, x, y, color int64)             { pixieOpPixel(el, x, y, color) }
func OpLine(el, x1, y1, x2, y2, color int64)    { pixieOpLine(el, x1, y1, x2, y2, color) }
func OpRect(el, x, y, w, h, color int64)        { pixieOpRect(el, x, y, w, h, color) }
func OpRectOutline(el, x, y, w, h, color int64) { pixieOpRectOutline(el, x, y, w, h, color) }
func OpCircle(el, x, y, r, color int64)         { pixieOpCircle(el, x, y, r, color) }
func OpCircleOutline(el, x, y, r, color int64)  { pixieOpCircleOutline(el, x, y, r, color) }
func OpTriangle(el, x1, y1, x2, y2, x3, y3, color int64) {
	pixieOpTriangle(el, x1, y1, x2, y2, x3, y3, color)
}
func OpTriangleOutline(el, x1, y1, x2, y2, x3, y3, color int64) {
	pixieOpTriangleOutline(el, x1, y1, x2, y2, x3, y3, color)
}
func OpSprite(el, x, y int64, path string, sx, sy, sw, sh, scale int64, flipX, flipY bool) {
	pixieOpSprite(el, x, y, path, sx, sy, sw, sh, scale, b2i(flipX), b2i(flipY))
}
func OpPixelText(el, x, y int64, text string, color int64) { pixieOpPixelText(el, x, y, text, color) }

// --- what an event carried, and what the engine answered --------------------

// EventInt, EventNum and EventText are what the event now being
// delivered carried. Text comes a character at a time: a string cannot
// cross as a callback argument, so it is fetched instead.
func EventInt() int64   { return pixieEventInt() }
func EventNum() float64 { return pixieEventNum() }
func EventText() string {
	n := pixieEventTextLength()
	r := make([]rune, n)
	for i := int64(0); i < n; i++ {
		r[i] = rune(pixieEventTextChar(i))
	}
	return string(r)
}

// Answer is the last string the engine was asked for: the clipboard,
// or the path a person chose in a dialog.
func Answer() string {
	n := pixieAnswerLength()
	r := make([]rune, n)
	for i := int64(0); i < n; i++ {
		r[i] = rune(pixieAnswerChar(i))
	}
	return string(r)
}

// --- the rest of the face ---------------------------------------------------

func Quit()                        { ensure(); pixieQuit() }
func KeyDown(name string) bool     { ensure(); return pixieKeyDown(name) != 0 }
func KeyPressed(name string) bool  { ensure(); return pixieKeyPressed(name) != 0 }
func KeyReleased(name string) bool { ensure(); return pixieKeyReleased(name) != 0 }
func ClipboardSet(text string)     { ensure(); pixieClipboardSet(text) }
func ClipboardGet() string         { ensure(); pixieClipboardGet(); return Answer() }
func Dialog(save bool, label string) string {
	ensure()
	pixieDialog(b2i(save), label)
	return Answer()
}
func AudioPlay(path string, volume float64) int64 { ensure(); return pixieAudioPlay(path, volume) }
func AudioStop() int64                            { ensure(); return pixieAudioStop() }

func SqliteExec(path, sql string, params []string) int64 {
	ensure()
	for _, p := range params {
		pixieSqliteBind(p)
	}
	return pixieSqliteExec(path, sql)
}

func SqliteRows(path, sql string, params []string) [][]string {
	ensure()
	for _, p := range params {
		pixieSqliteBind(p)
	}
	count := pixieSqliteQuery(path, sql)
	width := pixieSqliteColumns()
	rows := make([][]string, count)
	for r := int64(0); r < count; r++ {
		cells := make([]string, width)
		for c := int64(0); c < width; c++ {
			pixieSqliteCell(r, c)
			cells[c] = Answer()
		}
		rows[r] = cells
	}
	return rows
}

func b2i(v bool) int32 {
	if v {
		return 1
	}
	return 0
}

// --- the framework's own standard library ----------------------------------------

// The generic call the manifest's rows go through: the arguments are
// pushed, the row is named by number, and the answer is read back — a
// number from the call itself, text a cell at a time.
// Std runs one such call on one thread from the first piece pushed to
// the answer read. The face keeps the pieces in that thread's own
// state, and a goroutine is free to move between threads unless told
// not to — so any goroutine may call the library, a Task's or the
// app's own, and two of them at once keep out of each other's way.
func Std(call func()) {
	ensure()
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	call()
}

func StdReset()           { pixieStdReset() }
func StdArgStr(v string)  { pixieStdArgStr(v) }
func StdArgInt(v int64)   { pixieStdArgInt(v) }
func StdArgNum(v float64) { pixieStdArgNum(v) }
func StdArgList(vs []string) {
	pixieStdArgListBegin()
	for _, v := range vs {
		pixieStdArgStr(v)
	}
	pixieStdArgListEnd()
}

// StdCall runs the row. A row that fails is handed back by the face
// rather than ending the process from inside it, and is raised here as
// a Go panic whose value is the library's message: a handler's guard
// says it and stops the app, as the library's own plain form would,
// and a recover in the app receives it instead.
func StdCall(id int32) int64 {
	n := pixieStdCallChecked(id)
	if pixieStdFailed() != 0 {
		pixieStdPick(0, 0)
		panic(Answer())
	}
	return n
}
func StdAnswerNum() float64 { return pixieStdAnswerNum() }

// StdText is the one cell a text answer is.
func StdText() string {
	pixieStdPick(0, 0)
	return Answer()
}

// StdList is a list answer: one row per element.
func StdList() []string {
	n := pixieStdRows()
	out := make([]string, n)
	for i := int64(0); i < n; i++ {
		pixieStdPick(i, 0)
		out[i] = Answer()
	}
	return out
}

// StdRows is a query's rows.
func StdRows() [][]string {
	n := pixieStdRows()
	out := make([][]string, n)
	for r := int64(0); r < n; r++ {
		w := pixieStdCells(r)
		row := make([]string, w)
		for c := int64(0); c < w; c++ {
			pixieStdPick(r, c)
			row[c] = Answer()
		}
		out[r] = row
	}
	return out
}
