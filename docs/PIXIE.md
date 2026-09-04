# pixie

A GUI-first programming language that compiles through Rust and renders
with a GPU engine (gpui). You write reactive classes and a declarative
view; pixie emits borrow-clean Rust, so the Rust compiler acts as a
second verifier of every generated program — and Rust's crate ecosystem
serves as the standard library through thin `.rpi` type bindings.

```ruby
store Session {
  state name : String = ""
  state saved : Bool = false

  fn update(t: String) {
    name = t
  }

  async fn save {
    await Fs.writeString("/tmp/pixie-hello.txt", name)
    saved = true
  }
}

view Main {
  Column {
    TextField {
      text: Session.name
      placeholder: "your name"
      onTextChanged: Session.update(text)
    }
    Button { text: "save"; onClick: Session.save() }
    if Session.saved {
      Text { text: "saved: #{Session.name}" }
    }
  }
}
```

Typing lands in a real IME-capable text field, `await` ships the file
write to a worker thread and re-enters the UI loop, and the `if` row
appears when the store flips — every line above compiles and runs
today (with a two-line `.rpi` binding `Fs` over `std::fs::write`).

**New here? Take the [language tour](TOUR.md)** — basic syntax,
the type system, memory management (what is reclaimed and what is
not), components, animation, accessibility, packaging, and how the
two runs work, with diagrams.

## Quick start

```sh
cargo run -q -p pixie-cli -- install-runtime          # once per machine
cargo run -q -p pixie-cli -- build examples/counter/counter.pix --run
```

Tests are part of the language: `test fn doubleWorks { assert_eq(double(21), 42) }`,
or `test "a free form name" { .. }`, grouped in `suite "name" { .. }`,
with `assert_eq` / `assert_ne` / `assert_true` / `assert_false`. Each
test runs against a fresh World, and `pixie test <file.pix>` prints TAP.

Other verbs: `check` · `test` (TAP) · `fmt` · `watch` — edits to a view
body or a style hot-reload the running window **in-process in about a
millisecond** (state preserved), in the file you are editing or in one
it imports; anything else rebuilds and relaunches in about half a
second.
`build --release` ships an AOT-only binary: the hot-reload machinery
and the interpreter are stripped from the crate graph entirely
(13 MB vs 60 MB for the counter demo), behavior byte-identical.

Every built app is scriptable headless:
`PIXIE_SCRIPT="input:Ada,click:save,key:cmd-s,drop:/tmp/x.txt"` drives it and prints the element
tree, and `PIXIE_TIER=interp` replays the same script through the
hot-reload interpreter — the two runs must print byte-identical trees,
which is the project's standing divergence gate.

## Editor support

`extensions/vscode-pixie` gives VS Code **errors as you type, hover,
go-to-definition and completion** — the compiler's own diagnostics,
suggestions included — plus highlighting for `.pix` and `.rpi` files
that knows the widget catalog, element properties, styles, `#{}`
interpolation, postfix return types and error unions.

Build the server once, then install the extension in development
mode:

```sh
cargo build --release -p pixie-lsp
```

```sh
ln -sfn "$(pwd)/extensions/vscode-pixie" ~/.vscode/extensions/pixie-language-dev
```

and reload the window. The extension finds the server in the open
workspace's `target/`, so a checkout needs no configuration; set
`pixie.lsp.path` for an installed one. Cursor / VSCodium / Windsurf
read their own extensions directory; the same symlink works there.
Packaging and the grammar regression check are in the extension's own
README.

Any editor with an LSP client works — `pixie-lsp` is a plain stdio
server.

## Status

Early but real. Working today:

