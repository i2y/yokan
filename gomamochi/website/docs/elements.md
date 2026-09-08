<!-- Written by website/tools/elementspage from the table. Edit the table. -->
# Elements

33 elements, and the 15 methods every one of them has. This page is
written from `elements.toml`, the one table `go run ./tools/gen` writes
the Go an app calls from — `elements.go`, one type per element and one
method per keyword — together with the interpreter's view of the package
and the numbers both sides of the engine's C face count with. A method
that is not here is one an app cannot write, and Go itself says so: a
call to it is a type error, in `gomamochi check` and in `go build` alike.

Types on this page are Go's: **string**, **float64**, **int** and
**bool**; a list of strings or of numbers is written as a variadic
argument, `...string` or `...float64`, a list of lists as `[][]float64`,
and the closure that builds row i as `func(int) Element`. A **handler**
is a closure handed to the method the element names for it, taking what
the event carries: `func()`, `func(string)`, `func(bool)`, `func(int)` or
`func(float64)`; see [Handlers](tour-logic.md#handlers).

## The methods every element has

These fifteen are methods on every element, under one name and one
meaning. They are written once, on the box every element is built over,
rather than repeated on thirty of them — which is why an element that
owns one of the names under its own meaning keeps it: a `Text`'s
`.Width` is the text's own method, and it shadows the shared one.

| Method | Type | Default |
|---|---|---|
| `.Width` | `float64` | — |
| `.Height` | `float64` | — |
| `.MinWidth` | `float64` | — |
| `.MaxWidth` | `float64` | — |
| `.Disabled` | `bool` | `false` |
| `.Theme` | `string` | `""` |
| `.Animate` | `float64` | `0` |
| `.Easing` | `string` | `""` |
| `.Enter` | `bool` | `false` |
| `.Exit` | `bool` | `false` |
| `.ColSpan` | `int` | `1` |
| `.RowSpan` | `int` | `1` |
| `.Role` | `string` | `""` |
| `.A11yLabel` | `string` | `""` |
| `.Tooltip` | `string` | `""` |

## Text, buttons, links

### Text

A run of text. `wrap` is "", "nowrap" or "ellipsis"; a background with padding and a radius makes a pill.

Written as `Text(text string)`.

Sizes itself with its own `.Width`: those are the element's, and the shared methods leave them alone.

| Method | Type | Default |
|---|---|---|
| `.Size` | `float64` | `0` |
| `.Color` | `string` | `""` |
| `.Align` | `string` | `""` |
| `.Grow` | `float64` | `0` |
| `.Bold` | `bool` | `false` |
| `.Italic` | `bool` | `false` |
| `.Mono` | `bool` | `false` |
| `.Underline` | `bool` | `false` |
| `.Wrap` | `string` | `""` |
| `.MaxLines` | `int` | `0` |
| `.Width` | `float64` | `0` |
| `.Background` | `string` | `""` |
| `.Padding` | `float64` | `0` |
| `.BorderRadius` | `float64` | `0` |
| `.BorderWidth` | `float64` | `0` |
| `.BorderColor` | `string` | `""` |

### Button

A button. The block runs when it is pressed.

Written as `Button(label string)`.

Sizes itself with its own `.Width` / `.Height`: those are the element's, and the shared methods leave them alone.

| Method | Type | Default |
|---|---|---|
| `.OnClick` | `func()` | — |
| `.Width` | `float64` | `0` |
| `.Height` | `float64` | `0` |
| `.Size` | `float64` | `0` |
| `.Background` | `string` | `""` |
| `.Grow` | `float64` | `0` |
| `.Color` | `string` | `""` |
| `.HoverBackground` | `string` | `""` |
| `.ActiveBackground` | `string` | `""` |
| `.BorderRadius` | `float64` | `0` |
| `.BorderWidth` | `float64` | `0` |
| `.BorderColor` | `string` | `""` |
| `.Basis` | `float64` | `0` |

### Link

Text that opens a page when clicked. There is no handler: opening a page is not the app's state.

Written as `Link(label string, url string)`.

| Method | Type | Default |
|---|---|---|
| `.Size` | `float64` | `0` |

## The boxes that arrange

### Column

Its children down the page.

Written as `Column(kids ...Element)`.

Takes elements as its children, written as its last arguments.

| Method | Type | Default |
|---|---|---|
| `.Spacing` | `float64` | `-1` |
| `.Padding` | `float64` | `0` |
| `.Background` | `string` | `""` |
| `.Grow` | `float64` | `0` |
| `.BorderRadius` | `float64` | `0` |
| `.BorderWidth` | `float64` | `0` |
| `.BorderColor` | `string` | `""` |

### Row

Its children across the page.

Written as `Row(kids ...Element)`.

Takes elements as its children, written as its last arguments.

| Method | Type | Default |
|---|---|---|
| `.Spacing` | `float64` | `-1` |
| `.Padding` | `float64` | `0` |
| `.Background` | `string` | `""` |
| `.Grow` | `float64` | `0` |
| `.BorderRadius` | `float64` | `0` |
| `.BorderWidth` | `float64` | `0` |
| `.BorderColor` | `string` | `""` |

### Grid

Its children on tracks. `columns` counts the tracks; `col_span` on a child covers more than one.

Written as `Grid(kids ...Element)`.

Takes elements as its children, written as its last arguments.

| Method | Type | Default |
|---|---|---|
| `.Columns` | `int` | `2` |
| `.Rows` | `int` | `0` |
| `.Spacing` | `float64` | `-1` |
| `.Padding` | `float64` | `0` |
| `.Background` | `string` | `""` |
| `.Grow` | `float64` | `0` |
| `.BorderRadius` | `float64` | `0` |
| `.BorderWidth` | `float64` | `0` |
| `.BorderColor` | `string` | `""` |

### GridCell

The span written out: this and `col_span:` on the child itself are the same tree.

Written as `GridCell(kids ...Element)`.

Takes elements as its children, written as its last arguments.

No methods of its own.

### Stack

Its children on top of one another.

Written as `Stack(kids ...Element)`.

Takes elements as its children, written as its last arguments.

No methods of its own.

### ScrollView

A pane that scrolls when its children do not fit.

Written as `ScrollView(kids ...Element)`.

Takes elements as its children, written as its last arguments.

Sizes itself with its own `.Height`: those are the element's, and the shared methods leave them alone.

| Method | Type | Default |
|---|---|---|
| `.Height` | `float64` | `0` |

### HScrollView

A pane that scrolls sideways.

Written as `HScrollView(kids ...Element)`.

Takes elements as its children, written as its last arguments.

No methods of its own.

### Modal

A panel over the rest of the window while `open`.

Written as `Modal(kids ...Element)`.

Takes elements as its children, written as its last arguments.

| Method | Type | Default |
|---|---|---|
| `.Open` | `bool` | `true` |

## Fields and choosers

### TextField

A line a person types into. `on_change` fires per keystroke, `on_submit` when they press enter; `multiline` makes it a paragraph field.

Written as `TextField(value string)`.

| Method | Type | Default |
|---|---|---|
| `.Placeholder` | `string` | `""` |
| `.OnChange` | `func(string)` | — |
| `.OnSubmit` | `func(string)` | — |
| `.Multiline` | `bool` | `false` |
| `.Rows` | `float64` | `0` |

### NumberField

A field for a number: enter or leaving it commits, text that is not a number is dropped. `min`/`max` both 0 is unbounded, `step` 0 is free.

Written as `NumberField(value float64)`.

| Method | Type | Default |
|---|---|---|
| `.Min` | `float64` | `0` |
| `.Max` | `float64` | `0` |
| `.Step` | `float64` | `0` |
| `.Placeholder` | `string` | `""` |
| `.OnChange` | `func(float64)` | — |

### IntField

The same field for a whole number.

Written as `IntField(value int)`.

| Method | Type | Default |
|---|---|---|
| `.Min` | `int` | `0` |
| `.Max` | `int` | `0` |
| `.Step` | `int` | `1` |
| `.Placeholder` | `string` | `""` |
| `.OnChange` | `func(int)` | — |

### Checkbox

A box a person ticks. The block receives the new state.

Written as `Checkbox(label string)`.

Its own `label` is what a screen reader reads; `.A11yLabel` is refused on it, and stops the app with that reason.

| Method | Type | Default |
|---|---|---|
| `.Checked` | `bool` | `false` |
| `.OnChange` | `func(bool)` | — |

### Switch

A switch a person flips. The block receives the new state.

Written as `Switch(label string)`.

Its own `label` is what a screen reader reads; `.A11yLabel` is refused on it, and stops the app with that reason.

| Method | Type | Default |
|---|---|---|
| `.Checked` | `bool` | `false` |
| `.OnChange` | `func(bool)` | — |

### Slider

A track a person drags. The block receives the new number.

Written as `Slider()`.

| Method | Type | Default |
|---|---|---|
| `.Value` | `float64` | `0` |
| `.Min` | `float64` | `0` |
| `.Max` | `float64` | `1` |
| `.Step` | `float64` | `0` |
| `.OnChange` | `func(float64)` | — |

### Select

A drop-down. The block receives the chosen index.

Written as `Select()`.

| Method | Type | Default |
|---|---|---|
| `.Options` | `...string` | — |
| `.Selected` | `int` | `0` |
| `.OnChange` | `func(int)` | — |

### RadioGroup

A column of radio buttons. The block receives the chosen index.

Written as `RadioGroup()`.

| Method | Type | Default |
|---|---|---|
| `.Options` | `...string` | — |
| `.Selected` | `int` | `0` |
| `.OnChange` | `func(int)` | — |

### Segmented

A row of joined toggle buttons. The block receives the chosen index.

Written as `Segmented()`.

| Method | Type | Default |
|---|---|---|
| `.Options` | `...string` | — |
| `.Selected` | `int` | `0` |
| `.OnChange` | `func(int)` | — |

### TabBar

A row of tabs. The block receives the chosen index.

Written as `TabBar()`.

| Method | Type | Default |
|---|---|---|
| `.Labels` | `...string` | — |
| `.Active` | `int` | `0` |
| `.OnChange` | `func(int)` | — |

## Lists and tables

### ListView

Rows built on demand: the builder is called for the rows in view, not for all of them.

Written as `ListView(count int, row func(int) Element)`.

Sizes itself with its own `.Height`: those are the element's, and the shared methods leave them alone.

| Method | Type | Default |
|---|---|---|
| `.ItemHeight` | `float64` | `24` |
| `.Height` | `float64` | `0` |
| `.Virtualized` | `bool` | `true` |
| `.Grow` | `float64` | `0` |

### Table

A table whose rows are built on demand, laid on tracks whose shares are `widths`. `on_select` receives the row clicked, `on_sort` the header.

Written as `Table(columns []string, count int, row func(int) Element)`.

Sizes itself with its own `.Height`: those are the element's, and the shared methods leave them alone.

| Method | Type | Default |
|---|---|---|
| `.Widths` | `...float64` | — |
| `.ItemHeight` | `float64` | `24` |
| `.Height` | `float64` | `0` |
| `.Grow` | `float64` | `0` |
| `.Selected` | `int` | `-1` |
| `.OnSelect` | `func(int)` | — |
| `.Sort` | `int` | `-1` |
| `.Descending` | `bool` | `false` |
| `.OnSort` | `func(int)` | — |

### DataTable

The first `row` child is the header; the later ones are data rows, shaded in alternation, in a frame that comes with the element.

Written as `DataTable(kids ...Element)`.

Takes elements as its children, written as its last arguments.

No methods of its own.

## Charts and progress

### BarChart

Bars. `min`/`max` both 0 take the range from the data; `axis` draws ticks and gridlines; `series` draws several groups.

Written as `BarChart(data []float64)`.

Sizes itself with its own `.Width` / `.Height`: those are the element's, and the shared methods leave them alone.

| Method | Type | Default |
|---|---|---|
| `.Labels` | `...string` | — |
| `.Width` | `float64` | `0` |
| `.Height` | `float64` | `0` |
| `.Min` | `float64` | `0` |
| `.Max` | `float64` | `0` |
| `.Axis` | `bool` | `false` |
| `.Color` | `string` | `""` |
| `.Series` | `[][]float64` | — |
| `.Colors` | `...string` | — |

### LineChart

A line. Same arguments as the bars, and `series` draws several lines.

Written as `LineChart(data []float64)`.

Sizes itself with its own `.Width` / `.Height`: those are the element's, and the shared methods leave them alone.

| Method | Type | Default |
|---|---|---|
| `.Labels` | `...string` | — |
| `.Width` | `float64` | `0` |
| `.Height` | `float64` | `0` |
| `.Min` | `float64` | `0` |
| `.Max` | `float64` | `0` |
| `.Axis` | `bool` | `false` |
| `.Color` | `string` | `""` |
| `.Series` | `[][]float64` | — |
| `.Colors` | `...string` | — |

### Progress

A track filled to `value` (0 to 1); `indeterminate` sweeps instead, for work with no known length.

Written as `Progress(value float64)`.

Sizes itself with its own `.Width` / `.Height`: those are the element's, and the shared methods leave them alone.

Its own `label` is what a screen reader reads; `.A11yLabel` is refused on it, and stops the app with that reason.

| Method | Type | Default |
|---|---|---|
| `.Width` | `float64` | `0` |
| `.Height` | `float64` | `0` |
| `.Label` | `string` | `""` |
| `.Indeterminate` | `bool` | `false` |

## Pictures and the canvas

### Image

A picture from a file.

Written as `Image(source string)`.

Sizes itself with its own `.Width` / `.Height`: those are the element's, and the shared methods leave them alone.

| Method | Type | Default |
|---|---|---|
| `.Width` | `float64` | `0` |
| `.Height` | `float64` | `0` |

### Svg

A drawing from an SVG file, painted at any size.

Written as `Svg(source string)`.

Sizes itself with its own `.Width` / `.Height`: those are the element's, and the shared methods leave them alone.

| Method | Type | Default |
|---|---|---|
| `.Width` | `float64` | `0` |
| `.Height` | `float64` | `0` |

### Canvas

A grid of virtual pixels, painted by the commands written in its block. A color here is a NUMBER: the index of a color in `palette`, which is how drawing code written for a pixel machine ports line for line. `scale` is how many logical pixels one virtual pixel takes.

Written as `Canvas(width int, height int)`.

Sizes itself with its own `width` / `height`: those are the element's, and the shared methods leave them alone.

| Method | Type | Default |
|---|---|---|
| `.Scale` | `int` | `1` |
| `.Background` | `int` | `0` |
| `.Palette` | `...string` | — |
| `.Paint` | `func(*Painter)` | — |

## The small pieces

### Spacer

Takes the space its parent has left over; 0 is one share.

Written as `Spacer()`.

| Method | Type | Default |
|---|---|---|
| `.Grow` | `float64` | `0` |

### Divider

A rule across its parent: level in a column, upright in a row.

Written as `Divider()`.

| Method | Type | Default |
|---|---|---|
| `.Color` | `string` | `""` |
| `.Thickness` | `float64` | `0` |

### Spinner

A turning ring, for work with no known length.

Written as `Spinner()`.

| Method | Type | Default |
|---|---|---|
| `.Size` | `float64` | `0` |

## The canvas's drawing commands

The 10 commands below are methods on the `*Painter` a `Canvas`'s
`.Paint` closure is handed. They are not elements: they have none of the
methods above, nothing can click them, and they mean nothing outside the
canvas they are written in. Every coordinate is a whole virtual pixel
and every color is a number, the index of a color in the canvas's
palette. Go has no optional argument, so a value the table gives a
default to is written all the same.

| Command | Written as |
|---|---|
| `Pixel` | `p.Pixel(x int, y int, color int)` |
| `Line` | `p.Line(x1 int, y1 int, x2 int, y2 int, color int)` |
| `Rect` | `p.Rect(x int, y int, w int, h int, color int)` |
| `RectOutline` | `p.RectOutline(x int, y int, w int, h int, color int)` |
| `Circle` | `p.Circle(x int, y int, r int, color int)` |
| `CircleOutline` | `p.CircleOutline(x int, y int, r int, color int)` |
| `Triangle` | `p.Triangle(x1 int, y1 int, x2 int, y2 int, x3 int, y3 int, color int)` |
| `TriangleOutline` | `p.TriangleOutline(x1 int, y1 int, x2 int, y2 int, x3 int, y3 int, color int)` |
| `Sprite` | `p.Sprite(x int, y int, source string, u int, v int, w int, h int, colkey int, flipX bool, flipY bool)` |
| `PixelText` | `p.PixelText(x int, y int, text string, color int)` |

## Adding one

An element is a row in `elements.toml` and an arm in the engine's
`materialize`. `go run ./tools/gen` writes `elements.go` and
`internal/symbols/symbols.go` from it — one type per element, one method
per keyword, and the interpreter's view of the package — and the sweep
fails when either is behind the table, so an element cannot come to mean
one thing in Go and another where it is drawn. The same table is read by
the other three languages on this engine.
