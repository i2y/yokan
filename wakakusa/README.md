# Wakakusa

*The sentence that introduces Wakakusa is the owner's to write. A draft
to start from: **Wakakusa is a compiler for Ruby desktop apps: what you
run under CRuby is what it ships as a native binary, and each build
verifies it.***

Write ordinary Ruby against a small vocabulary of elements. While you
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
  binary:   demo/.gate/counter (15.0 MB)
```

## The vocabulary

Thirty-two elements: text and button, the fields and the four
choosers, the boxes that arrange (column, row, grid, stack, the panes
that scroll), the two charts, the two lists that build their rows on
demand, and the small pieces — spacer, divider, spinner, link,
progress, image, svg, modal.

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
- `demo/` — eighteen apps. `tools/gate_all.sh` — all of them, both
  runs.

## Numbers

Measured here, on macOS/arm64, with the shared build directory warm.

| what | value |
|---|---|
| the engine's crate, rebuilt after an edit | 2.6 s |
| the compiler's C output (the library and an app) | under 10 ms, 100 KB |
| `cc` link of the compiled run | 0.29 s |
| the compiled binary | 15.0 MB |
| one gate round, engine already built | 1.8 s |
| every demo, both runs | 31 s |

## How to start

macOS on Apple silicon, Ruby 4, Rust, and Xcode's Metal toolchain.

```console
$ export CARGO_TARGET_DIR=$HOME/.cache/pixie/target
$ just wakakusa-spinel                 # once per machine: fetch and build the compiler
$ just wakakusa-sweep                  # every demo, both runs
$ cd wakakusa
$ ./bin/wakakusa run demo/counter.rb   # a window
```

## What does not work yet

- No drawing surface: the canvas and its commands are not in the
  vocabulary, so neither are the two games.
- No live reload, and no way to do work off the window's thread. A
  timer works: `every(1.0) { … }` before `run`, ticking off the clock
  both runs share.
- Three shapes an app has to be written in, because the compiler
  cannot yet take the others, each refused by name with the rewrite in
  the message. State lives on the app object rather than in globals. A
  handler is a literal block, or a symbol naming one of the app's own
  methods — a proc handed through a keyword argument arrives broken.
  And a list grows by copying (`list.dup` then `push`) rather than by
  `list + [item]`.
- `check` names those three shapes and three more, with the line and
  what to write instead, but it does not yet see everything the
  compiler gets wrong; the gate is still what catches the rest.
- macOS only. The binary carries the engine it draws with, so even a
  small app weighs about 15 MB.
