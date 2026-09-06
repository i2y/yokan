# ビューと制御フロー

ビューの中に書くブロックは、普通の Ruby です。
`if`、`unless`、三項演算子、繰り返し、メソッド呼び出し、ローカル変数、どれも使えます。
書いた要素は、書いた場所にそのまま木へ加わります。
テンプレート言語を覚える必要はありません。
そもそもテンプレートを使わないからです。

## ビューの中の制御構造

<!-- script: dump,click:show,dump -->
```ruby
require "wakakusa"

class Control
  def initialize
    @open = false
    @rows = ["one", "two", "three"]
  end

  def line(s, i)
    row(spacing: 6.0) {
      text "#{i + 1}.", width: 22.0
      text s
    }
  end

  def view
    column(spacing: 8.0, padding: 14.0) {
      button(@open ? "hide" : "show") { @open = !@open }
      if @open
        @rows.each_with_index { |s, i| line(s, i) }
      else
        text "#{@rows.length} rows", color: "#8a8f98"
      end
      divider
      text "done"
    }
  end
end

run(Control.new, title: "control")
```

どちらかの枝で何も書かなければ、そこには何も出ません。
場所を埋めるための要素を置く必要も、`nil` を避ける書き方も要りません。
ブロックから呼んだメソッドは、開いているコンテナに書き込みます。
上の `line` に、どこへ入るかを伝える引数がないのはそのためです。

## ビューに書けない形

要素のブロックを繰り返しのブロックの中に書くことはできません。
コンパイルされた実行では、そのブロックを実行する時点で繰り返しの変数がもう残っていないからです。

```console
$ wakakusa check app.rb
app.rb:12:22: Wakakusa cannot take this — a block on an element cannot be written inside a loop: a compiled run has lost the loop's variables by the time it runs. Move this into a method that takes what it needs (`def line(item, i)`), and call that from the loop
        button(name) { @picked = i }
                     ^
```

上の例で `line` をメソッドにしてあるのはそのためです。
繰り返しは、行に必要なものを渡してそのメソッドを呼びます。
ボタンのブロックが見ているのは、繰り返しの変数ではなくメソッドの引数です。

`demo/control.rb` が、この話をまるごと一画面にしたものです。

## 入力の部品

どれも表示する値を引数に取り、変化した値はブロックが受け取ります。

```ruby
  text_field(@name, placeholder: "name") { |s| @name = s }
  int_field(@qty, min: 0, max: 99) { |n| @qty = n }
  number_field(@rate, min: 0.0, max: 1.0, step: 0.05) { |v| @rate = v }
  checkbox("ready", checked: @ready) { |on| @ready = on }
  switch("dark", checked: @dark) { |on| @dark = on }
  slider(value: @vol, min: 0.0, max: 1.0) { |v| @vol = v }
  select(options: ["red", "green", "blue"], selected: @pick) { |i| @pick = i }
  radio_group(options: ["one", "two"], selected: @pick) { |i| @pick = i }
  segmented(options: ["day", "week"], selected: @span) { |i| @span = i }
  tab_bar(labels: ["files", "settings"], active: @tab) { |i| @tab = i }
```

知らないうちに結び付けられるものはありません。
`@name` と書いたからその欄には `@name` が表示され、値を書き戻すのはブロックです。

選択の四つ、`select`、`radio_group`、`segmented`、`tab_bar` は、どれも渡した一覧の何番目かを答えます。
だからアプリが持つのは一覧と番号で、選ばれた文字列のコピーではありません。

`text_field` は `multiline: true` と `rows:` で段落の欄になります。
数を持つ二つの欄は、改行するか欄を離れると確定し、数でない文字は捨て、`min` と `max` に収め、`step` に合わせます。
`demo/quantities.rb` が、その動きをスクリプトで確かめたものです。

## ハンドラ

要素のハンドラは、普通はそのブロックです。
ハンドラを二つ持つ要素では、ブロックはすでに一つ目に使われています。
そこで二つ目は、引数を取らない proc をキーワード引数に渡して書きます。
イベントが運んできた値は、その proc の中で自分で取り出します。

```ruby
  text_field(@draft, on_submit: -> { add(event_text) }) { |s| @draft = s }
```

`event_text`、`event_number`、`event_index`、`event_on?` が、今届いているイベントの中身を返します。
キーワード引数で渡した proc は、コンパイルされた実行では引数を受け取りません。
値を自分で取り出すのは、そのためです。

