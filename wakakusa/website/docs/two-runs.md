# The two runs

An app is one file. It is run two ways, and the whole point of
Wakakusa is that the two are the same program. This page is what each
run is made of, what they share, and the few places they are not the
same.

**The two are two implementations of Ruby**: CRuby while you write,
and [spinel](https://github.com/matz/spinel) — an ahead-of-time Ruby
compiler — when you ship. What they do differently comes in four
kinds: [refused before it runs](#1-refused-before-it-runs),
[quietly different](#2-answers-differently-quietly),
[a second standard library](#3-the-standard-library-is-a-second-implementation),
and [the same program at different speeds](#4-the-same-program-at-different-speeds).

## What runs while you are writing

```console
$ ruby -Ilib -Idoor/cruby app.rb
```

That is all the interpreted run is: CRuby, reading your file, with
Wakakusa's shared half on the load path and one door beside it. The
door opens the engine as a shared library and declares its functions;
nothing above the door knows which run this is.

Your app is real Ruby here, run by the real interpreter. Its classes,
its blocks, its `require`s, its standard library, its garbage
collector — all CRuby's.

## What runs when you ship

```console
$ spinel -Ilib -Idoor/spinel --int-overflow=promote -c -o app.c app.rb
$ cc -O2 app.c libspinel_rt_mt.a libpixie_capi.a … -o app
```

[spinel](https://github.com/matz/spinel) compiles the same Ruby to C,
and `cc` turns that into a native binary with the engine linked in
statically. `wakakusa translate` stops after the first line and hands
you the C, which is a file you can read.

The compiled binary carries the engine, the compiled Ruby, and
spinel's runtime, and links nothing but the system's own libraries.

## What both of them drive

One engine, behind a C ABI: `crates/pixie-capi`, the same code in both
runs. The interpreted run opens it as a shared library, the compiled
run links the static one.

The face is deliberately narrow. An element is opened, written into by
number, and closed; a handler is a number the door hands out; a list of
strings crosses as a list of strings. **The engine never holds a Ruby
object**, which is why one implementation can serve an interpreter and
a compiled binary without knowing which it is talking to.

Those numbers are not written by hand either. `elements.toml` is the
one table: every element, every keyword it takes, its type and its
default. `tools/gen.rb` writes the Ruby an app calls, the key numbers
both sides count with, and the engine's own constants from that table,
and a test fails when a key has no arm on the other side. An element
cannot come to mean one thing in Ruby and another where it is drawn.

## Where the two are not the same

CRuby is the specification: where the two differ, the interpreted run
is right and the compiled one has a bug. The differences come in four
kinds, and it is worth knowing which kind you are looking at, because
only one of them can reach you quietly.

### 1. Refused before it runs

Whole-program compilation has no interpreter in the binary, so anything
that needs one is refused while compiling, by name and with the line.
You find these on the first `wakakusa build`, not in the field.

| What | What happens |
|---|---|
| `eval`, `instance_eval("…")` | refused (the block forms — `instance_eval { }` — compile) |
| `method_missing` | not dispatched; defining it warns at compile time |
| `define_method` with a computed name | only literal names compile |
| `ObjectSpace`, `TracePoint`, `set_trace_func` | refused |
| `binding` as an object, `callcc` | refused (`binding.local_variable_get(:x)` with a literal name works) |
| refinements (`refine` / `using`) | not resolved |
| `Class.new(parent) { … }`, `Klass.include(M)` after the class body | refused; the class graph is baked at compile time |
| `methods`, `instance_variables`, `instance_variable_get(name)` with a computed name | refused (a literal `:@x` works) |
| a `require` the compiler does not carry | a compile error, not a run-time surprise |
| a `Range` **object** over your own class | a compile error naming the class (`x.clamp(lo..hi)` still works) |

Wakakusa adds four of its own on top, for shapes that would compile and
then behave differently — [What Wakakusa refuses](refusals.md).

### 2. Answers differently, quietly

This is the kind the gate exists for. Each row below was run under both
implementations at the pinned revision:

| Shape | CRuby | The compiled run | Write instead |
|---|---|---|---|
| a block made inside a loop and kept for later | `0,1,2` | `2,2,2` — it sees the loop's last value | a method that takes what the row needs; Wakakusa refuses the shape inside a view |
| a comparison against a `nil` read out of an Integer array or Hash (`xs[9] < 5`) | raises `NoMethodError` | answers `true` — the `nil` is a sentinel inside the int slot | guard the read: `h.fetch(k, 0)`, or a `nil?` test |
| a float past what an integer holds: `1e20.to_i` | `100000000000000000000` | raises `RangeError` | keep it a Float, or bound it before converting |
| a seeded `Random` | one sequence | a different one | write the generator yourself, as the two ported games do |
| `Exception#backtrace`, `caller` | the frames | `[]` (the class and the message are right) | log the message |

Arithmetic itself is not on that list: the compiled run is built with
`--int-overflow=promote`, so an integer grows past a machine word
rather than wrapping, in a local, on a field and in an array alike.

### 3. The standard library is a second implementation

`File`, `JSON`, `CSV`, `Time`, `Net::HTTP`, `Enumerable` — you write
Ruby's own library, and there is no library of ours in front of it. But
the code answering is CRuby's in one run and the compiler's own runtime
and packages in the other, so the coverage is not identical:

- The libraries that need a `require` are the ones the compiler
  bundles: `base64`, `csv`, `digest`, `erb`, `forwardable`, `json`,
  `net/http`, `openssl`, `optparse`, `pathname`, `securerandom`, `set`,
  `stringio`, `strscan`, `tmpdir`, `uri`. A `require` of anything else
  is a compile error.
- `Time` itself needs no `require` and works; `require "time"`'s
  string parsing (`Time.parse`, `Time.strptime`) is not there.
- `Net::HTTP` is one request per connection: no keep-alive, no
  pipelining, no HTTP/2, no proxy, no cookie jar, and a redirect comes
  back as the 3xx it is rather than being followed.
- `openssl` is the outbound-client subset, over the operating system's
  own trust store.
- Strings are UTF-8 or ASCII-8BIT; other encodings are out of scope.

`demo/stdlib.rb`, `demo/files.rb` and `demo/reader.rb` are there to
hold the two to each other on the paths an app actually walks.

### 4. The same program, at different speeds

Threads are the one place where the two behave differently and neither
is wrong. The compiled run has a true M:N runtime with no global lock:
two threads of arithmetic run on two cores, and the window goes on
drawing. CRuby runs one thread at a time, so the same app takes twice
as long and the window stops until the work is done.

A long computation therefore looks worse while you are writing the app
than it will when you ship it.

How far a thread has got by a given moment is not something the two
agree about either — one is on the clock the machine keeps and the
other on the clock a script sets. That is why `task` exists: the engine
waits for the answer, so both runs reach the same place before the next
step. Anything perpetual should be an ordinary `Thread` whose results a
timer picks up.

### The compiler's own catalogue

The list above is the part a Wakakusa app meets. The full one is the
compiler's own — `docs/limitations.md` in the
[spinel](https://github.com/matz/spinel) checkout that
`tools/spinel_setup.sh` fetches, under `~/.cache/spinel/<sha>/`. It is
organised the same way: what is fundamental to compiling ahead of time,
what is limited but fixable, and what is a deliberate deviation.

## The clock

Both runs move on one clock. In a window it is the frames; under a
script it is `advance:<ms>`. A timer, an animation and a game's tick
all read that one clock, which is what makes them things the gate can
compare instead of things it has to wait out.

## What that leaves you to check

Nothing about the arrangement, and everything about your app. That is
`wakakusa gate`: one script, two runs, the transcripts compared byte
for byte — [Verify and ship](tour-ship.md).
