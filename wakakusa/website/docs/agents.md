# Building with an agent

An agent writes a file and reads what comes back. How the session goes
— how many turns it takes, whether the agent finds its own mistakes,
whether a person has to sit and watch — follows from what comes back.

Wakakusa's commands are shaped for that reading. Two of them start no
compiler and open no window, and each answers one question: is this
inside the vocabulary, what does it draw, and will the shipped app do
the same.

![The loop an agent works in: it writes app.rb at the centre of a ring, spins through wakakusa check and a headless dump in well under a second each, and leaves the ring for wakakusa gate, the compile that proves the shipped binary agrees, and then for ship](images/loop.svg#only-dark)

![The loop an agent works in: it writes app.rb at the centre of a ring, spins through wakakusa check and a headless dump in well under a second each, and leaves the ring for wakakusa gate, the compile that proves the shipped binary agrees, and then for ship](images/loop-light.svg#only-light)

## Three commands, three answers

### `wakakusa check` — is this inside the vocabulary?

```console
$ ./bin/wakakusa check app.rb
app.rb:5:19: Wakakusa cannot take this — `text` has no `weight:`. It takes a11y_label, align, animate, background, bold, border_color, border_radius, border_width, col_span, color, disabled, easing, enter, exit, grow, height, italic, max_lines, max_width, min_width, mono, padding, role, row_span, size, text, theme, tooltip, underline, width, wrap
    text("hello", weight: 700.0)
                  ^
```

It prints the refusal in `file:line:col` form with the line under it,
and says nothing at all when the app is inside the vocabulary. No
compiler is started, so the answer comes back in about a tenth of a
second.

The message is not "no": it is the repair, at the line where the repair
goes. [What Wakakusa refuses](refusals.md) is the whole list, each with
the code that triggers it.

### A headless run — what does it draw?

```console
$ PIXIE_SCRIPT="click:+1,dump" ./bin/wakakusa run app.rb
Column(spacing=12, padding=16)[Text(count: 0, fontSize=34), Row(spacing=8)[Button(+1), Button(+10), Button(reset)], TextField(), Text(hello, )]
Column(spacing=12, padding=16)[Text(count: 1, fontSize=34), Row(spacing=8)[Button(+1), Button(+10), Button(reset)], TextField(), Text(hello, )]
```

With `PIXIE_SCRIPT` set, `run` opens no window: it builds the tree,
drives it with the steps and prints the screen as text — once at the
start, once for every `dump`, once at the end. About seven tenths of a
second, again with no compiler.

This is the answer to "did the button do what I meant", and it is
readable without a screen. The step vocabulary is on
[Verify and ship](tour-ship.md#a-run-with-no-person-in-it).

For anything drawn on a canvas, `WAKAKUSA_FRAMES=<dir>` writes a PNG
after every step, painted by the same rasterizer the window uses — so
an agent can look at what it drew, not only read it.

### `wakakusa gate` — will the shipped app do the same?

```console
$ ./bin/wakakusa gate app.rb --script "click:+1,dump"
GATE OK — 3 dump lines identical in both runs
```

This one compiles, so it costs about two seconds. It is the proof at
the end of a piece of work, not the thing to run on every edit.

When it fails it says where:

```console
GATE FAILED — the two runs diverge:
  cruby:    Text("total: 3")
  compiled: Text("total: 0")
```

The `cruby:` line is the one that is right — that run is the real
interpreter. A divergence is a bug in the compiled run, and
[The two runs](two-runs.md) lists the handful of places where one is
already known.

## The loop, in one session

```console
$ ./bin/wakakusa check app.rb                       # refusal → fix it, in place
$ ./bin/wakakusa check app.rb                       # silence
$ PIXIE_SCRIPT="click:add,dump" ./bin/wakakusa run app.rb   # read the screen
$ ./bin/wakakusa gate app.rb --script "click:add,dump"      # both runs agree
```

The first three are under a second each, which is what makes them worth
running on every edit. The last one is the promise, and it is the one
to run before saying the work is done.

## What to hand an agent

Three pages, in this order:

- [First app](tour.md) and the rest of the tour — the language, in the
  order you meet it.
- [What Wakakusa refuses](refusals.md) — the shapes to write instead,
  with the exact message each one prints.
- [Elements](elements.md) — every element, every keyword, its type and
  its default, generated from the one table the engine is built from.

An agent that has read those writes the vocabulary correctly the first
time, instead of learning it from refusals one build at a time.

## Two things worth telling it

**A view only reads.** The commonest refusal by far is a write inside
`view`. The state changes in a handler, and the view is rebuilt from
what the state now says.

**A block on an element cannot be written inside a loop.** The rewrite
is always the same shape: a method that takes what the row needs,
called from the loop.
