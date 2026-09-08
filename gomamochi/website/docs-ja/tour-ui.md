<!-- Written by website/tools/tourpages from the tour. Edit the tour. -->
# 見た目とウィンドウ

すべての要素が取るキーワード、配色、そしてウィンドウ自身が持ってくるもの。

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

