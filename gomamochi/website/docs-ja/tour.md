<!-- Written by website/tools/tourpages from the tour. Edit the tour. -->
# はじめてのアプリ

Gomamochi を使うと、pixie のエンジンの上で動くデスクトップアプリを Go で書けます。
`gomamochi run` はファイルを読み、ビルドの手順なしにそのまま解釈実行します。
保存すると、ウィンドウが新しいコードを取り込み、それまでの値はそのまま残ります。
`gomamochi build` は同じファイルを Go コンパイラでコンパイルし、ネイティブバイナリにします。
どちらの実行も、pixie の C API を通して同じ描画エンジン（Zed エディタを支える **gpui**）に行き着きます。
その二つが同じプログラムかどうかは、`gomamochi gate` で確かめます。
一つのスクリプトで両方を動かし、描いた画面を 1 バイトずつ突き合わせます。
画面を組み立てる関数（`Text`、`Button`、`Column` など 30 あまり）はパッケージが用意しますが、アプリそのものは普通の Go の構造体です。
解釈実行が受け付ける Go の範囲が、このツアーで説明する方言です。
そこから外れる書き方は[Gomamochi が断る書き方](tour-ship.md#gomamochi-が断る書き方)に挙げてあります。

アプリは構造体で、状態はそのフィールドです。
ハンドラは、そのフィールドがそのまま見えるクロージャです。

## いちばん小さいアプリ

<!-- script: click:+1,dump,input:Momo -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Counter struct {
	count int
	name  string
}

func (c *Counter) View() Element {
	return Column(
		Text(fmt.Sprintf("count: %d", c.count)).Size(34),
		Row(
			Button("+1").OnClick(func() { c.count += 1 }),
			Button("reset").OnClick(func() { c.count = 0 }),
		).Spacing(8),
		TextField(c.name).Placeholder("your name").OnChange(func(s string) { c.name = s }),
		Text(fmt.Sprintf("hello, %s", c.name)),
	).Spacing(12).Padding(16)
}

func main() {
	Run(&Counter{}, Title("counter"))
}
```

アプリは、構造体を一つ持つ `package main` です。
状態は構造体のフィールド、`View` は要素を一つ返すメソッド、ハンドラはポインタ越しにその構造体を参照するクロージャです。
アプリを `Run` に渡すと、ウィンドウが開きます。
継承するものも、登録するものも、監視対象だと印を付けるものもありません。
ハンドラを抜けるたびに、そのときのフィールドからビューが組み直されます。

パッケージはドットインポートしてあるので、要素の名前はこのエンジンの上のほかの言語と同じに見えます（`Column`、`Text`、`Button`）。
これは好みの問題で、決まりではありません。
名前を付けてインポートすると、呼び出しはすべて `gm.Column(gm.Text(…))` のようにその名前で始まります。
そのときは、アプリの型にどんな名前でも使えます。
ドットインポートのときにパッケージが使う `App` も、型の名前にできます。
二つの実行も `check` も、どちらの書き方でも受け付けます。

```console
$ ./bin/gomamochi run app.go
```

これでウィンドウが開き、ファイルを見はじめます。
ビルドの手順はありません。
ファイルを読んで動かすのはコマンドに組み込まれたインタプリタで、保存するとその場で反映されます。


## 状態の持ち方

状態は構造体のフィールドだけです。
ハンドラが書き換え、ビューが読みます。
初期値は、`Run` に渡すリテラルに書くか、アプリを作る関数の中で与えます。

<!-- script: click:+1,click:mute,dump,input:live set,dump -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Mixer struct {
	volume int
	title  string
	muted  bool
}

func newMixer() *Mixer {
	return &Mixer{volume: 5, title: "untitled"}
}

func (m *Mixer) View() Element {
	cells := []Element{
		Text(fmt.Sprintf("%s — vol %d", m.title, m.volume)).Size(16),
		Row(
			Button("+1").OnClick(func() { m.volume += 1 }),
			Button("mute").OnClick(func() { m.muted = true }),
			Button("unmute").OnClick(func() { m.muted = false }),
		).Spacing(8),
	}
	if m.muted {
		cells = append(cells, Text("(muted)").Size(12).Color("#8a8f98"))
	}
	cells = append(cells, TextField(m.title).Placeholder("title").OnChange(func(t string) { m.title = t }))
	return Column(cells...).Spacing(10).Padding(14)
}

func main() {
	app := newMixer()
	Run(app, Title("mixer"))
}
```

断られる形が一つあります。
呼び出しの結果をそのまま `Run` に渡す書き方です。
`Run(newMixer(), …)` は解釈実行で失敗します。
解釈実行の値がパッケージの `App` インタフェースとして通るにはラッパーが要りますが、呼び出しの結果はそれを付けずに渡されるからです。
`app := newMixer()` でいったん受けてから、`Run(app, …)` と書きます。
`check` はこれを行番号つきで指摘します。

型は Go 自身のもので、書き足す注釈はありません。
`count int`、`title string`、`volume float64`、スライス、マップ、自分で書いた構造体です。
コンパイルした実行では Go コンパイラが型を付け、解釈実行は同じ宣言を読みます。
文字列と数を混ぜるには、あいだに `strconv` が要ります。
これは Go の決まりで、Gomamochi が足したものではありません。

