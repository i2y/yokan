<!-- Written by website/tools/tourpages from the tour. Edit the tour. -->
# Go, data, and work

Go's own library, the framework's, timers, work off the window's thread, and the window that follows your saves.

## Go's own standard library

`fmt`, `strings`, `strconv`, `sort`, `math`, `time`, `encoding/json`,
`encoding/csv`, `regexp`, `os`, `net/http`: the language's own, and
both runs call the same compiled packages. The interpreter does not
reimplement them — it calls the ones compiled into the command — so
what `fmt.Sprintf("%.2f", x)` answers is the same bytes in both runs,
and the gate never has to compare two implementations of a library.

<!-- script: click:stats,click:parse,click:scan,dump -->
```go
package main

import (
	"encoding/json"
	"fmt"
	"regexp"
	"sort"
	"strconv"

	. "github.com/i2y/yokan/gomamochi"
)

type Stdlib struct {
	scores []int
	spread string
	doc    string
	sum    int
}

func (s *Stdlib) stats() {
	sorted := append([]int{}, s.scores...)
	sort.Ints(sorted)
	s.spread = fmt.Sprintf("median %d min %d max %d", sorted[len(sorted)/2], sorted[0], sorted[len(sorted)-1])
}

func (s *Stdlib) parse() {
	var doc map[string]any
	json.Unmarshal([]byte(`{"name": "gomamochi", "ok": true}`), &doc)
	s.doc = fmt.Sprintf("%v %v", doc["name"], doc["ok"])
}

func (s *Stdlib) scan() {
	s.sum = 0
	for _, m := range regexp.MustCompile(`\d+`).FindAllString("a1b22c333", -1) {
		v, _ := strconv.Atoi(m)
		s.sum += v
	}
}

func (s *Stdlib) View() Element {
	return Column(
		Text("spread: "+s.spread),
		Text("json: "+s.doc),
		Text(fmt.Sprintf("scan: %d", s.sum)),
		Row(
			Button("stats").OnClick(func() { s.stats() }),
			Button("parse").OnClick(func() { s.parse() }),
			Button("scan").OnClick(func() { s.scan() }),
		).Spacing(6),
	).Spacing(6).Padding(14)
}

func main() {
	Run(&Stdlib{scores: []int{3, 5, 8, 13, 21}, spread: "-", doc: "-"}, Title("stdlib"))
}
```

What the interpreter knows of the library is Go 1.22's: the packages
and functions of that release. Of the language it knows less: `min`
and `max` (1.21), `range` over a number (1.22) and over a function
(1.23) are not there, nor is anything the library gained in 1.23 or
later, and `check` says so by name for the first three. Files are `os`, the
network is `net/http`, a big number is `math/big`; the demos read a
file, serve a page to themselves and fetch it, and parse CSV, with
nothing but the standard library.


## The framework's standard library

What the engine mediates comes from the package, and there one
implementation answers both runs, through the same C face: a scripted
run keeps a dialog answered by `file:<path>` and a sound silent in
both.

| Function | What it does |
|---|---|
| `SqliteExec(db, sql, params...)` | run a statement; answers the rows changed |
| `SqliteQueryText`, `SqliteQueryInt`, `SqliteQueryRows` | a query's first column, first cell, or whole rows |
| `SqliteQueryTextOr`, `SqliteQueryIntOr`, `SqliteQueryRowsOr` | the same, answering nothing rather than stopping when the query fails |
| `ClipboardSetText(s)`, `ClipboardGetText()` | the system clipboard |
| `OpenDialog(title)`, `SaveDialog(name)` | the platform's own panels; they wait, so call them inside a `Task` |
| `AudioPlay(path, volume)`, `AudioStop()` | a WAV file, played and forgotten |
| `NotifySend(title, body)` | a notification |

Write `?` in a statement and put the values after it, and text a
person typed can never become part of the statement:

<!-- script: click:setup,click:load,dump -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

const db = "demo/.gate/tour-notes.db"

type Notes struct {
	changed int
	rows    []string
}

func (nt *Notes) setup() {
	SqliteExec(db, "CREATE TABLE IF NOT EXISTS notes(t TEXT)")
	SqliteExec(db, "DELETE FROM notes")
	nt.changed = SqliteExec(db, "INSERT INTO notes VALUES (?), (?)", "alpha", "beta")
}

func (nt *Notes) View() Element {
	return Column(
		Text(fmt.Sprintf("inserted=%d rows=%d", nt.changed, len(nt.rows))),
		Row(
			Button("setup").OnClick(func() { nt.setup() }),
			Button("load").OnClick(func() { nt.rows = SqliteQueryText(db, "SELECT t FROM notes ORDER BY t") }),
		).Spacing(6),
		ListView(len(nt.rows), func(i int) Element { return Text(nt.rows[i]) }).ItemHeight(22).Height(80),
	).Spacing(8).Padding(12)
}

func main() {
	Run(&Notes{}, Title("notes"))
}
```

A query that may run before its table exists — the first load of a
ledger does — is the `…Or` twin, which answers nothing; the plain one
stops the app, as the library's own would.


## Timers and work off the window's thread

`Every(seconds, tick)` asks to be told every so often, declared before
`Run`. Both runs tick off the same clock: a frame in a window, and
`advance:<ms>` in a script, so the same number of ticks lands in both.
A tick is where a game moves and where the keyboard is read.

`Task(work, done)` runs `work` on a goroutine of its own and, when it
is done, calls `done` on the window's thread with what the work
answered. Nothing inside the work touches the app's fields or the
screen; that is the whole rule, and the reason the answer comes back
as an argument rather than the worker writing it anywhere.

<!-- script: click:start,dump -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Jobs struct {
	status string
	answer int
}

func (j *Jobs) start() {
	j.status = "working"
	Task(func() any {
		total := 0
		for i := 0; i < 300000; i++ {
			total += i % 7
		}
		return total
	}, func(v any) {
		j.answer = v.(int)
		j.status = "done"
	})
}

func (j *Jobs) View() Element {
	return Column(
		Text(fmt.Sprintf("%s: %d", j.status, j.answer)),
		Button("start").OnClick(func() { j.start() }),
	).Spacing(10).Padding(14)
}

func main() {
	Run(&Jobs{status: "idle"}, Title("tasks"))
}
```

A script settles the work before its next step, so `dump` after
`click:start` shows the answer in both runs. The work may call the
framework's library — a dialog, a query — from its goroutine, or from
any goroutine the app starts itself: each call keeps to one thread for
its own duration, and nothing in the app has to know.


## While you are writing it

`gomamochi run` watches the app's file. Save, and the window picks the
edit up: the file is read again by a fresh interpreter, and the app the
window is holding hands its values to the new one — every field with
the same name and type keeps what it had, a field the new file adds
starts at its zero value, and a field whose type changed starts over.
Timers and shortcuts the file declares are bound again to the new app.
A file that does not compile leaves the window on what it had and says
so in the terminal, and the next save that does compile takes.

A save is a fresh read of the whole file, and `main` runs again. The
starting values it gives the app do not replace the ones the window
has, since those are carried over — which is the point.

