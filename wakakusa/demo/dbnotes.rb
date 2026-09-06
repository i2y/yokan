# A database, reached through the engine so that both runs call one
# implementation. Write `?` in the statement and put the values beside
# it, and text a person typed can never become part of the statement.
require "wakakusa"

DB = "demo/.gate/notes.db"

class Notes
  def initialize
    @changed = 0
    @rows = []
  end

  def setup
    sqlite_exec(DB, "CREATE TABLE IF NOT EXISTS notes(t TEXT)")
    sqlite_exec(DB, "DELETE FROM notes")
    @changed = sqlite_exec(DB, "INSERT INTO notes VALUES ('alpha'),('beta'),('gamma')")
  end

  def load
    @rows = sqlite_column(DB, "SELECT t FROM notes ORDER BY t")
  end

  def note_row(i)
    text @rows[i]
  end

  def view
    column(spacing: 8.0, padding: 12.0) {
      text "inserted=#{@changed} rows=#{@rows.length}"
      row(spacing: 6.0) {
        button("setup") { setup }
        button("load") { load }
      }
      list_view(@rows.length, item_height: 22.0, height: 120.0) { |i| note_row(i) }
    }
  end
end

run(Notes.new, title: "dbnotes")
