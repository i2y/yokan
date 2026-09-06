# インストール

落雁は今のところリポジトリの中にあります。
インストールする配布物はなく、自分の cpanfile に足すものもありません。
チェックアウトを持ってきて、一つだけ最初に用意すれば、あとは `./bin/rakugan` がコマンドラインのすべてです。

## 必要なもの

- **Apple シリコンの macOS**。
  エンジンはプラットフォーム自身の GPU の仕組みで描くので、今のところ移植先はここだけです。
- **アプリを動かす perl 5.40 以降**。
  `class` を機能として当てにできるのがそこからです。
  `just rakugan-perl` が固定した 5.44.0 を取ってきて `~/.cache/perl/5.44.0` に建てます。
  手元の perl を使うなら `RAKUGAN_PERL=/path/to/perl` を指します。
- **PPI**。
  コマンド自身が使います。
  動くのはパスの先頭にある perl の下です。
  macOS のシステム perl には入っています。
  そうでなければ `cpanm PPI`、または `rakugan/` で `cpanm --installdeps .` です。
- **Rust**（[rustup](https://rustup.rs) から）。
  コンパイラの版はリポジトリが固定していて、最初のビルドで取ってきます。
- **Xcode の Metal ツールチェイン**。
  エンジンがビルド時にシェーダをコンパイルします。

コマンドとアプリで perl を分けているのは意図したものです。
翻訳器には `class` が出てこない普通の Perl なのでシステムの 5.34 でも動き、5.40 が要るのはアプリのほうです。

## マシンごとに一度

```console
$ export CARGO_TARGET_DIR=$HOME/.cache/pixie/target
$ just rakugan-perl        # 固定した perl を取ってきて建てる
```

`CARGO_TARGET_DIR` は、すべてのクレートと、生成されるアプリが共有します。
二度目以降のビルドが速いのはこれのおかげです。
シェルの設定に書いて、あとは忘れてかまいません。

ほかに黙って取ってくるものはありません。
エンジンは同じチェックアウトの中のクレートで、最初にゲートかビルドをしたときに `cargo` が建てます。
XS の門はコマンド自身がアプリの perl 向けに `~/.cache/rakugan/door/<版>/` へ建て、材料が変われば建て直します。

## コマンド

`rakugan/` から実行します。

```console
$ ./bin/rakugan run   demo/counter.pl                    # perl でウィンドウが開く
$ ./bin/rakugan check demo/counter.pl                    # 受け取れない書き方
$ ./bin/rakugan gate  demo/counter.pl --script "click:+1,dump"
$ ./bin/rakugan translate demo/counter.pl                # 読むための .pix
$ ./bin/rakugan build demo/counter.pl --release --app    # バイナリと .app
```

`run` はウィンドウを開き、ファイルを見ています。
保存すればその場で効きます。
`check` はコンパイラを起動しません。
肝心なのは `gate` です。
二つの実行を一つのスクリプトで動かし、1 バイトずつ突き合わせます。

`build` には二つの旗があります。
`--release` は symbol table を落とし、`--app` はバイナリを macOS のアプリケーションバンドルに包みます。
`gate` には `--fresh <path>` があり、実行のたびにその場所を消します。
ファイルやデータベースを持つアプリを、二つの実行とも同じ「何もない」状態から始めるためです。

## 最初のファイル

```perl
use Rakugan;

class Counter {
    use Rakugan;
    field $count = 0;

    method view {
        return column(
            text("count: $count", size => 34),
            button("+1", on_click => sub { $count += 1 }),
            spacing => 12,
            padding => 16,
        );
    }
}

run(Counter->new, title => "counter");
```

```console
$ ./bin/rakugan run app.pl
```

そのウィンドウを開けたまま編集して保存してみてください。
ファイルが読み直され、ウィンドウの持っているインスタンスが、値をすべて保ったまま新しい `view` で答えます。

## ビルドすると何ができるか

macOS/arm64 で、共有のビルドディレクトリが温まった状態で測っています。

| 何 | 値 |
|---|---|
| 翻訳器が書く `.pix` | 0.08 秒、カウンタで約 600 バイト |
| コンパイルしたバイナリ | 53.8 MB |
| 配るバイナリ（`--release`） | 11.9 MB |
| アプリケーションバンドル（`--app`） | 11.9 MB |
| 起動してからウィンドウが出るまで | 0.2 秒 |
| ゲート 1 回（エンジンは建て終わっている） | 2.8 秒 |
| 掃引すべて（デモ 41 本、二つのツアー、サイト） | 3 分 13 秒 |

バイナリはエンジンと翻訳したアプリを含み、システム自身のライブラリ以外は何もリンクしません。
受け取る人に、perl 5.40 もツールチェインも要りません。

## 全体が動いているかを確かめる

```console
$ just rakugan-sweep
```

すべてのデモを両方の実行で、断りの文面を置いてあるファイルと、表を perl の印字と、そして二つのツアーの完全な例をすべて確かめます。
語彙や門やエンジンに手を入れたら通さなければならないもので、手元の用意が正しいかを知るにもいちばん早い方法です。

## 次に読むもの

- [はじめてのアプリ](tour.md)。
  言語そのものを、読者の出会う順に。
- [デモ](demos.md)。
  41 本のアプリを、画面写真とソースつきで。
