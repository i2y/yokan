# Building with an agent

An agent writes a file and reads what comes back. How the session goes —
how many turns it takes, whether the agent finds its own mistakes,
whether a person has to sit and watch — follows from what comes back.

Rakugan's commands are shaped for that reading. Two of them start no
compiler and open no window, and each answers one question: can Rakugan
take this, what does it draw, and will the shipped app do the same.

![One session at the terminal: the agent writes app.pl, rakugan check refuses and names the fix, the agent fixes it, the same check answers with silence, a headless run prints the screen as text, and rakugan gate reports both runs drew the same screen — the two fast answers in 0.07 s each, the compile in 2.8 s](images/loop.svg#only-dark)

![One session at the terminal: the agent writes app.pl, rakugan check refuses and names the fix, the agent fixes it, the same check answers with silence, a headless run prints the screen as text, and rakugan gate reports both runs drew the same screen — the two fast answers in 0.07 s each, the compile in 2.8 s](images/loop-light.svg#only-light)

## Three commands, three answers

### `rakugan check` — can Rakugan take this?

```console
$ ./bin/rakugan check app.pl
app.pl:8:30: Rakugan cannot take this — `text` has no `weight =>`; it takes `a11y_label`, `align`, `animate`, `background`, `bold`, `border_color`, `border_radius`, `border_width`, `col_span`, `color`, `disabled`, `easing`, `enter`, `exit`, `grow`, `height`, `italic`, `max_lines`, `max_width`, `min_width`, `mono`, `padding`, `role`, `row_span`, `size`, `theme`, `tooltip`, `underline`, `width`, `wrap`
            return text("hello", weight => 700);
                                 ^
```

It prints the refusal in `file:line:col` form with the line under it and
a caret at the column, and says nothing at all when it can take the app.
perl checks the file first and the translator reads it second; no
compiler is started either way, so the answer comes back in about a
tenth of a second.

The message is not "no": it is the repair, at the place the repair goes.
[What Rakugan refuses](refusals.md) is the whole list, each quoted from
the file that holds its wording.

### A headless run — what does it draw?

```console
$ PIXIE_SCRIPT="click:+1,dump" ./bin/rakugan run app.pl
Column(spacing=12, padding=16)[Text(count: 0, fontSize=34), Row(spacing=8)[Button(+1), Button(+10), Button(reset)], TextField(), Text(hello, )]
Column(spacing=12, padding=16)[Text(count: 1, fontSize=34), Row(spacing=8)[Button(+1), Button(+10), Button(reset)], TextField(), Text(hello, )]
Column(spacing=12, padding=16)[Text(count: 1, fontSize=34), Row(spacing=8)[Button(+1), Button(+10), Button(reset)], TextField(), Text(hello, )]
```

With `PIXIE_SCRIPT` set, `run` opens no window: it builds the tree,
drives it with the steps and prints the screen as text — once at the
start, once for every `dump`, once at the end. Under a tenth of a
second, again with no compiler.

This is the answer to "did the button do what I meant", and it is
readable without a screen. The steps are listed under
[Headless runs and the gate](tour-ship.md#headless-runs-and-the-gate).

For anything drawn on a canvas, `PIXIE_FRAMES=<dir>` writes a PNG after
every step, painted by the same rasterizer the window uses, so an agent
can look at what it drew and not only read it.

### `rakugan gate` — will the shipped app do the same?

```console
$ ./bin/rakugan gate app.pl --script "click:+1,dump"
GATE OK — 3 dump lines identical in both runs
  script:   click:+1,dump
  emitted:  demo/.gate/app/src/main.pix
  binary:   ~/.cache/pixie/target/debug/main (53.8 MB)
```

This one compiles. It translates the app, builds the binary, runs both
with the one script and compares the transcripts byte for byte; a first
build of the engine takes minutes, and every one after that a few
seconds. A red gate prints the two transcripts and the first line where
they differ.

## The loop, in one session

1. Write the file.
2. `check` until it is silent. Every answer names the repair, so this
   costs turns rather than thought.
3. A headless run with a script that presses the things you added. Read
   the dump; it is the screen.
4. `gate` when the shape is right. That is the compile, and it is the
   proof.
5. `build --release --app` when it is done.

Steps 2 and 3 are the loop, and neither of them starts a compiler. Step
4 leaves the loop.

## What to hand an agent

- **The tour**, [starting here](tour.md). It is written in the order the
  language is met, and every complete example on it is gated by
  `tools/tour_check.pl`, so nothing on it is a shape the translator
  refuses.
- **[The elements page](elements.md)**, generated from the one table.
  Every keyword an app may write, with its type and default; a keyword
  not on it does not exist.
- **[What Rakugan refuses](refusals.md)**, so a refusal is recognised
  rather than worked around.
- **A demo close to the task**, from [the gallery](demos.md). Each one
  is a whole file, gated, and short.

## Two things worth telling it

**Read the refusal, do not route around it.** Every message names the
rewrite. An agent that treats a refusal as a wall will write something
stranger; an agent that reads it writes the intended Perl.

**A dump is a screen.** There is no need to ask a person whether the
window looks right until the tree says it is right. The dump is what the
gate compares, so an agent that reads dumps is looking at the same thing
the gate is.
