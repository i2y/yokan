# Views and control flow

Inside a view the block is ordinary Ruby. `if`, `unless`, a ternary, a
loop, a method call, a local — all of it works, and what it writes
joins the tree where it stands. There is no template language to learn,
because there is no template.

## Control flow in a view

<!-- script: dump,click:show,dump -->
```ruby
require "wakakusa"

class Control
  def initialize
    @open = false
    @rows = ["one", "two", "three"]
  end

  def line(s, i)
    row(spacing: 6.0) {
      text "#{i + 1}.", width: 22.0
      text s
    }
  end

  def view
    column(spacing: 8.0, padding: 14.0) {
      button(@open ? "hide" : "show") { @open = !@open }
      if @open
        @rows.each_with_index { |s, i| line(s, i) }
      else
        text "#{@rows.length} rows", color: "#8a8f98"
      end
      divider
      text "done"
    }
  end
end

run(Control.new, title: "control")
```

An `if` that writes nothing in one branch simply writes nothing: there
is no placeholder element and no `nil` to guard against. A method
called from the block writes into the container that is open, which is
why `line` above needs no argument saying where it goes.

## The one shape a view cannot take

An element's own block, written inside a loop's block, is refused: a
compiled run has lost the loop's variables by the time the element's
block runs.

```console
$ wakakusa check app.rb
app.rb:12:22: Wakakusa cannot take this — a block on an element cannot be written inside a loop: a compiled run has lost the loop's variables by the time it runs. Move this into a method that takes what it needs (`def line(item, i)`), and call that from the loop
        button(name) { @picked = i }
                     ^
```

That is why `line` in the example above is a method. The loop calls it
with what the row needs, and the button's block closes over the
method's own arguments instead of the loop's.

`demo/control.rb` is the whole story on one screen.

## Form controls

Each control takes the value it shows, and its block receives what
changed.

```ruby
  text_field(@name, placeholder: "name") { |s| @name = s }
  int_field(@qty, min: 0, max: 99) { |n| @qty = n }
  number_field(@rate, min: 0.0, max: 1.0, step: 0.05) { |v| @rate = v }
  checkbox("ready", checked: @ready) { |on| @ready = on }
  switch("dark", checked: @dark) { |on| @dark = on }
  slider(value: @vol, min: 0.0, max: 1.0) { |v| @vol = v }
  select(options: ["red", "green", "blue"], selected: @pick) { |i| @pick = i }
  radio_group(options: ["one", "two"], selected: @pick) { |i| @pick = i }
  segmented(options: ["day", "week"], selected: @span) { |i| @span = i }
  tab_bar(labels: ["files", "settings"], active: @tab) { |i| @tab = i }
```

Nothing is bound behind your back: the field shows `@name` because you
wrote `@name`, and the block is what puts it back.

The four choosers — `select`, `radio_group`, `segmented`, `tab_bar` —
all answer an index into the list they were given, so the app keeps
the list and the number rather than a copy of the chosen text.

`text_field` grows into a paragraph field with `multiline: true` and
`rows:`. The two numeric fields commit on enter or on leaving the
field, drop text that is not a number, clamp into `min`/`max` and snap
to `step`; `demo/quantities.rb` is that behaviour under a script.

## Handlers

An element's usual handler is its block. Where an element has a second
one, the block is already spoken for, so the other is written as a
keyword taking a proc of no arguments, which asks for what the event
carried:

```ruby
  text_field(@draft, on_submit: -> { add(event_text) }) { |s| @draft = s }
```

`event_text`, `event_number`, `event_index` and `event_on?` answer what
the event now being delivered carried. A proc handed through a keyword
is not given an argument in a compiled run, which is why it asks.

A handler is a block or such a proc, and nothing else — a symbol
naming a method is refused, with the rewrite in the message
([What Wakakusa refuses](refusals.md)).

Handlers may call the app's own methods freely, which is where anything
longer than a line belongs:

<!-- script: input:eggs,submit,dump,click:done,dump -->
```ruby
require "wakakusa"

class Todo
  def initialize
    @items = ["milk"]
    @draft = ""
    @done = -1
  end

  def add(t)
    items = @items.dup
    items.push(t)
    @items = items
    @draft = ""
  end

  def line(i)
    cells = [text("#{i + 1}. #{@items[i]}")]
    cells.push(text("done", color: "accent")) if i == @done
    cells.push(button("done") { @done = i })
    row(*cells, spacing: 8.0)
  end

  def view
    column(
      text("todo — #{@items.length} items", size: 16.0),
      text_field(@draft, placeholder: "add and press enter",
                 on_submit: -> { add(event_text) }) { |t| @draft = t },
      list_view(@items.length, item_height: 26.0, height: 280.0) { |i| line(i) },
      button("clear") { @items = [] },
      spacing: 10.0,
      padding: 14.0
    )
  end
end

run(Todo.new, title: "todo")
```

Note how `add` grows the list: it copies with `dup`, pushes, and puts
the copy back. Growing a list with `@items + [t]` is refused, because
the two runs do not agree about what that answers on a field.

## Rows built on demand

A list with a hundred thousand rows is not a hundred thousand elements.
`list_view` and `table` take a count and a block that builds row `i`,
and only the rows on screen are ever built.

```ruby
  list_view(@rows.length, item_height: 26.0, height: 280.0) { |i| line(i) }

  table(["name", "team", "score"], @names.length,
        widths: [2.0, 1.0, 1.0], height: 220.0,
        selected: @sel, sort: @sort, descending: @desc,
        on_select: -> { pick(event_index) },
        on_sort: -> { sort_by(event_index) }) { |i| line(i) }
```

The row block is called with the row number, and what it answers is
that row. It is called while the screen is being drawn, so it reads
state and never writes it.

`table` draws the header itself and lays the header and the rows on the
same tracks, whose shares are `widths`. Which row is highlighted
(`selected:`), which column carries the sort marker (`sort:`) and which
way it points (`descending:`) are values the app holds — the element
draws them, and the sorting itself is the app's own method.
`demo/roster.rb` sorts twenty-four people that way; `demo/csv_viewer.rb`
filters a hundred thousand rows as you type.

Where the rows are few and already in hand, `data_table` takes them as
children instead: its first `row` is the header, the later ones are
data rows shaded in alternation, and the frame comes with the element.

```ruby
  data_table(
    row(text("service", grow: 2.0), text("latency", grow: 1.0, align: "right")),
    row(text("api", grow: 2.0), text("42 ms", grow: 1.0, align: "right"))
  )
```

Columns line up because the cells of one column carry the same `grow`
share.

## Charts

Two charts take a list of numbers as their first argument, so a view
that computes the numbers reads in order.

```ruby
  bar_chart(@totals, labels: ["food", "transit", "fun"], axis: true, height: 90.0)
  line_chart(@series, min: 0.0, max: 100.0, height: 120.0)
```

`min:` and `max:` both zero take the range from the data, and a chart
whose values go below zero hangs those bars under the line. `axis:`
draws the tick labels and a faint gridline at each. `series:` takes one
list per line or per group, with `colors:` one color each —
`demo/charts.rb` is both of those on one screen.

## Where next

- [The canvas and the keyboard](tour-canvas.md) — the drawing surface,
  and the keys read as a device.
- [Look, and the window](tour-ui.md) — the keywords every element
  takes, themes, and what the window itself offers.
