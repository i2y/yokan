# The two runs

An app is one file. It is run two ways, and the whole point of Rakugan
is that the two are the same program. This page is what each run is made
of, what they share, and the few places they are not the same.

**Only one of the two is Perl.** While you write, your file is run by
perl itself. When you ship, it is translated to another language and
compiled, so the binary has no perl in it. That asymmetry is where every
difference between the two comes from, and it is why the dialect is a
subset rather than the whole language.

## Two languages, one file

Rakugan defines no language of its own. What you write is Perl, narrowed
once and then given a library:

| | What it takes |
|---|---|
| **perl** | all of Perl. It is what runs your app while you write it, and it reads the file first, so a shape perl rejects never reaches the translator. |
| **Rakugan** | the part a translator can carry to a compiled run: one type per name, a condition that is a `Bool`, a hash read with a fallback, a pattern written out, no `eval`, no `local`, no `tie`, no `ref`. `rakugan check` refuses the rest before anything is built: [What Rakugan refuses](refusals.md). |

On top of that subtraction Rakugan adds one thing, and it is a library
rather than a language: the subs that build the screen, and `run`,
`every`, `task`, `sqlite_exec` and their neighbours. Your classes, your
methods, your regular expressions and Perl's own functions are Perl's.

## What runs while you are writing

```console
$ perl -Ilib -I<door>/blib/lib -I<door>/blib/arch app.pl
```

That is all the interpreted run is: perl, reading your file, with
Rakugan's Perl half on the load path and the XS door beside it. The door
opens the engine's C face and declares its functions; nothing above the
door knows which run this is.

Your app is real Perl here, run by the real interpreter: its classes,
its subs, its regular expressions, its garbage collector.

## What runs when you ship

```console
$ rakugan translate demo/counter.pl
emitted: demo/.gate/counter/src/main.pix
$ cd demo/.gate/counter && pixie build
```

The translator reads your Perl with [PPI](https://metacpan.org/pod/PPI)
and writes [pixie](https://github.com/i2y/yokan/blob/main/docs/PIXIE.md),
the checked intermediate source Yokan emits from Python; pixie compiles
that, with the engine, into one native binary. `rakugan translate` stops
after the first step and hands you the `.pix`, which is a file you can
read.

The compiled binary carries the engine and the translated app, and links
nothing but the system's own libraries.

## What both of them drive

One engine, behind a C face: `crates/pixie-capi` — pixie's kernel, with
gpui drawing — the same code in both runs. The interpreted run opens it
through the XS door, the compiled run links it.

The face is deliberately narrow. An element is opened, written into by
number, and closed; a handler is a number the door hands out; a list of
strings crosses as a list of strings. **The engine never holds a Perl
value**, which is why one implementation can serve an interpreter and a
compiled binary without knowing which it is talking to.

Those numbers are not written by hand either. `elements.toml` is the one
table: every element, every keyword it takes, its type and its default.
`tools/gen.pl` writes the Perl subs an app calls and the table as Perl
data from it, and the sweep fails when either is behind the table. An
element cannot come to mean one thing in Perl and another where it is
drawn. The other three languages on this engine read the same table.

## Where the two are not the same

perl is the specification: where the two differ, the interpreted run is
right and the translation has a bug. The differences come in three
kinds, and it is worth knowing which kind you are looking at, because
they are caught by three different things.

### 1. Refused before it runs

Most of the gap is closed by subtraction. A shape that would translate
and then behave differently is refused with the line and the rewrite,
before anything is built and with no compiler started:
[What Rakugan refuses](refusals.md) is the whole list, quoted from the
fixtures that hold the wording.

Two of those deserve naming here, because they are Perl a Perl
programmer would write without thinking. A condition is a `Bool`, so
`if ($n)` is written `if ($n != 0)`; and `keys %h` is written
`sort keys %h`, because perl's order for a hash changes every time perl
starts and a screen cannot depend on it.

### 2. Answers differently

Perl's own functions are perl's in one run and a Rust twin in the other,
because the compiled run carries no perl. A twin that disagrees with
perl is a difference between the two runs, so the gate catches it — but
only where a script actually reaches it, and a demo reaches a few dozen
calls, not a library.

So the twins are held to perl directly. Every function carrying a Perl
name is measured against a table that perl itself printed —
`crates/rakugan-stdlib/tests/expected/`, a thousand rows over `sprintf`,
the string functions, the numeric ones, `List::Util`, `POSIX` and
regular expressions — and `cargo test -p rakugan-stdlib` compares
the twin against it, case by case. The tables are regenerated by running
the cases through perl, so when perl moves, the diff is the news.

Regular expressions are not twinned at all: the pattern is compiled when
the app is translated, and both runs match with one engine. That is also
why a pattern built while the app runs is refused.

The framework's own library is not twinned either. `fs_*`, `sqlite_*`,
`http_*`, `jsondoc_*`, the clipboard and the rest are one implementation
in Rust: the compiled run links it, and the interpreted run reaches the
same code through the engine's C face. What the gate compares there is
one library answering twice.

### 3. The same program, at different speeds

The compiled run is faster, and a long computation that merely feels
slow while you are writing is not a difference in behaviour. A handler
that blocks freezes the window in both, which is what `task` is for.

## The clock

Both runs tick off one clock. A timer declared with `every` fires on a
frame in a window and on an `advance:<ms>` step under a script, so the
same number of ticks lands in both. Nothing in an app should read a
wall clock and draw it: that is a difference the gate would catch, and
the reason `clock_format_ms` takes the milliseconds rather than
fetching them.

## What that leaves you to check

The gate. One script, both runs, byte for byte:

```console
$ ./bin/rakugan gate demo/counter.pl --script "click:+1,dump"
GATE OK — 3 dump lines identical in both runs
```

A green gate says the two runs agree on everything the script touched.
It says nothing about what a script never reached, and nothing at all
about the framework's own library, where one implementation answers both
runs and a mistake in it is a mistake in both. That is what the other two
checks are for: the perl-printed tables over every twin, and the sweep
that puts all forty-one demos through the gate. The three together are
the claim: the shipped binary behaves like the perl you ran, and where
the name is Perl's, both behave like perl.
