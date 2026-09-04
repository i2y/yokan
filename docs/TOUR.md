# A tour of pixie

A single pass over the whole language. Every snippet here compiles
and runs against today's tree (what does NOT work yet is collected
honestly at the end). 日本語版は [TOUR.ja.md](TOUR.ja.md).

## Contents

1. [pixie in 30 seconds](#1-pixie-in-30-seconds)
2. [How compilation works — two verifiers](#2-how-compilation-works)
3. [Basic syntax](#3-basic-syntax)
4. [The type system — traits, generics, `T?`](#4-the-type-system)
5. [Memory management — the World and handles](#5-memory-management)
6. [Classes, stores, reactivity](#6-classes-stores-reactivity)
7. [Views and styles](#7-views-and-styles)
8. [Error handling and `T?`](#8-error-handling-and-t)
9. [Async and HTTP](#9-async-and-http)
10. [Rust bindings — crates.io is the stdlib](#10-rust-bindings)
11. [Modules and packages](#11-modules-and-packages)
12. [Two-tier execution and hot reload](#12-two-tier-execution-and-hot-reload)
13. [CLI cheat sheet](#13-cli-cheat-sheet)
14. [What does not work yet](#14-what-does-not-work-yet)

---

## 1. pixie in 30 seconds

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

- A `store` is the process-wide reactive state
- A `view` is a declarative UI tree; state changes re-render it
- `async fn` + `await` ships blocking work to a worker thread
- `Fs` is a two-line binding over Rust's `std::fs::write`

Run it:

```sh
pixie build hello.pix --run     # opens the window
pixie watch hello.pix           # ~1 ms hot reload on save
PIXIE_SCRIPT="input:Ada,click:save" ./hello   # replay UI steps headless
```

---

## 2. How compilation works

The most important picture in the design:

```
  .pix source
     │
     ▼
┌──────────────────┐   parse · type check · visibility · style splice
│  pixie front end  │   (forked from Cute)
│                  │──── every pixie-level error surfaces HERE
└──────────────────┘
     │  emits Rust
     ▼
┌──────────────────┐
│  generated Rust  │   always passes the borrow checker —
│  (not for humans)│   pixie writes nothing else
└──────────────────┘
     │
     ▼
┌──────────────────┐
│  rustc           │◀─── the SECOND verifier. An error here is
│  (2nd verifier)  │     a pixie compiler bug, never yours
└──────────────────┘
     │
     ▼
  native binary (GPU-rendered via gpui)
```

pixie only ever writes Rust the borrow checker accepts, so ownership
and borrowing never reach you — and in exchange, rustc re-verifies
every program pixie writes.

---

## 3. Basic syntax

### Comments, literals, interpolation

```ruby
# comments run from # to end of line

let n = 42               # Int   (i64)
let x = 3.14             # Float (f64)
let ok = true            # Bool
let s = "hi"             # String
let msg = "n = #{n}"     # string interpolation is #{expr}
let xs = [1, 2, 3]       # List<Int>
let m : Map<String, Int> = { a: 1, "b-key": 2 }
                         # Map literal; identifier keys are string sugar
```

A whole number where a fractional one is expected is just that
number: `fontSize: 14`, `let ratio : Float = 3`, `30.0 * count`.
Mixing the two in one expression gives the wider one.

Interpolations take expressions and a format spec:

```ruby
"#{n * 2} of #{n + 1}"     # arithmetic
"#{v:.2f}"                 # 3.14
"#{n:>6}"   "#{n:04}"      # width, zero fill
"#{s:*^9}"                 # centred, filled with *
```

The spec is a width, an alignment (`<` `^` `>`), a zero or custom
fill, a `.precision`, or a radix (`x` `X` `o` `b`) — a trailing type
letter like `f` or `d` is allowed and ignored. Anything else is a
compile error naming the spec.

### let / var and assignment

```ruby
fn demo String {
  let a = 1          # immutable
  var b = 10         # mutable
  b += 5             # compound assignment: += -= *= /=
  var s : String = "x"
  s += "!"           # string concatenation is + / +=
  "#{a} #{b} #{s}"   # the trailing expression is the return value
}
```

### Functions

```ruby
fn add(a: Int, b: Int) Int {   # return type is postfix, no arrow
  a + b                        # last expression returns; `return` works too
}

fn greet(name: String) {       # no return type = Void
  # ...
}
```

### Control flow

```ruby
if n > 2 {
  # ...
} else {
  # ...
}

case color {                   # pattern match over enums
  when Red { ... }
  when Green { ... }
  when Blue { ... }
}

for x in xs {                  # iterate a List<T>
  if x == 2 { continue }
  if x > 8 { break }
  sum += x
}
for i in 0..n { ... }          # ranges: 0..n exclusive, 0..=n inclusive
while i > 0 { i -= 1 }
```

`case` over a declared enum is **exhaustiveness-checked**: missing a
variant is a compile **error** listing what is absent (`when _` is the
catch-all). Operators: `+ - * / %`, comparisons `< <= > >= == !=`,
logic `&& || !`.

### Reaching into a list or a map

```ruby
xs[i]            # T   — traps if there is no element i
xs.get(i)        # T?  — nil instead
xs.first()       # T?
m[k]             # V?  — absence is ordinary for a map
xs.length        # Int
```

A list is any expression of list type, so a field path works
directly: `for k in node.kids { ... }`, `bag.items.length`,
`kept[0].tag.label`.

### Structs (value types)

```ruby
struct Point {
  var x: Int
  var y: Int

  fn sum Int {
    self.x + self.y     # struct methods use self.field
  }
}

let p = Point(3, 4)     # positional construction
p.sum()                 # => 7
```

### Enums and errors

```ruby
enum Color {
  Red
  Green
  Blue
}

error MathError {        # an error enum (see §8)
  divByZero
  negative(v: Int)       # payload-carrying variant
}
```

### Tests

```ruby
test fn addition {
  assert_eq(add(2, 2), 4)
}

suite "edge cases" {
  test "zero" { assert_eq(add(0, 0), 0) }
}
```

`pixie test file.pix` runs them as TAP. Asserts:
`assert_eq / assert_neq / assert_true / assert_false`.

---

## 4. The type system

**Static, nominal typing — and rustc re-verifies every program.**

```
                   the type landscape
┌────────────────────────────────────────────────┐
│ primitives    Int  Float  Bool  String  Bytes  │
│                                                │
│ collections   List<T>      Map<K, V>           │
│  (COW values) (growable)   (sorted keys)       │
│                                                │
│ modifiers     T?           !T                  │
│               nullable     error union         │
│                                                │
│ user types    class C      struct S    enum E  │
│               (World-      (value)     (value) │
│                resident)                       │
│                                                │
│ abstraction   trait X  +  generics <T: X>      │
└────────────────────────────────────────────────┘
```

### Traits — one name for behavior several types share

A `trait` declares what something can do; an `impl` says how one type
does it. Both halves of the type system take part: a `class`, which
lives in the World, and a `struct`, which is a value.

```ruby
trait Labeled {
  fn tag String
}

class Dog    { pub prop name : String, default: "rex" }
struct Badge { var text: String }

impl Labeled for Dog   { fn tag String { "dog:#{name}" } }
impl Labeled for Badge { fn tag String { "badge:#{self.text}" } }
```

A function bounded by the trait takes either, and the compiler
generates a separate specialized version for each type it is called
with — a trait call costs the same as a direct one:

```ruby
fn describe<T: Labeled>(x: T) String {
  "<#{x.tag()}>"
}

describe(Dog())            # => <dog:rex>
describe(Badge("hi"))      # => <badge:hi>
```

Calling it with something that does not implement the trait is a
compile error at the call site: `type Int does not implement trait
Labeled`. Implementing a trait does not disturb a type's own
methods — `Badge` keeps whatever else it declares.

Still gated, with named errors: trait impls on generic classes, and
type parameters on a class or struct cannot carry bounds yet.

### Generics — rustc owns the monomorphization

pixie generics compile **directly to real Rust generics**. No
pixie-side stamping, no code-size tricks of our own.

```ruby
# generic structs — instantiation is inferred
struct Pair<T> {
  var a: T
  var b: T

  fn swapped Pair<T> { Pair(self.b, self.a) }
}
let p = Pair(1, 2)          # inferred Pair<Int>

# generic classes — construction takes explicit type args
class Basket<T> {
  pub prop items : List<T>, default: []
  pub fn put(v: T) { items.push(v) }
}
# in a view: let names = Basket<String>()
```

- Class/struct type params are currently **unbounded only**
  (bounded ones are a named error)

### Nullable `T?` and `nil`

```ruby
fn pick(id: Int) String? {
  if id == 1 {
    return "ada"      # wrapped into `some` automatically
  }
  nil                 # absent
}

case pick(2) {
  when some(v) { ... }   # v : String
  when nil { ... }
}

if let some(v) = pick(1) {     # the two-armed case, spelled shorter
  ...
} else {
  ...                          # `else` is optional
}

let held : String? = pick(1)   # T? lives in locals too
```

Both work in a method, a handler, and a view body — and in a view the
arms hold elements:

```ruby
if let some(name) = App.user {
  Text { text: "hello #{name}" }
  Button { text: "sign out"; onClick: App.signOut() }
} else {
  Text { text: "not signed in" }
}
```

---

## 5. Memory management

**Automatic reference counting for objects, copy-on-write for
values.** Nothing to free by hand, nothing scanning the heap in the
background, no borrow checker in your face. Two pillars, and an
honest account of what is reclaimed, after the first.

### Pillar 1: the World and handles (objects)

Every class instance lives in the **World** — a generational slot
map. What you hold is a **`Handle<T>`**: a Copy (index, generation)
pair.

```
        the World (one per process, main-thread only)
        ┌───────────────────────────────┐
        │ slot 0: gen=2 │ Counter {...}  │◀─┐
        │ slot 1: gen=0 │ Session {...}  │  │
        │ slot 2: gen=5 │ (free)         │  │
        └───────────────────────────────┘  │
                                            │
   Handle<Counter> { ix: 0, gen: 2 } ───────┘
   (Copy — closures can capture it freely)

   access:      handle.count(w)     — reads go through the World
   after death: generation mismatch — the handle KNOWS it is stale
                (dangling pointers are structurally impossible)
```

- A reference is never held across statements: every access
  round-trips World → value → World, so two conflicting borrows
  cannot arise
- Closures (event handlers) may capture **only Copy handles and
  values** (the capture rule) — which is what deletes the
  classic UI-callback lifetime problem

### What is reclaimed, and what is not

An object a method creates and never lets out is **reclaimed when its
scope ends** — the compiler can see that nothing else got hold of it,
so the slot is freed and the next allocation reuses it. That covers
the temporaries a loop makes, at an exact point in the program:

```ruby
for i in 0..1000000 {
  let n = Node()      # created and freed a million times
  n.v = i             # ... in one slot
}
```

Hand the object anywhere — return it, store it in another object,
push it into a list, pass it to anything, bind it to a second name —
and that no longer applies: it is now something else's to hold, and
the rule below takes over. The compiler assumes the object got away
whenever it cannot see otherwise, which is the safe direction.

An object that *is* kept — in a store property, in another object's
property, in a list — is freed when the last such reference goes
away, and it takes what it held with it:

```ruby
Doc.rows = []       # the old rows go, and so do their children
```

Freeing happens at the write that drops the last reference, so there
is no pause to schedule around. **Cycles are the exception** — two
objects that name each other keep each other alive. Mark one
direction `weak` and the cycle breaks:

```ruby
class Node {
  pub prop kids : List<Node>, default: []
  pub weak prop parent : List<Node>, default: []   # the back-edge
}
```

A `weak` reference does not keep its target alive. Reading one after
the target is gone is safe and observable — a handle always knows
whether the object it names is still there.

Appending to a list property is one operation, wherever the property
lives — this object's, a store's, or one reached through another
object:

```ruby
top.kids.push(kid)      # through an object
Doc.rows.push(row)      # through a store
```

An object a view owns is alive because the view holds it, so you can
hand it to a store, put it in a list, and let the list go — it comes
back unharmed:

```ruby
view Main {
  let mine = Tally()
  Column {
    Button { text: "stash"; onClick: Bin.take(mine) }
    Button { text: "bump";  onClick: mine.bump() }   # fine after Bin drops it
  }
}
```

Row seats (per-row component state) are the one thing that is
grow-only by design, so a list that once reached a million rows keeps
a million row objects — with a large list, prefer holding a selection
as an index in a store.

Values are still the cheaper default: a `store` property of type
`List<SomeStruct>` notifies on write, so a list or a document gets
reactivity at the property level with no World objects at all.

### Pillar 2: COW values (data)

`String / List / Map / Bytes` are **copy-on-write values**.
Assignment and passing share; only a write copies:

```
   let a = [1, 2, 3]      a ──┐
   let b = a                  ├──▶ [1, 2, 3]    (shared, no copy)
                          b ──┘

   b.push(4)              a ─────▶ [1, 2, 3]     (a untouched)
                          b ─────▶ [1, 2, 3, 4]  (copied HERE)
```

So you can hand values around as if they were copies — passing a
million-element list costs a pointer bump, and the real copy happens
only if somebody writes.

---

## 6. Classes, stores, reactivity

### class — prop / init / signals

```ruby
pub class Counter {
  pub prop count : Int, default: 0     # auto countChanged notification

  pub fn increment {
    count += 1        # bare prop name = setter = auto-notify
  }
}

pub class Tag {
  pub prop label : String              # no default = init must assign
  pub prop weight : Int, default: 1

  init(l: String, extra: Int) {        # constructor (one per class)
    label = l
    weight = weight + extra            # init bodies are World-free
  }
}
# construction: Tag("hi", 4)
```

### The kinds of member

```ruby
pub class Person {
  pub prop first : String, default: "Ada"   # the observable surface
  pub prop last : String, default: "L"

  pub let id : Int                          # arrives in init, never changes
  pub var seen : Int = 0                    # ordinary mutable state

  # Derived: stores nothing, runs when read, always current.
  pub prop full : String, bind { first + " " + last }

  init(n: Int) { id = n }

  pub fn greet String {
    seen += 1
    "hello #{full}"
  }

  # Runs when the last reference to this object goes, while it can
  # still read itself.
  deinit {
    Log.note("bye #{full}")
  }
}
```

`let` is checked: writing `id` anywhere but `init` is an error that
says so. A derived property takes no write at all — assign what it
reads.

A property holds any of the value types: `List<T>`, `Map<K, V>`,
`T?`, `Bytes`, a `struct`, or another object.

```ruby
struct Row {
  var name : String
  var score : Int = 0        # a default a construction site may omit
  var note : String? = nil
}

store Sheet {
  state rows : List<Row> = []
  state tally : Map<String, Int> = {}
  state picked : String? = nil
  state raw : Bytes = []                  # `[]` is the empty one
}
# Row("ada")  fills score and note from their defaults
```

In a view, a map's `keys` and `values` are lists a repeater takes,
`m[k]` answers `T?`, and an absent optional interpolates as nothing:

```ruby
for k in Sheet.tally.keys {
  Text { text: "#{k} = #{Sheet.tally[k]}" }
}
for r in Sheet.rows {
  Text { text: "#{r.name} #{r.score} [#{r.note}]" }
}
```

Inside a method, **`this`** is the object the method is running on.
Hand it to somebody else, or answer with it:

```ruby
pub fn adopt(k: Node) {
  kids.push(k)
  k.attach(this)        # the child now names its parent
}

pub fn me Node { this }
```

### class or struct?

|  | `class` | `struct` |
|---|---|---|
| lives in | the World (reached by `Handle`) | nowhere — it *is* a value |
| assignment | shares the same instance | copies |
| change notification | every field write notifies | none |
| construction | `init` (one per class) | positional |
| reclaimed | no (see §5) | yes, like any value |

Two questions decide it:

1. Must something **observe** it changing, or must two places share
   **one** instance and see each other's writes? → `class`. Props,
   signals, and handle identity exist for exactly that.
2. Otherwise → `struct`. Cheaper, copied, never in the World.

In practice most app data is the second. Structs nest freely —
struct in struct, a `List<Struct>` field, and recursive structs (a
tree) all work:

```ruby
struct Node {
  var v: Int
  var kids: List<Node>
}
```

Classes compose too, and that is where they earn their keep: a
class-typed field holds a *reference*, so two owners can name one
object and a write through either is visible through the other —
the one thing values cannot express.

```ruby
class Tag  { pub prop weight : Int, default: 0 }
class Note {
  pub prop tag : Tag                 # a handle, not a copy
  init(t: Tag) { tag = t }
}

let t = Tag()
let a = Note(t)
let b = Note(t)
a.tag.weight = 3
b.tag.weight                         # => 3, same object
```

A `static fn` belongs to the class rather than to an instance: no
receiver, no state, called through the class name.

```ruby
class Temp {
  pub prop celsius : Float, default: -40.0

  pub static fn fromF(f: Float) Float {
    (f - 32.0) * 5.0 / 9.0
  }
}

Temp.fromF(212.0)     # => 100.0
```

### store — the process singleton

```ruby
store App {
  state user : String = ""
  state theme : String = "dark"
  state session : Session = Session("guest")   # a store can own an object

  fn login(u: String) { user = u }
}
# anywhere: App.user / App.login(u) / App.session.token
```

A view reads through the whole chain, and a change anywhere along it
redraws — a write to `App.session.token` is seen even though the
store itself did not change.

### The reactive loop

```
   click
     │
     ▼
 ┌─ method runs ──────────────────────────────────┐
 │   count += 1                                   │
 │      │ setter queues a notify                  │
 │      ▼                                         │
 │   flush — deliver to listeners (deferred,      │
 │      │            never re-entrant)            │
 │      ▼                                         │
 │   mark dependent views dirty                   │
 └────────────────────────────────────────────────┘
     │
     ▼
   view build() re-runs → new Element tree → gpui repaints
```

Binding is **one-way**: views only read state; writes go through
methods — the data flow is traceable at a glance.

Writes are grouped for free: nothing rebuilds until the method
returns, and writing one property three times notifies once.

---

## 7. Views and styles

### The 18-widget catalog

Column / Row / Grid / Stack / Text / Button / TextField (IME-capable) /
ListView (optionally virtualized) / ScrollView / HScrollView /
Image / Svg / DataTable / Modal / BarChart / LineChart /
ProgressBar / Spinner.

```ruby
view Main {
  let items = Basket<String>()      # view-owned object
  state note : String = ""          # view-local reactive cell

  Column {
    Text { text: "count: #{items.items.length}" }
    if App.theme == "dark" {        # conditional rendering
      Text { text: "dark side" }
    }
    ListView {
      virtualized: true             # 100k rows → only ~14 visible built
      itemHeight: 24.0
      for x in items.items {        # repeater
        Text { text: x }
      }
    }
  }
}
```

A `for` body and an `if` branch each hold as many elements as you
write, and either can contain the other. A repeater takes any list
you can name, including one reached through the row itself — which is
what a table is:

```ruby
for row in App.rows {
  Text { text: row.name }
  if row.flagged {
    Text { text: "!" }
  }
  for cell in row.cells {
    Text { text: cell }
  }
}
```

(A `virtualized:` list is the one exception: it builds one element per
row on demand, so its `for` body holds exactly one — wrap several in
a `Column`.)

`case` decides between more than two, on an optional or an enum:

```ruby
case App.mode {
  when idle { Text { text: "waiting" } }
  when busy {
    Text { text: "working" }
    ProgressBar { value: App.pct }
  }
  when _ { Text { text: "done" } }
}
```

A variant the arms do not name contributes nothing — a view build
cannot fail halfway.

`Image` and `Svg` decode once and share a bounded cache, so scrolling
a long list of covers does not retain every one it passed. The budget
is 256 MB of decoded pixels; `PIXIE_IMAGE_BUDGET_MB` moves it.

### Grid — equal tracks instead of nested rows

`Column` and `Row` stack along one axis. `Grid` fills equal tracks and
wraps to the next line on its own.

```ruby
Grid {
  columns: 4        # equal columns, always
  rows: 5           # equal rows too — without it rows hug their content
  spacing: 8.0      # the gap on BOTH axes
  Button { text: "7"; onClick: Pad.press("7") }
  # ... fourteen more keys ...
  Button { text: "0"; colSpan: 2; onClick: Pad.press("0") }
}
```

- An item fills the cell it lands in — no `grow:` needed
- `colSpan:` / `rowSpan:` stretch one item over several tracks, and
  they work on **any** element (a Column, a chart, a component's
  Button), because they describe the parent's placement, not the
  element
- Tracks are equal **by construction**: the engine's template is
  `repeat(n, minmax(0, 1fr))`, so per-column widths (`100px 1fr auto`)
  are not expressible — a row of unequal columns stays a `Row` with
  `grow:`
- `examples/calcgrid` is `examples/calc`'s keypad as one Grid: same
  window, five `Row`s and the span arithmetic gone

### Handlers

An `onClick:` (or `onTextChanged:`, `onSubmitted:`) body runs the same
statements a method body does — control flow, locals, and objects
built right there:

```ruby
Button {
  text: "go"
  onClick: {
    var i = 0
    while i < 10 {
      i = i + 1
      if i > 3 { break }
    }
    for k in 0..i {
      Board.note("k#{k}")
    }

    let c = Chip("a")     # built here
    c.hits = 2
    c.bump()
    Board.keep(c)         # and handed to a store
  }
}
```

A single call needs no block: `onClick: Board.reset()`. A bare
`return` leaves the handler early, from inside a loop too:

```ruby
onClick: {
  if Board.locked { return }
  Board.commit()
}
```

The one thing a handler can do that the surrounding view body cannot
is **call a method**. Building a view only reads state — that is what
makes a rebuild safe — so a view body reads properties, and a handler
is where anything changes.

### Custom components — reusable stateful views

Every `view` other than `Main` is a **component**: parameterized,
usable as an element, with per-instance state. It resolves entirely
at compile time: the use site inlines, the engine's vocabulary never
grows, and both execution tiers expand it identically.

```ruby
view Counter(label: String, step: Int) {
  state n : Int = 0                  # ← EACH use site gets its own cell

  Row {
    Text { text: "#{label}: #{n}" }
    Button { text: "+#{step}"; onClick: { n = n + step } }
  }
}

view Card(title: String) {
  Column {
    Text { fontSize: 18.0; text: title }
    Slot { }                         # ← use-site children land here
  }
}

view Main {
  Column {
    Card {
      title: "counters"
      Counter { label: "a"; step: 1 }
      Counter { label: "b"; step: 10 }   # independent from "a"
    }
  }
}
```

- Use-site properties bind the declared params; param defaults work
- `state` / `let` in a component hoists per instance — adding or
  removing a stateful use site is a rebuild; body edits hot-reload
  like any view edit
- One `Slot { }` per component; recursion is a compile error
- **Per-row state**: a stateful component inside a `for` repeater
  gets one state set PER ROW, keyed by position — so a list that
  shrinks and regrows shows the old rows' state again, rather than
  starting them over. It works at any repeater depth and inside
  `virtualized:` lists; `let` object fields per row are still gated
- **Cross-module components**: `pub view` crosses modules — use
  qualified (`ui.Card { }`), aliased (`use ui as U` → `U.Card`), or
  selective (`use ui.{Card as MyCard}`) forms. A pub component's body
  resolves against its own module's views (private siblings included)

### Styles and themes

```ruby
style Key {
  background: "#313244"
  hover.background: "#45475a"       # pseudo-states via dotted keys
}
style Hot { background: "#fab387" }
style KeyOp = Key + Hot             # right-wins merge

view Main {
  Column {
    style: Pad                      # applied as a property
    Button { style: KeyOp; text: "÷"; onClick: ... }
    Text { color: "accent" }        # colors may name theme tokens
  }
}
```

An element that paints a box also takes a corner radius, a border
thickness and the border's color:

```ruby
style Card {
  padding: 10.0
  background: "panel"
  borderRadius: 10.0
  borderWidth: 1.0
  borderColor: "accent"     # a token, so a palette flip carries it
}
```

- Styles inline **completely at compile time** (zero runtime cost)
- A style resolves in the module that WROTE it, so a component you
  export carries its own styles with it — including the ones you did
  not mark `pub`. Marking one `pub` is what lets another module NAME
  it, which is a different question from whether it reaches there
- Color tokens (`"accent"`, `"panel"`, …) follow the dark/light
  theme; launch with `PIXIE_THEME=light`, flip live with Cmd+T
- `theme:` scopes a palette to one subtree, so a light panel can sit
  in a dark window — and it takes an expression, so an app owns its
  own theme as ordinary state:

```ruby
store App { state mode : String = "dark"
  fn light { mode = "light" } }

view Main {
  Column {
    theme: App.mode           # the app's switcher writes this
    grow: 1.0
    background: "windowBg"
    Button { text: "light"; onClick: App.light() }
    Column {
      theme: "light"          # ... and a subtree can pin its own
      background: "panel"
      Text { color: "text" }
    }
  }
}
```

- Tokens resolve on the element tree, once per rebuild — so
  `PIXIE_SCRIPT="theme:light"` prints the light tree, and an element
  with `animate:` crossfades when the palette flips
- Editing a style hot-reloads the running window in ~1 ms, exactly
  like a view-body edit — including a `pub style` in another module

### Animation

Animation is declared on the element whose values move, rather than
wrapped around the update that moves them.

```ruby
Button {
  text: "box"
  width: Panel.boxWidth             # any change to these tweens
  background: Panel.boxColor
  animate: 300.0                    # ms — the switch
  easing: "linear"                  # linear | in | out (default) | inOut
  onClick: Panel.narrow()
}

if Panel.openOn {
  Text { text: "hello"; animate: 200.0; enter: true; exit: true }
}
```

- `animate:` is what turns it on. `easing:` / `enter:` / `exit:`
  without it are a named error, not a silent no-op
- All four take expressions, so the curve and the fades can be state
  an app offers a control for (`easing: App.curve`, `exit: App.fades`)
- `enter:` fades the element in the first time it appears; `exit:`
  keeps painting it after the view stops emitting it, then fades it
  out. That retention is why `if` blocks can leave gracefully
- Numbers tween directly; colors tween when both ends are literal
  hex (a theme token resolves in the engine, so it snaps instead)
- The interpolation runs on the element tree, not in the renderer,
  so a headless script sees it too. `advance:<ms>` in `PIXIE_SCRIPT`
  stands the clock at an instant:

```sh
PIXIE_SCRIPT="click:show,advance:100" ./app   # dumps a half-faded frame
```

- A script that never mentions time settles every tween before it
  dumps, so animation never changes what a script means
- Reduced-motion settings zero every duration

### Accessibility

Most of an accessibility tree is derivable, so pixie derives it: a
Button is a button named by its label, a TextField is a text input
named by its placeholder and valued by its contents, a ProgressBar
reports its number. Layout containers report nothing and hand their
children upward — "group, group, group" is worse than silence.

Two riders cover what cannot be derived:

```ruby
Text { text: Doc.title; fontSize: 22.0; role: "heading" }

Row {
  role: "group"
  label: "toolbar"
  Svg { source: "save.svg"; label: "Save" }   # alt text
  Button { text: "save"; onClick: Doc.save() }
}
```

- `role:` comes from a closed vocabulary (`button`, `label`,
  `heading`, `textInput`, `image`, `list`, `listItem`, `table`,
  `dialog`, `progress`, `group`). A literal is checked at build time;
  an expression is allowed, so a row can be a heading or an item
  depending on its data, and a name the vocabulary does not know
  falls back to what the element derives
- `label:` is any string expression, so alt text can interpolate
- The tree is computed on the element tree, so a script can print it:

```sh
PIXIE_SCRIPT="a11y,click:open,a11y" ./app
# group[label "...", button "open"]
# group[label "...", button "open", dialog[label "Leave a note", ...]]
```

---

## 8. Error handling and `T?`

```ruby
error MathError {
  divByZero
  negative(v: Int)
}

fn safeDiv(a: Int, b: Int) !Int {      # !T = fallible
  if b == 0 {
    return MathError.divByZero         # returning the error
  }
  a / b
}

fn divideTwice(a: Int, b: Int) !Int {
  let once = try safeDiv(a, b)         # try propagates the error
  try safeDiv(once, b)
}

case safeDiv(1, 0) {
  when ok(v) { ... }
  when err(e) { ... }                  # e is the error enum; nestable
}
```

Fallibility shows in the type (`!Int`); silently swallowing is not
writable (`case` demands both `ok` and `err` arms). `T?` (§4) is
the separate tool for absence-that-is-not-failure.

---

## 9. Async and HTTP

```
     main thread                        worker pool (gpui's)
 ┌────────────────────┐              ┌──────────────────┐
 │  World + UI loop   │    await     │  blocking work    │
 │                    │─────────────▶│  fs / http / ...  │
 │  async fn bodies   │              └──────────────────┘
 │  resume every 16ms │◀─────────────  completion queue
 └────────────────────┘  result + conversion
```

```ruby
store Net {
  state body : String = ""

  async fn hit {
    case await Http.get("https://example.com/") {
      when ok(b) { body = b }
      when err(e) { body = "failed: #{e}" }
    }
  }
}
```

- `await` targets binding calls; the work runs on a worker, the
  result converts to pixie values back on the main thread. That is
  the whole of the async surface for now: an `async fn` returns
  nothing and cannot `await` another one
- ONE runtime (gpui's pool) — no second async runtime smuggled in
- The HTTP client is built in:
  `Http.get / getBytes (→ Bytes) / post / getWith(url, headers)` —
  headers are a `Map<String, String>`
- Windowed and headless runs share ONE execution semantics
  (headless settles deterministically)

---

## 10. Rust bindings

**crates.io is the standard library.** A `.rpi` file declares a
Rust crate's surface; call-site adapters convert the types.

```ruby
# fs.rpi (this is ALL of it, if written by hand)
class Fs {
  static fn writeString(path: String, contents: String) !Void @rust("std::fs::write")
  static fn read(path: String) !Bytes @rust("std::fs::read")
}
```

You usually don't write them: **rpi-gen** derives `.rpi` from
rustdoc JSON:

```
 Rust crate ──▶ rustdoc JSON ──▶ rpi-gen ──▶ .rpi
                                   │
                                   └─ anything unbindable is
                                      SKIPPED WITH A NAMED REASON
```

Adapter mapping (excerpt), as an argument and as a return:

| Rust side | pixie side |
|---|---|
| `&str` / `String` / `PathBuf` | `String` |
| `i64` (a wider return widens) | `Int` |
| `Vec<T>` | `List<T>` |
| `Vec<u8>` / `&[u8]` | `Bytes` |
| `Option<T>` | `T?` |
| `Result<T, E>` | `!T` (returns only) |
| the kernel's `Map<K, V>` | `Map<K, V>` |
| a C-like `enum` | an `enum` rpi-gen declares (below) |
| a `struct`, tuple ones included | a `struct` rpi-gen declares (below) |

A **return** is more forgiving than a parameter: any integer width and
a `PathBuf` widen on the way back, while a parameter is taken at the
pixie type's own Rust type (`i64`, `String`). A struct field can name
the Rust type it writes into, so it crosses both ways either way.

### Enums cross when the `.rpi` says how

A pixie `enum` and a Rust one are two different types, so the `.rpi`
writes the correspondence rather than guessing at it. **rpi-gen emits
this for you** — it declares every public C-like enum in the module
it binds:

```ruby
enum PathKind @rust("pixie_kernel::PathKind") {
  Missing
  File
  Dir
}

class Kernel {
  static fn pathKind(path: String) PathKind @rust("pixie_kernel::path_kind")
  static fn kindName(kind: PathKind) String @rust("pixie_kernel::kind_name")
}
```

A variant with no `@rust` of its own uses its own name, which is why
the generated form carries one attribute rather than one per variant.
Write `dir @rust("Dir")` when you want a different spelling on this
side.

What comes back is an ordinary pixie value — `case` matches it like
any other:

```ruby
case Kernel.pathKind("/tmp") {
  when Dir { note = "a directory" }
  when _ { note = "something else" }
}
```

Two limits, both named errors rather than surprises. A variant that
carries a **payload** cannot correspond: the conversion matches
variant for variant, and relating a payload would mean relating its
fields too — pass those fields instead. And matching by name alone,
with no attribute at all, was the other design and was rejected: it
breaks silently when the Rust side renames a variant, and a binding
is exactly where a silent break is worst.

### Structs cross the same way

A `struct` says which Rust struct it corresponds to, and each field
says which Rust field — and, when the name alone is not enough, which
Rust type:

```ruby
struct FileStat @rust("pixie_kernel::FileStat") {
  var len : Int @rust("len: u64")
  var readonly : Bool
}

class Kernel {
  static fn fileStat(path: String) FileStat @rust("pixie_kernel::file_stat")
  static fn statLine(stat: FileStat) String @rust("pixie_kernel::stat_line")
}
```

`readonly` needs no attribute: the two sides already agree on the name
and on the type. `len` needs one because the Rust side is a `u64` —
more on that below. rpi-gen writes all of this for you, camel-casing
the field names and adding an attribute only where something differs.

What comes back is an ordinary pixie value, and one built on this side
goes back over unchanged:

```ruby
let s = Kernel.fileStat("notes.txt")
size = s.len
line = Kernel.statLine(FileStat(1024, true))
```

A field crosses by the same rule the whole value does, so a struct may
hold a struct, an enum, a list or an optional — and a `List<FileStat>`
or a `FileStat?` crosses element by element:

```ruby
struct Entry @rust("pixie_kernel::Entry") {
  var name : String
  var kind : PathKind
  var stat : FileStat
}

class Kernel {
  static fn dirStats(path: String) List<Entry> @rust("pixie_kernel::dir_stats")
  static fn statTotal(entries: List<Entry>, only: PathKind?) Int @rust("pixie_kernel::stat_total")
}
```

None of `Entry`'s fields names a type, because each one already writes
back as what Rust expects.

**Why `len` does.** A number widens on the way here — `Int` absorbs
any integer width — but going back has to hit the width exactly, and
only the `.rpi` knows it. The attribute reads like the Rust field
declaration: a name, or a name and a type. A string writes into a
`PathBuf` the same way. Note that a width cast wraps, so a negative
`Int` written into a `u64` comes out enormous: the `.rpi` named the
width, and the conversion follows it.

**A tuple struct** works by position. pixie names the field, Rust
reaches it as `.0`, and rpi-gen writes both:

```ruby
struct Perms @rust("pixie_kernel::Perms") {
  var value : Int @rust("0: u32")
}
```

A field that cannot cross stops the whole struct, and the error names
that field. rpi-gen skips such a struct with its reason: a private
field (pixie could not fill it), a field of a type that does not
correspond, or a field whose ELEMENT would need a Rust type of its own
— the attribute names one, so a `Vec<u32>` field has nowhere to say
it.

---

## 11. Modules and packages

### Modules

File = module; paths mirror directories:

```ruby
use model                  # sibling model.pix (its pub items visible)
use ui.card                # ui/card.pix; card.cardTitle(..) qualifies
use model as m             # alias: m.decorate(..)
use model.{decorate}       # selective import
use model.{decorate as d}  # with a rename
pub use ui.card.{cardTitle} # re-export (build a package's face)
```

Same-named items in different modules coexist (mangled per module
internally). An ambiguous bare reference errors naming both
origins — qualify it or import selectively.

### Packages — pixie.toml

```toml
[package]
name = "myapp"
version = "0.1.0"

[crates]                    # Rust crates as direct dependencies
serde_json = "1"
mathkit = { path = "vendor/mathkit" }

[dependencies]              # pixie packages
ui-kit = { git = "https://…", tag = "v0.2" }
strkit = { path = "packages/strkit" }
kit = "1"                   # ← resolved via the registry (below)

[registry]
index = "https://…/index"   # static index holding <name>.toml files
```

```
                the build-time flow
 pixie.toml
   │  [crates] serde_json = "1"
   │     │
   │     ├─▶ rustdoc JSON ─▶ rpi-gen ─▶ .pixie/rpi/serde_json.rpi
   │     │                    (a cache you COMMIT — collaborators
   │     │                     never need the doc nightly)
   │     └─▶ injected into the generated Cargo.toml
   │             └─▶ cargo owns version resolution + lockfile
   │
   │  [dependencies] kit = "1"
   │     └─▶ index lookup ─▶ git fetch ─▶ rev pinned in pixie.lock
   │            (a locked dep resolves fully OFFLINE)
   ▼
 pixie build   (bare, inside a project = src/main.pix)
```

- pixie ships **no semver solver**: Rust deps resolve through
  cargo; pixie packages obey the lock's pinned revs
- A dependency's `pub style`s ride the style splice too

Managing all of this has CLI verbs — nothing requires hand-editing:

```sh
pixie new my_app                    # scaffold pixie.toml + src/main.pix
pixie add kit --git https://…       # pixie dep: fetch + pin into pixie.lock
pixie add kit 1                     # …via the registry index
pixie add serde_json 1 --crate      # Rust crate: cargo dep + derived bindings
pixie update [kit]                  # unpin, re-resolve, report old → new rev
pixie remove kit                    # drop entry + lock pin + caches
```

`add` syncs immediately and **rolls back** the manifest if the fetch
or derivation fails — a typo'd URL never leaves the project broken.

---

## 12. Two-tier execution and hot reload

Every `.pix` has two executors:

```
            ┌── tier 1: AOT-compiled (the production shape)
 .pix ──────┤
            └── tier 2: the view-slice interpreter
                (the RUNNING binary re-parses its own view body
                 and rebuilds against the live World ≈ 1 ms)

 the standing divergence gate:
   drive the SAME script through both tiers;
   one byte of output difference fails the suite
   (31 demos, always on)
```

- `pixie watch` classifies each save by fingerprint: view-body and
  style edits → tier 2 in **~1 ms**; anything else → rebuild
  (about half a second). It looks at the imports too, so a `pub
  style` or an exported component's body in ANOTHER module reloads
  in place as readily as one in the file you are editing
- `pixie build --release` **strips tier 2 entirely**: no reload
  machinery, no interpreter in the dependency graph — 60 MB → 13 MB
  for the counter demo, byte-identical behavior

---

## 13. CLI cheat sheet

```sh
pixie build app.pix --run        # build and launch
pixie build --release            # AOT-only optimized build
pixie build                      # src/main.pix when pixie.toml exists
pixie check app.pix              # type check only
pixie test values.pix            # TAP test runner
pixie fmt app.pix [--check]      # formatter
pixie watch app.pix              # hot-reload watcher
pixie install-runtime            # once per machine (prebuilds gpui)
pixie new my_app                 # scaffold a project
pixie add kit --git URL          # add a dependency (--crate = Rust crate)
pixie update [kit]               # re-resolve pixie deps, refresh the lock
pixie remove kit                 # drop a dependency + its caches

PIXIE_SCRIPT="click:go,input:hi" ./app     # headless step replay
PIXIE_SCRIPT="click:go,dump,click:go" ./app # ... printing the middle too
PIXIE_SCRIPT="click:go,advance:100" ./app  # ... standing 100 ms in
PIXIE_SCRIPT="a11y" ./app                  # print the accessibility tree
PIXIE_SCRIPT="theme:light" ./app           # flip the root palette
PIXIE_SCRIPT="mem" ./app                   # print the live-object count
PIXIE_TIER=interp PIXIE_SCRIPT=... ./app   # same steps, tier 2
PIXIE_THEME=light ./app                    # light theme
```

---

## 14. What does not work yet

Everything here fails with a NAMED error — nothing breaks
silently.

- Component gaps: `let` object fields per row, and qualified refs
  (`ui.Card`) written inside another module's component body
  (components cross modules, and per-row state now works at any
  repeater depth and inside `virtualized:` lists — §7, all in the
  gate)
- Animation inside a `virtualized:` list's rows (the pass that
  interpolates deliberately does not materialize lazy rows), chart
  data-swap tweens, and transitions other than fade
- A `struct` field of class type. This one is a *rule*, not a gap: a
  struct is copied on assignment, and a copied reference is a second
  reference to one object. Make the holder a `class`, or hold an id
- User-defined palettes (`theme:` takes the two built-ins) and
  per-token overrides
- Accessibility beyond role, name, and value: no build-time warning
  for an unlabelled image, no authored focus order (the Tab ring is
  document order), no AccessKit actions or live regions
- Grid track sizing beyond equal columns: per-column widths
  (`100px 1fr auto`) and explicit placement (`colStart:` / `rowStart:`)
  — equal tracks and spans are what the engine exposes
- init overloads (one `init` per class)
- Bounded class/struct type params (`class Sorted<T: Comparable>`)
  — a free `fn` takes bounds, the class/struct half does not yet
- Trait impls on generic classes; generic impls; generic stores
- Calling a generic method straight from a handler while the window
  is hot-reloading (a concrete wrapper method works)
- Bit flags (`flags Perms of Perm`) — the type checker knows the
  shape; nothing runs it yet
- Nested patterns (`when err(bad(v))`) and matching a literal
  (`when 42`); a `case` arm names a variant, `some`/`nil`, or `_`
- A binding whose Rust function takes or returns an `enum` with a
  **payload**, or a generic struct. A C-like `enum` and a `struct`
  both cross, tuple structs included (rpi-gen declares them); so does
  everything else, in both directions: numbers, bools, strings, bytes,
  maps, lists, optionals, and `!T` returns
- An HTTP **server** (the sketch is a declarative `service` block;
  deliberately last)
- Linux / Windows (the engine is exercised on macOS)
