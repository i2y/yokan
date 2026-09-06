# First app

Wakakusa turns a Ruby desktop app into one native binary. `wakakusa
build` takes your Ruby through [spinel](https://github.com/matz/spinel)
to C and links it with the drawing engine; while you are working, the
same file runs under CRuby instead, and each build verifies that the
two are the same program. Neither half is Wakakusa's own — spinel
compiles, the engine draws — and what it adds is the join between them
and the check that they agree. The methods that build the screen —
`text`, `button`, `column` and thirty more — come with Wakakusa; the
app itself is a plain Ruby class. How much of Ruby you can write is set out in
[Three Rubys, nested](two-runs.md#three-rubys-nested).

This tour is one pass over how apps are written, in the order you meet
it. Everything in it runs: `tools/tour_check.rb` pulls every complete
example out of these pages and puts it through the same command a demo
goes through, so a rename in the vocabulary breaks the page before a
reader meets it. What Wakakusa cannot do yet is collected, with
reasons, in [What does not work yet](tour-ship.md#what-does-not-work-yet).

## The smallest app

An app is an object with a `view` method that answers one element, and
`run` opens a window on it.

<!-- script: dump -->
```ruby
require "wakakusa"

class Hello
  def view
    text "hello", size: 28.0
  end
end

run(Hello.new, title: "hello")
```

```console
$ wakakusa run demo/hello.rb      # a window, under CRuby
$ wakakusa gate demo/hello.rb --script "dump"
GATE OK — 1 dump line identical in both runs
```

There is no framework object to inherit from and no method to
register. `run` takes the object, and everything it needs from it is
one method called `view`.

`run` also takes the window: `title:` names it, `width:` and `height:`
open it at a size (0.0 leaves the default), and `padding:` is the
margin around the whole screen (-1.0 leaves the default, 0.0 is the
edge, which is what a canvas wants).

```ruby
run(Game.new, title: "Pyxel Jump", width: 640.0, height: 480.0, padding: 0.0)
```

## Holding state

The state is the object's instance variables. A handler is a block, and
a block closes over the object, so it writes them the way any Ruby
method would. After a handler the whole view is built again from what
the state now says.

<!-- script: click:+1,click:+1,dump,input:Momo,dump -->
```ruby
require "wakakusa"

class Counter
  def initialize
    @count = 0
    @name = ""
  end

  def view
    column(
      text("count: #{@count}", size: 34.0),
      row(
        button("+1") { @count += 1 },
        button("+10") { @count += 10 },
        button("reset") { @count = 0 },
        spacing: 8.0
      ),
      text_field(@name, placeholder: "your name") { |s| @name = s },
      text("hello, #{@name}"),
      spacing: 12.0,
      padding: 16.0
    )
  end
end

run(Counter.new, title: "counter")
```

There is no separate store, no observation to declare, and nothing to
mark as a field. An app is an object; `initialize` is where its state
starts.

Nothing is bound behind your back either. The field shows `@name`
because you wrote `@name`, and the block is what puts it back — which
is why the screen and the state can never be two different stories.

## Writing views

A container takes its children as arguments, or as a block. Both build
the same tree, so which one to write is a question of how the screen
reads.

<!-- script: click:+1,dump -->
```ruby
require "wakakusa"

class Two
  def initialize
    @count = 0
  end

  def view
    column(spacing: 12.0, padding: 16.0) {
      text "count: #{@count}", size: 34.0
      row(spacing: 8.0) {
        button("+1") { @count += 1 }
        button("reset") { @count = 0 }
      }
    }
  end
end

run(Two.new, title: "two")
```

The block form nests without commas and reads down the page, which
suits a screen with structure. The argument form suits a row of three
things. `demo/counter.rb` and `demo/blockform.rb` are the same screen
in the two spellings, and the sweep gates both.

Inside a container's block, an element joins the container that is
open. That is the whole rule: `text` written inside `column`'s block is
a child of that column, and a `row` opened inside it collects what is
written inside it in turn.

## A piece of a screen is a method

A method that answers an element is a piece of a screen, and calling it
is how a view is broken up.

```ruby
  def field(label, value)
    row(spacing: 6.0) {
      text label, width: 90.0
      text value, bold: true
    }
  end

  def view
    column(spacing: 4.0, padding: 14.0) {
      field("name", @name)
      field("size", @size)
    }
  end
```

There is nothing to declare: a method that answers an element is one,
and one that takes elements as arguments wraps them.

```ruby
  def card(title, *kids)
    column(
      text(title, size: 18.0),
      *kids,
      spacing: 4.0, padding: 8.0,
      border_width: 1.0, border_color: "accent", border_radius: 8.0
    )
  end
```

The children arrive as an ordinary Ruby splat and go straight into the
container's arguments, so a wrapper is a method like any other.

`demo/cards.rb` is that pattern on a screen.

## A view only reads

A view is called to answer what the screen looks like now, and it can
be called again at any time. So it reads the state and never writes it:

```console
$ wakakusa check app.rb
app.rb:9:5: Wakakusa cannot take this — a view only reads. Move the write into a handler — the block on a button, or a method the app calls from one
    @seen = @seen + 1
    ^
```

The rewrite is in the message: the write belongs in a handler, or in a
method the handler calls. [What Wakakusa refuses](refusals.md) is the
whole list, each with the line it prints.

## Where next

- [Views and control flow](tour-logic.md) — `if` and loops inside a
  view, the form controls, handlers, and rows built on demand.
- [The canvas and the keyboard](tour-canvas.md) — a grid of virtual
  pixels, and the keys read as a device.
- [Verify and ship](tour-ship.md) — the gate, and the one binary.
