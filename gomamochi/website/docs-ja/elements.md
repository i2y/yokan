<!-- Written by website/tools/elementspage from the table. Edit the table. -->
# 要素

要素は 33 個、そのすべてが持つ共通のメソッドが 15 個あります。
このページは `elements.toml` から生成しています。
アプリが呼ぶ Go（`elements.go`、要素ごとの型とキーワードごとのメソッド）も、インタプリタに見せるパッケージの一覧も、エンジンの C API で両側が数える番号も、同じ表から `go run ./tools/gen` が書き出します。
ここにないメソッドは、アプリには書けません。
書いた場合は、Go 自身が型の誤りとして弾きます。
`gomamochi check` でも `go build` でも変わりません。

このページでの型の読み方です。
型は Go のものをそのまま書きます（**string**、**float64**、**int**、**bool**）。
文字列や数のリストは可変長引数（`...string`、`...float64`）、リストのリストは `[][]float64`、i 行目を作るクロージャは `func(int) Element` です。
**ハンドラ** は、要素がそのために用意したメソッドに渡すクロージャで、その出来事が運ぶものを受け取ります（`func()`、`func(string)`、`func(bool)`、`func(int)`、`func(float64)`）。
詳しくは[ハンドラ](tour-logic.md#ハンドラ)を見てください。

## すべての要素が持つメソッド

どの要素も、この 15 個を同じ名前と同じ意味で持ちます。
30 個の要素それぞれに同じメソッドを足すのではなく、どの要素も土台にしている一つの箱に一度だけ書いてあります。
同じ名前を要素自身が別の意味で持っている場合は、要素のものが優先されます。
`Text` の `.Width` は文字列そのものの幅で、共通のメソッドは手を出しません。

| メソッド | 型 | 既定値 |
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

## 文字とボタンとリンク

### Text

A run of text. `wrap` is "", "nowrap" or "ellipsis"; a background with padding and a radius makes a pill.

`Text(text string)` と書きます。

大きさは自分の `.Width` で決めます。
共通のメソッドは手を出しません。

| メソッド | 型 | 既定値 |
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

`Button(label string)` と書きます。

大きさは自分の `.Width` / `.Height` で決めます。
共通のメソッドは手を出しません。

| メソッド | 型 | 既定値 |
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

`Link(label string, url string)` と書きます。

| メソッド | 型 | 既定値 |
|---|---|---|
| `.Size` | `float64` | `0` |

## 並べる箱

### Column

Its children down the page.

`Column(kids ...Element)` と書きます。

要素を子に取ります。
子は最後の引数として書きます。

| メソッド | 型 | 既定値 |
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

`Row(kids ...Element)` と書きます。

要素を子に取ります。
子は最後の引数として書きます。

| メソッド | 型 | 既定値 |
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

`Grid(kids ...Element)` と書きます。

要素を子に取ります。
子は最後の引数として書きます。

| メソッド | 型 | 既定値 |
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

`GridCell(kids ...Element)` と書きます。

要素を子に取ります。
子は最後の引数として書きます。

固有のメソッドはありません。

### Stack

Its children on top of one another.

`Stack(kids ...Element)` と書きます。

要素を子に取ります。
子は最後の引数として書きます。

固有のメソッドはありません。

### ScrollView

A pane that scrolls when its children do not fit.

`ScrollView(kids ...Element)` と書きます。

要素を子に取ります。
子は最後の引数として書きます。

大きさは自分の `.Height` で決めます。
共通のメソッドは手を出しません。

| メソッド | 型 | 既定値 |
|---|---|---|
| `.Height` | `float64` | `0` |

### HScrollView

A pane that scrolls sideways.

`HScrollView(kids ...Element)` と書きます。

要素を子に取ります。
子は最後の引数として書きます。

固有のメソッドはありません。

### Modal

A panel over the rest of the window while `open`.

`Modal(kids ...Element)` と書きます。

要素を子に取ります。
子は最後の引数として書きます。

| メソッド | 型 | 既定値 |
|---|---|---|
| `.Open` | `bool` | `true` |

## 入力と選択

### TextField

A line a person types into. `on_change` fires per keystroke, `on_submit` when they press enter; `multiline` makes it a paragraph field.

`TextField(value string)` と書きます。

| メソッド | 型 | 既定値 |
|---|---|---|
| `.Placeholder` | `string` | `""` |
| `.OnChange` | `func(string)` | — |
| `.OnSubmit` | `func(string)` | — |
| `.Multiline` | `bool` | `false` |
| `.Rows` | `float64` | `0` |

### NumberField

A field for a number: enter or leaving it commits, text that is not a number is dropped. `min`/`max` both 0 is unbounded, `step` 0 is free.

`NumberField(value float64)` と書きます。

| メソッド | 型 | 既定値 |
|---|---|---|
| `.Min` | `float64` | `0` |
| `.Max` | `float64` | `0` |
| `.Step` | `float64` | `0` |
| `.Placeholder` | `string` | `""` |
| `.OnChange` | `func(float64)` | — |

### IntField

The same field for a whole number.

`IntField(value int)` と書きます。

| メソッド | 型 | 既定値 |
|---|---|---|
| `.Min` | `int` | `0` |
| `.Max` | `int` | `0` |
| `.Step` | `int` | `1` |
| `.Placeholder` | `string` | `""` |
| `.OnChange` | `func(int)` | — |

### Checkbox

A box a person ticks. The block receives the new state.

`Checkbox(label string)` と書きます。

画面読み上げが読むのは自分の `label` なので、`.A11yLabel` は受け取りません。
呼ぶと、その理由を出してアプリが止まります。

| メソッド | 型 | 既定値 |
|---|---|---|
| `.Checked` | `bool` | `false` |
| `.OnChange` | `func(bool)` | — |

### Switch

A switch a person flips. The block receives the new state.

`Switch(label string)` と書きます。

画面読み上げが読むのは自分の `label` なので、`.A11yLabel` は受け取りません。
呼ぶと、その理由を出してアプリが止まります。

| メソッド | 型 | 既定値 |
|---|---|---|
| `.Checked` | `bool` | `false` |
| `.OnChange` | `func(bool)` | — |

### Slider

A track a person drags. The block receives the new number.

`Slider()` と書きます。

| メソッド | 型 | 既定値 |
|---|---|---|
| `.Value` | `float64` | `0` |
| `.Min` | `float64` | `0` |
| `.Max` | `float64` | `1` |
| `.Step` | `float64` | `0` |
| `.OnChange` | `func(float64)` | — |

### Select

A drop-down. The block receives the chosen index.

`Select()` と書きます。

| メソッド | 型 | 既定値 |
|---|---|---|
| `.Options` | `...string` | — |
| `.Selected` | `int` | `0` |
| `.OnChange` | `func(int)` | — |

### RadioGroup

A column of radio buttons. The block receives the chosen index.

`RadioGroup()` と書きます。

| メソッド | 型 | 既定値 |
|---|---|---|
| `.Options` | `...string` | — |
| `.Selected` | `int` | `0` |
| `.OnChange` | `func(int)` | — |

### Segmented

A row of joined toggle buttons. The block receives the chosen index.

`Segmented()` と書きます。

| メソッド | 型 | 既定値 |
|---|---|---|
| `.Options` | `...string` | — |
| `.Selected` | `int` | `0` |
| `.OnChange` | `func(int)` | — |

### TabBar

A row of tabs. The block receives the chosen index.

`TabBar()` と書きます。

| メソッド | 型 | 既定値 |
|---|---|---|
| `.Labels` | `...string` | — |
| `.Active` | `int` | `0` |
| `.OnChange` | `func(int)` | — |

## リストと表

### ListView

Rows built on demand: the builder is called for the rows in view, not for all of them.

`ListView(count int, row func(int) Element)` と書きます。

大きさは自分の `.Height` で決めます。
共通のメソッドは手を出しません。

| メソッド | 型 | 既定値 |
|---|---|---|
| `.ItemHeight` | `float64` | `24` |
| `.Height` | `float64` | `0` |
| `.Virtualized` | `bool` | `true` |
| `.Grow` | `float64` | `0` |

### Table

A table whose rows are built on demand, laid on tracks whose shares are `widths`. `on_select` receives the row clicked, `on_sort` the header.

`Table(columns []string, count int, row func(int) Element)` と書きます。

大きさは自分の `.Height` で決めます。
共通のメソッドは手を出しません。

| メソッド | 型 | 既定値 |
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

`DataTable(kids ...Element)` と書きます。

要素を子に取ります。
子は最後の引数として書きます。

固有のメソッドはありません。

## グラフと進捗

### BarChart

Bars. `min`/`max` both 0 take the range from the data; `axis` draws ticks and gridlines; `series` draws several groups.

`BarChart(data []float64)` と書きます。

大きさは自分の `.Width` / `.Height` で決めます。
共通のメソッドは手を出しません。

| メソッド | 型 | 既定値 |
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

`LineChart(data []float64)` と書きます。

大きさは自分の `.Width` / `.Height` で決めます。
共通のメソッドは手を出しません。

| メソッド | 型 | 既定値 |
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

`Progress(value float64)` と書きます。

大きさは自分の `.Width` / `.Height` で決めます。
共通のメソッドは手を出しません。

画面読み上げが読むのは自分の `label` なので、`.A11yLabel` は受け取りません。
呼ぶと、その理由を出してアプリが止まります。

| メソッド | 型 | 既定値 |
|---|---|---|
| `.Width` | `float64` | `0` |
| `.Height` | `float64` | `0` |
| `.Label` | `string` | `""` |
| `.Indeterminate` | `bool` | `false` |

## 画像とキャンバス

### Image

A picture from a file.

`Image(source string)` と書きます。

大きさは自分の `.Width` / `.Height` で決めます。
共通のメソッドは手を出しません。

| メソッド | 型 | 既定値 |
|---|---|---|
| `.Width` | `float64` | `0` |
| `.Height` | `float64` | `0` |

### Svg

A drawing from an SVG file, painted at any size.

`Svg(source string)` と書きます。

大きさは自分の `.Width` / `.Height` で決めます。
共通のメソッドは手を出しません。

| メソッド | 型 | 既定値 |
|---|---|---|
| `.Width` | `float64` | `0` |
| `.Height` | `float64` | `0` |

### Canvas

A grid of virtual pixels, painted by the commands written in its block. A color here is a NUMBER: the index of a color in `palette`, which is how drawing code written for a pixel machine ports line for line. `scale` is how many logical pixels one virtual pixel takes.

`Canvas(width int, height int)` と書きます。

大きさは自分の `width` / `height` で決めます。
共通のメソッドは手を出しません。

| メソッド | 型 | 既定値 |
|---|---|---|
| `.Scale` | `int` | `1` |
| `.Background` | `int` | `0` |
| `.Palette` | `...string` | — |
| `.Paint` | `func(*Painter)` | — |

## 小さな要素

### Spacer

Takes the space its parent has left over; 0 is one share.

`Spacer()` と書きます。

| メソッド | 型 | 既定値 |
|---|---|---|
| `.Grow` | `float64` | `0` |

### Divider

A rule across its parent: level in a column, upright in a row.

`Divider()` と書きます。

| メソッド | 型 | 既定値 |
|---|---|---|
| `.Color` | `string` | `""` |
| `.Thickness` | `float64` | `0` |

### Spinner

A turning ring, for work with no known length.

`Spinner()` と書きます。

| メソッド | 型 | 既定値 |
|---|---|---|
| `.Size` | `float64` | `0` |

## キャンバスの描画命令

次の 10 個の命令は、`Canvas` の `.Paint` に渡すクロージャの中で、受け取った `*Painter` のメソッドとして書きます。
これらは要素ではありません。
上のメソッドをどれも持たず、押すこともできず、書かれたキャンバスの外では意味を持ちません。
座標はすべて仮想的な画素の整数で、色はすべて番号です。
番号はそのキャンバスの配色の何番目か、というだけのものです。
Go に省略できる引数はないので、表に既定値のある値もすべて書きます。

| 命令 | 書き方 |
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

## 要素を足すとき

要素を足す作業は、`elements.toml` に 1 行足すことと、エンジンの `materialize` に分岐を 1 つ足すことです。
`go run ./tools/gen` が、表から `elements.go`（要素ごとの型と、キーワードごとのメソッド）と `internal/symbols/symbols.go`（インタプリタに見せるパッケージの一覧）を書き出します。
どちらかが表より古ければ、`tools/gate_all.sh` が落ちます。
だから、ある要素が Go では 1 つの意味を持ち、描かれる側では別の意味を持つ、ということが起きません。
このエンジンの上にあるほかの三つの言語も、同じ表を読んでいます。
