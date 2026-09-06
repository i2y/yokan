# 要素

要素は 33 個、そのすべてが受け取る共通のキーワードが 15 個あります。
このページは `elements.toml` から生成しています。
アプリが呼ぶ Ruby のメソッドも、エンジンの C 面で両側が数える番号も、エンジン側の定数も、同じ表から書き出されます。
ここにないキーワードはアプリが書けないキーワードで、`wakakusa check` がその名前を挙げて断ります。

このページでの型の読み方です。
**文字列** は string、**数** は float、**整数** は integer、**真偽** は boolean、**〜のリスト** はそれぞれのリストです。
**ハンドラ** はブロック、またはキーワードで渡す引数なしの proc です（[ハンドラ](tour-logic.md#ハンドラ)）。

## すべての要素が受け取るキーワード

この 15 個は、どの要素にも同じ名前と同じ意味で乗ります。
30 個の要素にフィールドを繰り返すのではなく、要素を包む形で実装しているからです。
同じ名前を要素自身が別の意味で持っている場合は、要素のものが優先されます。
`text` の `width` は文字列そのものの幅で、外側の箱は手を出しません。

| キーワード | 型 | 既定値 |
|---|---|---|
| `width` | 数 | — |
| `height` | 数 | — |
| `min_width` | 数 | — |
| `max_width` | 数 | — |
| `disabled` | 真偽 | `false` |
| `theme` | 文字列 | `""` |
| `animate` | 数 | `0` |
| `easing` | 文字列 | `""` |
| `enter` | 真偽 | `false` |
| `exit` | 真偽 | `false` |
| `col_span` | 整数 | `1` |
| `row_span` | 整数 | `1` |
| `role` | 文字列 | `""` |
| `a11y_label` | 文字列 | `""` |
| `tooltip` | 文字列 | `""` |

## 文字とボタンとリンク

### `text`

A run of text. `wrap` is "", "nowrap" or "ellipsis"; a background with padding and a radius makes a pill.

`width` は自分で決めます。
共通キーワードは手を出しません。

| キーワード | 型 | 既定値 |
|---|---|---|
| `text` | 文字列 | 先頭に、この順で書く |
| `size` | 数 | `0` |
| `color` | 文字列 | `""` |
| `align` | 文字列 | `""` |
| `grow` | 数 | `0` |
| `bold` | 真偽 | `false` |
| `italic` | 真偽 | `false` |
| `mono` | 真偽 | `false` |
| `underline` | 真偽 | `false` |
| `wrap` | 文字列 | `""` |
| `max_lines` | 整数 | `0` |
| `width` | 数 | `0` |
| `background` | 文字列 | `""` |
| `padding` | 数 | `0` |
| `border_radius` | 数 | `0` |
| `border_width` | 数 | `0` |
| `border_color` | 文字列 | `""` |

### `button`

A button. The block runs when it is pressed.

`width` / `height` は自分で決めます。
共通キーワードは手を出しません。

| キーワード | 型 | 既定値 |
|---|---|---|
| `label` | 文字列 | 先頭に、この順で書く |
| `on_click` | ハンドラ | — |
| `width` | 数 | `0` |
| `height` | 数 | `0` |
| `size` | 数 | `0` |
| `background` | 文字列 | `""` |
| `grow` | 数 | `0` |
| `color` | 文字列 | `""` |
| `hover_background` | 文字列 | `""` |
| `active_background` | 文字列 | `""` |
| `border_radius` | 数 | `0` |
| `border_width` | 数 | `0` |
| `border_color` | 文字列 | `""` |
| `basis` | 数 | `0` |

### `link`

Text that opens a page when clicked. There is no handler: opening a page is not the app's state.

| キーワード | 型 | 既定値 |
|---|---|---|
| `label` | 文字列 | 先頭に、この順で書く |
| `url` | 文字列 | 先頭に、この順で書く |
| `size` | 数 | `0` |

## 並べる箱

### `column`

Its children down the page.

要素を子に取ります（引数として、またはブロックとして）。

| キーワード | 型 | 既定値 |
|---|---|---|
| `spacing` | 数 | `-1` |
| `padding` | 数 | `0` |
| `background` | 文字列 | `""` |
| `grow` | 数 | `0` |
| `border_radius` | 数 | `0` |
| `border_width` | 数 | `0` |
| `border_color` | 文字列 | `""` |

### `row`

Its children across the page.

要素を子に取ります（引数として、またはブロックとして）。

| キーワード | 型 | 既定値 |
|---|---|---|
| `spacing` | 数 | `-1` |
| `padding` | 数 | `0` |
| `background` | 文字列 | `""` |
| `grow` | 数 | `0` |
| `border_radius` | 数 | `0` |
| `border_width` | 数 | `0` |
| `border_color` | 文字列 | `""` |

### `grid`

Its children on tracks. `columns` counts the tracks; `col_span` on a child covers more than one.

要素を子に取ります（引数として、またはブロックとして）。

| キーワード | 型 | 既定値 |
|---|---|---|
| `columns` | 整数 | `2` |
| `rows` | 整数 | `0` |
| `spacing` | 数 | `-1` |
| `padding` | 数 | `0` |
| `background` | 文字列 | `""` |
| `grow` | 数 | `0` |
| `border_radius` | 数 | `0` |
| `border_width` | 数 | `0` |
| `border_color` | 文字列 | `""` |

### `grid_cell`

The span written out: this and `col_span:` on the child itself are the same tree.

要素を子に取ります（引数として、またはブロックとして）。

固有のキーワードはありません。

### `stack`

Its children on top of one another.

要素を子に取ります（引数として、またはブロックとして）。

固有のキーワードはありません。

### `scroll_view`

A pane that scrolls when its children do not fit.

要素を子に取ります（引数として、またはブロックとして）。

`height` は自分で決めます。
共通キーワードは手を出しません。

| キーワード | 型 | 既定値 |
|---|---|---|
| `height` | 数 | `0` |

### `h_scroll_view`

A pane that scrolls sideways.

要素を子に取ります（引数として、またはブロックとして）。

固有のキーワードはありません。

### `modal`

A panel over the rest of the window while `open`.

要素を子に取ります（引数として、またはブロックとして）。

| キーワード | 型 | 既定値 |
|---|---|---|
| `open` | 真偽 | `true` |

## 入力と選択

### `text_field`

A line a person types into. `on_change` fires per keystroke, `on_submit` when they press enter; `multiline` makes it a paragraph field.

| キーワード | 型 | 既定値 |
|---|---|---|
| `value` | 文字列 | 先頭に、この順で書く |
| `placeholder` | 文字列 | `""` |
| `on_change` | ハンドラ | — |
| `on_submit` | ハンドラ | — |
| `multiline` | 真偽 | `false` |
| `rows` | 数 | `0` |

### `number_field`

A field for a number: enter or leaving it commits, text that is not a number is dropped. `min`/`max` both 0 is unbounded, `step` 0 is free.

| キーワード | 型 | 既定値 |
|---|---|---|
| `value` | 数 | 先頭に、この順で書く |
| `min` | 数 | `0` |
| `max` | 数 | `0` |
| `step` | 数 | `0` |
| `placeholder` | 文字列 | `""` |
| `on_change` | ハンドラ | — |

### `int_field`

The same field for a whole number.

| キーワード | 型 | 既定値 |
|---|---|---|
| `value` | 整数 | 先頭に、この順で書く |
| `min` | 整数 | `0` |
| `max` | 整数 | `0` |
| `step` | 整数 | `1` |
| `placeholder` | 文字列 | `""` |
| `on_change` | ハンドラ | — |

### `checkbox`

A box a person ticks. The block receives the new state.

自身の `label` が画面読み上げの読む名前なので、共通の `a11y_label` はここでは受け取りません。

| キーワード | 型 | 既定値 |
|---|---|---|
| `label` | 文字列 | 先頭に、この順で書く |
| `checked` | 真偽 | `false` |
| `on_change` | ハンドラ | — |

### `switch`

A switch a person flips. The block receives the new state.

自身の `label` が画面読み上げの読む名前なので、共通の `a11y_label` はここでは受け取りません。

| キーワード | 型 | 既定値 |
|---|---|---|
| `label` | 文字列 | 先頭に、この順で書く |
| `checked` | 真偽 | `false` |
| `on_change` | ハンドラ | — |

### `slider`

A track a person drags. The block receives the new number.

| キーワード | 型 | 既定値 |
|---|---|---|
| `value` | 数 | `0` |
| `min` | 数 | `0` |
| `max` | 数 | `1` |
| `step` | 数 | `0` |
| `on_change` | ハンドラ | — |

### `select`

A drop-down. The block receives the chosen index.

| キーワード | 型 | 既定値 |
|---|---|---|
| `options` | 文字列のリスト | `[]` |
| `selected` | 整数 | `0` |
| `on_change` | ハンドラ | — |

### `radio_group`

A column of radio buttons. The block receives the chosen index.

| キーワード | 型 | 既定値 |
|---|---|---|
| `options` | 文字列のリスト | `[]` |
| `selected` | 整数 | `0` |
| `on_change` | ハンドラ | — |

### `segmented`

A row of joined toggle buttons. The block receives the chosen index.

| キーワード | 型 | 既定値 |
|---|---|---|
| `options` | 文字列のリスト | `[]` |
| `selected` | 整数 | `0` |
| `on_change` | ハンドラ | — |

### `tab_bar`

A row of tabs. The block receives the chosen index.

| キーワード | 型 | 既定値 |
|---|---|---|
| `labels` | 文字列のリスト | `[]` |
| `active` | 整数 | `0` |
| `on_change` | ハンドラ | — |

## リストと表

### `list_view`

Rows built on demand: the builder is called for the rows in view, not for all of them.

`height` は自分で決めます。
共通キーワードは手を出しません。

| キーワード | 型 | 既定値 |
|---|---|---|
| `count` | 整数 | 先頭に、この順で書く |
| `row` | 個数と、i 行目を作るブロック | 先頭に、この順で書く |
| `item_height` | 数 | `24` |
| `height` | 数 | `0` |
| `virtualized` | 真偽 | `true` |
| `grow` | 数 | `0` |

### `table`

A table whose rows are built on demand, laid on tracks whose shares are `widths`. `on_select` receives the row clicked, `on_sort` the header.

`height` は自分で決めます。
共通キーワードは手を出しません。

| キーワード | 型 | 既定値 |
|---|---|---|
| `columns` | 文字列のリスト | 先頭に、この順で書く |
| `count` | 整数 | 先頭に、この順で書く |
| `row` | 個数と、i 行目を作るブロック | 先頭に、この順で書く |
| `widths` | 数のリスト | `[]` |
| `item_height` | 数 | `24` |
| `height` | 数 | `0` |
| `grow` | 数 | `0` |
| `selected` | 整数 | `-1` |
| `on_select` | ハンドラ | — |
| `sort` | 整数 | `-1` |
| `descending` | 真偽 | `false` |
| `on_sort` | ハンドラ | — |

### `data_table`

The first `row` child is the header; the later ones are data rows, shaded in alternation, in a frame that comes with the element.

要素を子に取ります（引数として、またはブロックとして）。

固有のキーワードはありません。

## グラフと進捗

### `bar_chart`

Bars. `min`/`max` both 0 take the range from the data; `axis` draws ticks and gridlines; `series` draws several groups.

`width` / `height` は自分で決めます。
共通キーワードは手を出しません。

| キーワード | 型 | 既定値 |
|---|---|---|
| `data` | 数のリスト | 先頭に、この順で書く |
| `labels` | 文字列のリスト | `[]` |
| `width` | 数 | `0` |
| `height` | 数 | `0` |
| `min` | 数 | `0` |
| `max` | 数 | `0` |
| `axis` | 真偽 | `false` |
| `color` | 文字列 | `""` |
| `series` | 数のリストのリスト | `[]` |
| `colors` | 文字列のリスト | `[]` |

### `line_chart`

A line. Same arguments as the bars, and `series` draws several lines.

`width` / `height` は自分で決めます。
共通キーワードは手を出しません。

| キーワード | 型 | 既定値 |
|---|---|---|
| `data` | 数のリスト | 先頭に、この順で書く |
| `labels` | 文字列のリスト | `[]` |
| `width` | 数 | `0` |
| `height` | 数 | `0` |
| `min` | 数 | `0` |
| `max` | 数 | `0` |
| `axis` | 真偽 | `false` |
| `color` | 文字列 | `""` |
| `series` | 数のリストのリスト | `[]` |
| `colors` | 文字列のリスト | `[]` |

### `progress`

A track filled to `value` (0 to 1); `indeterminate` sweeps instead, for work with no known length.

`width` / `height` は自分で決めます。
共通キーワードは手を出しません。

自身の `label` が画面読み上げの読む名前なので、共通の `a11y_label` はここでは受け取りません。

| キーワード | 型 | 既定値 |
|---|---|---|
| `value` | 数 | 先頭に、この順で書く |
| `width` | 数 | `0` |
| `height` | 数 | `0` |
| `label` | 文字列 | `""` |
| `indeterminate` | 真偽 | `false` |

## 画像とキャンバス

### `image`

A picture from a file.

`width` / `height` は自分で決めます。
共通キーワードは手を出しません。

| キーワード | 型 | 既定値 |
|---|---|---|
| `source` | 文字列 | 先頭に、この順で書く |
| `width` | 数 | `0` |
| `height` | 数 | `0` |

### `svg`

A drawing from an SVG file, painted at any size.

`width` / `height` は自分で決めます。
共通キーワードは手を出しません。

| キーワード | 型 | 既定値 |
|---|---|---|
| `source` | 文字列 | 先頭に、この順で書く |
| `width` | 数 | `0` |
| `height` | 数 | `0` |

### `canvas`

A grid of virtual pixels, painted by the commands written in its block. A color here is a NUMBER: the index of a color in `palette`, which is how drawing code written for a pixel machine ports line for line. `scale` is how many logical pixels one virtual pixel takes.

`width` / `height` は自分で決めます。
共通キーワードは手を出しません。

| キーワード | 型 | 既定値 |
|---|---|---|
| `width` | 整数 | 先頭に、この順で書く |
| `height` | 整数 | 先頭に、この順で書く |
| `scale` | 整数 | `1` |
| `background` | 整数 | `0` |
| `palette` | 文字列のリスト | `[]` |

## 小さな部品

### `spacer`

Takes the space its parent has left over; 0 is one share.

| キーワード | 型 | 既定値 |
|---|---|---|
| `grow` | 数 | `0` |

### `divider`

A rule across its parent: level in a column, upright in a row.

| キーワード | 型 | 既定値 |
|---|---|---|
| `color` | 文字列 | `""` |
| `thickness` | 数 | `0` |

### `spinner`

A turning ring, for work with no known length.

| キーワード | 型 | 既定値 |
|---|---|---|
| `size` | 数 | `0` |

## 要素を足すとき

要素を足す作業は、`elements.toml` に 1 行足すことと、エンジンの `materialize` に分岐を 1 つ足すことです。
`tools/gen.rb` が表から Ruby と番号と Rust の定数を書き出し、片側に分岐のないキーが残っていればテストが落ちます。
だから、ある要素が Ruby では 1 つの意味を持ち、描かれる側では別の意味を持つ、ということが起きません。
