# Look, and the window

Two things are left after the elements themselves: how a screen is
arranged and coloured, and what the window offers that no element does.

## Arranging

`column` and `row` put their children down and across. Four more
containers arrange in other ways, and each takes its children as
arguments or as a block, like those two do.

```ruby
  grid(
    text("one"), text("two"),
    grid_cell(text("across both", align: "center"), col_span: 2),
    text("three"), text("four"),
    columns: 2, spacing: 6.0
  )

  stack(
    image("assets/postcard.png", width: 180.0, height: 90.0),
    text("over the picture", background: "#f9e2af", padding: 4.0)
  )

  scroll_view(column(*(1..12).map { |n| text("line #{n}") }), height: 90.0)
  h_scroll_view(row(*(1..10).map { |n| text("col #{n}", width: 70.0) }))
```

`grid` lays its children on `columns:` tracks. A child covers more than
one track with `col_span:` on itself, or by being wrapped in
`grid_cell` — the two spellings build the same tree. `stack` puts its
children on top of one another, in the order they are written. The two
scroll panes scroll when their children do not fit.

`modal` is a panel over the rest of the window while `open:`, which is
how a dialog is written: it exists when the app says it does.

```ruby
  modal(open: @asking) {
    column(spacing: 8.0, padding: 14.0) {
      text "delete the file?"
      row(spacing: 8.0) {
        button("cancel") { @asking = false }
        button("delete") { remove }
      }
    }
  }
```

`demo/panels.rb` is the four containers on one screen, and
`demo/dialog.rb` the modal.

## The small pieces

```ruby
  spacer(grow: 1.0)                     # takes what the parent has left over
  divider(thickness: 2.0, color: "accent")
  spinner(size: 18.0)                   # work with no known length
  progress(0.4, label: "copying…")      # a track filled from 0 to 1
  progress(0.0, indeterminate: true)    # …or sweeping, for the same
  link("Website", "https://example.org")
  image("assets/postcard.png", width: 180.0)
  svg("assets/search.svg", width: 20.0, height: 20.0)
```

A `spacer` in a row pushes what follows to the far edge; in a column it
does the same downward. `link` has no handler on purpose: opening a
page is not the app's state.

## The keywords every element takes

Fifteen properties ride on every element under one name and one
meaning.

| Keyword | Type | What it does |
|---|---|---|
| `width` `height` | number | a size the box takes, whatever the element is |
| `min_width` `max_width` | number | bounds on that width |
| `disabled` | true/false | the element is drawn inert and takes no input |
| `theme` | `"dark"` / `"light"` | the palette this subtree resolves its colors in |
| `animate` | number | how long a change takes, in milliseconds |
| `easing` | `"linear"` `"in"` `"out"` `"inOut"` | the shape of that change |
| `enter` `exit` | true/false | animate what appears, and what disappears |
| `col_span` `row_span` | whole number | how many of a grid's tracks this covers |
| `role` | string | what a screen reader calls this |
| `a11y_label` | string | what a screen reader reads instead of the text |
| `tooltip` | string | what the pointer shows |

```ruby
  button("save", disabled: @busy, tooltip: "write the file", width: 120.0)
  text "total", role: "heading", a11y_label: "the running total"
```

They are wrappers around the element rather than fields repeated on
thirty of them, which is why they can be written once, in
`elements.toml`, and mean the same thing everywhere.

An element that owns one of those names under its own meaning keeps
it: a `text`'s `width` is the text's own, and an `image`'s `width` and
`height` are the picture's, so the box leaves them alone. The
[Elements](elements.md) page marks which those are.

`demo/shared.rb` puts one shared property on each kind of element — a
theme scope on a spacer, a tween on a chooser, a tooltip on a rule, a
field spanning two grid tracks, and the lock that makes a field and a
button inert.

## Colors

A color is a hex string (`"#f38ba8"`), or the name of a token in the
palette, which is what lets one screen follow the window's own theme:

`windowBg`, `panel`, `fieldBg`, `surface`, `surfaceHover`,
`surfacePressed`, `border`, `text`, `textDim`, `accent`, `selection`,
`scrim`, `scrollbar`, `scrollbarActive`.

```ruby
  text "saved", color: "accent"
  column(background: "panel", border_color: "border", border_width: 1.0) { … }
```

## Themes and animation

```ruby
  column(theme: "dark") { … }
  segmented(options: ["read", "write"], selected: @tab,
            animate: 120.0, easing: "out") { |i| @tab = i }
  text "saved", animate: 150.0, easing: "out", enter: true
```

`theme:` swaps the palette under one part of the screen, and it takes a
value rather than a literal, so an app can flip it (`theme: @mode`).
`animate:` is how many milliseconds a change takes, with `enter:` and
`exit:` for what appears and disappears.

Both runs move on one clock: a frame in a window, an `advance:<ms>`
step in a script. That is what makes an animation something the gate
can compare rather than something it has to wait out.

A look worth reusing is an ordinary Hash, merged and splatted:

```ruby
KEY = { grow: 1.0, size: 20.0, background: "panel" }.freeze
OP = KEY.merge({ background: "#fab387", color: "#1e1e2e" }).freeze

  button("7", **KEY) { press("7") }
  button("+", **OP) { press("+") }
```

`demo/styled.rb` and the two calculators (`demo/calc.rb`,
`demo/calcgrid.rb`) are written that way.

## The window

Four things come from the window itself, declared before `run`, living
as long as the app does:

```ruby
shortcut("cmd+s") { app.save }
menu_item("File", "Open…") { app.open }
on_key { |chord| app.typed(chord) }
on_file_drop { |path| app.load(path) }
```

`shortcut` binds a chord; `menu_item` puts the same handler in the
application's menu bar under a menu of your naming; `on_key` receives
every chord that no shortcut took; `on_file_drop` receives the path of
a file dragged onto the window.

Two more read and write the world outside:

```ruby
  clipboard_set(@text)
  @text = clipboard_get

  job = task { open_dialog("choose a file") }
  on_done(job) { @path = task_answer }
```

A dialog waits for a person, so it belongs inside `task` — see
[Ruby, data, and work](tour-lib.md#work-off-the-windows-thread). A
headless script answers one with a `file:<path>` step, which is what
makes a dialog a checked interaction like any other.

`demo/keys.rb` is the shortcuts and the menu bar under a script;
`demo/picker.rb` is the dialogs and a dropped file.

Sound is a file played and then forgotten:

```ruby
  audio_play("demo/assets/sound/blip.wav")         # as it was recorded
  audio_play("demo/assets/sound/blast.wav", 0.4)   # at a level, 0.0 to 1.0
  audio_stop
```

The call answers at once; nothing waits for the end of the sound. A run
under a script is silent — a gate must not need a machine with speakers
— and a machine with no audio device, or a file that cannot be read,
plays nothing rather than failing the app. WAV is what the engine
decodes. `demo/sound.rb` is the whole of it, and the two ported games
use it.

## The window itself

```ruby
run(app, title: "ledger", width: 520.0, height: 640.0, padding: 0.0)
```

`title:` names the window and, when you ship, the application bundle.
`width:` and `height:` open it at a size, and `0.0` leaves the
default. `padding:` is the margin around the whole screen: `-1.0`
leaves the default and `0.0` runs the content to the edge, which is
what a canvas wants.

## Where next

- [Ruby, data, and work](tour-lib.md) — Ruby's own library, a
  database, timers, and work off the window's thread.
- [Elements](elements.md) — every element, every keyword, its type and
  its default.
