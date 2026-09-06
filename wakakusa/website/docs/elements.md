# Elements

Thirty-three elements, and the fifteen keywords every one of them
takes. This page is written from `elements.toml` — the one table
the Ruby methods, the numbers both sides of the engine's C face
count with, and the engine's own constants are generated from. A
keyword that is not here is one an app cannot write, and
`wakakusa check` says so by name.

Types on this page: **text** is a string, **number** a float,
**whole number** an integer, **true/false** a boolean, and a
**list of** either. A **handler** is the block, or a proc of no
arguments through a keyword — see
[Handlers](tour-logic.md#handlers).

## The keywords every element takes

These fifteen ride on every element under one name and one
meaning. They are wrappers around the element rather than fields
repeated on thirty of them, which is why an element that owns one
of the names under its own meaning keeps it — a `text`'s `width`
is the text's, and the box leaves it alone.

| Keyword | Type | Default |
|---|---|---|
| `width` | number | — |
| `height` | number | — |
| `min_width` | number | — |
| `max_width` | number | — |
| `disabled` | true/false | `false` |
| `theme` | text | `""` |
| `animate` | number | `0` |
| `easing` | text | `""` |
| `enter` | true/false | `false` |
| `exit` | true/false | `false` |
| `col_span` | whole number | `1` |
| `row_span` | whole number | `1` |
| `role` | text | `""` |
| `a11y_label` | text | `""` |
| `tooltip` | text | `""` |

## Text, buttons, links

### `text`

A run of text. `wrap` is "", "nowrap" or "ellipsis"; a background with padding and a radius makes a pill.

Sizes its own `width`: those are the element's, and the shared keyword leaves them alone.

| Keyword | Type | Default |
|---|---|---|
| `text` | text | written first, in order |
| `size` | number | `0` |
| `color` | text | `""` |
| `align` | text | `""` |
| `grow` | number | `0` |
| `bold` | true/false | `false` |
| `italic` | true/false | `false` |
| `mono` | true/false | `false` |
| `underline` | true/false | `false` |
| `wrap` | text | `""` |
| `max_lines` | whole number | `0` |
| `width` | number | `0` |
| `background` | text | `""` |
| `padding` | number | `0` |
| `border_radius` | number | `0` |
| `border_width` | number | `0` |
| `border_color` | text | `""` |

### `button`

A button. The block runs when it is pressed.

Sizes its own `width` / `height`: those are the element's, and the shared keyword leaves them alone.

| Keyword | Type | Default |
|---|---|---|
| `label` | text | written first, in order |
| `on_click` | a handler | — |
| `width` | number | `0` |
| `height` | number | `0` |
| `size` | number | `0` |
| `background` | text | `""` |
| `grow` | number | `0` |
| `color` | text | `""` |
| `hover_background` | text | `""` |
| `active_background` | text | `""` |
| `border_radius` | number | `0` |
| `border_width` | number | `0` |
| `border_color` | text | `""` |
| `basis` | number | `0` |

### `link`

Text that opens a page when clicked. There is no handler: opening a page is not the app's state.

| Keyword | Type | Default |
|---|---|---|
| `label` | text | written first, in order |
| `url` | text | written first, in order |
| `size` | number | `0` |

## The boxes that arrange

### `column`

Its children down the page.

Takes elements as its children — as arguments, or as a block.

| Keyword | Type | Default |
|---|---|---|
| `spacing` | number | `-1` |
| `padding` | number | `0` |
| `background` | text | `""` |
| `grow` | number | `0` |
| `border_radius` | number | `0` |
| `border_width` | number | `0` |
| `border_color` | text | `""` |

### `row`

Its children across the page.

Takes elements as its children — as arguments, or as a block.

| Keyword | Type | Default |
|---|---|---|
| `spacing` | number | `-1` |
| `padding` | number | `0` |
| `background` | text | `""` |
| `grow` | number | `0` |
| `border_radius` | number | `0` |
| `border_width` | number | `0` |
| `border_color` | text | `""` |

### `grid`

Its children on tracks. `columns` counts the tracks; `col_span` on a child covers more than one.

Takes elements as its children — as arguments, or as a block.

| Keyword | Type | Default |
|---|---|---|
| `columns` | whole number | `2` |
| `rows` | whole number | `0` |
| `spacing` | number | `-1` |
| `padding` | number | `0` |
| `background` | text | `""` |
| `grow` | number | `0` |
| `border_radius` | number | `0` |
| `border_width` | number | `0` |
| `border_color` | text | `""` |

### `grid_cell`

The span written out: this and `col_span:` on the child itself are the same tree.

Takes elements as its children — as arguments, or as a block.

No keywords of its own.

### `stack`

Its children on top of one another.

Takes elements as its children — as arguments, or as a block.

No keywords of its own.

### `scroll_view`

A pane that scrolls when its children do not fit.

Takes elements as its children — as arguments, or as a block.

Sizes its own `height`: those are the element's, and the shared keyword leaves them alone.

| Keyword | Type | Default |
|---|---|---|
| `height` | number | `0` |

### `h_scroll_view`

A pane that scrolls sideways.

Takes elements as its children — as arguments, or as a block.

No keywords of its own.

### `modal`

A panel over the rest of the window while `open`.

Takes elements as its children — as arguments, or as a block.

| Keyword | Type | Default |
|---|---|---|
| `open` | true/false | `true` |

## Fields and choosers

### `text_field`

A line a person types into. `on_change` fires per keystroke, `on_submit` when they press enter; `multiline` makes it a paragraph field.

| Keyword | Type | Default |
|---|---|---|
| `value` | text | written first, in order |
| `placeholder` | text | `""` |
| `on_change` | a handler | — |
| `on_submit` | a handler | — |
| `multiline` | true/false | `false` |
| `rows` | number | `0` |

### `number_field`

A field for a number: enter or leaving it commits, text that is not a number is dropped. `min`/`max` both 0 is unbounded, `step` 0 is free.

| Keyword | Type | Default |
|---|---|---|
| `value` | number | written first, in order |
| `min` | number | `0` |
| `max` | number | `0` |
| `step` | number | `0` |
| `placeholder` | text | `""` |
| `on_change` | a handler | — |

### `int_field`

The same field for a whole number.

| Keyword | Type | Default |
|---|---|---|
| `value` | whole number | written first, in order |
| `min` | whole number | `0` |
| `max` | whole number | `0` |
| `step` | whole number | `1` |
| `placeholder` | text | `""` |
| `on_change` | a handler | — |

### `checkbox`

A box a person ticks. The block receives the new state.

Its own `label` is what a screen reader reads, so the shared `a11y_label` is not offered here.

| Keyword | Type | Default |
|---|---|---|
| `label` | text | written first, in order |
| `checked` | true/false | `false` |
| `on_change` | a handler | — |

### `switch`

A switch a person flips. The block receives the new state.

Its own `label` is what a screen reader reads, so the shared `a11y_label` is not offered here.

| Keyword | Type | Default |
|---|---|---|
| `label` | text | written first, in order |
| `checked` | true/false | `false` |
| `on_change` | a handler | — |

### `slider`

A track a person drags. The block receives the new number.

| Keyword | Type | Default |
|---|---|---|
| `value` | number | `0` |
| `min` | number | `0` |
| `max` | number | `1` |
| `step` | number | `0` |
| `on_change` | a handler | — |

### `select`

A drop-down. The block receives the chosen index.

| Keyword | Type | Default |
|---|---|---|
| `options` | list of text | `[]` |
| `selected` | whole number | `0` |
| `on_change` | a handler | — |

### `radio_group`

A column of radio buttons. The block receives the chosen index.

| Keyword | Type | Default |
|---|---|---|
| `options` | list of text | `[]` |
| `selected` | whole number | `0` |
| `on_change` | a handler | — |

### `segmented`

A row of joined toggle buttons. The block receives the chosen index.

| Keyword | Type | Default |
|---|---|---|
| `options` | list of text | `[]` |
| `selected` | whole number | `0` |
| `on_change` | a handler | — |

### `tab_bar`

A row of tabs. The block receives the chosen index.

| Keyword | Type | Default |
|---|---|---|
| `labels` | list of text | `[]` |
| `active` | whole number | `0` |
| `on_change` | a handler | — |

## Lists and tables

### `list_view`

Rows built on demand: the builder is called for the rows in view, not for all of them.

Sizes its own `height`: those are the element's, and the shared keyword leaves them alone.

| Keyword | Type | Default |
|---|---|---|
| `count` | whole number | written first, in order |
| `row` | a count, and the block building row i | written first, in order |
| `item_height` | number | `24` |
| `height` | number | `0` |
| `virtualized` | true/false | `true` |
| `grow` | number | `0` |

### `table`

A table whose rows are built on demand, laid on tracks whose shares are `widths`. `on_select` receives the row clicked, `on_sort` the header.

Sizes its own `height`: those are the element's, and the shared keyword leaves them alone.

| Keyword | Type | Default |
|---|---|---|
| `columns` | list of text | written first, in order |
| `count` | whole number | written first, in order |
| `row` | a count, and the block building row i | written first, in order |
| `widths` | list of numbers | `[]` |
| `item_height` | number | `24` |
| `height` | number | `0` |
| `grow` | number | `0` |
| `selected` | whole number | `-1` |
| `on_select` | a handler | — |
| `sort` | whole number | `-1` |
| `descending` | true/false | `false` |
| `on_sort` | a handler | — |

### `data_table`

The first `row` child is the header; the later ones are data rows, shaded in alternation, in a frame that comes with the element.

Takes elements as its children — as arguments, or as a block.

No keywords of its own.

## Charts and progress

### `bar_chart`

Bars. `min`/`max` both 0 take the range from the data; `axis` draws ticks and gridlines; `series` draws several groups.

Sizes its own `width` / `height`: those are the element's, and the shared keyword leaves them alone.

| Keyword | Type | Default |
|---|---|---|
| `data` | list of numbers | written first, in order |
| `labels` | list of text | `[]` |
| `width` | number | `0` |
| `height` | number | `0` |
| `min` | number | `0` |
| `max` | number | `0` |
| `axis` | true/false | `false` |
| `color` | text | `""` |
| `series` | list of lists of numbers | `[]` |
| `colors` | list of text | `[]` |

### `line_chart`

A line. Same arguments as the bars, and `series` draws several lines.

Sizes its own `width` / `height`: those are the element's, and the shared keyword leaves them alone.

| Keyword | Type | Default |
|---|---|---|
| `data` | list of numbers | written first, in order |
| `labels` | list of text | `[]` |
| `width` | number | `0` |
| `height` | number | `0` |
| `min` | number | `0` |
| `max` | number | `0` |
| `axis` | true/false | `false` |
| `color` | text | `""` |
| `series` | list of lists of numbers | `[]` |
| `colors` | list of text | `[]` |

### `progress`

A track filled to `value` (0 to 1); `indeterminate` sweeps instead, for work with no known length.

Sizes its own `width` / `height`: those are the element's, and the shared keyword leaves them alone.

Its own `label` is what a screen reader reads, so the shared `a11y_label` is not offered here.

| Keyword | Type | Default |
|---|---|---|
| `value` | number | written first, in order |
| `width` | number | `0` |
| `height` | number | `0` |
| `label` | text | `""` |
| `indeterminate` | true/false | `false` |

## Pictures and the canvas

### `image`

A picture from a file.

Sizes its own `width` / `height`: those are the element's, and the shared keyword leaves them alone.

| Keyword | Type | Default |
|---|---|---|
| `source` | text | written first, in order |
| `width` | number | `0` |
| `height` | number | `0` |

### `svg`

A drawing from an SVG file, painted at any size.

Sizes its own `width` / `height`: those are the element's, and the shared keyword leaves them alone.

| Keyword | Type | Default |
|---|---|---|
| `source` | text | written first, in order |
| `width` | number | `0` |
| `height` | number | `0` |

### `canvas`

A grid of virtual pixels, painted by the commands written in its block. A color here is a NUMBER: the index of a color in `palette`, which is how drawing code written for a pixel machine ports line for line. `scale` is how many logical pixels one virtual pixel takes.

Sizes its own `width` / `height`: those are the element's, and the shared keyword leaves them alone.

| Keyword | Type | Default |
|---|---|---|
| `width` | whole number | written first, in order |
| `height` | whole number | written first, in order |
| `scale` | whole number | `1` |
| `background` | whole number | `0` |
| `palette` | list of text | `[]` |

## The small pieces

### `spacer`

Takes the space its parent has left over; 0 is one share.

| Keyword | Type | Default |
|---|---|---|
| `grow` | number | `0` |

### `divider`

A rule across its parent: level in a column, upright in a row.

| Keyword | Type | Default |
|---|---|---|
| `color` | text | `""` |
| `thickness` | number | `0` |

### `spinner`

A turning ring, for work with no known length.

| Keyword | Type | Default |
|---|---|---|
| `size` | number | `0` |

## Adding one

An element is a row in `elements.toml` and an arm in the engine's
`materialize`. `tools/gen.rb` writes the Ruby, the numbers and the
Rust constants from the table, and a test fails when a key has no
arm on the other side — so an element cannot come to mean one
thing in Ruby and another where it is drawn.
