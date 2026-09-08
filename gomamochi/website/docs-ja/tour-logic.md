<!-- Written by website/tools/tourpages from the tour. Edit the tour. -->
# ビューと制御構造

画面をどう組み立て、どう分け、アプリの持つもので動かすか。

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
それが[タイマーと、ウィンドウの外でする処理](tour-lib.md#タイマーとウィンドウの外でする処理)の話です。


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

