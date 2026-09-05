# A counter: containers take their children as arguments, and a handler
# is a block at the leaf.
#
#   wakakusa run   demo/counter.rb
#   wakakusa gate  demo/counter.rb --script "click:+1,click:+10,dump"
require "wakakusa"

$count = 0

def view
  column(
    text("count: #{$count}", size: 34.0),
    row(
      button("+1") { $count += 1 },
      button("+10") { $count += 10 },
      button("reset") { $count = 0 },
      spacing: 8.0
    ),
    spacing: 12.0,
    padding: 16.0
  )
end

run("counter") { view }
