# A list whose rows are built on demand: the builder is called for the
# rows in view, not for all of them. The row number is an ordinary
# argument inside it, so the line, the marker and that row's own button
# all read the same `i`.
require "wakakusa"

class Todo
  def initialize
    @items = ["milk"]
    @draft = ""
    @done = -1
  end

  def add(t)
    items = @items.dup
    items.push(t)
    @items = items
    @draft = ""
  end

  def line(i)
    cells = [text("#{i + 1}. #{@items[i]}")]
    cells.push(text("done", color: "accent")) if i == @done
    cells.push(button("done") { @done = i })
    row(*cells, spacing: 8.0)
  end

  def view
    column(
      text("todo — #{@items.length} items", size: 16.0),
      text_field(@draft, placeholder: "add and press enter",
                 on_submit: :add) { |t| @draft = t },
      list_view(@items.length, item_height: 26.0, height: 280.0) { |i| line(i) },
      button("clear") { @items = [] },
      spacing: 10.0,
      padding: 14.0
    )
  end
end

run(Todo.new, title: "todo")
