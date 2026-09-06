# インストール

落雁は今のところリポジトリの中にあります。
インストールする配布物はなく、cpanfile に足すものもありません。
チェックアウトを取ってきて、最初に一つだけ用意すれば、あとは `./bin/rakugan` だけで足ります。

## 必要なもの

- **Apple シリコンの macOS**。
  エンジンはプラットフォーム自身の GPU の仕組みで描くので、今のところ動くのはこの環境だけです。
- **アプリを動かす perl 5.40 以降**。
  `class` を機能として当てにできるのがそこからです。
  `just rakugan-perl` が、固定してある 5.44.0 を取ってきて `~/.cache/perl/5.44.0` にビルドします。
  手元の perl を使うなら、`RAKUGAN_PERL=/path/to/perl` でその場所を指します。
- **PPI**。
  コマンド自身が使います。
  コマンドが動くのは、パスの先頭にある perl です。
  macOS のシステム perl には入っています。
  入っていなければ `cpanm PPI`、または `rakugan/` で `cpanm --installdeps .` です。
- **Rust**（[rustup](https://rustup.rs) から）。
  コンパイラの版はリポジトリが固定していて、最初のビルドで取ってきます。
- **Xcode の Metal ツールチェイン**。
  エンジンがビルド時にシェーダをコンパイルするからです。

コマンドとアプリで perl を分けているのは意図したものです。
翻訳器そのものは `class` の出てこない普通の Perl なので、システムの 5.34 でも動きます。
5.40 が要るのはアプリのほうです。

## マシンごとに一度

```console
$ export CARGO_TARGET_DIR=$HOME/.cache/pixie/target
$ just rakugan-perl        # 固定した perl を取ってきて建てる
```

`CARGO_TARGET_DIR` は、すべてのクレートと、生成されるアプリが共有します。
二度目以降のビルドが速いのはこれのおかげです。
シェルの設定に書いて、あとは忘れてかまいません。

ほかに黙って取ってくるものはありません。
エンジンは同じチェックアウトの中のクレートで、最初にゲートかビルドをしたときに `cargo` がビルドします。
XS の門は、コマンド自身がアプリの perl 向けに `~/.cache/rakugan/door/<版>/` へビルドし、元のファイルが変わればビルドし直します。

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

`build` には二つのフラグがあります。
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

そのウィンドウを開いたまま編集して保存してみてください。
ファイルが読み直され、ウィンドウの持っているインスタンスが、値をすべて保ったまま新しい `view` で答えます。

## ビルドすると何ができるか

macOS/arm64 で、共有のビルドディレクトリが温まった状態で測っています。

| もの | 値 |
|---|---|
| 翻訳器が書く `.pix` | 0.08 秒、カウンタで約 600 バイト |
| コンパイルしたバイナリ | 53.8 MB |
| 配るバイナリ（`--release`） | 11.9 MB |
| アプリケーションバンドル（`--app`） | 11.9 MB |
| 起動してからウィンドウが出るまで | 0.2 秒 |
| ゲート 1 回（エンジンはビルド済み） | 2.8 秒 |
| 掃引 1 回（デモ 41 本、二つのツアー、サイト） | 3 分 13 秒 |

バイナリはエンジンと翻訳したアプリを含み、システム自身のライブラリ以外は何もリンクしません。
受け取る人に、perl 5.40 もツールチェインも要りません。

## 全体が動いているかを確かめる

```console
$ just rakugan-sweep
```

すべてのデモを両方の実行で動かし、断りの文面を置いてあるファイル、perl が印字した表、そして二つのツアーの完全な例を、すべて確かめます。
語彙や門、エンジンに手を入れたときに通すものですが、手元の用意が揃っているかを確かめるいちばん速い方法でもあります。

## 次に読むもの

- [はじめてのアプリ](tour.md)。
  言語そのものが、読者の出会う順に並んでいます。
- [デモ](demos.md)。
  41 本のアプリを、画面写真とソースつきで。
