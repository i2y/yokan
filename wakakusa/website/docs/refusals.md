# What Wakakusa refuses

`wakakusa check` reads the app and names what it cannot take, with the
line, a caret under it, and what to write instead. It starts no
compiler and opens no window, so the answer comes back in about a
second, and it runs before every build and every gate.

```console
$ wakakusa check demo/counter.rb        # silence: the app is inside the dialect
```

Every rule here stands for something the compiled run gets wrong: a
shape that either fails to build, or — worse — builds and then quietly
behaves differently from the interpreted one. The gate catches those
too, but only after a build; a refusal catches them while the app is
still being written.

Each message below is the one `check` actually prints. A fixture under
`test/refuse/` holds every one of them, and `tools/refuse_test.sh`
fails when the wording drifts, so a refusal cannot quietly become a
different sentence.

## A view that writes

```ruby
  def view
    @seen = @seen + 1
    text("seen #{@seen}")
  end
```

```
app.rb:9:5: Wakakusa cannot take this — a view only reads. Move the write into a handler — the block on a button, or a method the app calls from one
    @seen = @seen + 1
    ^
```

A view is called to answer what the screen looks like now, and it can
be called again at any time; a view that changed something would change
it a different number of times in a window and under a script. The same
refusal covers a write inside a container's block, which is still the
view being written.

**Write instead:** the change in a handler — the block on a button, or
a method the app calls from one.

## A block on an element, inside a loop

```ruby
  def view
    column(spacing: 8.0) {
      @items.each_with_index do |name, i|
        button(name) { @picked = i }
      end
    }
  end
```

```
app.rb:12:22: Wakakusa cannot take this — a block on an element cannot be written inside a loop: a compiled run has lost the loop's variables by the time it runs. Move this into a method that takes what it needs (`def line(item, i)`), and call that from the loop
        button(name) { @picked = i }
                     ^
```

The loop's `name` and `i` are gone by the time the button's block is
pressed, in a compiled run.

**Write instead:** a method that takes what the row needs, called from
the loop. The button's block then closes over the method's own
arguments.

```ruby
  def line(name, i)
    button(name) { @picked = i }
  end

  def view
    column(spacing: 8.0) {
      @items.each_with_index { |name, i| line(name, i) }
    }
  end
```

## A global written from inside a block

```ruby
$name = ""

class App
  def view
    text_field($name) { |s| $name = s }
  end
end
```

```
app.rb:7:29: Wakakusa cannot take this — `$name` is a global, and a compiled run cannot write one from inside a block. The app's state belongs on the app: write `@name` in a class with a `view` method, and hand it to `run`
    text_field($name) { |s| $name = s }
                            ^
```

A compiled run keeps the type the global's first value gave it, and has
nothing to convert the block's argument to.

**Write instead:** the state on the app object — `@name` in a class
with a `view` method, handed to `run`. Which is where an app's state
belongs anyway.

## A handler that is not a block or a written-out proc

```ruby
  def view
    button("+1", on_click: :bump)
  end
```

```
app.rb:9:18: Wakakusa cannot take this — `on_click:` takes a proc of no arguments, written out here (`on_click: -> { add(event_text) }`), or leave it out and write the block instead. A proc given through a keyword is not handed what the event carried in a compiled run, so it asks for it
    button("+1", on_click: :bump)
                 ^
```

A proc handed through a keyword reaches the compiled run as its
address, and would be called with a number where a string was meant.

**Write instead:** the block, which is where an element's usual handler
goes, or — for the second handler on an element whose block is already
spoken for — a proc of no arguments that asks for what the event
carried:

```ruby
  text_field(@draft, on_submit: -> { add(event_text) }) { |s| @draft = s }
```

## A list grown with `+`

```ruby
  def add(t)
    @items = @items + [t]
  end
```

```
app.rb:9:5: Wakakusa cannot take this — growing a list with `+ [...]` answers something different in a compiled run. Copy it and push: `items = @items.dup`, `items.push(...)`, `@items = items`
    @items = @items + [t]
    ^
```

The shorthand is refused in its own words:

```
app.rb:9:5: Wakakusa cannot take this — growing a list with `+= [...]` answers something different in a compiled run. Copy it and push: `items = @items.dup`, `items.push(...)`, `@items = items`
    @items += [t]
    ^
```

**Write instead:** copy, push, put back — which is what the message
spells out, with your own field's name in it:

```ruby
  def add(t)
    items = @items.dup
    items.push(t)
    @items = items
  end
```

## A keyword an element does not take

```ruby
  def view
    text("hello", weight: 700.0)
  end
```

```
app.rb:5:19: Wakakusa cannot take this — `text` has no `weight:`. It takes a11y_label, align, animate, background, bold, border_color, border_radius, border_width, col_span, color, disabled, easing, enter, exit, grow, height, italic, max_lines, max_width, min_width, mono, padding, role, row_span, size, text, theme, tooltip, underline, width, wrap
    text("hello", weight: 700.0)
                  ^
```

The list in the message is generated from `elements.toml`, the same
table the Ruby methods and the engine's constants are written from — so
the refusal cannot name a keyword the element does not really have.
[Elements](elements.md) is that table, laid out.

**Write instead:** whichever of the listed keywords means what you
wanted — `bold: true`, here.

## What `check` does not catch yet

It names those shapes and no others. It does not yet see everything the
compiler gets wrong, so the gate is still what catches the rest — which
is the reason a build is not finished until a script has been through
both runs.

When a refusal is a limit rather than a design, it is listed under
[What does not work yet](tour-ship.md#what-does-not-work-yet), and the
rule goes away together with the reason it existed.
