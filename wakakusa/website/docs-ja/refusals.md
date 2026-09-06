# 若草が断る書き方

`wakakusa check` はアプリを読み、受け取れない書き方があれば、その行と、行の下のキャレットと、書き換え方を示します。
コンパイラは起動せず、ウィンドウも開かないので、答えは 0.1 秒ほどで返ります。
ビルドの前にもゲートの前にも、毎回走ります。

```console
$ wakakusa check demo/counter.rb        # 何も出なければ、その形は通る
```

ここにある規則はどれも、コンパイルされた実行がうまく扱えない書き方に対応しています。
ビルドに失敗する形か、もっと悪い場合には、ビルドが通ってからインタプリタ側と違う振る舞いをする形です。
ゲートでも見つかりますが、見つかるのはビルドのあとです。
断りは、アプリを書いている最中に知らせます。

以下の文面は、`check` が実際に印字するものです。
`test/refuse/` にそのすべてが置いてあり、`tools/refuse_test.sh` が文面のずれで落ちます。
だから、断りの文面が黙って別の文に変わることはありません。

## ビューが書き換える

```ruby
  def view
    @seen = @seen + 1
    text("seen #{@seen}")
  end
```

```
app.rb:9:5: Wakakusa cannot take this — a view only reads. Move the write into a handler — the block on a button, or a method the app calls from one
    @seen = @seen + 1
    ^
```

ビューは今の画面を組み立てるために呼ばれ、いつ呼び直されるかわかりません。
何かを書き換えるビューは、ウィンドウとスクリプトで違う回数だけ書き換えることになります。
コンテナのブロックの中の書き換えも同じ断りにあたります。
そこもまだビューを書いている最中だからです。

**こう書きます。**
書き換えはハンドラに置きます。
ボタンのブロックか、ハンドラから呼ぶメソッドです。

## 繰り返しの中の、要素のブロック

```ruby
  def view
    column(spacing: 8.0) {
      @items.each_with_index do |name, i|
        button(name) { @picked = i }
      end
    }
  end
```

```
app.rb:12:22: Wakakusa cannot take this — a block on an element cannot be written inside a loop: a compiled run has lost the loop's variables by the time it runs. Move this into a method that takes what it needs (`def line(item, i)`), and call that from the loop
        button(name) { @picked = i }
                     ^
```

コンパイルされた実行では、ボタンが押される時点で繰り返しの `name` と `i` がもう残っていません。

**こう書きます。**
行に必要なものを引数に取るメソッドを作り、繰り返しからそれを呼びます。
ボタンのブロックは、繰り返しの変数ではなくメソッドの引数を閉じ込めます。

```ruby
  def line(name, i)
    button(name) { @picked = i }
  end

  def view
    column(spacing: 8.0) {
      @items.each_with_index { |name, i| line(name, i) }
    }
  end
```

## ブロックの中から書き換えるグローバル変数

```ruby
$name = ""

class App
  def view
    text_field($name) { |s| $name = s }
  end
end
```

```
app.rb:7:29: Wakakusa cannot take this — `$name` is a global, and a compiled run cannot write one from inside a block. The app's state belongs on the app: write `@name` in a class with a `view` method, and hand it to `run`
    text_field($name) { |s| $name = s }
                            ^
```

コンパイルされた実行は、そのグローバル変数に最初に入った値で型を決めます。
ブロックの引数をその型に変換する手がかりがありません。

**こう書きます。**
状態はアプリのオブジェクトに持たせます。
`view` メソッドを持つクラスの `@name` にして、そのオブジェクトを `run` に渡します。
アプリの状態が本来あるべき場所でもあります。

## ブロックでも proc でもないハンドラ

```ruby
  def view
    button("+1", on_click: :bump)
  end
```

```
app.rb:9:18: Wakakusa cannot take this — `on_click:` takes a proc of no arguments, written out here (`on_click: -> { add(event_text) }`), or leave it out and write the block instead. A proc given through a keyword is not handed what the event carried in a compiled run, so it asks for it
    button("+1", on_click: :bump)
                 ^
```

キーワード引数で渡した proc は、コンパイルされた実行にはアドレスとして届きます。
文字列が渡るはずのところに、番号を渡して呼ぶことになります。

**こう書きます。**
要素の普通のハンドラはブロックです。
ブロックがすでに一つ目に使われている要素の二つ目には、引数を取らない proc を書き、イベントの中身は自分で取り出します。

```ruby
  text_field(@draft, on_submit: -> { add(event_text) }) { |s| @draft = s }
```

## `+` で足す配列

```ruby
  def add(t)
    @items = @items + [t]
  end
```

```
app.rb:9:5: Wakakusa cannot take this — growing a list with `+ [...]` answers something different in a compiled run. Copy it and push: `items = @items.dup`, `items.push(...)`, `@items = items`
    @items = @items + [t]
    ^
```

短く書いた場合も、その書き方の文面で断ります。

```
app.rb:9:5: Wakakusa cannot take this — growing a list with `+= [...]` answers something different in a compiled run. Copy it and push: `items = @items.dup`, `items.push(...)`, `@items = items`
    @items += [t]
    ^
```

**こう書きます。**
写して、足して、書き戻します。
文面にも、自分のフィールドの名前でその三行が入っています。

```ruby
  def add(t)
    items = @items.dup
    items.push(t)
    @items = items
  end
```

## 要素が取らないキーワード

```ruby
  def view
    text("hello", weight: 700.0)
  end
```

```
app.rb:5:19: Wakakusa cannot take this — `text` has no `weight:`. It takes a11y_label, align, animate, background, bold, border_color, border_radius, border_width, col_span, color, disabled, easing, enter, exit, grow, height, italic, max_lines, max_width, min_width, mono, padding, role, row_span, size, text, theme, tooltip, underline, width, wrap
    text("hello", weight: 700.0)
                  ^
```

文面に並ぶ一覧は `elements.toml` から生成しています。
アプリが呼ぶ Ruby のメソッドも、エンジン側の定数も、同じ表から書き出されます。
だから、その要素が実際には持たないキーワードを断りが挙げることはありません。
表そのものを並べたのが[要素](elements.md)です。

**こう書きます。**
一覧のうち、やりたかったことに当たるものを選びます。
この例なら `bold: true` です。

## `check` がまだ見ないもの

`check` が見るのは、ここに挙げた形だけです。
コンパイラが取りこぼすものをすべて見ているわけではないので、残りを見つけるのは今もゲートです。
ビルドが済んだだけでは終わりにならず、スクリプトを二つの実行に通してはじめて終わる、というのはそのためです。

設計ではなく今の限界にすぎない断りは、[まだできないこと](tour-ship.md#まだできないこと)に挙げてあります。
その限界がなくなるときは、規則もその理由もいっしょになくなります。
