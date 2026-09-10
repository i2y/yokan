# Gomamochi 言語ツアー

Gomamochi を使うと、pixie のエンジンの上で動くデスクトップアプリを Go で書けます。
`gomamochi run` はファイルを読み、ビルドの手順なしにそのまま解釈実行します。
保存すると、ウィンドウが新しいコードを取り込み、それまでの値はそのまま残ります。
`gomamochi build` は同じファイルを Go コンパイラでコンパイルし、ネイティブバイナリにします。
どちらの実行も、pixie の C API を通して同じ描画エンジン（Zed エディタを支える **gpui**）に行き着きます。
その二つが同じプログラムかどうかは、`gomamochi gate` で確かめます。
一つのスクリプトで両方を動かし、描いた画面を 1 バイトずつ突き合わせます。
画面を組み立てる関数（`Text`、`Button`、`Column` など 30 あまり）はパッケージが用意しますが、アプリそのものは普通の Go の構造体です。
解釈実行が受け付ける Go の範囲が、このツアーで説明する方言です。
そこから外れる書き方は[Gomamochi が断る書き方](#gomamochi-が断る書き方)に挙げてあります。

このページには、言語そのものが読者の出会う順に並んでいます。
ここに書いたものはすべて実際に動きます。
`go run ./tools/tourcheck` がこのファイルから完全なアプリをすべて取り出し、デモと同じコマンドに通すからです。
要素やキーワードの名前を変えれば、読者がそれを目にする前にこのページが壊れます。
英語版は [TOUR.md](TOUR.md) です。

## 目次

- [いちばん小さいアプリ](#いちばん小さいアプリ)
- [状態の持ち方](#状態の持ち方)
- [ビューの書き方](#ビューの書き方)
- [ビューの中の制御構造](#ビューの中の制御構造)
- [入力の要素](#入力の要素)
- [ハンドラ](#ハンドラ)
- [一覧、グラフ、必要な行だけ作る一覧](#一覧グラフ必要な行だけ作る一覧)
- [マップ](#マップ)
- [値の構造体](#値の構造体)
- [キャンバス](#キャンバス)
- [キーボード](#キーボード)
- [すべての要素が取るキーワード](#すべての要素が取るキーワード)
- [テーマとアニメーション](#テーマとアニメーション)
- [ウィンドウまわり](#ウィンドウまわり)
- [Go 自身の標準ライブラリ](#go-自身の標準ライブラリ)
- [フレームワークの標準ライブラリ](#フレームワークの標準ライブラリ)
- [タイマーと、ウィンドウの外でする処理](#タイマーとウィンドウの外でする処理)
- [書いているあいだ](#書いているあいだ)
- [ウィンドウなしの実行とゲート](#ウィンドウなしの実行とゲート)
- [Gomamochi が断る書き方](#gomamochi-が断る書き方)
- [リリース](#リリース)
- [まだできないこと](#まだできないこと)

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

## ビューの書き方

`View` は要素を一つ返します。
要素は値です。
コンストラクタが取るのは、その要素に欠かせないものだけです（`Text` の文字列、`Button` のラベル、`Column` の子）。
ほかの性質はすべてメソッドで指定します。
だからメソッドをつないだ形は、キーワードを並べたように読めます。

```go
Text("Badges").Size(20).Bold(true)
Button("save").Width(120).Background("#313244")
Column(a, b, c).Spacing(12).Padding(16)
```

エンジンがツリーを求めるまで、そこには何も渡りません。
だからビューの中では、画面の一部をどんな順番で組み立てても、そのまま受け渡せます。
名前のある画面の一部は、要素を返すメソッドです。
ほかの要素を包むものは、それらを引数として受け取ります。

<!-- script: click:+1,click:+10,dump -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Cards struct {
	a, b int
}

func (c *Cards) card(title string, kids ...Element) Element {
	cells := []Element{Text(title).Size(18)}
	cells = append(cells, kids...)
	return Column(cells...).Spacing(4).Padding(8).
		BorderWidth(1).BorderColor("accent").BorderRadius(8)
}

func (c *Cards) View() Element {
	return Column(
		c.card("counters",
			Row(Text(fmt.Sprintf("a: %d", c.a)), Button("+1").OnClick(func() { c.a += 1 })).Spacing(6),
			Row(Text(fmt.Sprintf("b: %d", c.b)), Button("+10").OnClick(func() { c.b += 10 })).Spacing(6)),
		Text("outside the card").Size(12),
	).Spacing(10).Padding(16)
}

func main() {
	Run(&Cards{}, Title("cards"))
}
```

ビューは読むだけです。
何かが変わるたびに、同じ状態からビューが組み直されるからです。
フィールドを書き換えることも、goroutine を起動することも、時計、環境変数、ファイル、キーボード、乱数を読むことも、処理を始めることもできません。
`check` はそのどれも行番号とともに指摘します。
これらを書く場所はハンドラかタイマーです。
`Element` を返す関数と、`*Painter` を渡されるクロージャもビューです。
ボタンや入力欄に付けたクロージャがハンドラで、書き換えはそこでします。

## ビューの中の制御構造

ビューの中に書くのも、普通の Go です。
`if` と `switch`、要素のスライスに追加していく `for`、画面の一部を返すメソッドが、そのまま使えます。

<!-- script: click:pick 1,dump,click:hint,click:tab 2,dump -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Control struct {
	items    []string
	picked   int
	showHint bool
	tab      int
}

func (c *Control) line(name string, i int) Element {
	label := "  " + name
	if i == c.picked {
		label = "▸ " + name
	}
	return Row(Text(label), Button(fmt.Sprintf("pick %d", i)).OnClick(func() { c.picked = i })).Spacing(8)
}

func (c *Control) View() Element {
	kids := []Element{Text("control flow").Size(18).Bold(true)}
	if c.showHint {
		kids = append(kids, Text("pick one").Size(12).Color("#8a8f98"))
	} else {
		kids = append(kids, Text("hidden").Size(12))
	}
	for i, name := range c.items {
		kids = append(kids, c.line(name, i))
	}
	if c.picked >= 0 {
		kids = append(kids, Text(fmt.Sprintf("picked %s", c.items[c.picked])).Color("accent"))
	}
	tabs := []Element{Button("hint").OnClick(func() { c.showHint = !c.showHint })}
	for n := 0; n < 3; n++ {
		tab := n
		tabs = append(tabs, Button(fmt.Sprintf("tab %d", n)).OnClick(func() { c.tab = tab }))
	}
	kids = append(kids, Row(tabs...).Spacing(6), Text(fmt.Sprintf("tab %d", c.tab)))
	return Column(kids...).Spacing(10).Padding(14)
}

func main() {
	Run(&Control{items: []string{"milk", "eggs", "rice"}, picked: -1, showHint: true}, Title("control"))
}
```

この `for` の中の `tab := n` は、Go 1.22 以降では要りません。
1.22 以降は繰り返しのたびに変数が新しく作られ、Gomamochi はその決まりを両方の実行で守ります。
インタプリタには古い決まりが残っているので、ファイルを読む前に、クロージャがキャプチャするループ変数のコピーを、本体の先頭の同じ行に作ります。
`for` は、コンパイラのために書くのと同じように書きます。
断られる形は一つだけです。
初期化と条件と後処理を書く形の `for` で、クロージャがキャプチャしているループ変数に、本体の中で代入する書き方です。
コピーがあると、その代入がループ側に伝わらなくなるからです。

## 入力の要素

人が操作する要素と、それぞれがハンドラに渡すものです。

| 要素 | ハンドラ | 受け取るもの |
|---|---|---|
| `Checkbox(label)`, `Switch(label)` | `OnChange(func(bool))` | 新しい状態 |
| `Slider()`, `NumberField(value)` | `OnChange(func(float64))` | 新しい数 |
| `IntField(value)` | `OnChange(func(int))` | 新しい整数 |
| `Select()`, `RadioGroup()`, `Segmented()`, `TabBar()` | `OnChange(func(int))` | 選ばれたインデックス |
| `TextField(value)` | `OnChange(func(string))`, `OnSubmit(func(string))` | 文字列 |
| `Button(label)` | `OnClick(func())` | なし |
| `Table(…)` | `OnSelect(func(int))`, `OnSort(func(int))` | 行の番号、列の番号 |

<!-- script: click:Dark mode,slide:7,select:banana -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Forms struct {
	dark   bool
	volume float64
	fruits []string
	fruit  int
	note   string
}

func (f *Forms) View() Element {
	return Column(
		Checkbox("Dark mode").Checked(f.dark).OnChange(func(on bool) { f.dark = on }),
		Slider().Value(f.volume).Min(0).Max(10).Step(1).OnChange(func(v float64) { f.volume = v }),
		Select().Options(f.fruits...).Selected(f.fruit).OnChange(func(i int) { f.fruit = i }),
		TextField(f.note).Placeholder("notes").Multiline(true).Rows(3).OnChange(func(t string) { f.note = t }),
		Text(fmt.Sprintf("dark=%v  vol=%.1f  fruit#%d", f.dark, f.volume, f.fruit)),
	).Spacing(10).Padding(14)
}

func main() {
	Run(&Forms{volume: 5, fruits: []string{"apple", "banana", "cherry"}}, Title("forms"))
}
```

入力欄が表示するのは、アプリが持っている値だけです。
`OnChange` でフィールドに書き戻していない `TextField` に打ち込んでも、次の組み直しでは元の値に戻ります。
そうなるのが狙いです。
本当の値はアプリのフィールドにしかなく、入力の要素はそれを映しているだけです。

## ハンドラ

ハンドラは Go のクロージャで、型はイベントが運ぶ値で決まります。
クリックなら `func()`、文字列なら `func(string)`、ほかは `func(bool)`、`func(int)`、`func(float64)` です。
ハンドラはポインタレシーバ経由でアプリを捕まえているので、更新はその中の `c.count += 1` だけで済みます。
そのあと、ビューが組み直されます。
同じ処理に名前を付けたいときは、メソッド値をそのまま渡せます。

```go
func (t *Todo) add(s string) { t.items = append(t.items, s) }

TextField(t.draft).OnSubmit(t.add)
```

ハンドラはウィンドウのスレッドで、ビューを組み直す合間に一つずつ実行されます。
時間のかかるハンドラ（データの取得、大きなテーブルへの問い合わせ、人の操作を待つダイアログ）は、`Task` に置きます。
これについては[タイマーと、ウィンドウの外でする処理](#タイマーとウィンドウの外でする処理)で説明します。

## 一覧、グラフ、必要な行だけ作る一覧

`ListView` が組み立てるのは画面に見えている行だけで、すべての行ではありません。
受け取るのは、行数と、行番号からその行の要素を作る関数です。

<!-- script: input:eggs,submit,dump,click:done,dump -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Todo struct {
	items []string
	draft string
	done  int
}

func (t *Todo) add(s string) {
	t.items = append(t.items, s)
	t.draft = ""
}

func (t *Todo) line(i int) Element {
	cells := []Element{Text(fmt.Sprintf("%d. %s", i+1, t.items[i]))}
	if i == t.done {
		cells = append(cells, Text("done").Color("accent"))
	}
	cells = append(cells, Button("done").OnClick(func() { t.done = i }))
	return Row(cells...).Spacing(8)
}

func (t *Todo) View() Element {
	return Column(
		Text(fmt.Sprintf("todo — %d items", len(t.items))).Size(16),
		TextField(t.draft).Placeholder("add and press enter").
			OnChange(func(s string) { t.draft = s }).OnSubmit(t.add),
		ListView(len(t.items), t.line).ItemHeight(26).Height(280),
		Button("clear").OnClick(func() { t.items = nil }),
	).Spacing(10).Padding(14)
}

func main() {
	Run(&Todo{items: []string{"milk"}, done: -1}, Title("todo"))
}
```

`Table(columns, count, row)` は同じ作りに見出しを足したもので、列の幅の割合は `Widths` で決めます。
選ばれている行は `Selected` が指し、並べ替えの状態はアプリが `Sort` と `Descending` に持ちます。
人がどの行を選び、どの列で並べ替えたかは、`OnSelect` と `OnSort` が伝えます。
`DataTable` はもっと単純で、最初の子が見出し、残りが行になり、枠は最初から付いています。

グラフは、描く数値を最初の引数に取ります。
`BarChart(data)` と `LineChart(data)` は `Labels`、`Axis`、`Min`、`Max` を共通に持ちます（`Min` と `Max` がどちらも 0 なら、範囲はデータから決まります）。
`Series` を渡すと複数の線やグループを描き、色は `Colors` から一つずつ使います。

```go
BarChart(profit).Labels(months...).Axis(true).Height(150)
LineChart(nil).Series([][]float64{requests, errors}).Colors("accent", "#f38ba8").Max(90).Height(150)
```

## マップ

マップも、ほかと変わらないフィールドです。
キーは二つ目の戻り値と一緒に読みます。
要素の数を数えるのも、ハンドラの中で書き加えるのも、そのままできます。

<!-- script: click:apple,dump,click:cherry,dump -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Lookup struct {
	prices map[string]int
	picked int
	label  string
}

func (l *Lookup) price(name string, fallback int) int {
	if v, ok := l.prices[name]; ok {
		return v
	}
	return fallback
}

func (l *Lookup) View() Element {
	return Column(
		Text(fmt.Sprintf("picked=%d n=%d %s", l.picked, len(l.prices), l.label)),
		Row(
			Button("apple").OnClick(func() { l.picked = l.price("apple", -1) }),
			Button("cherry").OnClick(func() {
				l.prices["cherry"] = 200
				l.picked = l.price("cherry", -1)
				l.label = "cherry known"
			}),
		).Spacing(6),
	).Spacing(8).Padding(12)
}

func main() {
	Run(&Lookup{prices: map[string]int{"apple": 120}, label: "none"}, Title("lookup"))
}
```

ビューの中でできないのは、マップを `range` で回すことです。
Go がマップのキーを返す順序は実行のたびに変わるので、そうしたビューは二つの実行で違う画面を描きます。
ゲートはその違いを、何回かに一回、見つけることになります。
`check` は、その行番号を示して断ります。
ハンドラなら、マップを `range` で回せます。
キーを集めて並べ替えるのは、ハンドラの側でします。
並べ替えたスライスをアプリに持たせ、ビューはそれを読みます。

## 値の構造体

自分で書いた構造体は、アプリが持つ値です。
そのスライスは、値のリストになります。
二つのゲームは、ティックごとにそのリストを作り直して、新しいスライスをフィールドに入れます。
印を付ける必要も、共有する必要も、監視する必要もありません。

<!-- script: click:right,click:measure,dump,click:swap,dump -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Point struct {
	x, y int
}

type Points struct {
	sel  Point
	dist int
}

func (p *Points) View() Element {
	return Column(
		Text(fmt.Sprintf("p=(%d, %d) d2=%d", p.sel.x, p.sel.y, p.dist)),
		Row(
			Button("right").OnClick(func() { p.sel = Point{p.sel.x + 5, p.sel.y} }),
			Button("swap").OnClick(func() { p.sel = Point{p.sel.y, p.sel.x} }),
			Button("measure").OnClick(func() { p.dist = p.sel.x*p.sel.x + p.sel.y*p.sel.y }),
		).Spacing(6),
	).Spacing(8).Padding(12)
}

func main() {
	Run(&Points{sel: Point{3, 4}}, Title("points"))
}
```

何もないかもしれない値は、nil を取りうるポインタで表します。
互いを指し合うオブジェクトには、両方向にポインタを持たせます。
Go のガベージコレクタは循環参照も問題なく回収するので、ツリーは素直に書けます。

## キャンバス

キャンバスは仮想的な画素の格子で、命令を一つずつ実行して塗っていきます。
格子を用意するのは `Canvas(width, height)` です。
`Scale` は、仮想的な画素一つを論理画素いくつ分で描くかを決めます。
64x40 のキャンバスに 6 を与えると、画面では 384x240 になります。
色に名前を付けるのは `Palette` で、`Paint` には描画命令を持つ `*Painter` が渡されます。

<!-- script: advance:50,dump,keydown:left,advance:50,dump -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

var palette = []string{"#11111b", "#89b4fa", "#f38ba8", "#eeeeee", "#a6e3a1"}

type Sky struct {
	frame int
	x, dx int
}

func (s *Sky) tick() {
	s.frame += 1
	// The keyboard is read here, in the tick, never in a view.
	if KeyDown("left") {
		s.dx = -1
	}
	if KeyDown("right") {
		s.dx = 1
	}
	s.x += s.dx
	if s.x < 4 || s.x > 59 {
		s.dx = -s.dx
		s.x += s.dx
	}
}

func (s *Sky) View() Element {
	return Column(
		Canvas(64, 40).Scale(6).Background(0).Palette(palette...).Paint(func(p *Painter) {
			p.Rect(2, 2, 12, 6, 1)
			p.CircleOutline(34, 5, 4, 3)
			p.Line(2, 11, 61, 11, 2)
			p.Circle(s.x, 24, 3, 3)
			p.PixelText(2, 14, fmt.Sprintf("FRAME %d", s.frame), 3)
		}),
	).Spacing(12).Padding(16)
}

func main() {
	app := &Sky{x: 30, dx: 1}
	Every(0.05, func() { app.tick() })
	Run(app, Title("canvas"))
}
```

色はどれも番号です。
パレットの何番目か、という意味の番号です。
ドット絵の道具はどれもこの作りなので、そうした道具のために書かれた描画のコードは、数字を書き換えずにそのまま移せます。
`*Painter` の命令は `Pixel`、`Line`、`Rect`、`RectOutline`、`Circle`、`CircleOutline`、`Triangle`、`TriangleOutline`、`Sprite`、`PixelText` です。
引数はどれも仮想的な画素を表す整数で、省略できるものはありません。
`Sprite` は、画像、切り出す矩形、カラーキー（`-1` ならすべての画素を写します）、二つの反転を、この順に取ります。
`Paint` のクロージャの中の繰り返しは、ただの繰り返しです。
その本体が塗ったものは、書いた場所でそのまま絵に加わります。

命令は要素ではありません。
塗ったものは、押すことも、テーマを変えることも、大きさを与えることも、動かすこともできません。
命令が意味を持つのは、それを書いたキャンバスの中だけです。
一コマは、ウィンドウを開かなくても取り出せます。
`PIXIE_FRAMES=<dir>` を与えると、スクリプトの一手ごとにキャンバスを PNG に書き出します。
絵を描くのは、ウィンドウが使うのと同じラスタライザです。
ギャラリーにある二つのゲームの録画も、こうして撮りました。

## キーボード

問いは二つあり、答え方もそれぞれ違います。

「今どのキーが押されているか」は、コマを描くアプリのための問いです。
`KeyDown("left")` はキーが押されているあいだ真で、`KeyPressed` は押された瞬間に一度だけ、`KeyReleased` は離された瞬間に一度だけ真になります。
渡す名前はキー一つだけで、`cmd+s` のような組み合わせは書けません。
これらを読むのはタイマーの中だけで、ビューの中では読みません。
ビューでキーボードを読むと、ウィンドウとスクリプトで違う画面を描いてしまいます。
そうなると、ゲートは違う二つのアプリを比べることになります。

「今どのキーが押されたか」は組み合わせで、アプリが動き出す前に宣言しておいたハンドラに届きます。

<!-- script: click:+1,key:cmd+s,dump,key:x,menu:Clear,dump -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Keys struct {
	count, saved int
	last         string
}

func (k *Keys) View() Element {
	return Column(
		Text(fmt.Sprintf("count: %d  saved: %d  last key: %s", k.count, k.saved, k.last)),
		Button("+1").OnClick(func() { k.count += 1 }),
	).Spacing(8).Padding(12)
}

func main() {
	app := &Keys{last: "-"}
	MenuItem("Count", "Save", func() { app.saved = app.count })
	MenuItem("Count", "Clear", func() { app.count, app.saved = 0, 0 })
	Shortcut("cmd+s", func() { app.saved = app.count })
	OnKey(func(chord string) { app.last = chord })
	Run(app, Title("keys"))
}
```

`Shortcut` には、プラットフォームのつづりのままの組み合わせ（`cmd+s`、`cmd+shift+r`）を渡します。
`MenuItem` は同じハンドラを、宣言した順にアプリケーションのメニューバーに並べます。
`OnKey` にはどのキーも、押されたときの組み合わせのまま届きます。
`OnFileDrop` には、ウィンドウに落とされたファイルのパスが届きます。
スクリプトでは、`key:cmd+s` で押し、`menu:Clear` で選び、`drop:<path>` でファイルを落とします。

## すべての要素が取るキーワード

次の 15 の性質は、どの要素でも同じ意味を持ち、どれもメソッドとして呼べます。

| メソッド | 型 | 働き |
|---|---|---|
| `Width(v)`, `Height(v)` | float64 | 固定の大きさ。0 でもそのまま書かれる |
| `MinWidth(v)`, `MaxWidth(v)` | float64 | 大きさの下限と上限 |
| `Disabled(v)` | bool | 操作を受け付けず、その見た目で描かれる |
| `Theme(v)` | string | 下にある要素が色の名前を引く配色。`"dark"` か `"light"` |
| `Animate(ms)`, `Easing(v)` | float64, string | 変化にかける時間と、その曲がり方（`"out"`、`"inOut"`） |
| `Enter(v)`, `Exit(v)` | bool | 現れるとき、消えるときの動き |
| `ColSpan(v)`, `RowSpan(v)` | int | グリッドの列や行をいくつ分またぐか |
| `Role(v)`, `A11yLabel(v)` | string | 読み上げに伝える内容 |
| `Tooltip(v)` | string | ポインタを合わせたときに出る文字 |

同じ名前をもともと自分の意味で持っている要素では、要素側の意味がそのまま通ります。
`Button` の `Width` はボタン自身の幅で、ボタンはその幅で描かれます。
`Column` の `Width` は、中身を囲む箱の幅です。
ラベルがそのまま読み上げの名前になる要素（`Checkbox`、`Switch`、`Progress`）は、`A11yLabel` を断ります。
与えるべき二つ目の名前がないからです。

## テーマとアニメーション

色は 16 進の値か、エンジンが持つ色の名前（`"accent"`、`"panel"`、`"text"`、`"textDim"`）です。
名前で書いた色は、上にあるいちばん近い `Theme` の配色で決まります。
だからメソッド一つで、パネル全体の色が切り替わります。

<!-- script: click:+1,dump,click:flip,dump -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

func key(b *ButtonEl) *ButtonEl { return b.Background("#313244").HoverBackground("#45475a") }

type Styled struct {
	mode string
	n    int
}

func (s *Styled) flip() {
	if s.mode == "dark" {
		s.mode = "light"
	} else {
		s.mode = "dark"
	}
}

func (s *Styled) View() Element {
	return Column(
		Text(fmt.Sprintf("n=%d", s.n)).Size(18).Color("accent"),
		Row(
			key(Button("+1")).OnClick(func() { s.n += 1 }),
			key(Button("flip")).Background("#fab387").OnClick(func() { s.flip() }),
		).Spacing(6),
	).Spacing(8).Padding(12).Background("panel").Theme(s.mode)
}

func main() {
	Run(&Styled{mode: "dark"}, Title("styled"))
}
```

見た目を一か所にまとめるには、渡された要素すべてに同じ性質を付ける関数を書きます。
上の `key` がそれです。
返るのは元の要素の型なので、そのあともメソッドをつなげられます。
要素に `Animate(120).Easing("out")` と書くと、その要素は変化のたびに 120 ミリ秒かけてなめらかに動きます。
現れるときと消えるときにも、`Enter` と `Exit` で同じ動きがつきます。

## ウィンドウまわり

`Run` はアプリとそのオプションを取ります。
オプションは `Title`、`Size(width, height)`、それにウィンドウの縁とアプリのツリーのあいだの余白である `Padding` です。
`Quit()` は、エンジンの次のフレームでウィンドウを閉じます。
ウィンドウなしの実行ではこれが効かないので、スクリプトは最後まで走ります。

並べたり覆ったりする要素は、次のとおりです。
`Grid` は子要素を `Columns` で決めた数の列に並べ、`GridCell(child).ColSpan(2)` はその列をまたぎます。
`Stack` は子要素を重ねて置きます。
`ScrollView` と `HScrollView` はスクロールする領域です。
`Modal` は、`Open` のあいだ、ウィンドウのほかの部分に重なって出るパネルです。
`Spacer` は親要素に余っている場所を取り、`Divider` は罫線を引き、`Link` はページを開き、`Spinner` と `Progress` は処理中であることを示し、`Image` と `Svg` はファイルを表示します。

<!-- script: click:about,dump,click:close,dump -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Panels struct {
	open bool
}

func (p *Panels) View() Element {
	var lines []Element
	for n := 1; n <= 12; n++ {
		lines = append(lines, Text(fmt.Sprintf("line %d", n)))
	}
	return Stack(
		Column(
			Row(Text("Panels").Size(18), Spacer(), Spinner().Size(14)).Spacing(8),
			Grid(Text("one"), Text("two"), GridCell(Text("across both").Align("center")).ColSpan(2)).Columns(2).Spacing(6),
			ScrollView(Column(lines...).Spacing(2)).Height(90),
			Button("about").OnClick(func() { p.open = true }),
		).Spacing(10).Padding(14),
		Modal(
			Column(
				Text("A panel over the rest of it.").Size(14),
				Button("close").OnClick(func() { p.open = false }),
			).Spacing(8).Padding(12).Background("panel"),
		).Open(p.open),
	)
}

func main() {
	Run(&Panels{}, Title("panels"), Size(420, 360))
}
```

## Go 自身の標準ライブラリ

`fmt`、`strings`、`strconv`、`sort`、`math`、`time`、`encoding/json`、`encoding/csv`、`regexp`、`os`、`net/http` は言語自身のもので、両方の実行が同じコンパイル済みのパッケージを呼びます。
インタプリタはそれらを実装し直しません。
呼ぶのはコマンドに組み込まれたパッケージそのものです。
だから `fmt.Sprintf("%.2f", x)` が返すバイト列は、両方の実行で同じです。
ゲートがライブラリの二つの実装を比べることもありません。

<!-- script: click:stats,click:parse,click:scan,dump -->
```go
package main

import (
	"encoding/json"
	"fmt"
	"regexp"
	"sort"
	"strconv"

	. "github.com/i2y/yokan/gomamochi"
)

type Stdlib struct {
	scores []int
	spread string
	doc    string
	sum    int
}

func (s *Stdlib) stats() {
	sorted := append([]int{}, s.scores...)
	sort.Ints(sorted)
	s.spread = fmt.Sprintf("median %d min %d max %d", sorted[len(sorted)/2], sorted[0], sorted[len(sorted)-1])
}

func (s *Stdlib) parse() {
	var doc map[string]any
	json.Unmarshal([]byte(`{"name": "gomamochi", "ok": true}`), &doc)
	s.doc = fmt.Sprintf("%v %v", doc["name"], doc["ok"])
}

func (s *Stdlib) scan() {
	s.sum = 0
	for _, m := range regexp.MustCompile(`\d+`).FindAllString("a1b22c333", -1) {
		v, _ := strconv.Atoi(m)
		s.sum += v
	}
}

func (s *Stdlib) View() Element {
	return Column(
		Text("spread: "+s.spread),
		Text("json: "+s.doc),
		Text(fmt.Sprintf("scan: %d", s.sum)),
		Row(
			Button("stats").OnClick(func() { s.stats() }),
			Button("parse").OnClick(func() { s.parse() }),
			Button("scan").OnClick(func() { s.scan() }),
		).Spacing(6),
	).Spacing(6).Padding(14)
}

func main() {
	Run(&Stdlib{scores: []int{3, 5, 8, 13, 21}, spread: "-", doc: "-"}, Title("stdlib"))
}
```

インタプリタが知っているライブラリは Go 1.22 のもので、そのリリースにあるパッケージと関数です。
言語のほうは、それより手前までしか知りません。
`min` と `max`（1.21）も、数を渡す `range`（1.22）も、関数を渡す `range`（1.23）もありません。
1.23 以降にライブラリへ足されたものも使えません。
最初の三つは、`check` がその名前を挙げて知らせます。
ファイルは `os`、ネットワークは `net/http`、大きな数は `math/big` です。
デモはファイルを読み、自分でページを立てて取得し、CSV を解析しますが、使うのは標準ライブラリだけです。

## フレームワークの標準ライブラリ

エンジンが仲立ちするものは、フレームワークのパッケージから来ます。
そこでは一つの実装が、同じ C API を通して両方の実行に答えます。
スクリプトで走らせるときは、どちらの実行でもダイアログに `file:<path>` の手順が答え、音は鳴りません。

| 関数 | 働き |
|---|---|
| `SqliteExec(db, sql, params...)` | 文を実行し、変わった行数を返す |
| `SqliteQueryText`, `SqliteQueryInt`, `SqliteQueryRows` | 問い合わせの最初の列、最初のセル、行全体 |
| `SqliteQueryTextOr`, `SqliteQueryIntOr`, `SqliteQueryRowsOr` | 同じもの。問い合わせが失敗しても止まらず、何も返さない |
| `ClipboardSetText(s)`, `ClipboardGetText()` | システムのクリップボード |
| `OpenDialog(title)`, `SaveDialog(name)` | プラットフォーム自身のパネル。人の操作を待つので、`Task` の中で呼ぶ |
| `AudioPlay(path, volume)`, `AudioStop()` | WAV ファイルを鳴らして、あとは放っておく |
| `NotifySend(title, body)` | 通知 |

文には `?` を書き、値はそのあとの引数として渡します。
そうすれば、人が打った文字が文の一部になることはありません。

<!-- script: click:setup,click:load,dump -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

const db = "demo/.gate/tour-notes.db"

type Notes struct {
	changed int
	rows    []string
}

func (nt *Notes) setup() {
	SqliteExec(db, "CREATE TABLE IF NOT EXISTS notes(t TEXT)")
	SqliteExec(db, "DELETE FROM notes")
	nt.changed = SqliteExec(db, "INSERT INTO notes VALUES (?), (?)", "alpha", "beta")
}

func (nt *Notes) View() Element {
	return Column(
		Text(fmt.Sprintf("inserted=%d rows=%d", nt.changed, len(nt.rows))),
		Row(
			Button("setup").OnClick(func() { nt.setup() }),
			Button("load").OnClick(func() { nt.rows = SqliteQueryText(db, "SELECT t FROM notes ORDER BY t") }),
		).Spacing(6),
		ListView(len(nt.rows), func(i int) Element { return Text(nt.rows[i]) }).ItemHeight(22).Height(80),
	).Spacing(8).Padding(12)
}

func main() {
	Run(&Notes{}, Title("notes"))
}
```

テーブルができる前に走るかもしれない問い合わせは、何も返さない `…Or` の形で書きます。
帳簿の最初の読み込みがそうです。
素の形は、ライブラリ自身と同じく panic になり、その値はライブラリの文言です。
ハンドラの中の `recover` が受け取れます。
受け取らなければ、アプリはそこで止まります。

## タイマーと、ウィンドウの外でする処理

決まった間隔で呼んでほしい処理は、`Every(seconds, tick)` の `tick` に渡します。
書くのは `Run` より前です。
二つの実行は一つの時計で動きます。
その時計を進めるのは、ウィンドウでは 1 フレーム、スクリプトでは `advance:<ms>` の 1 手順です。
だから、どちらの実行にも同じ数のティックが届きます。
ゲームが動くのも、キーボードを読むのも、このティックの中です。

`Task(work, done)` は、`work` を専用の goroutine で走らせます。
`work` が終わると、それが返した値を渡して、`done` をウィンドウのスレッドで呼びます。
`work` の中では、アプリのフィールドにも画面にも触りません。
決まりはこれだけです。
結果をどこかに書き込むのではなく `done` の引数で受け取るのも、そのためです。

<!-- script: click:start,dump -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Jobs struct {
	status string
	answer int
}

func (j *Jobs) start() {
	j.status = "working"
	Task(func() any {
		total := 0
		for i := 0; i < 300000; i++ {
			total += i % 7
		}
		return total
	}, func(v any) {
		j.answer = v.(int)
		j.status = "done"
	})
}

func (j *Jobs) View() Element {
	return Column(
		Text(fmt.Sprintf("%s: %d", j.status, j.answer)),
		Button("start").OnClick(func() { j.start() }),
	).Spacing(10).Padding(14)
}

func main() {
	Run(&Jobs{status: "idle"}, Title("tasks"))
}
```

スクリプトは、次の手順に進む前にタスクが終わるのを待ちます。
だから `click:start` のあとの `dump` は、どちらの実行でも結果を出力します。
フレームワークのライブラリ（ダイアログや問い合わせ）は、タスクの goroutine からでも、アプリが自分で始めた goroutine からでも呼べます。
一つの呼び出しは、終わるまで同じスレッドで動きます。
アプリの側で気にすることは何もありません。

## 書いているあいだ

`gomamochi run` はアプリのファイルを監視しています。
保存すると、その変更がウィンドウに反映されます。
ファイルは新しいインタプリタで読み直され、ウィンドウが持っているアプリが、自分の値を新しいアプリに引き継ぎます。
名前と型が同じフィールドは、それまでの値をそのまま保ちます。
新しいファイルで足したフィールドは `main` が与えた値から始まり（与えなければゼロ値です）、型が変わったフィールドは値を引き継ぎません。
ファイルが宣言しているタイマーとショートカットは、新しいアプリに結び直されます。
コンパイルできないファイルを保存しても、ウィンドウは直前の表示のままで、端末にそのことが出ます。
次にコンパイルできるファイルを保存すれば、それが効きます。

保存するとファイル全体が読み直され、`main` も走り直します。
ただし、`main` がアプリに渡す初期値で、ウィンドウの持っている値が置き換わることはありません。
値は引き継がれます。
それがこの仕組みの狙いです。

## ウィンドウなしの実行とゲート

`PIXIE_SCRIPT` が人の操作の代わりになります。
エンジンがツリーを組み、そこに書かれた手順のとおりに動かして、dump を出力します。

```
click:<label>      表示されている文字でボタンを押す
input:<text>       欄に打つ            submit    その欄で改行する
slide / select     つまみを動かす、選択肢を選ぶ
click@1:<label>    同じ文字のボタンの二つ目（n はツリー順に 0 から数える）
                   input@n:、submit@n、slide@n:、select@n: も同じ
key:<chord>        Shortcut に結ばれた打鍵
keydown:<key> / keyup:<key>    キーを押したままにする、離す
menu:<item>        メニュー項目を選ぶ  file:<path>   ダイアログの答え
drop:<path>        ウィンドウにファイルを落とす
advance:<ms>       時計を進める        theme:dark|light
dump               ツリーを出力            a11y   読み上げが読むものを出力
```

`gomamochi gate` は、一つのスクリプトでアプリを二回走らせます。
片方はインタプリタが動かすファイルで、もう片方は Go コンパイラがそのファイルから作ったバイナリです。
最後に、二つの記録を一バイトずつ比べます。

```console
$ ./bin/gomamochi gate demo/counter.go --script "click:+1,dump,input:Momo"
GATE OK — 3 dump lines identical in both runs
```

ゲートが、Gomamochi の約束そのものです。
書いているあいだ見ているのはインタプリタで、配るのはコンパイラが作ったバイナリです。
ゲートは、その二つが一致することを言っています。
このページのほかの部分では、そのゲートを通るものをどう書くかを説明しています。
`--fresh <path>` は、実行のたびに指定したパスを消します。
ファイルやデータベースを使うアプリでも、二つの実行を同じ何もない状態から始められます。

## Gomamochi が断る書き方

`gomamochi check` はアプリを読みます。
受け取れない書き方があれば、その行番号と、位置を指す `^` と、書き換え方を出力します。
実行、ビルド、ゲートの前には毎回走ります。
言うことがなければ何も出力しません。

```console
$ ./bin/gomamochi check demo/broken.go
demo/broken.go:8:2: Gomamochi cannot take this — a view only reads. Move the write into a handler — the closure on a button, or a method the app calls from one
    	a.n += 1
    	^
```

チェックは二段階です。
一つ目はファイルだけを読むので、どこでも走ります。
二つ目は Go 自身の型チェッカです。
ツールチェーンのエクスポートデータを渡し、Go の型エラーを Go の言葉のまま出します。
型がわかって初めて決まることは、ここで加わります。
これが走るのは `go` がパスにあるときで、`build` と `gate` はどのみち `go` を使います。

解釈実行が、コンパイルした実行と同じようには扱えないものは次のとおりです。

- `min` と `max`（Go 1.21）、数の上を回る `range`（1.22）、関数の上を回る `range`（1.23）。
  インタプリタにはありません。
  比較を自分で書くか、`for i := 0; i < n; i++` と書きます。
- 呼び出しの結果をそのまま `Run` に渡すこと。
  いったん変数に入れて、その名前を渡します。
- `%T` と `reflect`。
  インタプリタは、アプリ自身の型を別の名前で呼びます。
- `unsafe`、cgo、`//go:embed`、標準ライブラリの外のモジュール。
- クロージャに捕まえられた変数を書き換える、3 節の `for`。

ビューにできないことは次のとおりです。
ビューは、何かが変わるたびに同じ状態から組み直されるからです。

- アプリのフィールドを書き換えること、goroutine を起動すること。
- 時計、環境、ファイル、ストリーム、ネットワーク、乱数、キーボードを読むこと。
  タスクやタイマーを始めること。
  音を鳴らすこと。
- マップの上を `range` で回ること。

どれも、出力される文面ごと `test/refuse/` に置いてあります。
だから、断りの文面が黙って変わることはありません。

## リリース

```console
$ ./bin/gomamochi build demo/todo.go --release --app
built: demo/.gate/todo/todo (1.9 MB)
bundle: demo/dist/todo.app (22.3 MB)
```

`build` は、cgo を切って Go コンパイラでファイルをコンパイルします。
できたバイナリは、システム自身のライブラリ以外は何もリンクしません。
エンジンは、そのバイナリの隣に置く共有ライブラリです。
エンジンを呼び出す部分が、まずその場所を探してロードします。
`--release` は symbol table を落とします。
`--app` はその二つを macOS のアプリケーションバンドルに包み、ad-hoc 署名をつけます。
アプリと同じ場所に `<stem>.png` か `<stem>.icns` を置いておくと、それがアイコンになります。
このバンドルだけでプログラムが完結します。
Go もツールチェーンも入っていないマシンで、そのまま開きます。
Linux では、`--app` は代わりに AppDir を書きます。
`--appimage` はそれを一つのファイルにまとめ、`--carry-libs` はデスクトップのライブラリも一緒に運びます。
そのライブラリが入っていないかもしれないマシンのためです。

## まだできないこと

- インタプリタのライブラリは Go 1.22 のもので、言語はそこに届いていません。
  `min` と `max`（1.21）、数の上を回る `range`（1.22）、関数の上を回る `range`（1.23）、1.23 以降に標準ライブラリへ足されたものはありません。
  最初の三つは `check` が名前を挙げて断ります。
  残りは、インタプリタ自身のエラーがその名前を出します。
- 標準ライブラリの外のモジュールは、解釈実行がまだ読めません。
  アプリは 1 ファイルです。
- ドットインポートを使っているあいだは、パッケージが公開している名前（`App`、`Element`、`Text` など）をアプリの側で宣言できません。
  パッケージに名前を付けてインポートすれば宣言できます。
- recover した panic の文面は、二つの実行で違います。
  `%T` が出力する名前も違います。
  どちらも人に見せるものではありません。
- 解釈実行は、回数の多いループではコンパイルした実行より二桁遅くなります。
  インタプリタを挟んでいる分のコストです。
  それでも、どちらのゲームも 1 フレームは 1 ミリ秒かかりません。
- macOS と Linux です。
  バンドルと AppDir はエンジンを含むので、小さなアプリでも 22 MB ほどになります。
  Linux 向けのパッケージングは Yokan と同じ書き方ですが、まだ Linux のマシンでは試していません。
