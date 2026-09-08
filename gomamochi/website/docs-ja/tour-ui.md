<!-- Written by website/tools/tourpages from the tour. Edit the tour. -->
# 見た目とウィンドウ

すべての要素が取るキーワード、配色、そしてウィンドウ自身が持ってくるもの。

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

