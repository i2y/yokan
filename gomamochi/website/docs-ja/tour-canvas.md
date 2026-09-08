<!-- Written by website/tools/tourpages from the tour. Edit the tour. -->
# キャンバスとキーボード

仮想的な画素の格子と、知らせを待つのではなくこちらから尋ねるキーボード。

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

