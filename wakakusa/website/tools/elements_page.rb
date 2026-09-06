#!/usr/bin/env ruby
# frozen_string_literal: true
#
# The site's Elements page, written from elements.toml.
#
# The table is the one the Ruby methods and the engine's constants are
# generated from, so a reference page written from anything else would
# be a second opinion. Nothing here is typed by hand except the prose
# that frames it.
#
#   tools/elements_page.rb            write docs/elements.md and docs-ja/
#   tools/elements_page.rb --check    fail if either is behind the table

require "fileutils"

SITE = File.expand_path("..", __dir__)
ROOT = File.expand_path("..", SITE)

# --- the TOML this file speaks (the same subset tools/gen.rb reads) ---------

def toml_value(s)
  case s
  when /\A"(.*)"\z/m then Regexp.last_match(1).gsub('\\"', '"').gsub("\\\\", "\\")
  when "true" then true
  when "false" then false
  when "[]" then []
  when /\A-?\d+\z/ then s.to_i
  else s.to_f
  end
end

def toml_inline(s)
  h = {}
  s.scan(/(\w+)\s*=\s*("(?:[^"\\]|\\.)*"|\[\]|[^,}\s]+)/) { |k, v| h[k] = toml_value(v) }
  h
end

def parse_toml(text)
  doc = {}
  cur = nil
  key = nil
  arr = nil
  text.each_line do |raw|
    line = raw.strip
    next if line.empty? || line.start_with?("#")
    if arr
      if line.start_with?("]")
        cur[key] = arr
        arr = nil
      elsif line.start_with?("{")
        arr << toml_inline(line)
      end
      next
    end
    if (m = line.match(/\A\[\[(\w+)\]\]\z/))
      cur = {}
      (doc[m[1]] ||= []) << cur
      next
    end
    k, v = line.split("=", 2)
    k = k.strip
    v = v.strip
    if v == "["
      key = k
      arr = []
    else
      cur[k] = toml_value(v)
    end
  end
  doc
end

TABLE = parse_toml(File.read(File.join(ROOT, "..", "crates", "pixie-capi", "elements.toml")))
RIDERS = TABLE.fetch("rider")
ELEMENTS = TABLE.fetch("element")

# --- how the page is laid out ----------------------------------------------

# The order a reader meets them, which is not the order the table is
# written in: the table is ordered by the numbers the two sides count
# with, and those may never be reordered.
GROUPS = [
  ["text", %w[text button link]],
  ["arrange", %w[column row grid grid_cell stack scroll_view h_scroll_view modal]],
  ["fields", %w[text_field number_field int_field checkbox switch slider
                select radio_group segmented tab_bar]],
  ["rows", %w[list_view table data_table]],
  ["charts", %w[bar_chart line_chart progress]],
  ["pictures", %w[image svg canvas]],
  ["small", %w[spacer divider spinner]],
].freeze

