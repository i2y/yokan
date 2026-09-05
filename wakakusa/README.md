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

## What runs today

One file, `demo/counter.rb`. Containers take their children as
arguments and a handler is a block at the leaf:

```ruby
require "wakakusa"

$count = 0

def view
  column(
    text("count: #{$count}", size: 34.0),
    row(
      button("+1") { $count += 1 },
      button("+10") { $count += 10 },
      button("reset") { $count = 0 },
      spacing: 8.0
    ),
    spacing: 12.0,
    padding: 16.0
  )
end

run("counter") { view }
```

Both runs open the same window (`screenshots/`), and driven headless
by `--script "click:+1,click:+10,dump"` they print the same three
lines:

```console
$ wakakusa gate demo/counter.rb --script "click:+1,click:+10,dump"
GATE OK — 3 dump lines identical in both runs
  script:   click:+1,click:+10,dump
  emitted:  demo/.gate/counter.c
  binary:   demo/.gate/counter (35.4 MB)
```

## The pieces

- `crates/pixie-capi` (in the substrate, not here) — the engine behind
  a C ABI. The interpreted run opens it as a shared library, the
  compiled run links the static one, and both then drive exactly the
  same code. Elements cross as integer handles, handlers as integer
  ids; the engine never holds a Ruby object.
- `door/cruby/wakakusa.rb`, `door/spinel/wakakusa.rb` — the two doors.
  Below the ABI, the declarations each run needs; above it, the same
  Ruby word for word, which the sweep checks before it gates anything.
- `bin/wakakusa` — `check`, `run`, `translate`, `build`, `gate`.
- `demo/` — the apps. `tools/gate_all.sh` — all of them, both runs.

## Numbers

Measured here, on macOS/arm64, with the shared build directory warm.

| what | value |
|---|---|
| the engine's crate, rebuilt after an edit | 2.6 s |
| the compiler's C output (door + app) | under 10 ms, 25 KB |
| `cc` link of the compiled run | 0.32 s |
| the compiled binary | 35.4 MB |
| one gate round, engine already built | 1.5 s |
| the engine, static / shared | 75.1 MB / 14.3 MB |

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

- Four elements of the engine's eighteen, and none of the shared
  properties: no styling, no theme, no text field.
- No live reload, no timers, no background work, and state lives in a
  global rather than an object.
- `check` accepts everything. The shapes the compiler gets wrong are
  known and reproduced, but nothing refuses them by name yet, so the
  gate is what catches them.
- macOS only. The compiled binary links the whole engine, so its size
  is the engine's.
