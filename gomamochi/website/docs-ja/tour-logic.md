<!-- Written by website/tools/tourpages from the tour. Edit the tour. -->
# ビューと制御構造

画面をどう組み立て、どう分け、アプリの持つもので動かすか。

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
これについては[タイマーと、ウィンドウの外でする処理](tour-lib.md#タイマーとウィンドウの外でする処理)で説明します。


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

