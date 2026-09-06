# The edges: an index past the end of a list, and a number far past what
# a machine word holds that keeps growing. Both runs have to answer the
# same, and this is the demo that says so.
#
# The number starts big rather than growing into it. A value that begins
# inside a machine word and then passes 2**63 is where the two runs part
# company today: the compiled run keeps the slot it first chose and the
# value wraps to zero, which the compiler's own notes call out.
require "wakakusa"

class Edges
  def initialize
    @xs = [7]
    @picked = 0
    @big = 18_446_744_073_709_551_616
    @steps = 0
  end

  def view
    column(
      text("picked=#{@picked} steps=#{@steps}"),
      text("big=#{@big}"),
      button("oob") { @picked = @xs[5].nil? ? -1 : @xs[5] },
      button("grow") { @big = @big * 4 },
      button("partial") do
        @steps += 1
        @picked = @xs[9].nil? ? -1 : @xs[9]
      end,
      spacing: 8.0,
      padding: 12.0
    )
  end
end

run(Edges.new, title: "edges")
