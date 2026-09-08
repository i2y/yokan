// Money kept in a database, with the values bound rather than spliced:
// an item called o'brien is an apostrophe and never a piece of SQL.
package main

import (
	"fmt"
	"strconv"

	. "github.com/i2y/yokan/gomamochi"
)

const db = "demo/.gate/ledger.db"

type Ledger struct {
	name    string
	amount  string
	count   int
	grand   int
	food    int
	transit int
	fun     int
	rows    []string
}

func newLedger() *Ledger {
	l := &Ledger{}
	l.load()
	return l
}

func (l *Ledger) reset() {
	SqliteExec(db, "CREATE TABLE IF NOT EXISTS expenses(name TEXT, amount INTEGER, cat TEXT)")
	SqliteExec(db, "DELETE FROM expenses")
	l.load()
}

func (l *Ledger) add(cat string) {
	yen, _ := strconv.Atoi(l.amount)
	if yen <= 0 {
		return
	}
	SqliteExec(db, "INSERT INTO expenses VALUES (?, ?, ?)", l.name, strconv.Itoa(yen), cat)
	l.load()
}

// A query that may run before the table exists — the first load does —
// answers no rows rather than stopping the app.
func (l *Ledger) oneNumber(sql string, params ...string) int {
	rows := SqliteQueryRowsOr(db, sql, params...)
	if len(rows) == 0 {
		return 0
	}
	v, _ := strconv.Atoi(rows[0][0])
	return v
}

func (l *Ledger) load() {
	l.count = l.oneNumber("SELECT COUNT(*) FROM expenses")
	l.grand = l.oneNumber("SELECT COALESCE(SUM(amount),0) FROM expenses")
	by := "SELECT COALESCE(SUM(amount),0) FROM expenses WHERE cat=?"
	l.food = l.oneNumber(by, "food")
	l.transit = l.oneNumber(by, "transit")
	l.fun = l.oneNumber(by, "fun")
	// whole rows, every column as text: the line is written here
	// rather than assembled in SQL
	l.rows = nil
	for _, r := range SqliteQueryRowsOr(db, "SELECT name, amount, cat FROM expenses ORDER BY rowid") {
		l.rows = append(l.rows, fmt.Sprintf("%s  ¥%s  (%s)", r[0], r[1], r[2]))
	}
}

func (l *Ledger) entryRow(i int) Element {
	return Text(l.rows[i])
}

func (l *Ledger) View() Element {
	return Column(
		Text("ledger").Size(20).Color("accent"),
		Row(
			TextField(l.name).Placeholder("item").OnChange(func(t string) { l.name = t }),
			TextField(l.amount).Placeholder("yen").OnChange(func(t string) { l.amount = t }),
		).Spacing(6),
		Row(
			Button("food").OnClick(func() { l.add("food") }),
			Button("transit").OnClick(func() { l.add("transit") }),
			Button("fun").OnClick(func() { l.add("fun") }),
			Button("reset").OnClick(func() { l.reset() }),
		).Spacing(6),
		Text(fmt.Sprintf("%d entries, ¥%d in all", l.count, l.grand)).Size(12).Color("#8a8f98"),
		Text(fmt.Sprintf("food ¥%d · transit ¥%d · fun ¥%d", l.food, l.transit, l.fun)).Size(12).Color("#8a8f98"),
		BarChart([]float64{float64(l.food), float64(l.transit), float64(l.fun)}).
			Labels("food", "transit", "fun").Axis(true).Height(90),
		ListView(len(l.rows), func(i int) Element { return l.entryRow(i) }).ItemHeight(22).Height(120),
	).Spacing(10).Padding(14).Background("panel")
}

func main() {
	app := newLedger()
	Run(app, Title("ledger"))
}
