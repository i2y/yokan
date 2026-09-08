<!-- Written by website/tools/tourpages from the tour. Edit the tour. -->
# はじめてのアプリ

Gomamochi は、pixie のエンジンの上で、Go でデスクトップアプリを作ります。
`gomamochi run` はファイルを読み、ビルドの手順なしにそのまま解釈実行します。
保存すると、ウィンドウは新しいコードを受け取り、持っていた値はそのまま保ちます。
`gomamochi build` は同じファイルを Go コンパイラでコンパイルし、ネイティブバイナリにします。
どちらも pixie の C API を通して、同じ描画エンジン（Zed エディタを支える **gpui**）に届きます。
その二つが同じプログラムかどうかは、願うものではなく確かめるものです。
`gomamochi gate` が一つのスクリプトで両方を動かし、描いた画面を 1 バイトずつ突き合わせます。
画面を組み立てる関数（`Text`、`Button`、`Column` など 30 あまり）はパッケージが用意しますが、アプリそのものは普通の Go の構造体です。
解釈実行が受け取る Go の範囲は、このツアーが説明する方言です。
そこから外れる書き方は[Gomamochi が断る書き方](tour-ship.md#gomamochi-が断る書き方)に挙げてあります。

アプリは構造体で、状態はそのフィールドです。
ハンドラはフィールドを閉じ込めたクロージャです。

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
構造体のフィールドが状態で、`View` は要素を一つ返すメソッドで、ハンドラはポインタ越しにその構造体を閉じ込めたクロージャです。
`Run` にアプリを渡すと、ウィンドウが開きます。
継承するものも、登録するものも、監視の対象だと印を付けるものもありません。
ハンドラを抜けるたびに、そのときのフィールドからビューが組み直されます。

パッケージはドットインポートしてあるので、要素はこのエンジンの上のほかの言語と同じ見た目で読めます（`Column`、`Text`、`Button`）。
これは好みの問題で、決まりではありません。
名前を付けてインポートすれば、呼び出しはすべてその接頭辞を持ち（`gm.Column(gm.Text(…))`）、アプリは自分の型にどんな名前でも使えます。
ドットインポートがパッケージのために取っておく `App` も、そのときは使えます。
どちらの実行もどちらの形も受け取り、`check` も同じです。

```console
$ ./bin/gomamochi run app.go
```

これでウィンドウが開き、ファイルが監視されます。
ビルドの手順はありません。
ファイルはコマンドに組み込まれたインタプリタが読んで動かし、保存はその場で効きます。


## 状態の持ち方

状態は構造体のフィールドで、それ以外にはありません。
ハンドラがそこに書き、ビューがそこを読みます。
初期値は、`Run` に渡すリテラルで与えるか、アプリを作る関数で与えます。

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
呼び出しの結果をそのまま `Run` に渡すことです。
`Run(newMixer(), …)` は解釈実行で失敗します。
解釈実行の型がパッケージの `App` インタフェースの代わりを務めるには包みが要りますが、解釈実行は呼び出しの結果をその包みなしで渡してしまうからです。
`app := newMixer()` と書いてから `Run(app, …)` と書きます。
`check` は行番号とともにそう言います。

型は Go 自身のもので、書き足す注釈はありません。
`count int`、`title string`、`volume float64`、スライス、マップ、自分で書いた構造体です。
コンパイルした実行の型は Go コンパイラが付け、解釈実行は同じ宣言を読みます。
文字列と数は、あいだに `strconv` を挟まずには出会いません。
これは Go がそうなっているのであって、こちらの決まりではありません。

