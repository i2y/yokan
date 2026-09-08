<!-- Written by website/tools/tourpages from the tour. Edit the tour. -->
# キャンバスとキーボード

仮想的な画素の格子と、知らせを待つのではなくこちらから尋ねるキーボード。

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