ハンドラの渡し方はその二つだけです。
メソッド名のシンボルを渡す書き方は、書き換え方つきで断ります（[若草が断る書き方](refusals.md)）。

ハンドラからはアプリのメソッドを自由に呼べます。
一行に収まらない処理は、そこに置きます。

<!-- script: input:eggs,submit,dump,click:done,dump -->
```ruby
require "wakakusa"

class Todo
  def initialize
    @items = ["milk"]
    @draft = ""
    @done = -1
  end

  def add(t)
    items = @items.dup
    items.push(t)
    @items = items
    @draft = ""
  end

  def line(i)
    cells = [text("#{i + 1}. #{@items[i]}")]
    cells.push(text("done", color: "accent")) if i == @done
    cells.push(button("done") { @done = i })
    row(*cells, spacing: 8.0)
  end

  def view
    column(
      text("todo — #{@items.length} items", size: 16.0),
      text_field(@draft, placeholder: "add and press enter",
                 on_submit: -> { add(event_text) }) { |t| @draft = t },
      list_view(@items.length, item_height: 26.0, height: 280.0) { |i| line(i) },
      button("clear") { @items = [] },
      spacing: 10.0,
      padding: 14.0
    )
  end
end

run(Todo.new, title: "todo")
```

`add` の書き方に注目してください。
`dup` で写しを作り、`push` して、書き戻しています。
フィールドの配列に `@items + [t]` で足す書き方は断ります。
それが何を返すかが、二つの実行で一致しないからです。

## 必要な行だけ作る一覧

十万行の一覧は、十万個の要素ではありません。
`list_view` と `table` が受け取るのは、行数と、`i` 番目の行を作るブロックです。
組み立てられるのは、画面に出ている行だけです。

```ruby
  list_view(@rows.length, item_height: 26.0, height: 280.0) { |i| line(i) }

  table(["name", "team", "score"], @names.length,
        widths: [2.0, 1.0, 1.0], height: 220.0,
        selected: @sel, sort: @sort, descending: @desc,
        on_select: -> { pick(event_index) },
        on_sort: -> { sort_by(event_index) }) { |i| line(i) }
```

行のブロックは行番号を渡して呼ばれ、返したものがその行になります。
画面を描いている最中に呼ばれるので、ブロックの中では状態を読むだけにとどめ、書き換えません。

`table` は見出しの行を自分で描き、見出しと行を同じ列の幅で並べます。
その幅の比が `widths:` です。
どの行が選ばれているか（`selected:`）、どの列に並べ替えの印が付くか（`sort:`）、その向き（`descending:`）は、どれもアプリが持つ値です。
要素はそれを描くだけで、並べ替えそのものはアプリのメソッドが行います。
`demo/roster.rb` が二十四人をそうやって並べ替え、`demo/csv_viewer.rb` は十万行を打ちながら絞り込みます。

行数が少なく、すでに手元にある場合は、`data_table` が子として受け取ります。
最初の `row` が見出しで、以降がデータ行になり、交互に色が変わります。
枠は要素が持っています。

```ruby
  data_table(
    row(text("service", grow: 2.0), text("latency", grow: 1.0, align: "right")),
    row(text("api", grow: 2.0), text("42 ms", grow: 1.0, align: "right"))
  )
```

列が揃うのは、同じ列のセルが同じ `grow` の比を持っているからです。

## グラフ

グラフは二つとも、数のリストを第一引数に取ります。
だから、数を計算してから描くビューが上から順に読めます。

```ruby
  bar_chart(@totals, labels: ["food", "transit", "fun"], axis: true, height: 90.0)
  line_chart(@series, min: 0.0, max: 100.0, height: 120.0)
```

`min:` と `max:` がどちらも 0 なら、範囲はデータから決まります。
0 より小さい値があれば、その棒は線の下に垂れます。
`axis:` は目盛りの文字と、薄い補助線を引きます。
`series:` には線ごと、あるいは組ごとに一つの一覧を渡し、`colors:` にはそれぞれの色を渡します。
`demo/charts.rb` が、その両方を一画面にしたものです。

## 次に読むもの

- [キャンバスとキーボード](tour-canvas.md)：絵を描く面と、装置として読むキー。
- [見た目とウィンドウ](tour-ui.md)：すべての要素が取るキーワード、テーマ、ウィンドウそのものが渡すもの。