- **Widgets** — an 18-element catalog: Column / Row / Grid / Stack /
  Text / Button / TextField (IME, selection, Tab ring) / ListView /
  ScrollView / HScrollView / Image / Svg / DataTable / Modal /
  BarChart / LineChart / ProgressBar / Spinner, plus `if` / `if let`
  / `case` conditional rendering and `for` repeaters. `Grid` gives
  equal tracks with `colSpan:` / `rowSpan:` placement on any
  element. Reusable **components** are just views: parameters with
  defaults, per-instance `state`,
  `Slot { }` children, and per-row state at any repeater depth
  (nested `for`s and virtualized lists included). A `for` body or an
  `if` branch holds as many elements as you write, repeaters nest
  over any list you can name and bind the row's index when asked
  (`for row, i in xs`), and numbers are numbers: `fontSize: 14`
  and `unit * qty` need no decimal points or conversions.
- **Animation** — declared on the element whose values move rather
  than wrapped around the update that moves them: `animate: 200.0`
  with `easing:`, plus `enter:` / `exit:` fades.
  The interpolation runs on the element tree, not in the renderer,
  so a headless script sees it too —
  `PIXIE_SCRIPT="click:go,advance:100"` dumps the frame that instant
  would have painted.
- **Accessibility** — roles, names and values are *derived* for the
  whole catalog, so most of a tree needs no authoring; `role:` and
  `label:` cover what cannot be (a heading, an icon's alt text, a
  named toolbar). Layout containers report nothing rather than
  announcing "group, group, group". The tree is a checked output:
  `PIXIE_SCRIPT="a11y"` prints it, and the window feeds AccessKit
  from the same derivation.
- **Shared properties** — a property that means the same on every
  element is a wrapper the compiler puts around whatever element it
  is written on, never a field repeated per widget: `tooltip:`, `role:` / `label:`, `disabled:` (dimmed,
  inert in the window and in a script, marked for AccessKit),
  `width:` / `height:` / `minWidth:` / `maxWidth:` (on elements
  without native sides), `theme:`, the animation properties and
  `colSpan:` / `rowSpan:`, nested in one fixed order in both runs.
- **Styles and themes** — named property bags (`style Key {
  background: "#313244" }`), merged with `+`, applied as `style: Key`
  on any element and inlined at compile time; `pub style` shares them
  across modules — and a style resolves where it was written, so an
  exported component carries its own, `pub` or not;
  `hover.background:`/`active.background:` style button states;
  colors can name semantic theme tokens ("accent", "panel") that
  follow the dark/light theme. `theme:` scopes a
  palette to a subtree — a light panel inside a dark window — and
  takes an expression, so an app owns its theme as ordinary state and
  can offer a switcher. Put `animate:` on the element and a theme
  flip crossfades. Editing a style hot-reloads the running window in
  about a millisecond, like a view-body edit.
- **Virtualized lists that are actually lazy** — a 100,000-row list
  builds only the ~14 visible rows per frame.
- **Async and HTTP** — `async fn` + `await` on binding calls,
  blocking work on gpui's thread pool, one execution semantics
  windowed and headless; a built-in HTTP client rides it
  (`await Http.get(url)` / `getBytes` → `Bytes` / `post` /
  `getWith(url, headers)` with `Map<String, String>` headers).
- **Declarations that run themselves** — `fn tick @every(1000)` is a
  repeating callback, `fn save @key("cmd-s")` is a shortcut and
  `fn save @menu("File", "Save")` is an item in the application's
  menu bar, all bound the moment the store exists;
  `fn typed(k: String) @key` sees every key as the chord it was, and
  `fn opened(p: String) @drop` takes a file dragged onto the window.
  They fire off the same clock and the same dispatch a window uses, so
  `PIXIE_SCRIPT="advance:1000"`, `PIXIE_SCRIPT="key:cmd-s"` and
  `PIXIE_SCRIPT="menu:Save"` reach exactly what a second, a keystroke
  and a menu pick would.
- **Hot reload** — the running binary re-parses its own view body and
  rebuilds against the live World; `pixie watch` decides per save
  whether an in-process reload or a full rebuild is needed.
