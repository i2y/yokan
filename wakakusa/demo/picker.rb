# The platform's own panels, and a file dragged onto the window. A
# dialog waits for a person, so it is asked for off the window's
# thread; a script answers one with `file:<path>` and drops one with
# `drop:<path>`.
require "wakakusa"

class Picker
  def initialize
    @chosen = "(nothing yet)"
    @body = ""
    @saved = "(not saved)"
  end

  def took(path)
    @chosen = path
    @body = read_or(path, "(unreadable)") unless path.empty?
  end

  def read_or(path, fallback)
    File.read(path)
  rescue SystemCallError
    fallback
  end

  def open_one
    job = task { Dialog.open("Choose a file") }
    on_done(job) { took(task_answer) }
  end

  def save_as
    job = task { Dialog.save("notes.txt") }
    on_done(job) do
      path = task_answer
      unless path.empty?
        File.write(path, @body)
        @saved = path
      end
    end
  end

  def view
    column(spacing: 8.0, padding: 12.0) {
      text "chosen: #{@chosen}"
      text "first line: #{@body[0, 40]}"
      text "saved to: #{@saved}"
      row(spacing: 6.0) {
        button("open…", tooltip: "the platform's own panel") { open_one }
        button("save as…") { save_as }
      }
    }
  end
end

app = Picker.new
on_file_drop { |path| app.took(path) }
run(app, title: "picker")
