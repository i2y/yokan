# Control flow in the handlers: a loop that skips, a loop that stops, a
# while, and a method that wraps another one. Both runs do the same
# thing, which is what the gate compares.
require "wakakusa"

class Flow
  def initialize
    @count = 0
    @total = 0
    @status = "start"
  end

  def double(v)
    v * 2
  end

  # A wrapper around a handler: it says what it is doing, runs the
  # handler, and says it is done.
  def announced
    @status = "working"
    yield
    @status = "done"
  end

  def step
    @count += 1
    @status = if @count > 3 && @count < 100
                "big"
              elsif @count == 3
                "three"
              else
                "small"
              end
  end

  def tally
    @total = 0
    (1...6).each do |i|
      next if i == 3

      @total += double(i)
    end
  end

  def bump3
    announced do
      @count += 1 while @count < 3
    end
  end

  def find
    (0...10).each do |i|
      if i * i > 10
        @count = i
        break
      end
    end
  end

  def view
    column(spacing: 8.0, padding: 12.0) {
      text "count=#{@count} total=#{@total} status=#{@status}"
      row(spacing: 6.0) {
        button("step") { step }
        button("tally") { tally }
        button("bump3") { bump3 }
        button("find") { find }
      }
    }
  end
end

run(Flow.new, title: "flow")
