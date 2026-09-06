# Wakakusa language tour

Wakakusa turns a Ruby desktop app into one native binary. `wakakusa
build` takes your Ruby through [spinel](https://github.com/matz/spinel)
to C and links it with the drawing engine; while you are working, the
same file runs under CRuby instead. Whether the two are the same
program is something you check rather than hope: `wakakusa gate` drives
both with one interaction script and compares the screens they drew,
byte for byte. Neither half is Wakakusa's own — spinel
compiles, the engine draws — and what it adds is the join between them
and the check that they agree. The methods that build the screen —
`text`, `button`, `column` and thirty more — come with Wakakusa; the
app itself is a plain Ruby class. How much of Ruby you can write is what spinel takes,
minus the shapes under [What Wakakusa refuses](#what-wakakusa-refuses).

This page is the language, in the order you meet it. Everything in it
runs: `tools/tour_check.rb` pulls every complete example out of this
file and puts it through the same command a demo goes through, so a
rename in the vocabulary breaks this page before a reader meets it.
日本語版は [TOUR.ja.md](TOUR.ja.md).

## Table of contents

- [The smallest app](#the-smallest-app)
- [Holding state](#holding-state)
- [Writing views](#writing-views)
- [Control flow in a view](#control-flow-in-a-view)
- [Form controls](#form-controls)
- [Handlers](#handlers)
- [Lists, charts, and rows built on demand](#lists-charts-and-rows-built-on-demand)
- [The canvas](#the-canvas)
- [The keyboard](#the-keyboard)
- [The keywords every element takes](#the-keywords-every-element-takes)
- [Themes and animation](#themes-and-animation)
- [The window](#the-window)
- [Ruby's own standard library](#rubys-own-standard-library)
- [A database](#a-database)
- [Timers, work off the window's thread, and threads](#timers-work-off-the-windows-thread-and-threads)
- [While you are writing it](#while-you-are-writing-it)
- [Headless runs and the gate](#headless-runs-and-the-gate)
- [What Wakakusa refuses](#what-wakakusa-refuses)
- [Shipping](#shipping)
- [What does not work yet](#what-does-not-work-yet)

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

## Holding state

The state is the object's instance variables. A handler is a block,
and a block closes over the object, so it writes them the way any Ruby
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
things.

A method that answers an element is a piece of a screen, and calling
it is how a view is broken up.

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

## Control flow in a view

Inside a view the block is ordinary Ruby. `if`, `unless`, a ternary, a
loop, a method call, a local — all of it works, and what it writes
joins the tree where it stands.

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

The one shape that is refused is an element's own block written inside
a loop's block: a compiled run has lost the loop's variables by the
time it runs. That is why `line` above is a method — which is the
rewrite the refusal names.

## Form controls

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

Each of them takes the value it shows, and the block receives what
changed. Nothing is bound behind your back: the field shows `@name`
because you wrote `@name`, and the block is what puts it back.

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

## Lists, charts, and rows built on demand

A list with a hundred thousand rows is not a hundred thousand
elements. `list_view` and `table` take a count and a block that builds
row `i`, and only the rows on screen are ever built.

```ruby
  list_view(@rows.length, item_height: 26.0, height: 280.0) { |i| line(i) }

  table(["name", "size"], @files.length, widths: [3.0, 1.0],
        item_height: 22.0, height: 300.0,
        on_select: -> { @chosen = event_index }) { |i| cells(i) }

  bar_chart(@totals, labels: ["food", "transit", "fun"], axis: true, height: 90.0)
  line_chart(@series, min: 0.0, max: 100.0, height: 120.0)
```

The row block is called with the row number, and what it answers is
that row. It is called while the screen is being drawn, so it reads
state and never writes it.

## The canvas

`canvas` is a grid of virtual pixels, painted by the commands in its
block. Inside it a color is a NUMBER: the index of a color in the
palette the app declares. That is how tools for pixel art work, so
drawing code written for a pixel machine ports line for line with its
numbers unchanged.

<!-- script: advance:50,dump -->
```ruby
require "wakakusa"

PALETTE = ["#11111b", "#89b4fa", "#f38ba8", "#eeeeee", "#a6e3a1"].freeze

class Sky
  def initialize
    @frame = 0
  end

  def tick
    @frame += 1
  end

  def view
    canvas(64, 40, scale: 6, background: 0, palette: PALETTE) {
      rect(2, 2, 12, 6, 1)
      rect_outline(16, 2, 12, 6, 2)
      circle_outline(34, 5, 4, 3)
      line(2, 11, 61, 11, 2)
      triangle(3, 37, 8, 28, 13, 37, 4)
      circle(30, 18, 3, 3)
      pixel_text(2, 14, "FRAME #{@frame}", 3)
    }
  end
end

app = Sky.new
every(0.05) { app.tick }
run(app, title: "sky")
```

The commands are `pixel`, `line`, `rect`, `rect_outline`, `circle`,
`circle_outline`, `triangle`, `triangle_outline`, `sprite` and
`pixel_text`. They are not elements: nothing here can be clicked,
themed, sized or animated, and a loop inside the canvas is the
ordinary loop.

`sprite` copies a rectangle out of an image that shares the canvas's
palette, and `colkey` names the index that is not copied:

```ruby
  sprite(@px, @py, SHEET, 0, 0, 16, 16, colkey: SKY)
```

## The keyboard

A game asks what the hands are doing rather than waiting to be told.

```ruby
  def tick
    @x -= 2 if key_down("left")
    @x += 2 if key_down("right")
    fire if key_pressed("space")
    quit if key_pressed("q")
  end
```

`key_down` is "held right now", `key_pressed` is "went down since the
previous tick" (a held key answers once), and `key_released` is the
other edge. Read them in a timer, never in a view: a view that read
the keyboard would draw one thing in a window and another under a
script.

`demo/jump.rb` and `demo/shooter.rb` are two of Pyxel's own examples
(Takashi Kitao, MIT), ported to this vocabulary and gated.

A canvas can be looked at without a window. `WAKAKUSA_FRAMES=<dir>`
writes a PNG of the first canvas after every script step, drawn by the
same rasterizer the window uses, and `WAKAKUSA_FRAME_SCALE` draws the
grid bigger than the app asks. The dump says what a frame IS; this says
what it looks like, over ssh, in CI, or while the screen is locked.

```console
$ WAKAKUSA_FRAMES=frames PIXIE_SCRIPT="advance:34,advance:34" ./demo/.gate/jump
$ ls frames
0000.png  0001.png
```

## The keywords every element takes

Fifteen properties ride on every element under one name and one
meaning: `width`, `height`, `min_width`, `max_width`, `disabled`,
`theme`, `animate`, `easing`, `enter`, `exit`, `col_span`, `row_span`,
`role`, `a11y_label`, `tooltip`.

```ruby
  button("save", disabled: @busy, tooltip: "write the file", width: 120.0)
  text "total", role: "heading", a11y_label: "the running total"
```

An element that owns one of those names under its own meaning keeps
it: a `text`'s `width` is the text's, and the box leaves it alone.

## Themes and animation

The palette and any movement are keywords too.

```ruby
  column(theme: "dark") { ... }
  text "saved", animate: 150.0, easing: "out", enter: true
```

`theme:` swaps the palette under one part of the screen. `animate:` is
how many milliseconds a change takes and `easing:` is its shape
(`linear`, `in`, `out`, `inOut`), with `enter:` and `exit:` for what
appears and disappears.

## The window

Four things come from the window itself:

```ruby
shortcut("cmd+s") { app.save }
menu_item("File", "Open…") { app.open }
on_key { |chord| app.typed(chord) }
on_file_drop { |path| app.load(path) }
```

These are declared before `run`, and they live for as long as the app
does.

```ruby
  clipboard_set(@text)
  @text = clipboard_get

  job = task { open_dialog("choose a file") }
  on_done(job) { @path = task_answer }
```

A dialog waits for a person, so it belongs inside `task`. A headless
script answers one with a `file:<path>` step, which is what makes a
dialog a checked interaction like any other.

Sound is a file played and then forgotten:

```ruby
  audio_play("demo/assets/sound/blip.wav")         # as it was recorded
  audio_play("demo/assets/sound/blast.wav", 0.4)   # at a level, 0.0 to 1.0
  audio_stop
```

The call answers at once; nothing waits for the end of the sound. A run
under a script is silent — a gate must not need a machine with speakers
— and a machine with no audio device, or a file that cannot be read,
plays nothing rather than failing the app. WAV is what the engine
decodes. `demo/sound.rb` is the whole of it, and the two ported games
use it.

## Ruby's own standard library

Ruby's own library is in both runs — `File`, `Dir`, `JSON`, `CSV`,
`Time`, `Math`, `Net::HTTP`, sockets, threads, everything `Enumerable`
answers. There is no library of ours in front of it, and nothing to
learn twice.

```ruby
  require "json"

  def load
    @rows = JSON.parse(File.read(PATH))
  end
```

`demo/stdlib.rb`, `demo/files.rb` and `demo/reader.rb` are there to
hold both runs to it.

## A database

A database is the exception, because it is no use unless both runs
read the one file the same way. It reaches the same sqlite through the
engine.

```ruby
  sqlite_exec(DB, "CREATE TABLE IF NOT EXISTS notes(body TEXT)")
  sqlite_exec(DB, "INSERT INTO notes VALUES (?)", [@draft])
  rows = sqlite_rows(DB, "SELECT rowid, body FROM notes ORDER BY rowid")
  bodies = sqlite_column(DB, "SELECT body FROM notes")
```

Write `?` in the statement and put the values beside it: text a person
typed can never become part of the statement that way. Every value
comes back as text, and the column's affinity converts on the way in.

## Timers, work off the window's thread, and threads

Work you want repeated goes to `every`.

```ruby
app = Clock.new
every(1.0) { app.tick }
run(app, title: "clock")
```

A timer is declared before `run` and lives as long as the app. Both
runs tick off one clock: a frame in a window, an `advance:` step in a
script.

A handler that blocks freezes the window. Hand the slow work to
`task`, and write what to do when it is done in `on_done`.

```ruby
  def start
    job = task { something_slow }
    # This block runs on the window's thread once the work has
    # finished; `task_answer` inside it is what the work answered.
    on_done(job) { @answer = task_answer }
  end
```

Neither call waits. `task` starts the work and answers a number,
`on_done` only says what to do later; the handler ends and the window
carries on. Nothing inside the work may touch the app's state or the
screen — the handler is where that belongs, and it is why the answer
comes back through `on_done` rather than from the work itself.

Anything perpetual is an ordinary `Thread`, and what it produces
should be picked up by a timer rather than written into the app's
state from the worker. In the compiled run such a thread is on a core
of its own and the window keeps drawing; under CRuby it is one thread
at a time, so a long computation freezes the window while you are
writing the app and does not once you ship it.

## While you are writing it

`wakakusa run` watches the app's file. Save, and the window picks the
edit up: the file is read again, the class with it, and the object the
window is holding is an instance of that same class, so it answers
with the new `view` and keeps every value it had. `initialize` is not
run again on it, which is the point — that is where the state came
from. A file that does not parse leaves the window on what it had and
says so in the terminal.

Reading the file runs the bottom of it too, so the object made there
is a second one and it is dropped: an `initialize` that opens a
database or starts a thread does so once per save. What was declared
before `run` keeps what it was given when the window opened. A timer
goes on ticking at the period it had, and one added while the window
is open takes effect the next time you start the app.

## Headless runs and the gate

`PIXIE_SCRIPT` replaces the person. The engine builds the tree, drives
it with the steps, and prints the dumps:

```
click:<label>      press a button by the label it shows
input:<text>       type into a field   submit    press enter in it
slide / select     move a slider, pick an option
key:<chord>        a keystroke bound to a shortcut
keydown:<key> / keyup:<key>    hold a key down, let it up
menu:<item>        pick a menu item    file:<path>   answer a dialog
drop:<path>        a file dragged onto the window
advance:<ms>       move the clock      theme:dark|light
dump               print the tree      a11y   print what a reader reads
```

`wakakusa gate` runs the app twice with one script — CRuby through the
door, and the compiled binary through the same C ABI linked in — and
compares the two transcripts byte for byte.

```console
$ wakakusa gate demo/counter.rb --script "click:+1,dump,input:Momo"
GATE OK — 3 dump lines identical in both runs
```

The gate is the promise. Everything else in this page is a way of
writing something the gate can keep.

## What Wakakusa refuses

`wakakusa check` reads the app and names what it cannot take, with the
line and the rewrite. It runs before every build and every gate.

- State in a global written from inside a block. Put it on the app.
- A list grown with `list + [item]` on a field. Write `dup` then
  `push`: the two runs do not agree about what the first one answers.
- A handler given as anything but a literal block or a proc of no
  arguments through a keyword.
- An element's own block written inside a loop's block. Move it into a
  method that takes what it needs.

Each of those has a fixture under `test/refuse/` holding the message
it prints, so a refusal cannot quietly change its wording.

## Shipping

```console
$ wakakusa build demo/todo.rb --release --app
built: demo/.gate/todo (12.3 MB)
bundle: demo/dist/todo.app (12.5 MB)
```

`--release` drops the symbol table; `--app` wraps the binary in a
macOS application bundle, ad-hoc signed, with `<stem>.png` or
`<stem>.icns` beside the app as its icon. The binary carries the
engine and the compiled Ruby and links nothing but the system's own
libraries, so the bundle is the whole program: it opens on a machine
with neither Ruby nor the compiler installed.

## What does not work yet

- A seeded `Random` is not the same generator in the two runs, so a
  program that wants one sequence in both writes the generator itself.
  The two games do, in six lines of arithmetic.
- Three shapes an app has to be written in, listed under
  [What Wakakusa refuses](#what-wakakusa-refuses), because the
  compiler cannot yet take the others.
- `check` names those and five more, but it does not yet see
  everything the compiler gets wrong; the gate is still what catches
  the rest.
- macOS only. The binary carries the engine it draws with, so even a
  small app weighs about 12 MB.
