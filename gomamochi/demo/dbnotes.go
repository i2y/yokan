// A database, reached through the engine so that both runs call one
// implementation. Write `?` in the statement and put the values beside
// it, and text a person typed can never become part of the statement.
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

const db = "demo/.gate/notes.db"

type Notes struct {
	changed int
	rows    []string
}

func (nt *Notes) setup() {
	SqliteExec(db, "CREATE TABLE IF NOT EXISTS notes(t TEXT)")
	SqliteExec(db, "DELETE FROM notes")
	nt.changed = SqliteExec(db, "INSERT INTO notes VALUES ('alpha'),('beta'),('gamma')")
}

func (nt *Notes) load() {
	nt.rows = SqliteQueryText(db, "SELECT t FROM notes ORDER BY t")
}

func (nt *Notes) noteRow(i int) Element {
	return Text(nt.rows[i])
}

func (nt *Notes) View() Element {
	return Column(
		Text(fmt.Sprintf("inserted=%d rows=%d", nt.changed, len(nt.rows))),
		Row(
			Button("setup").OnClick(func() { nt.setup() }),
			Button("load").OnClick(func() { nt.load() }),
		).Spacing(6),
		ListView(len(nt.rows), func(i int) Element { return nt.noteRow(i) }).ItemHeight(22).Height(120),
	).Spacing(8).Padding(12)
}

func main() {
	Run(&Notes{}, Title("dbnotes"))
}
