#!/usr/bin/env ruby
# frozen_string_literal: true
#
# Read elements.toml and write the three files that have to agree about
# it: the numbers both sides count with, the Ruby an app calls, and the
# Rust constants the engine reads. Nothing here is written by hand, so
# an element cannot mean one thing in Ruby and another in the engine.
#
#   tools/gen.rb            write the files
#   tools/gen.rb --check    fail if what is on disk is not what this
#                           would write (the sweep runs this)
#
# The TOML this reads is the TOML we write: array-of-tables headers,
# scalars, and arrays of one-line inline tables. The Rust side parses
# the same file with a real parser, and its test fails if a key here
# never reached an arm — so a mistake in the reader below cannot pass
# quietly.

require "fileutils"

ROOT = File.expand_path("..", __dir__)
REPO = File.expand_path("..", ROOT)

# --- the TOML this file speaks ----------------------------------------------

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

TABLE = parse_toml(File.read(File.join(ROOT, "elements.toml")))
RIDERS = TABLE.fetch("rider")
ELEMENTS = TABLE.fetch("element")

# --- the numbers ------------------------------------------------------------

# A keyword is numbered once, by its name, however many elements take
# it: `width` is one key whether a button owns it or a box rides on it.
KEY_ID = {}
def key_id(name)
  KEY_ID[name] ||= KEY_ID.length + 1
end
RIDERS.each { |r| key_id(r.fetch("name")) }
ELEMENTS.each { |e| e.fetch("props").each { |p| key_id(p.fetch("name")) } }

KIND_ID = {}
ELEMENTS.each_with_index { |e, i| KIND_ID[e.fetch("name")] = i + 1 }

PAYLOADS = { "none" => 0, "text" => 1, "bool" => 2, "int" => 3, "float" => 4 }.freeze

def const(name) = name.upcase

def handler?(prop) = prop.key?("handler")

# What a prop's Ruby default reads as at the call site.
def ruby_default(prop)
  return "nil" if handler?(prop)
  d = prop["default"]
  case prop.fetch("type")
  when "str" then d.inspect
  when "num" then format("%.1f", d)
  when "int" then d.to_s
  when "bool" then d.to_s
  when "strs", "nums", "nums2" then "[]"
  when "rows" then "nil"
  end
end

# A rider whose box is built because somebody WROTE it arrives as nil.
def rider_default(rider) = rider["presence"] ? "nil" : nil

def banner(tool)
  "# Generated from elements.toml by tools/gen.rb. Do not edit by hand;\n" \
    "# edit the table and run `tools/gen.rb`#{tool}.\n"
end

# --- lib/wakakusa/keys.rb ---------------------------------------------------

def gen_keys
  out = +""
  out << banner("")
  out << "\n"
  out << "# The numbers the two sides of the engine's C face count with.\n"
  out << "module WK\n"
  out << "  # What a handler is called with.\n"
  PAYLOADS.each { |name, n| out << "  PAY_#{const(name)} = #{n}\n" }
  out << "\n  # The elements.\n"
  KIND_ID.each { |name, n| out << "  KIND_#{const(name)} = #{n}\n" }
  out << "\n  # The keyword arguments, numbered once across every element.\n"
  KEY_ID.each { |name, n| out << "  K_#{const(name)} = #{n}\n" }
  out << "end\n"
  out
end

# --- lib/wakakusa/elements.rb -----------------------------------------------

def send_call(prop, var)
  key = "WK::K_#{const(prop.fetch("name"))}"
  case prop.fetch("type")
  when "str" then "PixieC.pixie_str(el, #{key}, #{var})"
  when "num" then "PixieC.pixie_num(el, #{key}, #{var})"
  when "int" then "PixieC.pixie_int(el, #{key}, #{var})"
  when "bool" then "PixieC.pixie_bool(el, #{key}, #{var} ? 1 : 0)"
  end
end

def prop_lines(prop)
  name = prop.fetch("name")
  key = "WK::K_#{const(name)}"
  return ["  wakakusa_send_#{prop.fetch("handler")}(el, #{key}, #{name}) unless #{name}.nil?"] if handler?(prop)

  case prop.fetch("type")
  when "strs"
    ["  #{name}.each { |v| PixieC.pixie_push_str(el, #{key}, v) }"]
  when "nums"
    ["  #{name}.each { |v| PixieC.pixie_push_num(el, #{key}, v) }"]
  when "nums2"
    ["  #{name}.each do |inner|",
     "    PixieC.pixie_list_break(el, #{key})",
     "    inner.each { |v| PixieC.pixie_push_num(el, #{key}, v) }",
     "  end"]
  when "rows"
    ["  wakakusa_rows(el, #{key}, &blk)"]
  else
    call = send_call(prop, name)
    # A positional argument is always written, so it always crosses; a
    # keyword only crosses when it differs from what the engine would
    # have used anyway.
    prop["pos"] ? ["  #{call}"] : ["  #{call} if #{name} != #{ruby_default(prop)}"]
  end
end

