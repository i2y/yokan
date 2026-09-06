# A hundred thousand rows, filtered as you type. The list is
# virtualized: only the rows in the window are ever built, so the
# filter is the only thing that touches all of them.
#
# The numbers are arithmetic rather than random, because the two runs
# do not share a generator and a viewer whose rows cannot be compared
# would not be worth gating.
require "wakakusa"

N = 100_000
CATS = %w[alpha beta gamma delta epsilon].freeze
STEMS = %w[kuro shiro aka ao momo yuki hana sora].freeze
TAILS = %w[maru suke chan gou ta emon].freeze

class Viewer
  def initialize
    @names = Array.new(N) { |i| format("%s%s-%06d", STEMS[i % 8], TAILS[(i / 8) % 6], i) }
    @cats = Array.new(N) { |i| CATS[i % 5] }
    @values = Array.new(N) { |i| ((i * 37 % 4000) / 100.0) + 30.0 }
    @q = ""
    @idx = (0...N).to_a
  end

  def filter(q)
    @q = q
    @idx = if q.empty?
             (0...N).to_a
           else
             low = q.downcase
             (0...N).select { |i| @names[i].include?(low) || @cats[i].include?(low) }
           end
  end

  def line(k)
    i = @idx[k]
    row(spacing: 12.0) {
      text format("%06d", i), size: 12.0, color: "#8a8f98"
      text @names[i], grow: 1.0
      text @cats[i], size: 12.0, color: "#7aa2f7"
      text format("%.2f", @values[i]), align: "right"
    }
  end

  def view
    column(spacing: 10.0, padding: 14.0) {
      text "csv viewer — #{N} rows, virtualized", size: 13.0, color: "#8a8f98"
      text_field(@q, placeholder: "filter…") { |t| filter(t) }
      text "#{@idx.length} / #{N} rows match", size: 12.0
      list_view(@idx.length, item_height: 26.0, height: 430.0) { |k| line(k) }
    }
  end
end

run(Viewer.new, title: "csv_viewer")
