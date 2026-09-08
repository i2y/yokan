# 見た目とウィンドウ

要素そのものの話のあとに残るのは二つです。
画面をどう並べて何色にするか、そして、要素ではなくウィンドウが用意しているものです。

## 並べる

`column` と `row` は、子を上下と左右に並べます。
並べかたの違うコンテナがもう四つあり、どれも `column` と同じように、子を引数としてもブロックとしても取ります。

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

`grid` は子を `columns:` 本の列に並べます。
一つの子が複数の列にまたがるときは、その子に `col_span:` を書くか、`grid_cell` で包みます。
どちらの書き方でも同じ木になります。
`stack` は子を書いた順に重ねます。
スクロールする二つの面は、子が収まらないときにスクロールします。

`open:` が真のあいだ、`modal` はウィンドウの手前に重なって出ます。
ダイアログはこれで書きます。
出すかどうかを決めるのはアプリです。

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

`demo/panels.rb` が四つのコンテナを一画面に、`demo/dialog.rb` が modal を見せます。

## 小さな要素

```ruby
  spacer(grow: 1.0)                     # 親の余りを取る
  divider(thickness: 2.0, color: "accent")
  spinner(size: 18.0)                   # 終わりの見えない処理に
  progress(0.4, label: "copying…")      # 0 から 1 まで満ちるバー
  progress(0.0, indeterminate: true)    # 同じ処理を、往復で見せる
  link("Website", "https://example.org")
  image("assets/postcard.png", width: 180.0)
  svg("assets/search.svg", width: 20.0, height: 20.0)
```

`row` の中の `spacer` は、続くものを端まで押しやります。
`column` の中では同じことを下に向かって行います。
`link` にハンドラがないのは、ページを開くことがアプリの状態ではないからです。

## すべての要素が取るキーワード

どの要素も、次の共通のプロパティを同じ名前と同じ意味で取ります。

| キーワード | 型 | はたらき |
|---|---|---|
| `width` `height` | 数 | 要素が何であれ、外側の箱が取る大きさ |
| `min_width` `max_width` | 数 | その幅の下限と上限 |
| `disabled` | 真偽 | 触れない見た目になり、入力を受け取らない |
| `theme` | `"dark"` か `"light"` | この要素から下で使う配色 |
| `animate` | 数 | 変化にかける時間、ミリ秒 |
| `easing` | `"linear"` `"in"` `"out"` `"inOut"` | その変化の速さの付け方 |
| `enter` `exit` | 真偽 | 現れるとき、消えるときにも動かす |
| `col_span` `row_span` | 整数 | グリッドで何列ぶん、何行ぶんを占めるか |
| `role` | 文字列 | 画面読み上げがこれを何と呼ぶか |
| `a11y_label` | 文字列 | 画面読み上げが表示文字の代わりに読む名前 |
| `tooltip` | 文字列 | ポインタを重ねたときに出る説明 |

```ruby
  button("save", disabled: @busy, tooltip: "write the file", width: 120.0)
  text "total", role: "heading", a11y_label: "the running total"
```

これらは三十個の要素にフィールドを繰り返したものではなく、要素を包む形で作ってあります。
だから `elements.toml` に一度だけ書けて、どこでも同じ意味になります。

同じ名前をもともと自分の意味で持っている要素では、要素側の意味がそのまま通ります。
`text` の `width` は文字の幅を指し、`image` の `width` と `height` は画像そのものの大きさなので、共通のプロパティは手を出しません。
どの要素がそうなのかは[要素](elements.md)に印をつけてあります。

`demo/shared.rb` は、この共通のプロパティを一つずつ、違う種類の要素に付けたものです。
spacer にテーマ、選択にアニメーション、区切り線に説明、フィールドに二列ぶんの幅、そして欄とボタンを操作できなくする `disabled` です。

## 色

色は 16 進の文字列（`"#f38ba8"`）か、配色の中の名前です。
名前で書けば、その画面はウィンドウ自身のテーマに従います。

`windowBg`、`panel`、`fieldBg`、`surface`、`surfaceHover`、`surfacePressed`、`border`、`text`、`textDim`、`accent`、`selection`、`scrim`、`scrollbar`、`scrollbarActive` があります。

