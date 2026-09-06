---
title: "Perl で書いて、ネイティブで配る"
hide:
  - navigation
  - toc
---

<div class="rk-hero" markdown>
<img class="rk-hero__mark" src="images/logo.svg#only-dark" alt="">
<img class="rk-hero__mark" src="images/logo-light.svg#only-light" alt="">

# 落雁

<p class="rk-hero__tag">Perl で書いて、ネイティブで配る</p>

<p class="rk-hero__lede">
落雁は、Perl で書いたデスクトップアプリを 1 本のネイティブバイナリにします。
<strong>perl で動かしたものがそのまま配られ、そのことを確かめるのが
<code>rakugan gate</code> です</strong>。
配るときは、アプリを
<a href="https://github.com/i2y/yokan/blob/main/docs/PIXIE.md">pixie</a>
に翻訳し、Zed エディタを支える <strong>gpui</strong> の上に組んだ描画エンジンと一緒にコンパイルして、インタプリタの入っていない 1 本のバイナリにします。
書いているあいだは、同じファイルを perl が動かし、小さな XS の門を通して同じエンジンに触ります。
<code>rakugan gate</code> は一つのスクリプトで両方を動かし、描いた画面を 1 バイトずつ突き合わせます。
アプリそのものは普通の Perl のクラスです。
落雁が足すのは、画面を組み立てるサブルーチンと、二つの実行が一致するという確認です。
</p>

