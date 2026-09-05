# The table that builds its rows on demand: the block builds row i as a
# row of one cell per column, and the header and the rows sit on tracks
# whose shares are `widths`. Picking a row and sorting a column are the
# app's own methods, named rather than handed over.
require "wakakusa"

class Roster
  def initialize
    @teams = ["red", "blue", "green", "gold"]
    @names = []
    @team_of = []
    @scores = []
    24.times do |i|
      @names.push("member #{i}")
      @team_of.push(@teams[i % 4])
      @scores.push((i * 37 + 11) % 100)
    end
    @sel = -1
    @line = ""
    @sort = -1
    @desc = false
  end

  def pick(i)
    @sel = i
    @line = "#{@names[i]} (#{@team_of[i]}, #{@scores[i]})"
  end

  def sort_by(col)
    @desc = col == @sort ? !@desc : false
    @sort = col
    order = (0...@names.length).to_a
    order.sort_by! { |i| col == 2 ? @scores[i] : @names[i] }
    order.reverse! if @desc
    @names = order.map { |i| @names[i] }
    @team_of = order.map { |i| @team_of[i] }
    @scores = order.map { |i| @scores[i] }
    @sel = -1
    @line = ""
  end

  def line(i)
    row(
      text(@names[i], grow: 2.0),
      text(@team_of[i], grow: 1.0),
      text(@scores[i].to_s, grow: 1.0, align: "right")
    )
  end

  def view
    column(
      text("Roster — #{@names.length} people", size: 16.0),
      table(["name", "team", "score"], @names.length,
            widths: [2.0, 1.0, 1.0], height: 220.0,
            selected: @sel, sort: @sort, descending: @desc,
            on_select: :pick, on_sort: :sort_by) { |i| line(i) },
      text(@line.empty? ? "nobody picked" : @line, size: 12.0),
      spacing: 10.0,
      padding: 14.0
    )
  end
end

run(Roster.new, title: "roster")
