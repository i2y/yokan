# Wakakusa

**Wakakusa is a compiler for Ruby desktop apps: what you run under
CRuby is what it ships as a native binary, and each build verifies
it.**

Build the screen out of elements — `text`, `button`, `column` and
thirty more — and write ordinary Ruby for everything else. While you
are working, the program runs under CRuby, so it is the real
interpreter answering. When you ship, the whole program becomes one
native binary. The gate runs both, drives them with the same
interaction script, and compares what they drew, byte for byte — so
"it worked while I was writing it" and "it works as shipped" are one
claim, not two.

## What an app looks like

An app is an object. Its state is its instance variables, `view`
answers one element, and a handler is a block that closes over it.

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

```console
$ wakakusa gate demo/counter.rb --script "click:+1,dump,input:Momo"
GATE OK — 3 dump lines identical in both runs
  script:   click:+1,dump,input:Momo
  emitted:  demo/.gate/counter.c
  binary:   demo/.gate/counter (16.2 MB)
```

Children can be written the other way round, as the container's block,
which is closer to how a Ruby library would usually put it. Both
spellings build the same tree — `demo/counter.rb` and
`demo/blockform.rb` are the same screen, and the sweep gates both.

```ruby
  def view
    column(spacing: 12.0, padding: 16.0) {
      text "count: #{@count}", size: 34.0
      row(spacing: 8.0) {
        button("+1") { @count += 1 }
        button("reset") { @count = 0 }
      }
    }
  end
```

Inside a view the block is ordinary Ruby: `if`, `unless`, a ternary, a
loop, a method that answers part of the screen. The one shape that is
refused is a block written inside a loop's block — a compiled run has
lost the loop's variables by then — and `check` says so with the line
and the rewrite. `demo/control.rb` is the whole story in one screen.

## The vocabulary

Thirty-three elements: text and button, the fields and the four
choosers, the boxes that arrange (column, row, grid, stack, the panes
that scroll), the two charts, the two lists that build their rows on
demand, the small pieces — spacer, divider, spinner, link, progress,
image, svg, modal — and the canvas.

They are written once, in `elements.toml`: every element, every
keyword it takes, its type and its default. `tools/gen.rb` turns that
one table into the Ruby an app calls, the numbers the two sides of the
engine's C face count with, and the engine's own constants — so an
element cannot come to mean one thing in Ruby and another where it is
drawn. Adding one is a row in the table and an arm in the engine.

Fifteen properties ride on every element under one name and one
meaning: `width`, `height`, `min_width`, `max_width`, `disabled`,
`theme`, `animate`, `easing`, `enter`, `exit`, `col_span`, `row_span`,
`role`, `a11y_label`, `tooltip`.

## Drawing, and the games

`canvas` is a grid of virtual pixels painted by the commands in its
block. Inside it a color is a number, the index of a color in the
palette the app declares, which is what lets drawing code written for a
pixel machine port line for line with its numbers unchanged.

```ruby
canvas(64, 40, scale: 6, background: 0, palette: PALETTE) {
  rect(2, 2, 12, 6, 1)
  circle(@ball_x, @ball_y, 3, 3)
  pixel_text(2, 14, "FRAME #{@frame}", 3)
}
```

The commands are not elements: nothing here can be clicked, themed,
sized or animated, and a loop inside the canvas is the ordinary loop.
A game asks what the hands are doing rather than waiting to be told, so
`key_down`, `key_pressed` and `key_released` answer that — in a timer,
never in a view.

`demo/jump.rb` and `demo/shooter.rb` are two of Pyxel's own examples
(Takashi Kitao, MIT), ported and gated. Both draw with sprites cut from
the example's own image bank.

A canvas can be looked at without a window: `WAKAKUSA_FRAMES=<dir>`
writes a PNG of it after every script step, drawn by the same
rasterizer the window uses. The two GIFs in `demo/screenshots/` were
recorded that way, from a script that plays the game.

## While you are writing it

`wakakusa run` watches the app's file. Save, and the window picks the
edit up: the file is read again, the class with it, and the object the
window is holding is an instance of that same class, so it answers
with the new `view` and keeps every value it had. A file that does not
parse leaves the window on what it had and says so in the terminal.

`initialize` is not run again on that object, which is the point —
that is where the state came from. Reading the file does run the
bottom of it again, and the object made there is a second one that is
dropped, so an `initialize` that opens a database or starts a thread
does so once per save. A timer keeps the one it was given, and one
added while the window is open takes effect the next time you start
the app.

```ruby
every(1.0) { app.tick }   # before `run`; both runs tick off one clock

# Neither call waits: `task` starts the work and answers a number,
# `on_done` only says what to do later. The handler ends here and the
# window carries on; the block runs on the window's thread once the
# work has finished.
job = task { something_slow }
on_done(job) { @answer = task_answer }
```

Ruby's own standard library is in both runs — `File`, `Dir`, `JSON`,
`CSV`, `Time`, `Math`, `Net::HTTP`, sockets, threads, everything
`Enumerable` answers — so there is no library of ours in front of it.
`demo/stdlib.rb`, `demo/files.rb` and `demo/reader.rb` are there to
hold it to that.