```ruby
  text "saved", color: "accent"
  column(background: "panel", border_color: "border", border_width: 1.0) { … }
```

## テーマとアニメーション

```ruby
  column(theme: "dark") { … }
  segmented(options: ["read", "write"], selected: @tab,
            animate: 120.0, easing: "out") { |i| @tab = i }
  text "saved", animate: 150.0, easing: "out", enter: true
```

`theme:` は、書いた要素から下だけ配色を差し替えます。
決め打ちの文字列だけでなく変数も渡せるので、アプリの側で切り替えられます（`theme: @mode`）。
`animate:` には変化にかける時間をミリ秒で渡し、現れるときと消えるときの動きは `enter:` と `exit:` で指定します。

どちらの実行も、一つの時計で動きます。
ウィンドウではフレームが、スクリプトでは `advance:<ms>` がその時計を進めます。
だからアニメーションは、ゲートが待つものではなく、比べられるものになります。

使い回したい見た目は、普通の Hash です。
merge して、`**` で渡します。

```ruby
KEY = { grow: 1.0, size: 20.0, background: "panel" }.freeze
OP = KEY.merge({ background: "#fab387", color: "#1e1e2e" }).freeze

  button("7", **KEY) { press("7") }
  button("+", **OP) { press("+") }
```

`demo/styled.rb` と二つの電卓（`demo/calc.rb`、`demo/calcgrid.rb`）がその書き方です。

## ウィンドウから受け取るもの

ウィンドウから受け取るものは、次の四つです。
どれも `run` の前に書き、一度書けばアプリが動いているあいだずっと有効です。

```ruby
shortcut("cmd+s") { app.save }
menu_item("File", "Open…") { app.open }
on_key { |chord| app.typed(chord) }
on_file_drop { |path| app.load(path) }
```

`shortcut` は、キーの組み合わせにハンドラを結び付けます。
`menu_item` は同じハンドラを、自分で名づけたメニューの下に置きます。
`on_key` は、どの shortcut にも取られなかった打鍵を受け取ります。
`on_file_drop` は、ウィンドウに落とされたファイルの場所を受け取ります。

外の世界を読み書きするものが、もう二つあります。

```ruby
  clipboard_set(@text)
  @text = clipboard_get

  job = task { open_dialog("choose a file") }
  on_done(job) { @path = task_answer }
```

ダイアログは人の操作を待つので、`task` の中に置きます（[Ruby、データ、ジョブ](tour-lib.md#ウィンドウの外のジョブ)）。
ウィンドウなしで走らせるときは、スクリプトの `file:<path>` がその答えになります。
だからダイアログも、ほかの操作と同じように検査できます。

`demo/keys.rb` が shortcut とメニューをスクリプトで動かし、`demo/picker.rb` がダイアログと落とされたファイルを見せます。

音は、ファイルを鳴らして、あとは放っておく形です。

```ruby
  audio_play("demo/assets/sound/blip.wav")         # 録音そのままの大きさで
  audio_play("demo/assets/sound/blast.wav", 0.4)   # 0.0 から 1.0 の音量で
  audio_stop
```

呼び出しはすぐ返り、鳴り終わるのを待ちません。
スクリプトの下では無音になります。
ゲートが、スピーカーのあるマシンを要求してはいけないからです。
音の出ないマシンや、読めないファイルでは、アプリを止めずに何も鳴りません。
エンジンが読めるのは WAV です。
`demo/sound.rb` がこの機能の全部で、移植した二つのゲームもこれを使っています。

## ウィンドウそのもの

```ruby
run(app, title: "ledger", width: 520.0, height: 640.0, padding: 0.0)
```

`title:` はウィンドウの名前で、リリースしたときはアプリケーションバンドルの名前にもなります。
`width:` と `height:` は開いたときの大きさで、`0.0` なら既定のままです。
`padding:` は画面全体の余白で、`-1.0` なら既定のまま、`0.0` なら内容が端まで届きます。
キャンバスを置くときは `0.0` にします。

## 次に読むもの

- [Ruby、データ、ジョブ](tour-lib.md)：Ruby 自身のライブラリ、データベース、タイマー、ウィンドウの外のジョブ。
- [要素](elements.md)：すべての要素と、そのキーワード、型、既定値。
