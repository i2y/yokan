# data_table draws the table itself: the first row inside it is the
# header, every later row is a data row shaded in alternation, and the
# frame comes with the element. Columns line up because the cells of
# one column carry the same `grow` share.
require "wakakusa"

class Fleet
  def initialize
    @latency = { "api" => 42, "db" => 17, "cache" => 8, "edge" => 95 }
    @polls = 0
  end

  def refresh
    @polls += 1
    @latency["api"] = (@latency["api"] * 3 + 29) % 140
    @latency["db"] = (@latency["db"] * 5 + 11) % 140
    @latency["cache"] = (@latency["cache"] * 7 + 3) % 140
    @latency["edge"] = (@latency["edge"] * 2 + 47) % 140
  end

  def health(ms)
    label = "ok"
    label = "watch" if ms > 60
    label = "slow" if ms > 100
    label
  end

  def service_row(name)
    row(
      text(name, grow: 2.0),
      text("#{@latency[name]} ms", grow: 1.0, align: "right"),
      text(health(@latency[name]), grow: 1.0, align: "center"),
      spacing: 8.0
    )
  end

  def view
    column(
      text("fleet latency — #{@polls} polls", size: 16.0),
      data_table(
        row(
          text("service", grow: 2.0),
          text("latency", grow: 1.0, align: "right"),
          text("health", grow: 1.0, align: "center"),
          spacing: 8.0
        ),
        service_row("api"),
        service_row("db"),
        service_row("cache"),
        service_row("edge")
      ),
      button("refresh") { refresh },
      spacing: 10.0,
      padding: 14.0
    )
  end
end

run(Fleet.new, title: "table")
