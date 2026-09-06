# 最初のアプリ

若草は、Ruby で書いたデスクトップアプリを 1 本のネイティブバイナリにして配ります。
Ruby を C にするのは [spinel](https://github.com/matz/spinel)、画面を描くのは、Zed エディタを支える **gpui** の上に組んだ pixie のエンジンで、`wakakusa build` はその二つを結びます。
書いているあいだは、同じファイルを CRuby が動かします。
配るバイナリと、書いているあいだの実行が同じプログラムかどうかは、`wakakusa gate` で確かめます。
一つのスクリプトで両方を動かし、描いた画面を 1 バイトずつ突き合わせます。
画面を組み立てるメソッド（`text`、`button`、`column` など）は若草が用意しますが、アプリそのものは普通の Ruby のクラスです。
書ける Ruby がどこまでかは、[二つの実行](two-runs.md#三つの-ruby)にまとめてあります。

このツアーは、言語を読者の出会う順に並べたものです。
ここに書いたものはすべて実際に動きます。
`tools/tour_check.rb` がこのページから完全なアプリをすべて取り出し、デモと同じコマンドに通すからです。
要素やキーワードの名前を変えれば、読者がそれを目にする前にこのページが壊れます。
まだできないことは、理由とともに[まだできないこと](tour-ship.md#まだできないこと)にまとめてあります。

## いちばん小さいアプリ

アプリは `view` メソッドを持つオブジェクトです。
`view` は要素を一つ返し、そのオブジェクトを `run` に渡すとウィンドウが開きます。

<!-- script: dump -->
```ruby
require "wakakusa"

class Hello
  def view
    text "hello", size: 28.0
  end
end

run(Hello.new, title: "hello")
```

```console
$ wakakusa run demo/hello.rb      # CRuby でウィンドウが開く
$ wakakusa gate demo/hello.rb --script "dump"
GATE OK — 1 dump line identical in both runs
```

継承すべきクラスも、登録すべきメソッドもありません。
`run` はオブジェクトを受け取り、そこから使うのは `view` という名前のメソッドだけです。

ウィンドウの指定も `run` に渡します。
`title:` が名前、`width:` と `height:` が開いたときの大きさ（0.0 なら既定のまま）、`padding:` が画面全体の余白です。
`padding:` は -1.0 で既定のまま、0.0 で内容が端まで届きます。
キャンバスを置くときは 0.0 にします。

```ruby
run(Game.new, title: "Pyxel Jump", width: 640.0, height: 480.0, padding: 0.0)
```

## 状態の持ち方

状態はオブジェクトのインスタンス変数です。
ハンドラはブロックで、ブロックの中からはそのオブジェクトがそのまま見えます。
だから書き換え方は、普通のメソッドと変わりません。
ハンドラを抜けると、そのときの状態からビュー全体が組み直されます。

<!-- script: click:+1,click:+1,dump,input:Momo,dump -->
```ruby
require "wakakusa"

class Counter
  def initialize
    @count = 0
    @name = ""
  end

  def view
    column(
      text("count: #{@count}", size: 34.0),
      row(
        button("+1") { @count += 1 },
        button("+10") { @count += 10 },
        button("reset") { @count = 0 },
        spacing: 8.0
      ),
      text_field(@name, placeholder: "your name") { |s| @name = s },
      text("hello, #{@name}"),
      spacing: 12.0,
      padding: 16.0
    )
  end
end

run(Counter.new, title: "counter")
```

ストアを別に用意することはありません。
監視の設定を書く必要も、これはフィールドだと印を付ける必要もありません。
アプリはオブジェクトであり、その状態は `initialize` から始まります。

知らないうちに結び付けられるものもありません。
`@name` と書いたからその欄には `@name` が表示され、値を書き戻すのはブロックです。
だから画面と状態が食い違うことがありません。

## ビューの書き方

コンテナは、子を引数として取ることも、ブロックとして取ることもできます。
どちらも組み立てる木は同じなので、画面が読みやすくなるほうを選びます。

<!-- script: click:+1,dump -->
```ruby
require "wakakusa"

class Two
  def initialize
    @count = 0
  end

  def view
    column(spacing: 12.0, padding: 16.0) {
      text "count: #{@count}", size: 34.0
      row(spacing: 8.0) {
        button("+1") { @count += 1 }
        button("reset") { @count = 0 }
      }
    }
  end
end

run(Two.new, title: "two")
```

ブロックの形はカンマを挟まずに入れ子になり、上から下へ読めます。
構造のある画面にはこの形が向き、三つほどを並べるだけの場面には引数の形が向きます。
`demo/counter.rb` と `demo/blockform.rb` は同じ画面を二つの書き方で書いたもので、全デモのゲートはその両方を通します。

コンテナのブロックの中では、要素は開いているコンテナに加わります。
規則はそれだけです。
`column` のブロックの中に書いた `text` はその column の子になり、その中で開いた `row` は、さらにその中に書いたものを集めます。

## 画面の一部はメソッド

要素を返すメソッドは画面の一部にあたり、それを呼ぶことでビューを分けて書けます。

```ruby
  def field(label, value)
    row(spacing: 6.0) {
      text label, width: 90.0
      text value, bold: true
    }
  end

  def view
    column(spacing: 4.0, padding: 14.0) {
      field("name", @name)
      field("size", @size)
    }
  end
```

宣言するものはありません。
要素を返すメソッドが画面の一部で、要素を引数に取るメソッドがそれを包むものです。

```ruby
  def card(title, *kids)
    column(
      text(title, size: 18.0),
      *kids,
      spacing: 4.0, padding: 8.0,
      border_width: 1.0, border_color: "accent", border_radius: 8.0
    )
  end
```

子は普通の Ruby の splat で渡され、そのままコンテナの引数になります。
だから、包むメソッドもほかのメソッドと変わりません。
`demo/cards.rb` がその書き方を一画面にしたものです。

## ビューは読むだけ

ビューは今の画面を組み立てるために呼ばれ、いつ呼び直されるかわかりません。
だからビューは状態を読むだけで、書き換えません。

```console
$ wakakusa check app.rb
app.rb:9:5: Wakakusa cannot take this — a view only reads. Move the write into a handler — the block on a button, or a method the app calls from one
    @seen = @seen + 1
    ^
```

どう直すかは、エラーの文面に書いてあります。
書き換えはハンドラか、ハンドラから呼ぶメソッドに置きます。
断る書き方の一覧は[若草が断る書き方](refusals.md)にあり、それぞれ実際に印字される文面を載せてあります。

## 次に読むもの

- [ビューと制御フロー](tour-logic.md)：ビューの中の `if` と繰り返し、入力の部品、ハンドラ、必要な行だけ作る一覧。
- [キャンバスとキーボード](tour-canvas.md)：仮想的な画素の格子と、キーボードの読み方。
- [検証と配布](tour-ship.md)：ゲートと、一本のバイナリ。
