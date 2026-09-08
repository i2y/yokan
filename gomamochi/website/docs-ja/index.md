---
title: "Write Go. Ship what you saw."
hide:
  - navigation
  - toc
---

<div class="gm-hero" markdown>
<img class="gm-hero__mark" src="images/logo.svg#only-dark" alt="">
<img class="gm-hero__mark" src="images/logo-light.svg#only-light" alt="">

# Gomamochi

<p class="gm-hero__tag">Write Go. Ship what you saw.</p>

<!-- 日本語は行の折り返しが空白として描画されるので、リード文は一行で書く -->
<p class="gm-hero__lede">Gomamochi（胡麻餅）は、pixie のエンジンの上で Go のデスクトップアプリを作るためのものです。<strong>書いているあいだに見ていたものが、そのまま配るものになります。そのことを確かめるのが <code>gomamochi gate</code> です</strong>。<code>gomamochi run</code> はファイルを読み、そのまま解釈実行します。ビルドの手順はなく、保存すればウィンドウが新しいコードを取り込み、持っていた値はそのまま残ります。<code>gomamochi build</code> は同じファイルを Go のコンパイラでコンパイルして、ネイティブバイナリにします。どちらの実行も、Zed エディタを支える <strong>gpui</strong> の上に組んだ同じ描画エンジンに、<a href="https://github.com/i2y/yokan/blob/main/docs/PIXIE.md">pixie</a> の C API を通して届きます。cgo は使いません。<code>gomamochi gate</code> は一つのスクリプトで両方を動かし、描いた画面を 1 バイトずつ突き合わせます。アプリそのものは、<code>View</code> メソッドを持つ Go の構造体です。Gomamochi が足すのは、画面を組み立てる要素と、二つの実行が一致することを確かめる仕組みです。</p>

