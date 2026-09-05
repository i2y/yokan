# The controls a person changes: a box, a switch, a track, and the four
# choosers. Each hands its new value to the block.
require "wakakusa"

class Forms
  def initialize
    @dark = false
    @wifi = true
    @volume = 5.0
    @fruits = ["apple", "banana", "cherry"]
    @fruit = 0
    @sizes = ["small", "medium", "large"]
    @size = 1
    @tabs = ["general", "details", "about"]
    @tab = 0
    @note = ""
  end

  def panel
    return text("general panel", size: 12.0) if @tab == 0
    return text("details panel", size: 12.0) if @tab == 1

    text("about panel", size: 12.0)
  end

  def view
    column(
      checkbox("Dark mode", checked: @dark,
               tooltip: "the whole window follows this") { |on| @dark = on },
      switch("Wi-Fi", checked: @wifi) { |on| @wifi = on },
      slider(value: @volume, min: 0.0, max: 10.0, step: 1.0,
             tooltip: "0 to 10, in whole steps") { |v| @volume = v },
      select(options: @fruits, selected: @fruit) { |i| @fruit = i },
      radio_group(options: @sizes, selected: @size) { |i| @size = i },
      tab_bar(labels: @tabs, active: @tab) { |i| @tab = i },
      panel,
      text("volume #{@volume}, #{@fruits[@fruit]}, #{@sizes[@size]}", size: 12.0),
      text_field(@note, placeholder: "a note") { |t| @note = t },
      spacing: 10.0,
      padding: 14.0,
      theme: @dark ? "dark" : "light"
    )
  end
end

run(Forms.new, title: "forms")