A database is the exception. It reaches the same sqlite through the
engine, because a database is no use unless both runs read the one
file the same way:

```ruby
sqlite_exec(DB, "INSERT INTO expenses VALUES (?, ?, ?)", [name, yen, cat])
rows = sqlite_rows(DB, "SELECT name, amount FROM expenses ORDER BY rowid")
```

## When you ship it

`wakakusa build --release --app` writes a macOS application bundle
beside the app. The binary carries the engine and the compiled Ruby and
links nothing but the system's own libraries, so the bundle is the whole
program: it opens on a machine with neither Ruby nor the compiler
installed. A `<stem>.png` or `<stem>.icns` next to the app becomes its
icon.

```console
$ wakakusa build demo/todo.rb --release --app
built: demo/.gate/todo (12.3 MB)
bundle: demo/dist/todo.app (12.5 MB)
```

## The pieces

- `crates/pixie-capi` (in the substrate, not here) — the engine behind
  a C ABI. The interpreted run opens it as a shared library, the
  compiled run links the static one, and both then drive exactly the
  same code. An element is opened, written into by number and closed;
  handlers are numbers the door hands out. The engine never holds a
  Ruby object.
- `lib/` — the same Ruby in both runs: the generated element methods,
  and the registries a build starts over.
- `door/cruby/`, `door/spinel/` — one file each, holding the ABI
  declarations that run needs. One line differs between them.
- `bin/wakakusa` — `check`, `run`, `translate`, `build`, `gate`.
- `demo/` — forty-three apps, with `demo/screenshots/` showing what
  each one draws. `tools/gate_all.sh` — all of them, both runs.

## Numbers

Measured here, on macOS/arm64, with the shared build directory warm.

| what | value |
|---|---|
| the engine's crate, rebuilt after an edit | 2.7 s |
| the compiler's C output (the library and an app) | under 10 ms, 100 KB |
| `cc` link of the compiled run | 0.29 s |
| the compiled binary | 16.2 MB |
| the shipped binary (`--release`) | 12.3 MB |
| the application bundle | 12.5 MB |
| launch to a window on screen | under 0.3 s |
| one gate round, engine already built | 2.1 s |
| every demo, both runs, and the tour | 1 min 53 s |

## How to start

macOS on Apple silicon, Ruby 4, Rust, and Xcode's Metal toolchain.

```console
$ export CARGO_TARGET_DIR=$HOME/.cache/pixie/target
$ just wakakusa-spinel                 # once per machine: fetch and build the compiler
$ just wakakusa-sweep                  # every demo, both runs
$ cd wakakusa
$ ./bin/wakakusa run demo/counter.rb   # a window
```

The [language tour](TOUR.md) is the whole language in the order you
meet it, and every example in it runs. 日本語版は
[TOUR.ja.md](TOUR.ja.md).

`website/` is the documentation site, in both languages: the tour split
into six pages, a vocabulary reference generated from `elements.toml`,
the two runs set out side by side, every refusal with the message it
prints, and all forty-three demos with their screenshots and their
source. `just wakakusa-site-serve` builds it and serves both.

## What does not work yet

- No sound. The engine has no audio verb, which is why the two games
  are silent where their originals are not.
- A seeded `Random` is not the same generator in the two runs, so a
  program that wants one number sequence in both writes the generator
  itself. The two games do, in six lines of arithmetic.
- A thread the app starts for its own reasons runs, but how far it gets
  is not something the two runs agree about: one is on the clock the
  machine keeps and the other on the clock a script sets. That is why
  `task` exists — the engine waits for the answer, so both runs reach
  the same place before the next step. Anything perpetual (a poller, a
  watcher) is an ordinary `Thread`, and what it produces should be
  picked up by a timer rather than written into the app's state from
  the worker.
- The two runs also differ in how much a thread costs the window, and
  the shipped one is the better half. Two seconds of arithmetic in each
  of two threads takes two seconds in the compiled run — they are on
  two cores, and the window goes on drawing at its usual rate. The same
  app under CRuby takes twice that, because the interpreter runs one
  thread at a time, and the window stops until the work is done. So a
  long computation can look worse while you are writing the app than it
  will when you ship it.
- Three shapes an app has to be written in, because the compiler
  cannot yet take the others, each refused by name with the rewrite in
  the message. State lives on the app object rather than in globals. A
  handler is a literal block. The second handler on an element, where
  the block is already spoken for, is a proc of no arguments that asks
  for what the event carried (`on_submit: -> { add(event_text) }`) — a
  proc given through a keyword is not handed anything in a compiled
  run.
  And a list grows by copying (`list.dup` then `push`) rather than by
  `list + [item]`.
- `check` names those three shapes and five more, with the line and
  what to write instead, but it does not yet see everything the
  compiler gets wrong; the gate is still what catches the rest.
- macOS only. The binary carries the engine it draws with, so even a
  small app weighs about 12 MB.
