# A Hash on the app: reading with a fallback, asking whether a key is
# there, and adding one while the window is open.
require "wakakusa"

class Lookup
  def initialize
    @prices = { "apple" => 120, "banana" => 80 }
    @picked = 0
    @label = "none"
  end

  def pick_apple
    @picked = @prices.fetch("apple", -1)
    @label = @prices.key?("cherry") ? "cherry known" : "no cherry"
  end

  def add_cherry
    @prices["cherry"] = 200
    @picked = @prices.fetch("cherry", -1)
    @label = "cherry known" if @prices.key?("cherry")
  end

  def view
    column(spacing: 8.0, padding: 12.0) {
      text "picked=#{@picked} n=#{@prices.length} #{@label}"
      text "apple costs #{@prices.fetch("apple", -1)} right now", size: 12.0
      row(spacing: 6.0) {
        button("apple") { pick_apple }
        button("cherry") { add_cherry }
        button("miss") { @picked = @prices.fetch("durian", -7) }
      }
    }
  end
end

run(Lookup.new, title: "lookup")
