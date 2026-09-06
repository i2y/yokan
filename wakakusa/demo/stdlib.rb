# Ruby's own standard library, under the gate.
#
# Nothing here is Wakakusa's. `Math`, `Time`, `JSON`, `CSV`,
# `format`, the regular expressions and everything Enumerable answers
# are the language's, and both runs call the same ones. What the gate
# says is that they answer the same — which is the only claim worth
# making about a standard library shared between two implementations.
require "wakakusa"
require "json"
require "csv"

class Stdlib
  def initialize
    @hyp = 0.0
    @spread = "-"
    @sift = "-"
    @tally = "-"
    @runs = "-"
    @stamp = "-"
    @doc = "-"
    @row = "-"
    @words = "-"
    @unique = "-"
    @scores = [3, 5, 8, 13, 21]
    @votes = %w[ivy momo ivy ada momo ivy ada]
  end

  def measure
    @hyp = Math.sqrt(3.0 * 3.0 + 4.0 * 4.0)
  end

  def stats
    mean = @scores.sum.to_f / @scores.length
    sorted = @scores.sort
    median = sorted[sorted.length / 2]
    @spread = format("mean %.1f median %d min %d max %d",
                     mean, median, sorted.first, sorted.last)
  end

  def sift
    big = @scores.select { |n| n > 5 }
    small = @scores.reject { |n| n > 5 }
    @sift = "big #{big.join(",")} small #{small.join(",")}"
  end

  def count
    @tally = @votes.tally.sort_by { |name, n| [-n, name] }
                   .map { |name, n| "#{name}:#{n}" }.join(" ")
  end

  def combine
    steps = @scores.each_cons(2).map { |a, b| b - a }
    pairs = @scores.first(3).zip(%w[a b c]).map { |n, s| "#{s}#{n}" }
    @runs = "steps #{steps.join(",")} pairs #{pairs.join(",")}"
  end

  def stamp
    @stamp = Time.at(1_700_000_000).utc.strftime("%Y-%m-%d %H:%M:%S UTC")
  end

  def parse
    src = '{"name": "wakakusa", "parts": [1, 2, 3], "ok": true}'
    doc = JSON.parse(src)
    @doc = "#{doc["name"]} #{doc["parts"].sum} #{doc["ok"]}"
  end

  def write
    @doc = JSON.generate({ "n" => @scores.length, "top" => @scores.max })
  end

  def read_row
    @row = CSV.parse_line("api,42,\"one, two\"").join(" | ")
  end

  def words
    line = "  the quick brown fox  "
    @words = "#{line.strip.split.map(&:capitalize).join("-")} (#{line.strip.length})"
  end

  # The set of names, without repeats.
  def unique
    @unique = @votes.uniq.sort.join(",")
  end

  def find_numbers
    @spread = "a1b22c333".scan(/\d+/).map(&:to_i).sum.to_s
  end

  def view
    column(spacing: 6.0, padding: 14.0) {
      text "Ruby's own, in both runs", size: 16.0, bold: true
      text "hypotenuse: #{@hyp}"
      text "spread: #{@spread}"
      text "sift: #{@sift}"
      text "tally: #{@tally}"
      text "runs: #{@runs}"
      text "stamp: #{@stamp}"
      text "json: #{@doc}"
      text "csv: #{@row}"
      text "words: #{@words}"
      text "set: #{@unique}"
      row(spacing: 6.0) {
        button("measure") { measure }
        button("stats") { stats }
        button("sift") { sift }
        button("count") { count }
      }
      row(spacing: 6.0) {
        button("combine") { combine }
        button("stamp") { stamp }
        button("parse") { parse }
        button("write") { write }
      }
      row(spacing: 6.0) {
        button("csv") { read_row }
        button("words") { words }
        button("set") { unique }
        button("scan") { find_numbers }
      }
    }
  end
end

run(Stdlib.new, title: "stdlib")