def signature(el)
  props = el.fetch("props")
  head = []
  head << "*kids" if el["children"]
  props.select { |p| p["pos"] }.reject { |p| p["type"] == "rows" }.each do |p|
    head << (p.key?("default") ? "#{p.fetch("name")} = #{ruby_default(p)}" : p.fetch("name"))
  end
  props.reject { |p| p["pos"] }.each { |p| head << "#{p.fetch("name")}: #{ruby_default(p)}" }
  head << "**riders"
  head << "&blk" if el["primary"] || props.any? { |p| p["type"] == "rows" }
  head.join(", ")
end

def wrap_signature(name, sig)
  line = "def #{name}(#{sig})"
  return [line] if line.length <= 78

  parts = sig.split(", ")
  lines = []
  cur = +"def #{name}("
  indent = " " * cur.length
  parts.each_with_index do |p, i|
    piece = p + (i == parts.length - 1 ? "" : ",")
    if cur.length + piece.length > 78 && cur.strip != "def #{name}("
      lines << cur.rstrip
      cur = indent.dup
    end
    cur << piece << " "
  end
  lines << "#{cur.rstrip})"
  lines
end

def gen_elements
  out = +""
  out << banner("")
  out << "\n"
  out << "# One method per element. Its own keywords are spelled out; the\n"
  out << "# keywords every element takes ride in `riders` and are applied\n"
  out << "# below, so an element's own `width` wins over the box's.\n"
  ELEMENTS.each do |el|
    name = el.fetch("name")
    out << "\n"
    doc = el["doc"]
    if doc
      words = doc.split(" ")
      line = +"#"
      words.each do |w|
        if line.length + w.length + 1 > 74
          out << line << "\n"
          line = +"#"
        end
        line << " " << w
      end
      out << line << "\n"
    end
    out << wrap_signature(name, signature(el)).join("\n") << "\n"
    out << "  el = PixieC.pixie_el(WK::KIND_#{const(name)})\n"
    el.fetch("props").each do |p|
      if el["primary"] == p["name"]
        # The block is the usual way to write this element's handler; a
        # symbol naming one of the app's methods is the other. Only a
        # literal block survives being handed on in a compiled run, so
        # the two paths stay apart here rather than meeting in a local.
        k = "WK::K_#{const(p.fetch("name"))}"
        h = p.fetch("handler")
        out << "  if #{p.fetch("name")}.nil?\n"
        out << "    wakakusa_on_#{h}(el, #{k}, &blk) unless blk.nil?\n"
        out << "  else\n"
        out << "    wakakusa_send_#{h}(el, #{k}, #{p.fetch("name")})\n"
        out << "  end\n"
      else
        prop_lines(p).each { |l| out << l << "\n" }
      end
    end
    out << "  wakakusa_children(el, kids)\n" if el["children"]
    out << "  wakakusa_riders(el, riders, #{!!el["owns_label"]})\n"
    out << "  PixieC.pixie_end(el)\n"
    out << "end\n"
  end

  out << "\n"
  out << "# The keywords every element takes. An element that owns one of\n"
  out << "# these names under its own meaning never gets here: Ruby binds it\n"
  out << "# to that element's own keyword first.\n"
  out << "def wakakusa_rider(el, name, v)\n"
  out << "  case name\n"
  RIDERS.each do |r|
    key = "WK::K_#{const(r.fetch("name"))}"
    call = case r.fetch("type")
           when "str" then "PixieC.pixie_str(el, #{key}, v)"
           when "num" then "PixieC.pixie_num(el, #{key}, v)"
           when "int" then "PixieC.pixie_int(el, #{key}, v)"
           when "bool" then "PixieC.pixie_bool(el, #{key}, v ? 1 : 0)"
           end
    out << "  when :#{r.fetch("name")} then #{call}\n"
  end
  out << "  else\n"
  out << "    raise ArgumentError, \"no property `\#{name}` — every element takes \" \\\n"
  out << "                         \"#{RIDERS.map { |r| r.fetch("name") }.join(", ")}\"\n"
  out << "  end\n"
  out << "end\n"
  out
end


# --- lib/wakakusa/table.rb ---------------------------------------------------

def gen_table
  out = +""
  out << banner("")
  out << "\n"
  out << "# What the checker reads: which keywords each element takes, and\n"
  out << "# which of them name a handler.\n"
  out << "module WK\n"
  out << "  RIDERS = %w[#{RIDERS.map { |r| r.fetch("name") }.join(" ")}].freeze\n"
  out << "\n  ELEMENTS = {\n"
  ELEMENTS.each do |el|
    own = el.fetch("props").map { |p| p.fetch("name") }
    own -= ["row"] if el.fetch("props").any? { |p| p["type"] == "rows" }
    allowed = own + RIDERS.map { |r| r.fetch("name") } - (el["owns_label"] ? ["a11y_label"] : [])
    out << "    \"#{el.fetch("name")}\" => %w[#{allowed.uniq.sort.join(" ")}],\n"
  end
  out << "  }.freeze\n"
  handlers = ELEMENTS.flat_map { |el| el.fetch("props").select { |p| handler?(p) }.map { |p| p.fetch("name") } }
  out << "\n  HANDLERS = %w[#{handlers.uniq.sort.join(" ")}].freeze\n"
  out << "end\n"
  out
