# Gomamochi 言語ツアー

Gomamochi は、pixie のエンジンの上で、Go でデスクトップアプリを作ります。
`gomamochi run` はファイルを読み、ビルドの手順なしにそのまま解釈実行します。
保存すると、ウィンドウは新しいコードを受け取り、持っていた値はそのまま保ちます。
`gomamochi build` は同じファイルを Go コンパイラでコンパイルし、ネイティブバイナリにします。
どちらも pixie の C API を通して、同じ描画エンジン（Zed エディタを支える **gpui**）に届きます。
その二つが同じプログラムかどうかは、願うものではなく確かめるものです。
`gomamochi gate` が一つのスクリプトで両方を動かし、描いた画面を 1 バイトずつ突き合わせます。
画面を組み立てる関数（`Text`、`Button`、`Column` など 30 あまり）はパッケージが用意しますが、アプリそのものは普通の Go の構造体です。
解釈実行が受け取る Go の範囲は、このツアーが説明する方言です。
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

## ビューの書き方

`View` は要素を一つ返します。
要素は値です。
コンストラクタは、その要素に欠かせないもの（`Text` の文字列、`Button` のラベル、`Column` の子）を取り、ほかの性質はすべてその要素のメソッドです。
だから連鎖は、キーワードの並びを読むように読めます。

```go
Text("Badges").Size(20).Bold(true)
Button("save").Width(120).Background("#313244")
Column(a, b, c).Spacing(12).Padding(16)
```

エンジンが木を求めるまで、エンジンには何も書かれません。
だからビューは、画面の一部をどんな順序で組んでも、それを受け渡せます。
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
何かが変わるたびに同じ状態から組み直されるので、フィールドに書くことも、goroutine を始めることも、時計、環境、ファイル、キーボード、乱数を読むことも、処理を始めることもできません。
`check` はそのどれも行番号とともに名指しし、それらの居場所はハンドラかタイマーです。
`Element` を返す関数と、`*Painter` を渡されるクロージャもビューです。
ボタンや欄に付けたクロージャはハンドラで、書くのはそこです。

## ビューの中の制御構造

ビューの中に書くのも、普通の Go です。
`if`、`switch`、要素のスライスに追加していく繰り返し、画面の一部を返すメソッドがそのまま使えます。

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

この繰り返しの中の `tab := n` は、Go 1.22 以降では要りません。
そこでは反復ごとに変数が一つずつあり、Gomamochi はその決まりを両方の実行で守ります。
インタプリタにはまだ古い決まりが残っているので、ファイルを読む前に、クロージャが捕まえる繰り返しの変数はどれも、本体の先頭の同じ行に自分の写しを与えられます。
繰り返しは、コンパイラのために書くのと同じように書きます。
断られる形は一つだけで、クロージャに捕まえられている自分の変数に本体の中で代入する、3 節の `for` です。
写しがその代入を繰り返しから隠してしまうからです。

## 入力の要素

人が変える要素と、それぞれがハンドラに渡すものです。

| 要素 | ハンドラ | 運ぶもの |
|---|---|---|
| `Checkbox(label)`, `Switch(label)` | `OnChange(func(bool))` | 新しい状態 |
| `Slider()`, `NumberField(value)` | `OnChange(func(float64))` | 新しい数 |
| `IntField(value)` | `OnChange(func(int))` | 新しい整数 |
| `Select()`, `RadioGroup()`, `Segmented()`, `TabBar()` | `OnChange(func(int))` | 選ばれた添字 |
| `TextField(value)` | `OnChange(func(string))`, `OnSubmit(func(string))` | 文字列 |
| `Button(label)` | `OnClick(func())` | なし |
| `Table(…)` | `OnSelect(func(int))`, `OnSort(func(int))` | 行、列 |

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

欄が表示するのは、アプリが持っているものだけです。
`OnChange` がフィールドに書き戻さない `TextField` に打ち込むと、次の組み直しではまた古い値が表示されます。
それがこの作りの狙いです。
本当の値はアプリのフィールドだけにあり、入力の要素はそれを映すビューです。

## ハンドラ

ハンドラは Go のクロージャで、その型はイベントが運ぶもので決まります。
クリックなら `func()`、文字列なら `func(string)`、残りは `func(bool)`、`func(int)`、`func(float64)` です。
ポインタレシーバ越しにアプリを閉じ込めているので、その中の `c.count += 1` が更新のすべてで、そのあとにビューが組み直されます。
メソッド値は、同じことを名前のある形でします。

```go
func (t *Todo) add(s string) { t.items = append(t.items, s) }

TextField(t.draft).OnSubmit(t.add)
```

