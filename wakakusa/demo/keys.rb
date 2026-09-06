# The keyboard as a set of chords, and the same handlers in the
# application's menu bar. A script presses one with `key:cmd+s` and
# picks one with `menu:Save`.
require "wakakusa"

class Keys
  def initialize
    @count = 0
    @saved = 0
    @last = "-"
    @pasted = "(nothing)"
  end

  def save
    @saved = @count
  end

  def clear
    @count = 0
    @saved = 0
  end

  def copy_count
    clipboard_set("count=#{@count}")
  end

  def paste
    @pasted = clipboard_get
  end

  def typed(key)
    @last = key
  end

  def view
    column(spacing: 8.0, padding: 12.0) {
      text "count: #{@count}  saved: #{@saved}"
      text "last key: #{@last}"
      text "pasted: #{@pasted}"
      row(spacing: 6.0) {
        button("+1") { @count += 1 }
        button("save") { save }
        button("copy") { copy_count }
        button("paste") { paste }
      }
    }
  end
end

app = Keys.new

menu_item("Count", "Save") { app.save }
menu_item("Count", "Clear") { app.clear }

shortcut("cmd+s") { app.save }
shortcut("cmd+shift+r") { app.clear }
shortcut("cmd+shift+c") { app.copy_count }
shortcut("cmd+shift+v") { app.paste }
on_key { |chord| app.typed(chord) }

run(app, title: "keys")