end

# --- crates/pixie-capi/src/vocab.rs -------------------------------------------

def rust_str(s) = s.inspect

def gen_rust
  out = +""
  out << "//! Generated from `wakakusa/elements.toml` by `wakakusa/tools/gen.rb`.\n"
  out << "//! Do not edit by hand; edit the table and run the generator.\n"
  out << "//!\n"
  out << "//! The numbers here are the ones the caller's side counts with, and\n"
  out << "//! the defaults are what a property reads as when the caller left it\n"
  out << "//! out — which is why only what an app actually wrote has to cross.\n"
  out << "\n#![allow(dead_code)]\n\n"
  out << "/// What a handler is called with. The engine names the kind when\n"
  out << "/// it hands an event over, because a caller keeps one registry per\n"
  out << "/// kind: a list of blocks all called with the same sort of value is\n"
  out << "/// one a compiler can type, and a mixed one is not.\n"
  PAYLOADS.each { |name, n| out << "pub const PAY_#{const(name)}: i64 = #{n};\n" }
  out << "\n"
  KIND_ID.each { |name, n| out << "pub const KIND_#{const(name)}: i32 = #{n};\n" }
  out << "\n"
  KEY_ID.each { |name, n| out << "pub const K_#{const(name)}: i32 = #{n};\n" }

  defs = { "num" => [], "int" => [], "bool" => [], "str" => [] }
  ELEMENTS.each do |el|
    kind = "KIND_#{const(el.fetch("name"))}"
    el.fetch("props").each do |p|
      next if handler?(p) || p["pos"]
      key = "K_#{const(p.fetch("name"))}"
      case p.fetch("type")
      when "num" then defs["num"] << "    (#{kind}, #{key}, #{format("%.1f", p["default"])}),"
      when "int" then defs["int"] << "    (#{kind}, #{key}, #{p["default"]}),"
      when "bool" then defs["bool"] << "    (#{kind}, #{key}, #{p["default"]}),"
      when "str" then defs["str"] << "    (#{kind}, #{key}, #{rust_str(p["default"])}),"
      end
    end
  end
  RIDERS.each do |r|
    next if r["presence"]
    key = "K_#{const(r.fetch("name"))}"
    case r.fetch("type")
    when "num" then defs["num"] << "    (0, #{key}, #{format("%.1f", r["default"])}),"
    when "int" then defs["int"] << "    (0, #{key}, #{r["default"]}),"
    when "bool" then defs["bool"] << "    (0, #{key}, #{r["default"]}),"
    when "str" then defs["str"] << "    (0, #{key}, #{rust_str(r["default"])}),"
    end
  end
  out << "\n/// `(kind, key, value)` — what a property reads as when nobody wrote\n"
  out << "/// it. Kind 0 is a rider, which means the same on every element.\n"
  { "num" => "f64", "int" => "i64", "bool" => "bool", "str" => "&str" }.each do |t, ty|
    out << "pub const DEF_#{t.upcase}: &[(i32, i32, #{ty})] = &[\n"
    out << defs[t].join("\n") << "\n];\n"
  end

  out << "\n/// Every element and every keyword the table declares, by name, so\n"
  out << "/// a test can read the table and check that each one reached an arm.\n"
  out << "pub const KINDS: &[(&str, i32)] = &[\n"
  KIND_ID.each_key { |name| out << "    (#{rust_str(name)}, KIND_#{const(name)}),\n" }
  out << "];\n"
  out << "pub const KEYS: &[(&str, i32)] = &[\n"
  KEY_ID.each_key { |name| out << "    (#{rust_str(name)}, K_#{const(name)}),\n" }
  out << "];\n"

  %w[size width height].each do |flavor|
    kinds = ELEMENTS.select { |e| e["native"] == flavor }.map { |e| "KIND_#{const(e.fetch("name"))}" }
    out << "\n/// The elements that size their own #{flavor == "size" ? "width and height" : flavor},\n"
    out << "/// so the box rider leaves that side alone.\n"
    out << "pub const NATIVE_#{flavor.upcase}: &[i32] = &[#{kinds.join(", ")}];\n"
  end
  out
end

# --- write, or check --------------------------------------------------------

FILES = {
  File.join(ROOT, "lib", "wakakusa", "keys.rb") => gen_keys,
  File.join(ROOT, "lib", "wakakusa", "elements.rb") => gen_elements,
  File.join(ROOT, "lib", "wakakusa", "table.rb") => gen_table,
  File.join(REPO, "crates", "pixie-capi", "src", "vocab.rs") => gen_rust,
}.freeze

check = ARGV.include?("--check")
stale = []
FILES.each do |path, body|
  if check
    stale << path unless File.exist?(path) && File.read(path) == body
  else
    FileUtils.mkdir_p(File.dirname(path))
    File.write(path, body)
    puts "wrote #{path.delete_prefix("#{REPO}/")}"
  end
end

if check && !stale.empty?
  warn "gen: these are not what elements.toml says — run wakakusa/tools/gen.rb"
  stale.each { |p| warn "  #{p.delete_prefix("#{REPO}/")}" }
  exit 1
end