ハンドラはウィンドウのスレッドで、組み直しと組み直しのあいだに、一つずつ走ります。
時間のかかるハンドラ（取得、大きな表への問い合わせ、人を待つダイアログ）は `Task` に置きます。
それが[タイマーと、ウィンドウの外でする処理](#タイマーとウィンドウの外でする処理)の話です。

## 一覧、グラフ、必要な行だけ作る一覧

`ListView` が求められるのは見えている行だけで、すべての行ではありません。
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

`Table(columns, count, row)` は同じ作りに、見出しと、`Widths` で幅の割合を決める列と、`Selected` の行と、アプリが持つ `Sort` と `Descending` を加えたものです。
人がどの行とどの列を選んだかは、`OnSelect` と `OnSort` が伝えます。
`DataTable` はもっと単純なもので、最初の子が見出し、残りが行で、枠が付いてきます。

グラフは、数を最初の引数に取ります。
`BarChart(data)` と `LineChart(data)` は `Labels`、`Axis`、`Min`、`Max` を共有し（どちらも 0 ならデータから範囲を取ります）、`Series` は複数の線やグループを、`Colors` から一色ずつ取って描きます。

```go
BarChart(profit).Labels(months...).Axis(true).Height(150)
LineChart(nil).Series([][]float64{requests, errors}).Colors("accent", "#f38ba8").Max(90).Height(150)
```

## マップ

マップは、ほかと変わらないフィールドです。
鍵は二つ目の戻り値とともに読み、要素の数を数え、ハンドラの中で足します。

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

ビューにできないのは、マップの上を `range` で回ることです。
Go がマップを歩く順序は走らせるたびに違うので、そうしたビューは二つの実行で違う画面を描くことになり、ゲートはそれを、何回かに一回、指摘することになります。
`check` はそれを行番号とともに断ります。
ハンドラはマップの上を回れます。
鍵を集めて並べ替えるのはその方法で、そのあとアプリに持たせるもの（並べ替えたスライス）をビューが読みます。

## 値の構造体

自分で書いた構造体は、アプリが持つ値です。
そのスライスは、値のリストです。
二つのゲームはティックごとに自分のものを作り直し、新しいスライスをフィールドに渡します。
印を付けるものも、共有するものも、監視するものもありません。

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

何もないかもしれない値は、nil かもしれないポインタです。
互いを指し合うオブジェクトは、両方向のポインタです。
Go のコレクタは循環を難なく回収するので、木は読むとおりに書けます。

## キャンバス

キャンバスは仮想的な画素の格子で、命令ごとに塗られます。
`Canvas(width, height)` が格子を開きます。
`Scale` は仮想的な画素一つが論理画素いくつ分にあたるかで、64x40 のキャンバスは 6 なら画面では 384x240 になります。
`Palette` が色に名前を付け、`Paint` には命令を備えた `*Painter` が渡されます。

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
配色の何番目か、という意味の番号です。
ドット絵の道具はどれもこの作りなので、そうした道具のために書かれた描画のコードは、数字を書き換えずにそのまま移せます。
`*Painter` の命令は `Pixel`、`Line`、`Rect`、`RectOutline`、`Circle`、`CircleOutline`、`Triangle`、`TriangleOutline`、`Sprite`、`PixelText` です。
引数はどれも仮想的な画素の整数で、省略できる引数はありません。
`Sprite` は、画像、切り出す矩形、カラーキー（`-1` ならすべての画素を写します）、二つの反転を、この順に取ります。
`Paint` のクロージャの中の繰り返しは、ただの繰り返しです。
その本体が塗ったものは、書いた場所でそのまま絵に加わります。

命令は要素ではありません。
塗ったものは、押すことも、テーマを変えることも、大きさを与えることも、動かすこともできず、命令は書かれたキャンバスの外では何も意味しません。
一コマは、ウィンドウなしでも手に入ります。
`PIXIE_FRAMES=<dir>` を与えると、スクリプトの一手ごとにキャンバスを PNG に書き出します。
絵を描くのは、ウィンドウが使うのと同じラスタライザで、ギャラリーにある二つのゲームの録画はこうして作りました。

## キーボード

二つの違う問いに、二つの違う答え方があります。

「今何が押されているか」は、コマを描くアプリのための問いです。
`KeyDown("left")` はキーが押されているあいだ真で、`KeyPressed` は押された瞬間に一度だけ、`KeyReleased` は離された瞬間に一度だけ真になります。
名前はキー一つだけで、`cmd+s` のような組み合わせではありません。
これらを読むのはタイマーの中だけで、ビューの中では読みません。
ビューでキーボードを読むと、ウィンドウとスクリプトで違う画面を描くことになり、ゲートは違う二つのアプリを比べることになります。

「今何が押されたか」は組み合わせで、アプリには、動き出す前に宣言したハンドラとして届きます。

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

`Shortcut` は、プラットフォームのつづりで書いた組み合わせ（`cmd+s`、`cmd+shift+r`）を取ります。
`MenuItem` は同じハンドラを、宣言した順にアプリケーションのメニューバーへ置きます。
`OnKey` にはすべてのキーが押されたときの組み合わせのまま伝えられ、`OnFileDrop` にはウィンドウに落とされたファイルのパスが伝えられます。
スクリプトは `key:cmd+s` でそれを押し、`menu:Clear` でそれを選び、`drop:<path>` でファイルを落とします。

## すべての要素が取るキーワード

どの要素も、次の 15 の性質を同じ意味で取り、どれもメソッドとして持っています。

| メソッド | 型 | 働き |
|---|---|---|
| `Width(v)`, `Height(v)` | float64 | 固定の大きさ。0 でも書かれる |
| `MinWidth(v)`, `MaxWidth(v)` | float64 | 大きさの下限と上限 |
| `Disabled(v)` | bool | 反応しなくなり、そのように描かれる |
| `Theme(v)` | string | その下の要素が色を解決する配色。`"dark"` か `"light"` |
| `Animate(ms)`, `Easing(v)` | float64, string | 変化にかける時間と、その曲がり方（`"out"`、`"inOut"`） |
| `Enter(v)`, `Exit(v)` | bool | 現れるとき、消えるときの動き |
| `ColSpan(v)`, `RowSpan(v)` | int | グリッドの列や行をいくつ分またぐか |
| `Role(v)`, `A11yLabel(v)` | string | 読み上げに伝えるもの |
| `Tooltip(v)` | string | ポインタを載せたときに出るもの |

同じ名前をもともと自分の意味で持っている要素では、要素側の意味がそのまま通ります。
`Button` の `Width` はボタン自身のもので、ボタンの望む大きさになりますが、`Column` の `Width` はそれを囲む箱の大きさです。
自分のラベルがそのまま読み上げの読む名前になる要素（`Checkbox`、`Switch`、`Progress`）は、`A11yLabel` を断ります。
与える二つ目の名前がないからです。

## テーマとアニメーション

色は、エンジンの配色の名前（`"accent"`、`"panel"`、`"text"`、`"textDim"`）か、16 進の値です。
名前はいちばん近い上の `Theme` の配色で解決されるので、メソッド一つでパネル全体が切り替わります。

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

一か所にまとめた見た目とは、渡された要素すべてに同じ性質を付ける関数のことで、上の `key` がそれです。
要素の型がそのまま返るので、連鎖はそのあとも続きます。
要素に `Animate(120).Easing("out")` と書くと、その要素への変化は 120 ミリ秒かけてなめらかに動きます。
`Enter` と `Exit` は、現れるときと消えるときに同じことをします。

## ウィンドウまわり

`Run` はアプリとそのオプションを取ります。
`Title`、`Size(width, height)`、それにウィンドウの縁と木のあいだの余白である `Padding` です。
`Quit()` は、エンジンの次のフレームでウィンドウを閉じます。
ウィンドウなしの実行はこれを受け取らないので、スクリプトは最後まで走ります。

並べたり覆ったりする要素は、次のとおりです。
`Grid` は子を `Columns` の列に並べ、`GridCell(child).ColSpan(2)` はその列をまたぎます。
`Stack` は子を重ねます。
`ScrollView` と `HScrollView` はスクロールする領域です。
`Modal` は、`Open` のあいだウィンドウの残りの上に出るパネルです。
`Spacer` は親に余った場所を取り、`Divider` は罫線を引き、`Link` はページを開き、`Spinner` と `Progress` は処理中であることを示し、`Image` と `Svg` はファイルを表示します。

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
コマンドに組み込まれたものを呼ぶので、`fmt.Sprintf("%.2f", x)` が返すのは両方の実行で同じバイト列で、ゲートがライブラリの二つの実装を比べることはありません。

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

インタプリタが知っているライブラリは Go 1.22 のもので、その版のパッケージと関数です。
言語についてはそれより手前で、`min` と `max`（1.21）、数の上を回る `range`（1.22）、関数の上を回る `range`（1.23）はなく、1.23 以降にライブラリへ足されたものもありません。
最初の三つは、`check` が名前を挙げてそう言います。
ファイルは `os`、ネットワークは `net/http`、大きな数は `math/big` です。
デモはファイルを読み、自分自身にページを配ってそれを取得し、CSV を読みますが、標準ライブラリのほかには何も使いません。

## フレームワークの標準ライブラリ

エンジンが仲立ちするものは、パッケージから来ます。
そこでは一つの実装が、同じ C API を通して両方の実行に答えます。
スクリプトで走らせるときは、どちらの実行でもダイアログには `file:<path>` が答え、音は鳴りません。

| 関数 | 働き |
|---|---|
| `SqliteExec(db, sql, params...)` | 文を実行し、変わった行数を答える |
| `SqliteQueryText`, `SqliteQueryInt`, `SqliteQueryRows` | 問い合わせの最初の列、最初のセル、行全体 |
| `SqliteQueryTextOr`, `SqliteQueryIntOr`, `SqliteQueryRowsOr` | 同じものだが、問い合わせが失敗しても止まらず、何も答えない |
| `ClipboardSetText(s)`, `ClipboardGetText()` | システムのクリップボード |
| `OpenDialog(title)`, `SaveDialog(name)` | プラットフォーム自身のパネル。人を待つので、`Task` の中で呼ぶ |
| `AudioPlay(path, volume)`, `AudioStop()` | WAV ファイルを鳴らして、あとは放っておく |
| `NotifySend(title, body)` | 通知 |

文には `?` を書き、値はそのあとに渡します。
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

表ができる前に走るかもしれない問い合わせ（帳簿の最初の読み込みがそうです）は、何も答えない `…Or` の形で書きます。
素の形はアプリを止めます。
ライブラリ自身がそうするのと同じです。

## タイマーと、ウィンドウの外でする処理

`Every(seconds, tick)` は、決まった間隔で `tick` を呼んでもらう宣言で、`Run` より前に書きます。
二つの実行は一つの時計で刻みます。
その時計を進めるのは、ウィンドウでは 1 フレーム、スクリプトでは `advance:<ms>` なので、同じ数のティックが両方に届きます。
ゲームが動くのも、キーボードを読むのも、ティックの中です。

`Task(work, done)` は `work` を自分の goroutine で走らせ、終わったら、その答えを渡して `done` をウィンドウのスレッドで呼びます。
`work` の中からアプリのフィールドや画面に触ることはありません。
決まりはそれだけで、答えがどこかに書き込まれるのではなく引数として返ってくるのも、そのためです。

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

スクリプトは次の手順に進む前に処理を終わらせるので、`click:start` のあとの `dump` は、両方の実行で答えを表示します。
処理は、自分の goroutine からでも、アプリが自分で始めたどの goroutine からでも、フレームワークのライブラリ（ダイアログや問い合わせ）を呼べます。
呼び出しはそれぞれ、自分が続くあいだ一つのスレッドにとどまり、アプリの側で知っておくことは何もありません。

## 書いているあいだ

`gomamochi run` はアプリのファイルを監視しています。
保存すると、その変更がウィンドウに反映されます。
ファイルは新しいインタプリタで読み直され、ウィンドウが持っているアプリは、自分の値を新しいアプリに渡します。
名前と型が同じフィールドは持っていた値を保ち、新しいファイルが足したフィールドはゼロ値から始まり、型が変わったフィールドは最初からやり直します。
ファイルが宣言するタイマーとショートカットは、新しいアプリに結び直されます。
コンパイルできないファイルを保存しても、ウィンドウは直前の表示のままで、端末にそのことが出ます。
次にコンパイルできるファイルを保存すれば、それが効きます。

保存はファイル全体の読み直しで、`main` も走り直します。
ただし `main` がアプリに与える初期値は、ウィンドウが持っている値を置き換えません。
値は引き継がれるからです。
それがこの仕組みの狙いです。

## ウィンドウなしの実行とゲート

`PIXIE_SCRIPT` が人の操作の代わりになります。
エンジンが木を組み、そこに書かれた手順のとおりに動かして、dump を出力します。

```
click:<label>      表示されている文字でボタンを押す
input:<text>       欄に打つ            submit    その欄で改行する
slide / select     つまみを動かす、選択肢を選ぶ
key:<chord>        Shortcut に結ばれた打鍵
keydown:<key> / keyup:<key>    キーを押したままにする、離す
menu:<item>        メニュー項目を選ぶ  file:<path>   ダイアログの答え
drop:<path>        ウィンドウにファイルを落とす
advance:<ms>       時計を進める        theme:dark|light
dump               ツリーを出力            a11y   読み上げが読むものを出力
```

`gomamochi gate` は、一つのスクリプトでアプリを二回走らせます。
片方はインタプリタが動かすファイルで、もう片方は Go コンパイラがそこから作ったバイナリです。
最後に、二つの記録を一バイトずつ比べます。

```console
$ ./bin/gomamochi gate demo/counter.go --script "click:+1,dump,input:Momo"
GATE OK — 3 dump lines identical in both runs
```

ゲートが、Gomamochi の約束そのものです。
ここでゲートが言っているのは、書いているあいだに見ていたインタプリタが、配るものを作るコンパイラと一致する、ということです。
このページのほかの部分では、そのゲートを通るものをどう書くかを説明しています。
`--fresh <path>` は毎回の実行の前にそのパスを消すので、ファイルやデータベースを持つアプリも、両方の実行を同じ何もない状態から始められます。

## Gomamochi が断る書き方

`gomamochi check` はアプリを読み、受け取れない書き方があれば、その行番号と、位置を示す `^` と、書き換え方を示します。
実行の前にも、ビルドの前にも、ゲートの前にも毎回走り、言うことがなければ何も出力しません。

```console
$ ./bin/gomamochi check demo/broken.go
demo/broken.go:8:2: Gomamochi cannot take this — a view only reads. Move the write into a handler — the closure on a button, or a method the app calls from one
    	a.n += 1
    	^
```

層は二つあります。
一つ目はファイルだけを読み、どこでも走ります。
二つ目は Go 自身の型検査器で、ツールチェインのエクスポートデータを与えられて、Go の型エラーを Go の言葉で言い、型があって初めて決められることを加えます。
これが走るのは `go` がパスにあるときで、`build` と `gate` はどのみち `go` を必要とします。

解釈実行が、コンパイルした実行と同じようには走らせられないものは次のとおりです。

- `min` と `max`（Go 1.21）、それに数の上を回る `range`（1.22）と関数の上を回る `range`（1.23）。
  インタプリタにはありません。
  比較をそのまま書くか、`for i := 0; i < n; i++` と書きます。
- 呼び出しの結果をそのまま `Run` に渡すこと。
  先に名前を与えます。
- `%T` と `reflect`。
  インタプリタはアプリ自身の型を別の名前で呼びます。
- `unsafe`、cgo、`//go:embed`、標準ライブラリの外のモジュール。
- クロージャに捕まえられている自分の変数に書く、3 節の `for`。

ビューにできないことは次のとおりです。
何かが変わるたびに同じ状態から組み直されるからです。

- アプリのフィールドに書くこと、goroutine を始めること。
- 時計、環境、ファイル、ストリーム、ネットワーク、乱数、キーボードを読むこと。
  処理やタイマーを始めること。
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

`build` はファイルを Go コンパイラで、cgo を切ってコンパイルし、システム自身のライブラリ以外は何もリンクしないバイナリにします。
エンジンはその隣に置かれる共有ライブラリで、それを開く門はまずそこを探します。
`--release` は symbol table を落とします。
`--app` はその二つを macOS のアプリケーションバンドルに包み、ad-hoc 署名をつけます。
アプリと同じ場所に `<stem>.png` か `<stem>.icns` を置いておくと、それがアイコンになります。
このバンドルだけでプログラムが完結し、Go もツールチェインも入っていないマシンで、そのまま開きます。
Linux では `--app` は代わりに AppDir を書き、`--appimage` はそれを一つのファイルにまとめ、`--carry-libs` はデスクトップのライブラリも一緒に運びます。
それらのライブラリが入っていないかもしれないマシンのためです。

## まだできないこと

- インタプリタのライブラリは Go 1.22 のもので、言語はそれより手前です。
  `min` と `max`（1.21）、数の上を回る `range`（1.22）、関数の上を回る `range`（1.23）、1.23 以降に標準ライブラリへ足されたものはありません。
  最初の三つは `check` が名前を挙げ、残りはインタプリタ自身のエラーが名前を挙げます。
- 標準ライブラリの外のモジュールは、解釈実行がまだ読めません。
  アプリは 1 ファイルです。
- ドットインポートのもとでは、アプリはパッケージが公開している名前（`App`、`Element`、`Text` など）を宣言できません。
  名前を付けてパッケージをインポートすれば宣言できます。
- recover した panic の文面は二つの実行で違い、`%T` は違う名前を出力します。
  どちらも人に見せるものではありません。
- 解釈実行は、密な繰り返しではコンパイルした実行より二桁遅く、それがインタプリタの代価です。
  それでも、どちらのゲームも一コマは 1 ミリ秒かかりません。
- macOS と Linux です。
  バンドルと AppDir はエンジンを含むので、小さなアプリでも 22 MB ほどになります。
  Linux 向けのパッケージ作りは Yokan のものと同じ書き方で、まだ Linux のマシンでは動かしていません。
