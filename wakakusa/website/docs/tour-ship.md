# Verify and ship

The gate is the promise. Everything else in this tour is a way of
writing something the gate can keep.

## A run with no person in it

`PIXIE_SCRIPT` replaces the person. The engine builds the tree, drives
it with the steps, and prints what the steps asked for.

| Step | What it does |
|---|---|
| `click:<label>` | press a button by the label it shows |
| `input:<text>` | type into a field |
| `submit` | press enter in it |
| `slide:<n>` | move a slider |
| `select:<option>` | pick an option in a chooser |
| `key:<chord>` | a keystroke bound to a shortcut |
| `keydown:<key>` / `keyup:<key>` | hold a key down, let it up |
| `menu:<item>` | pick a menu item |
| `file:<path>` | answer a file dialog |
| `drop:<path>` | a file dragged onto the window |
| `advance:<ms>` | move the clock |
| `theme:dark` / `theme:light` | switch the palette |
| `dump` | print the element tree |
| `a11y` | print what a screen reader is handed |
| `mem` | print how many objects the engine is holding |

Where several elements answer the same description, `@n` picks one:
`click@1:done` presses the second button labelled `done`, and
`input@0:` types into the first field. Steps are separated by commas,
and a comma inside a step's own text is written `\,`.

`dump` is the interesting one. It prints the element tree as text —
the same bytes a run prints at its start and end — so a script that
dumps in the middle is comparing the middle of a run, not only its
ending.

## The gate

`wakakusa gate` runs the app twice with one script — CRuby through the
door, and the compiled binary through the same C ABI linked in — and
compares the two transcripts byte for byte.

```console
$ wakakusa gate demo/counter.rb --script "click:+1,dump,input:Momo"
GATE OK — 3 dump lines identical in both runs
  script:   click:+1,dump,input:Momo
  emitted:  demo/.gate/counter.c
  binary:   demo/.gate/counter (16.2 MB)
```

When they differ, it says where, line by line:

```console
GATE FAILED — the two runs diverge:
  cruby:    Text("total: 3")
  compiled: Text("total: 0")
```

An app that keeps a file or a database has to start both runs from the
same nothing, which is what `--fresh` is for. It deletes the path
before each run, and it is repeatable:

```console
$ wakakusa gate demo/ledger.rb --fresh demo/.gate/ledger.db \
    --script "click:reset,input@0:o'brien,input@1:250,click:food,dump"
```

`wakakusa check` runs before every gate and every build, so a refusal
arrives before a compiler is ever started.
[What Wakakusa refuses](refusals.md) is that list.

`tools/gate_all.sh` is the sweep: every demo, then every complete
example in both language tours and on this site. It is what a change to
the vocabulary, the door or the engine has to pass.

## What the gate proves, and what it does not

It proves the two runs of *your app* draw the same screens under the
steps you gave it. It does not prove the steps covered the app — an
interaction nobody scripted is an interaction nobody compared.

What it does not have to prove is that the interpreted run is right
about Ruby: that run *is* CRuby. So a green gate means the compiled
run — a second implementation of Ruby, with a standard library of its
own — agrees with the real interpreter on the paths the script walked,
and where they part, CRuby is the side that is right by definition.
The few places where the two are not the same program are named on
[The two runs](two-runs.md).

## Shipping

```console
$ wakakusa build demo/todo.rb --release --app
built: demo/.gate/todo (12.3 MB)
bundle: demo/dist/todo.app (12.5 MB)
  not gate-checked — `gate` with a script proves the two runs agree
```

`--release` drops the symbol table, which is a fifth of the file.
`--app` wraps the binary in a macOS application bundle under `dist/`,
ad-hoc signed and double-clickable, with `<stem>.icns` or `<stem>.png`
beside the app as its icon.

The binary carries the engine and the compiled Ruby and links nothing
but the system's own libraries, so the bundle is the whole program: it
opens on a machine with neither Ruby nor the compiler installed.

`wakakusa translate` writes the C the compiled run is built from, at
any point along the way — it is the same file the gate keeps beside
the binary.

## The numbers

Measured on macOS/arm64, with the shared build directory warm.

| What | Value |
|---|---|
| `wakakusa check` | 0.1 s |
| a headless run that prints the screen | 0.7 s |
| the compiler's C output | under 10 ms, about 120 KB |
| `cc` link of the compiled run | 0.29 s |
| the compiled binary | 16.2 MB |
| the shipped binary (`--release`) | 12.3 MB |
| the application bundle (`--app`) | 12.5 MB |
| launch to a window on screen | under 0.3 s |
| one gate round, engine already built | 2.1 s |

## What does not work yet

- **No sound.** The engine has no audio verb, which is why the two
  ported games are silent where their originals are not.
- **A seeded `Random` is not the same generator in the two runs**, so a
  program that wants one sequence in both writes the generator itself.
  The two games do, in six lines of arithmetic.
- **Three shapes an app has to be written in**, because the compiler
  cannot yet take the others: state on the app object rather than in
  globals, a handler as a literal block (or a proc of no arguments
  through a keyword), and a list grown by copying rather than by
  `list + [item]`. Each is refused by name, with the rewrite in the
  message — [What Wakakusa refuses](refusals.md).
- **`check` does not see everything the compiler gets wrong.** It names
  those three and five more; the gate is still what catches the rest.
- **A thread the app starts for itself** runs in both, but how far it
  gets by a given moment is not something the two runs agree about —
  which is what `task` is for.
- **macOS on Apple silicon only.** The binary carries the engine it
  draws with, so even a small app weighs about 12 MB.