<div class="gm-hero__cta" markdown>
[はじめる](installation.md){ .md-button .md-button--primary }
[言語ツアー](tour.md){ .md-button }
[デモ](demos.md){ .md-button }
[GitHub](https://github.com/i2y/yokan){ .md-button }
</div>
</div>

## 全体の姿

一つのソースと、それを動かす二つの道です。

![Gomamochi がアプリを動かす道筋。1 本の Go のファイルが、書いているあいだは gomamochi コマンドの中のインタプリタで動き、配るときは Go のコンパイラのバイナリになる。エンジンのライブラリは一つで、どちらも purego で開き、ゲートが二つを突き合わせる](images/architecture-ja.svg#only-dark)

![Gomamochi がアプリを動かす道筋。1 本の Go のファイルが、書いているあいだは gomamochi コマンドの中のインタプリタで動き、配るときは Go のコンパイラのバイナリになる。エンジンのライブラリは一つで、どちらも purego で開き、ゲートが二つを突き合わせる](images/architecture-ja-light.svg#only-light)

どちらの道も一つのエンジンに行き着き、届き方も同じです。
pixie の C API は一つの共有ライブラリで、どちらの実行もそれを purego で開きます。
cgo は使いません。
解釈実行は `gomamochi` コマンドの中の [yaegi](https://github.com/traefik/yaegi) で、コンパイルした実行は、同じファイルから `go build` が作ったバイナリです。
そのバイナリの横に、同じライブラリを置きます。
エンジンは Go の値を持ちません。
だからインタプリタと、コンパイルしたバイナリとが、まったく同じコードを動かせます。

---

## 書いて、動かして、配る

いちばん小さいアプリの全文です。

```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Counter struct {
	count int
}

func (c *Counter) View() Element {
	return Column(
		Text(fmt.Sprintf("count: %d", c.count)).Size(34),
		Button("+1").OnClick(func() { c.count += 1 }),
	).Spacing(12).Padding(16)
}

func main() {
	Run(&Counter{}, Title("counter"))
}
```

アプリは構造体です。
状態はそのフィールド、`View` は要素を一つ返すメソッド、ハンドラはそのフィールドを閉じ込めたクロージャです。
継承するものも、登録するものも、監視対象だと印を付けるものもありません。
Go にはキーワード引数がないので、要素のキーワードは要素のメソッドです。
`Text("…").Size(34)` や `Column(…).Spacing(12).Padding(16)` のように、キーワードを並べる代わりにメソッドをつなげます。

dot import は好みの問題です。
パッケージに名前を付けて import すれば、呼び出しはすべて `gm.Text(…)`、`gm.Run(…)` の形になり、アプリは自分の型にどんな名前でも使えます。
どちらの書き方も、両方の実行が受け取ります。

```console
$ ./bin/gomamochi run app.go
```

これでウィンドウが開き、ファイルを見はじめます。
編集して保存すると、ウィンドウがそれを取り込みます。
新しいインタプリタがファイルを読み直し、ウィンドウの持っている構造体の値は、すべて新しいコードに引き継がれます。
ビルドの手順はありません。
Go のツールチェインもこのループには関わりません。
インタプリタはコマンドの中にあります。

配ります。

```console
$ ./bin/gomamochi build demo/counter.go --release --app
built: demo/.gate/counter/counter (1.9 MB)
bundle: demo/dist/counter.app (22.2 MB)
```

バイナリは Go のコンパイラが作るそのもので、cgo は使いません。
エンジンは一つの共有ライブラリとして、その横に置かれます。
`--app` は二つを一つのバンドルにまとめます（Linux では AppDir になり、`--appimage` がそれを一つのファイルに詰めます）。
受け取る人は、Go もツールチェインも入れずに開けます。

---

## どんな画面になるか

![Gomamochi で書いた家計簿。入力欄と棒グラフと、sqlite に入っている行](images/demos/ledger.png)

*`demo/ledger.go`。
家計簿をデータベースに置き、値は文に直接書かず `?` で渡し、合計を棒グラフにしています。
普通の Go で書かれ、配るときは Go のコンパイラが作るバイナリになります。*

---

## 「手元では動いたのに」

操作の並びを渡すと、解釈実行とコンパイルしたバイナリの両方でそれを再生し、できあがった画面を突き合わせます。
Gomamochi はこれを**ゲート**と呼びます。

```console
$ ./bin/gomamochi gate app.go --script "click:+1,dump"
GATE OK — 3 dump lines identical in both runs
```

コンパイルした実行は Go のコンパイラそのものです。
だからゲートが通れば、書いているあいだに見ていたウィンドウが、スクリプトの触れた範囲では配るものと一致していたということです。
違いうるのはインタプリタのほうです。
違う場所は理由とともに[二つの実行](two-runs.md)に挙げてあり、何かを組み立てる前に名指しで断られます。

---

## Go の標準ライブラリは、両方の実行で同じコード

`fmt`、`strings`、`strconv`、`math`、`sort`、`time`、`encoding/json`、`net/http`、`os` は Go 自身のものです。
これらの前に Gomamochi のライブラリが立つことはありません。
インタプリタはコマンドにコンパイルされているパッケージを呼び、バイナリは同じパッケージをリンクします。
だから数の書式も、整数の溢れも、文字列の大文字化も同じになり、それを押さえる表は要りません。

```go
	sort.Slice(order, func(a, b int) bool { return scores[order[a]] < scores[order[b]] })
	line := fmt.Sprintf("mean %.1f, %d over five", mean, len(big))
```

データベース、クリップボード、OS のダイアログ、音、通知はフレームワークのものです。
そちらでは、エンジンの中の一つの実装が、C API を通して両方の実行に答えます。

```go
	SqliteExec(db, "INSERT INTO expenses VALUES (?, ?, ?)", name, strconv.Itoa(yen), cat)
	rows := SqliteQueryRowsOr(db, "SELECT name, amount, cat FROM expenses ORDER BY rowid")
```

---

## 移植した二つのゲーム

Pyxel 自身の例（Takashi Kitao、MIT）を二つ、ほぼ 1 行ずつ移してデモに入れてあります。
画素のキャンバスも、毎秒 30 フレームという速さも、押しっぱなしのキーの読み方も同じです。
打鍵とフレームからなるスクリプトが両方の実行を再生するので、ゲートはゲームの全フレームを突き合わせます。

<p align="center">
  <img src="images/demos/shooter.gif" width="240" align="middle">
  <img src="images/demos/jump.gif" width="320" align="middle">
</p>

*`demo/shooter.go` と `demo/jump.go`。
キャンバスの中で色は、配色の何番目かというだけの番号です。
だから、ドット絵の道具のために書かれた描画が、数字を変えずにそのまま移せます。*

---

## エージェントが書くとき

エージェントはファイルを書き、返ってきたものを読みます。
だから、返ってくるものの形で作業の進み方が決まります。
三つのコマンドのうち二つは、何もビルドせず、ウィンドウも開かずに、1 秒前後で答えます。
何を書けばよいかが書かれた断りと、文字になった画面です。
最後の証明がゲートです。

![エージェントが回るループ。輪の中心で app.go を書き、gomamochi check とウィンドウなしの実行を 1 秒前後ずつで回り、輪の外に出て gomamochi gate で、配るバイナリが一致することを証明する](images/cycle-ja.svg#only-dark)

![エージェントが回るループ。輪の中心で app.go を書き、gomamochi check とウィンドウなしの実行を 1 秒前後ずつで回り、輪の外に出て gomamochi gate で、配るバイナリが一致することを証明する](images/cycle-ja-light.svg#only-light)

ループ全体は[エージェントと一緒に書く](agents.md)にあります。

---

## ほかに入っているもの

<div class="grid cards" markdown>

-   :material-table-large: __要素はすべて一つの表から__

    33 個の要素、15 個の共通キーワード、10 個の描画命令が、`elements.toml` に一度だけ書いてあります。
    アプリが呼ぶ Go も、インタプリタから見えるパッケージも、エンジンが数える番号も、そこから生成されます。
    だから一つの要素が二つの意味を持つことはありません。
    このエンジンの上のほかの三つの言語も、同じ表を読んでいます。

-   :material-brush-variant: __キャンバスとキーボード__

    仮想的な画素の格子を、命令をひとつずつ並べて塗ります。
    色は配色の番号で指し、キーが押されているかどうかはタイマーの中で読みます。
    音は `AudioPlay` で WAV を鳴らします。
    ウィンドウを開かずに、どのフレームも PNG に書き出せます。

-   :material-shield-check: __教える断り方__

    解釈実行がコンパイルした実行と同じには動かせない書き方は、何かを組み立てるより先に断ります。
    断りには、その行と、代わりにどう書くかが出ます。
    Go 自身の誤りは、Go 自身の言葉で返ります。
    どの断りにも、出力される文面をそのまま置いたファイルがあります。

</div>

---

## できないこと

- 解釈実行は yaegi で、その Go は 1.22 のものです。
  `min` と `max`、数や関数への `range`、1.22 より後に標準ライブラリに入ったものはありません。
  最初の三つは名指しで断られます。
  全体の一覧はツアーの最後の節にあります。
- アプリは 1 ファイルで、import できるのは標準ライブラリとこのパッケージです。
  標準ライブラリの外のモジュールは断られます。
  解釈実行がまだそれを読めないからです。
- 配るアプリは、バイナリとその横に置くエンジンのライブラリの二つ、またはその二つを入れたバンドルです。
  1 ファイルにする形はありません。
- dot import のもとでは、パッケージが公開している名前（`App`、`Element`、`Text`、`Run` など）はもう使われています。
  アプリ自身の型はそれを避けて名付けるか、パッケージに名前を付けて import します。
- Apple シリコンの macOS と Linux。

理由は、ツアーの[まだできないこと](tour-ship.md#まだできないこと)にあります。

---

## 次に読むもの

<div class="grid cards" markdown>

-   :material-rocket-launch: __[インストール](installation.md)__

    必要なもの、一度だけの用意、そして四つのコマンド。
    Apple シリコンの macOS と Linux です。

-   :material-book-open-variant: __[言語ツアー](tour.md)__

    アプリの書き方をひととおり。
    状態、ビュー、キャンバス、ウィンドウ、データベース、ゲート。
    最後はまだできないことで閉じます。

-   :material-view-gallery: __[デモ](demos.md)__

    44 本のアプリを、画面写真とソース全体つきで。

-   :material-github: __[ソース](https://github.com/i2y/yokan)__

    コマンドと、エンジンを開くパッケージと、エンジンと、デモ。

</div>

---

_名前は胡麻餅、黒胡麻を練り込んだ餅です。
エンジンを共にする羊羹と若草と落雁と同じく、和菓子から採りました。_
