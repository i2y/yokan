# Money kept in a database, with the values bound rather than spliced:
# an item called o'brien is an apostrophe and never a piece of SQL.
require "wakakusa"

DB = "demo/.gate/ledger.db"

HEADING = { size: 20.0, color: "accent" }.freeze
FAINT = { size: 12.0, color: "#8a8f98" }.freeze

class Ledger
  def initialize
    @name = ""
    @amount = ""
    @count = 0
    @grand = 0
    @food = 0
    @transit = 0
    @fun = 0
    @rows = []
    load
  end

  def reset
    sqlite_exec(DB, "CREATE TABLE IF NOT EXISTS expenses(name TEXT, amount INTEGER, cat TEXT)")
    sqlite_exec(DB, "DELETE FROM expenses")
    load
  end

  def add(cat)
    yen = @amount.to_i
    return unless yen > 0

    sqlite_exec(DB, "INSERT INTO expenses VALUES (?, ?, ?)", [@name, yen, cat])
    load
  end

  def one_number(sql, params)
    rows = sqlite_rows(DB, sql, params)
    rows.empty? ? 0 : rows[0][0].to_i
  end

  def load
    @count = one_number("SELECT COUNT(*) FROM expenses", [])
    @grand = one_number("SELECT COALESCE(SUM(amount),0) FROM expenses", [])
    by = "SELECT COALESCE(SUM(amount),0) FROM expenses WHERE cat=?"
    @food = one_number(by, ["food"])
    @transit = one_number(by, ["transit"])
    @fun = one_number(by, ["fun"])
    # whole rows, every column as text: the line is written here rather
    # than assembled in SQL
    @rows = sqlite_rows(DB, "SELECT name, amount, cat FROM expenses ORDER BY rowid")
                .map { |r| "#{r[0]}  ¥#{r[1]}  (#{r[2]})" }
  end

  def chart
    [@food.to_f, @transit.to_f, @fun.to_f]
  end

  def entry_row(i)
    text @rows[i]
  end

  def view
    column(spacing: 10.0, padding: 14.0, background: "panel") {
      text "ledger", **HEADING
      row(spacing: 6.0) {
        text_field(@name, placeholder: "item") { |t| @name = t }
        text_field(@amount, placeholder: "yen") { |t| @amount = t }
      }
      row(spacing: 6.0) {
        button("food") { add("food") }
        button("transit") { add("transit") }
        button("fun") { add("fun") }
        button("reset") { reset }
      }
      text "#{@count} entries, ¥#{@grand} in all", **FAINT
      text "food ¥#{@food} · transit ¥#{@transit} · fun ¥#{@fun}", **FAINT
      bar_chart(chart, labels: ["food", "transit", "fun"], axis: true, height: 90.0)
      list_view(@rows.length, item_height: 22.0, height: 120.0) { |i| entry_row(i) }
    }
  end
end

run(Ledger.new, title: "ledger")