WORDS = {
  en: {
    title: "Elements",
    intro: <<~MD,
      Thirty-three elements, and the fifteen keywords every one of them
      takes. This page is written from `elements.toml` — the one table
      the Ruby methods, the numbers both sides of the engine's C face
      count with, and the engine's own constants are generated from. A
      keyword that is not here is one an app cannot write, and
      `wakakusa check` says so by name.

      Types on this page: **text** is a string, **number** a float,
      **whole number** an integer, **true/false** a boolean, and a
      **list of** either. A **handler** is the block, or a proc of no
      arguments through a keyword — see
      [Handlers](tour-logic.md#handlers).
    MD
    riders_head: "The keywords every element takes",
    riders_intro: <<~MD,
      These fifteen ride on every element under one name and one
      meaning. They are wrappers around the element rather than fields
      repeated on thirty of them, which is why an element that owns one
      of the names under its own meaning keeps it — a `text`'s `width`
      is the text's, and the box leaves it alone.
    MD
    groups: {
      "text" => "Text, buttons, links",
      "arrange" => "The boxes that arrange",
      "fields" => "Fields and choosers",
      "rows" => "Lists and tables",
      "charts" => "Charts and progress",
      "pictures" => "Pictures and the canvas",
      "small" => "The small pieces",
    },
    th: ["Keyword", "Type", "Default"],
    positional: "written first, in order",
    handler: "a handler",
    children: "Takes elements as its children — as arguments, or as a block.",
    owns: "Sizes its own %s: those are the element's, and the shared keyword leaves them alone.",
    owns_label: "Its own `label` is what a screen reader reads, so the shared `a11y_label` is not offered here.",
    none: "No keywords of its own.",
    footer: <<~MD,
      ## Adding one

      An element is a row in `elements.toml` and an arm in the engine's
      `materialize`. `tools/gen.rb` writes the Ruby, the numbers and the
      Rust constants from the table, and a test fails when a key has no
      arm on the other side — so an element cannot come to mean one
      thing in Ruby and another where it is drawn.
    MD
  },
  ja: {
    title: "要素",
    intro: <<~MD,
      要素は 33 個、そのすべてが受け取る共通のキーワードが 15 個あります。
      このページは `elements.toml` から生成しています。
      アプリが呼ぶ Ruby のメソッドも、エンジンの C ABI で両側が数える番号も、エンジン側の定数も、同じ表から書き出されます。
      ここにないキーワードは、アプリには書けません。
      書いた場合は `wakakusa check` が、その要素が受け取るキーワードを並べて断ります。

      このページでの型の読み方です。
      **文字列** は string、**数** は float、**整数** は integer、**真偽** は boolean、**〜のリスト** はそれぞれのリストです。
      **ハンドラ** はブロック、またはキーワードで渡す引数なしの proc です（[ハンドラ](tour-logic.md#ハンドラ)）。
    MD
    riders_head: "すべての要素が受け取るキーワード",
    riders_intro: <<~MD,
      どの要素も、この 15 個を同じ名前と同じ意味で受け取ります。
      30 個の要素にそれぞれフィールドを足すのではなく、要素を包む形で実装してあります。
      同じ名前を要素自身が別の意味で持っている場合は、要素のものが優先されます。
      `text` の `width` は文字列そのものの幅で、外側の箱は手を出しません。
    MD
    groups: {
      "text" => "文字とボタンとリンク",
      "arrange" => "並べる箱",
      "fields" => "入力と選択",
      "rows" => "リストと表",
      "charts" => "グラフと進捗",
      "pictures" => "画像とキャンバス",
      "small" => "小さな部品",
    },
    th: ["キーワード", "型", "既定値"],
    positional: "先頭に、この順で書く",
    handler: "ハンドラ",
    children: "要素を子に取ります（引数として、またはブロックとして）。",
    owns: "%s は自分で決めます。\n共通キーワードは手を出しません。",
    owns_label: "自身の `label` が画面読み上げの読む名前なので、共通の `a11y_label` はここでは受け取りません。",
    none: "固有のキーワードはありません。",
    footer: <<~MD,
      ## 要素を足すとき

      要素を足す作業は、`elements.toml` に 1 行足すことと、エンジンの `materialize` に分岐を 1 つ足すことです。
      `tools/gen.rb` が表から Ruby と番号と Rust の定数を書き出し、片側に分岐のないキーが残っていればテストが落ちます。
      だから、ある要素が Ruby では 1 つの意味を持ち、描かれる側では別の意味を持つ、ということが起きません。
    MD
  },
}.freeze

TYPES = {
  en: { "str" => "text", "num" => "number", "int" => "whole number",
        "bool" => "true/false", "strs" => "list of text", "nums" => "list of numbers",
        "nums2" => "list of lists of numbers", "rows" => "a count, and the block building row i" },
  ja: { "str" => "文字列", "num" => "数", "int" => "整数",
        "bool" => "真偽", "strs" => "文字列のリスト", "nums" => "数のリスト",
        "nums2" => "数のリストのリスト", "rows" => "個数と、i 行目を作るブロック" },
}.freeze

def show_default(prop)
  return nil unless prop.key?("default")

  v = prop["default"]
  case v
  when true, false then "`#{v}`"
  when Array then "`[]`"
  when Float then "`#{format("%g", v)}`"
  else "`#{v.inspect}`"
  end
end

def prop_rows(el, lang)
  el.fetch("props", []).map do |p|
    name = p.fetch("name")
    type =
      if p.key?("handler")
        WORDS.fetch(lang).fetch(:handler)
      else
        TYPES.fetch(lang).fetch(p.fetch("type"))
      end
    default =
      if p["pos"]
        WORDS.fetch(lang).fetch(:positional)
      else
        show_default(p) || "—"
      end
    "| `#{name}` | #{type} | #{default} |"
  end
end

def element_section(el, lang)
  w = WORDS.fetch(lang)
  out = ["### `#{el.fetch("name")}`", "", el.fetch("doc"), ""]
  out << "#{w.fetch(:children)}\n" if el["children"]
  owned = RIDERS.select { |r| r["owned"].to_s.split(",").include?(el["native"].to_s) }
                .map { |r| "`#{r.fetch("name")}`" }
  out << format(w.fetch(:owns), owned.join(" / ")) + "\n" unless owned.empty?
  out << "#{w.fetch(:owns_label)}\n" if el["owns_label"]
  rows = prop_rows(el, lang)
  if rows.empty?
    out << "#{w.fetch(:none)}\n"
  else
    out << "| #{w.fetch(:th).join(" | ")} |"
    out << "|---|---|---|"
    out.concat(rows)
    out << ""
  end
  out.join("\n")
end

def page(lang)
  w = WORDS.fetch(lang)
  by_name = ELEMENTS.to_h { |e| [e.fetch("name"), e] }
  out = ["# #{w.fetch(:title)}", "", w.fetch(:intro).strip, ""]

  out << "## #{w.fetch(:riders_head)}"
  out << ""
  out << w.fetch(:riders_intro).strip
  out << ""
  out << "| #{w.fetch(:th).join(" | ")} |"
  out << "|---|---|---|"
  RIDERS.each do |r|
    out << "| `#{r.fetch("name")}` | #{TYPES.fetch(lang).fetch(r.fetch("type"))} | " \
           "#{show_default(r) || "—"} |"
  end
  out << ""

  seen = []
  GROUPS.each do |key, names|
    out << "## #{w.fetch(:groups).fetch(key)}"
    out << ""
    names.each do |name|
      el = by_name.fetch(name) { abort "elements_page: no element `#{name}` in the table" }
      seen << name
      out << element_section(el, lang)
      out << ""
    end
  end

  missing = by_name.keys - seen
  abort "elements_page: #{missing.join(", ")} is in the table and in no group" unless missing.empty?

  out << w.fetch(:footer).strip
  out << ""
  out.join("\n").gsub(/\n{3,}/, "\n\n")
end

FILES = { en: File.join(SITE, "docs", "elements.md"),
          ja: File.join(SITE, "docs-ja", "elements.md") }.freeze

check = ARGV.include?("--check")
stale = []
FILES.each do |lang, path|
  text = page(lang)
  if check
    stale << path unless File.exist?(path) && File.read(path) == text
  else
    FileUtils.mkdir_p(File.dirname(path))
    File.write(path, text)
    puts "wrote #{path.delete_prefix("#{ROOT}/")} (#{ELEMENTS.length} elements, #{RIDERS.length} shared keywords)"
  end
end

if check
  unless stale.empty?
    warn "elements_page: behind elements.toml: #{stale.map { |p| p.delete_prefix("#{ROOT}/") }.join(", ")}"
    exit 1
  end
  puts "elements page: up to date"
end