- **Bindings** — `.rpi` files over Rust crates, and `rpi-gen` derives
  them from rustdoc JSON (measured against the real `std.json`:
  `std::fs` binds 14 functions, everything unbindable is skipped with
  the reason). Numbers, bools, strings, bytes, maps, lists and
  optionals cross in both directions, plus `Result<T, E>` as `!T` on
  the way back — so `std::fs::read` comes back as a copy-on-write
  `Bytes`, not a list of ints. A C-like Rust `enum` and a Rust
  `struct` cross too, tuple structs included — rpi-gen declares each
  with its variants or its fields, a field can name the Rust type it
  writes back into, and one that cannot correspond (a payload variant,
  a private field) is a named error rather than a surprise.
- **Projects and packages** — `pixie.toml` with a `[crates]` table:
  name a Rust crate and pixie derives its `.rpi` binding surface
  automatically (rustdoc JSON → rpi-gen, a committable version-keyed
  cache) while cargo owns version resolution. `[dependencies]` pulls
  pixie packages by path, git, or a registry-index version, pinned
  in `pixie.lock` (locked deps resolve fully offline); a package's
  `src/lib.pix` is its face (`pub use` re-exports). Modules alias
  (`use foo as F`), import selectively with renames
  (`use foo.{X as A}`), nest (`use ui.buttons`), and same-named
  items across modules coexist. `pixie build` in the project
  directory builds `src/main.pix`.
- **Class members** — a `prop` is the observable surface; `let` is
  init-once and the compiler holds you to it; `var` is ordinary
  mutable state; and a property can be *derived* — `bind { first + " "
  + last }` stores nothing, runs when read, and stays current on its
  own. `deinit` runs when the last reference to an object goes,
  while it can still read itself. A property holds any value type —
  a list, a map, an optional, bytes, a struct — or another object.
- **Memory management you do not do** — automatic reference counting
  for objects, copy-on-write for values. Nothing to free by hand,
  nothing scanning the heap in the background, no borrow checker in
  your face.
  Objects a method creates and never
  lets out are reclaimed when their scope ends, proved at compile
  time. Objects a store or another object *keeps* are freed when the
  last reference goes, taking what they held with them — at the write
  itself, so there is no pause. Cycles are the one exception and
  `weak` breaks them. Values (`String`, `List`, `Map`, `Bytes`,
  `struct`) are copy-on-write and never enter the object store at
  all.
- **Objects that refer to objects** — a class-typed field holds a
  reference, so two owners can name one object and a write through
  either is visible through the other. A store can own one outright,
  a view reads through the whole chain, and a change anywhere along
  it redraws. Values nest freely too, trees and recursion included.
- **Speed** — value work compiles to the same Rust and runs at the
  same speed (measured: 0.19 ns per iteration of an arithmetic loop
  against 0.13 for hand-written Rust — both sub-nanosecond). Reading
  and writing an object's property costs about 3 ns, roughly 3.5x a
  raw struct field, which is what a reference that cannot dangle and
  a reactive loop that knows when something changed are worth. The
  benchmark ships in the tree.
- **Traits** — declare behavior once, implement it for as many types
  as you like, and write one function that takes all of them. Both a
  `class` and a `struct` can implement the same trait, and the
  compiler specializes each call, so the abstraction costs nothing at
  run time.
- **The values layer** — generics end to end: trait-bounded fns and
  methods, generic structs (`Pair(1, 2)` infers), and generic
  classes (`Basket<String>()`) with `init` constructors — all
  compiled to real Rust generics and monomorphized by rustc. Plus
  `Map<K, V>` with literals, `List<T>`, `Bytes`, `T?` with `nil`,
  error enums with `try`/`case`, and the full compound-assignment
  operator set. `case` and `if let` read an optional or an enum
  anywhere — a method, a handler, or a view body, where the arms
  hold elements.
- **Engine** — gpui from the Zed tree at a pinned revision, with a
  vendored `gpui_macos` carrying pixie's panic-containment patch for
  the macOS input-method callbacks.

The toolchain pins itself via `rust-toolchain.toml`. The gate
replays a scripted reading through both runs and fails on any byte of
difference — accessibility trees, mid-animation frames, live-object
counts and destructor tallies included. `examples/` holds runnable
demos of every feature above.
