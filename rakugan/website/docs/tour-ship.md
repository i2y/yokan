<!-- Written by website/tools/tour_pages.pl from the tour. Edit the tour. -->
# Verify and ship

The gate, what the dialect refuses, the bundle, and what does not work yet.

## Headless runs and the gate

`PIXIE_SCRIPT` replaces the person. The engine builds the tree, drives
it with the steps, and prints the dumps:

```
click:<label>      press a button by the label it shows
input:<text>       type into a field   submit    press enter in it
slide / select     move a slider, pick an option
click@1:<label>    the second button with that label (n counts from 0, in tree order)
                   and the same for input@n:, submit@n, slide@n:, select@n:
key:<chord>        a keystroke bound to a shortcut
keydown:<key> / keyup:<key>    hold a key down, let it up
menu:<item>        pick a menu item    file:<path>   answer a dialog
drop:<path>        a file dragged onto the window
advance:<ms>       move the clock      theme:dark|light
dump               print the tree      a11y   print what a reader reads
```

`rakugan gate` runs the app twice with one script — perl through the XS
door, and the binary the translator's `.pix` was built into — and
compares the two transcripts byte for byte.

```console
$ rakugan gate demo/counter.pl --script "click:+1,dump,input:Momo"
GATE OK — 3 dump lines identical in both runs
```

The gate is the promise. Everything else in this page is a way of
writing something the gate can keep.


## What Rakugan refuses

`rakugan check` reads the app and names what it cannot take, with the
line and the rewrite. perl reads the file first — it is the parser of
record, so a shape perl rejects never reaches the translator — and then
the translator names the first thing outside the dialect. It runs before
every build and every gate, and prints nothing when there is nothing to
say.

```console
$ rakugan check demo/broken.pl
demo/broken.pl:9:26: Rakugan cannot take this — a hash may not have that
key, so say what to answer when it does not: `$prices{$k} // 0`
        $picked = $prices{"apple"};
                  ^
```

What it refuses, and what to write instead:

- A field with no initializer, or with `:param` on the app's own class.
  The initializer is where the type comes from.
- A list or hash that starts empty without saying what it holds. Write
  `empty(Str)`.
- A list holding two types.
- A method with parameters and no `:Sig`.
- A condition that is not a `Bool`. Compare it: `!= 0`, `ne ""`.
- `+` with a string on one side. `0 + $s` reads a string as a number.
- `++` on a string. Perl counts letters there (`"az"++` is `"ba"`), and
  the compiled run does not.
- A hash read with no `//`, and `keys` without `sort`.
- An index a view cannot prove is inside the list.
- A method that writes a field, called while a view is being built.
- `$1` outside the `if` that matched, a pattern built at run time, and
  `/e` on a substitution.
- `print`, `printf`, `say`: a compiled app writes its screen, not its
  standard output. `warn` goes to standard error and is taken.
- A string `eval`, `goto`, `local`, `wantarray`, `each`, `tie`, `bless`,
  `ref`, `AUTOLOAD`.
- `finally`, a `try` around a loop or around a method that can fail, and
  a library call inside a `try` with no form the catch could receive.
- A line after `die` in the same block (perl never reaches it) and a
  line after `task` in the same handler (perl reaches it at once, the
  compiled run when the work is done).
- A handler that is not a sub, or takes a parameter it is not called
  with.
- An unknown keyword on an element, or one given the wrong type — the
  message lists what that element takes.

Each of those has a fixture under `test/refuse/` holding the message it
prints, so a refusal cannot quietly change its wording.


## Shipping

```console
$ rakugan build demo/todo.pl --release --app
built: ~/.cache/pixie/target/release/main (11.9 MB)
bundle: demo/dist/todo.app (11.9 MB)
```

`--release` drops the symbol table; `--app` wraps the binary in a macOS
application bundle, ad-hoc signed, with `<stem>.png` or `<stem>.icns`
beside the app as its icon. The binary carries the engine and the
translated app and links nothing but the system's own libraries, so the
bundle is the whole program: it opens on a machine with neither perl
5.40 nor the toolchain installed.


## What does not work yet

- The dialect is a subset, and the list under
  [What Rakugan refuses](#what-rakugan-refuses) is what it leaves out.
  Every entry there is a shape the translator cannot yet carry to the
  compiled run, not a judgement about Perl.
- A whole number that passes 64 bits while the app runs. perl grows it
  into a number with a fraction; the compiled run stops the handler
  there. A literal that comes to that is refused; a sum that reaches it
  at run time is not seen by `check`, and the gate is what catches it.
- A list read past its end. perl answers `undef` and carries on; the
  compiled run stops the handler. `$xs[$i] // $d` says what to answer
  instead, and `check` does not yet ask for it.
- No references except the ones named here: a list or a hash passed to
  an element, and a class of your own. No code references beyond
  handlers, no references to references, no `ref`.
- No modules of your own. An app is one file, and the only functions
  from a module the translator knows are `List::Util`'s and `POSIX`'s;
  a `use` of anything else is read and ignored, and a call into it is
  refused by name.
- No `sprintf` beyond `%s %d %i %f %F %e %E %g %G %x %X %o %b %%`, with
  a width, a precision, and the `-`, `+`, ` `, `0` and `#` flags.
- Random numbers are not the same generator in the two runs, so a
  program that wants one sequence in both writes the generator itself.
  The two games do, in a few lines of arithmetic.
- `check` names what is listed above, but it does not yet see everything
  the translator gets wrong; the gate is still what catches the rest.
- macOS and Linux. `--app` is a macOS shape and names itself off it;
  the binary `build` writes is native on both. It carries the engine it
  draws with, so even a small app weighs about 12 MB.