<div class="rk-hero__cta" markdown>
[はじめる](installation.md){ .md-button .md-button--primary }
[言語ツアー](tour.md){ .md-button }
[デモ](demos.md){ .md-button }
[GitHub](https://github.com/i2y/yokan){ .md-button }
</div>
</div>

## 書いて、動かして、配る

いちばん小さい、完全なアプリです。

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

アプリは、perl 5.40 以降の `class` 機能で書いたクラスです。
状態はそのフィールド、`view` は要素を一つ返すメソッド、ハンドラはそのフィールドが見える無名サブルーチンです。
継承するものも、登録するものも、監視対象だと印を付けるものもありません。

このファイルのどこにも型は書いてありませんが、コンパイルした実行には型があります。
フィールドの型は初期値から、ハンドラの引数の型はそれが書かれている要素から読みます。
空で始まる入れ物は `field @items = empty(Str);` と、何を入れるかを言います。
引数のあるメソッドは `method add :Sig(Int) ($by)` と、それが何かを言います。
型を書く場所はこの二つだけです。

```console
$ ./bin/rakugan run app.pl
```

これでウィンドウが開き、ファイルを見はじめます。
編集して保存すると、ウィンドウがそれを取り込みます。
ファイルが読み直され、ウィンドウの持っているインスタンスが、値をすべて保ったまま新しい `view` で答えます。

配ります。

```console
$ ./bin/rakugan build demo/todo.pl --release --app
built: ~/.cache/pixie/target/release/main (11.9 MB)
bundle: demo/dist/todo.app (11.9 MB)
```

バイナリはエンジンと翻訳したアプリを含み、システム自身のライブラリ以外は何もリンクしません。
受け取る人は、perl もツールチェインも入れません。

---

## どんな画面になるか

![落雁で書いた家計簿。入力欄と棒グラフと、sqlite に入っている行](images/demos/ledger.png)

*`demo/ledger.pl`。
お金をデータベースに置き、値は文に埋め込まず束縛して渡し、合計をグラフにしています。
普通の Perl で、配るときは 1 本のネイティブバイナリです。*

---

## 「手元では動いたのに」

操作の並びを渡すと、perl の実行とコンパイルしたバイナリの両方でそれを再生し、できあがった画面を突き合わせます。
落雁はこれを **ゲート** と呼びます。

```console
$ ./bin/rakugan gate app.pl --script "click:+1,dump"
GATE OK — 3 dump lines identical in both runs
```

解釈実行は perl そのものです。
だから緑のゲートは、配るバイナリが本物のインタプリタと、スクリプトの触れたすべてについて一致した、ということを意味します。
二つが同じプログラムでない場所は、理由とともに[二つの実行](two-runs.md)に挙げてあります。

---

## 名前が Perl のものであるところ、仕様は perl

`length`、`substr`、`uc`、`sort`、`grep`、`map`、`sprintf`、`List::Util`、`POSIX`、正規表現は言語自身のものです。
その前に立つ、こちらのライブラリはありません。
コンパイルした実行に perl は入っていないので、これらは Rust で一度ずつ書いてリンクしてあります。
そのうえで、perl 自身が印字した 1000 行あまりの表に照らして測ります。
perl と一致することは、願いではなく検査です。

```perl
    my @big  = grep { $_ > 5 } @scores;
    my $line = sprintf("mean %.1f max %d", sum(@scores) / scalar @scores, max(@scores));
```

ファイル、データベース、ネットワーク、クリップボードは枠組みのものです。
そちらでは、一つの実装が両方の実行に答えます。

```perl
    sqlite_exec($db, "INSERT INTO expenses VALUES (?, ?, ?)", [$name, $yen, $cat]);
    my @rows = sqlite_query_rows($db, "SELECT name, amount FROM expenses ORDER BY rowid");
```

---

## 移植した二つのゲーム

Pyxel 自身の例（Takashi Kitao、MIT）を二つ、ほぼ 1 行ずつ移してデモに入れてあります。
同じ画素のキャンバス、同じ毎秒 30 フレーム、押されているあいだ読まれる同じキー。
打鍵とフレームからなるスクリプトが両方の実行を再生するので、ゲートはゲームの全フレームを突き合わせます。

<p align="center">
  <img src="images/demos/shooter.gif" width="240" align="middle">
  <img src="images/demos/jump.gif" width="320" align="middle">
</p>

*`demo/shooter.pl` と `demo/jump.pl`。
キャンバスの中では色は番号、配色の何番目かというだけの番号です。
だから、画素の機械のために書かれた描画が、数字を変えずにそのまま移せます。*

---

## ほかに入っているもの

<div class="grid cards" markdown>

-   :material-table-large: __一つの表、一つの語彙__

    33 個の要素、15 個の共通キーワード、10 個の描画命令が、`elements.toml` に一度だけ書いてあります。
    アプリが呼ぶ Perl も、エンジンが数える番号も、そこから生成します。
    だから一つの要素が二つの意味を持つことはありません。
    このエンジンの上のほかの二つの言語も、同じ表を読んでいます。

-   :material-brush-variant: __キャンバスとキーボード__

    仮想的な画素の格子を命令をひとつずつ並べて塗り、色は配色の番号で指し、キーはタイマーから装置として読みます。
    音は `audio_play` で WAV を鳴らします。
    そして、ウィンドウを開かずにどのフレームでも PNG にできます。

-   :material-shield-check: __教える断り方__

    翻訳器が受け取れない書き方は、何かを組み立てる前に、行と書き換え方とともに名指しで断ります。
    どの断りにも、印字されるべき文面をそのまま置いたファイルがあります。

</div>

---

## 次に読むもの

<div class="grid cards" markdown>

-   :material-rocket-launch: __[インストール](installation.md)__

    必要なもの、一度だけの用意、そして五つのコマンド。
    今のところ Apple シリコンの macOS だけです。

-   :material-book-open-variant: __[言語ツアー](tour.md)__

    アプリの書き方をひととおり。
    状態、ビュー、キャンバス、ウィンドウ、データベース、ゲート。
    最後はまだできないことで閉じます。

-   :material-view-gallery: __[デモ](demos.md)__

    41 本のアプリを、画面写真とソース全体つきで。

-   :material-github: __[ソース](https://github.com/i2y/yokan)__

    翻訳器と、エンジンと、デモ。

</div>

---

_名前は落雁、押して乾かす干菓子です。
エンジンを共にする羊羹と若草と同じく、和菓子から採りました。_
