# The same counter as demo/counter.rb, written the other way. A
# container takes its children as a block, and each element inside
# joins the one that is open.
#
# Both spellings build the same tree; this demo exists to say so.
require "wakakusa"

class Counter
  def initialize
    @count = 0
    @name = ""
  end

  def view
    column(spacing: 12.0, padding: 16.0) {
      text "count: #{@count}", size: 34.0
      row(spacing: 8.0) {
        button("+1") { @count += 1 }
        button("+10") { @count += 10 }
        button("reset") { @count = 0 }
      }
      text_field(@name, placeholder: "your name") { |s| @name = s }
      text "hello, #{@name}"
    }
  end
end

run(Counter.new, title: "blockform")
