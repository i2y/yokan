# The same calculator as demo/calc.rb, on a grid instead of five rows.
# `col_span:` is what makes the zero key twice as wide.
require "wakakusa"
require_relative "calc_style"

class CalcGrid
  def initialize
    @display = "0"
    @acc = 0.0
    @op = ""
    @fresh = true
    @has_dot = false
  end

  def to_f_or_zero(s)
    Float(s)
  rescue ArgumentError, TypeError
    0.0
  end

  def press(d)
    if @fresh
      @display = d
      @fresh = false
      @has_dot = false
    elsif @display == "0"
      @display = d
    else
      @display = @display + d
    end
  end

  def dot
    if @fresh
      @display = "0."
      @fresh = false
      @has_dot = true
    elsif !@has_dot
      @display = @display + "."
      @has_dot = true
    end
  end

  def negate
    v = to_f_or_zero(@display)
    return if v == 0.0

    @display = "#{0.0 - v}"
    @fresh = false
  end

  def percent
    @display = "#{to_f_or_zero(@display) / 100.0}"
    @fresh = true
    @has_dot = false
  end

  def apply(nxt)
    if @fresh && @op != ""
      @op = nxt
      return
    end
    cur = to_f_or_zero(@display)
    @acc = cur if @op == ""
    @acc += cur if @op == "+"
    @acc -= cur if @op == "-"
    @acc *= cur if @op == "×"
    if @op == "÷"
      if cur == 0.0
        @display = "Error"
        @acc = 0.0
        @op = ""
        @fresh = true
        return
      end
      @acc /= cur
    end
    @display = "#{@acc}"
    @op = nxt
    @fresh = true
  end

  def clear
    @display = "0"
    @acc = 0.0
    @op = ""
    @fresh = true
    @has_dot = false
  end

  def digit(d)
    button(d, **KEY) { press(d) }
  end

  def op_key(name)
    button(name, **OP) { apply(name) }
  end

  def view
    column(spacing: 8.0, padding: 16.0, grow: 1.0) {
      text @display, **READOUT
      grid(columns: 4, rows: 5, spacing: 8.0, grow: 5.0) {
        button("C", **FUN) { clear }
        button("±", **FUN) { negate }
        button("%", **FUN) { percent }
        op_key("÷")
        digit("7")
        digit("8")
        digit("9")
        op_key("×")
        digit("4")
        digit("5")
        digit("6")
        op_key("-")
        digit("1")
        digit("2")
        digit("3")
        op_key("+")
        button("0", col_span: 2, **KEY) { press("0") }
        button(".", **KEY) { dot }
        button("=", **OP) { apply("") }
      }
    }
  end
end

run(CalcGrid.new, title: "calcgrid")
