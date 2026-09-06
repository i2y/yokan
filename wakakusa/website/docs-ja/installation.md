# インストール

若草は今のところリポジトリの中にあります。
インストールする gem はなく、Gemfile に足すものもありません。
チェックアウトを取ってきて、二つのものを一度だけビルドすれば、あとは `./bin/wakakusa` がコマンドラインのすべてです。

## 必要なもの

- **macOS の Apple silicon。**
  エンジンはこのプラットフォーム自身の GPU の仕組みで描くので、今あるのはこの移植だけです。
- **Ruby 4**（CRuby）。
  書いているあいだ、アプリを動かすインタプリタです。
- **Rust**（[rustup](https://rustup.rs) から）。
  使うコンパイラの版はリポジトリが固定していて、最初のビルドで取ってきます。
- **Xcode の Metal ツールチェーン。**
  エンジンがビルド時にシェーダをコンパイルするからです。

## 機械ごとに一度だけ

```console
$ export CARGO_TARGET_DIR=$HOME/.cache/pixie/target
$ just wakakusa-spinel     # 固定してある Ruby コンパイラを取ってきてビルドする
$ just wakakusa-capi       # エンジンの C 面をビルドする
```

`just wakakusa-spinel` は、若草が固定している版の [spinel](https://github.com/matz/spinel) を取ってきます。
リリースしたバイナリをビルドするのが、この Ruby コンパイラです。
`~/.cache/spinel/<sha>` に置かれ、数分かかりますが、一度きりです。

`CARGO_TARGET_DIR` は、すべてのクレートと生成されるアプリで共有します。
二度目からのビルドが速いのは、これがあるからです。
シェルの設定ファイルに書いて、あとは忘れて構いません。

ほかに黙って取ってくるものはありません。
エンジンは同じチェックアウトの中のクレートで、最初にゲートかビルドを走らせたときに `cargo` がビルドします。

## コマンド

以下は `wakakusa/` から実行します。

```console
$ ./bin/wakakusa run   demo/counter.rb                    # CRuby でウィンドウが開く
$ ./bin/wakakusa check demo/counter.rb                    # 受け取れない書き方を挙げる
$ ./bin/wakakusa gate  demo/counter.rb --script "click:+1,dump"
$ ./bin/wakakusa translate demo/counter.rb                # 読むための C
$ ./bin/wakakusa build demo/counter.rb --release --app    # バイナリと .app
```

`run` はウィンドウを開き、ファイルを監視します。
だから保存がその場で効きます。
`check` はコンパイラをまったく起動しません。
`gate` がいちばん大事なもので、二つの実行を一つのスクリプトで動かし、一バイトずつ比べます。

`build` に付くフラグが二つあります。
`--release` は symbol table を落とし、`--app` はバイナリを macOS のアプリケーションバンドルに包みます。
`gate` に付くフラグが一つあります。
`--fresh <path>` は実行のたびにその場所を消すので、ファイルやデータベースを残すアプリでも、どちらの実行も同じ「何もない」状態から始まります。

## 最初のファイル

```ruby
require "wakakusa"

class Counter
  def initialize
    @count = 0
  end

  def view
    column(spacing: 12.0, padding: 16.0) {
      text "count: #{@count}", size: 34.0
      button("+1") { @count += 1 }
    }
  end
end

run(Counter.new, title: "counter")
```

```console
$ ./bin/wakakusa run app.rb
```

そのウィンドウを開いたまま書き換えて保存してみてください。
ファイルが読み直され、ウィンドウが持っているオブジェクトは、それまでの値をすべて保ったまま新しい `view` を返します。

## ビルドで何ができるか

macOS の arm64 で、共有のビルドディレクトリが温まった状態で測ったものです。

| もの | 値 |
|---|---|
| コンパイラが書き出す C | 10 ミリ秒未満、120 KB ほど |
| コンパイルされた実行の `cc` リンク | 0.29 秒 |
| コンパイルしたバイナリ | 16.2 MB |
| リリースしたバイナリ（`--release`） | 12.3 MB |
| アプリケーションバンドル（`--app`） | 12.5 MB |
| 起動してウィンドウが出るまで | 0.3 秒未満 |
| ゲート一往復（エンジンはビルド済み） | 2.1 秒 |

バイナリはエンジンとコンパイル済みの Ruby を含んでいて、システム自身のライブラリ以外は何もリンクしません。
受け取る人に、Ruby もコンパイラも要りません。

## 全体が動くことを確かめる

```console
$ just wakakusa-sweep
```

すべてのデモを二つの実行に通し、続けて二つの言語のツアーとこのサイトにある完全な例をすべて通します。
語彙やドア、エンジンに手を入れたときに通すものですが、手元の環境が揃っているかを確かめるいちばん速い方法でもあります。

## 次に読むもの

- [最初のアプリ](tour.md)：言語そのものが、出会う順に並んでいます。
- [デモ](demos.md)：43 本のアプリを、スクリーンショットとソースつきで。
