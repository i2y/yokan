# The reference: everything in this file is what Wakakusa takes. The
# app is an object, its state is its instance variables, and a handler
# is a block that closes over it.
#
#   wakakusa run  demo/counter.rb
#   wakakusa gate demo/counter.rb --script "click:+1,input:Momo"
require "wakakusa"

class Counter
  def initialize
    @count = 0
    @name = ""
  end

  def view
    column(
      text("count: #{@count}", size: 34.0),
      row(
        button("+1") { @count += 1 },
        button("+10") { @count += 10 },
        button("reset") { @count = 0 },
        spacing: 8.0
      ),
      text_field(@name, placeholder: "your name") { |s| @name = s },
      text("hello, #{@name}"),
      spacing: 12.0,
      padding: 16.0
    )
  end
end

run(Counter.new, title: "counter")
