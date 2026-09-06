# キャンバスとキーボード

`canvas` は仮想的な画素の格子で、ブロックの中に書いた命令がそこに絵を描きます。
キャンバスの中では、色を**番号**で指定します。
その番号は、`canvas` に渡した palette の何番目かという意味です。
ドット絵の道具はどれもこの作りなので、そうした環境のために書かれた描画コードは、数字を書き換えずにそのまま移せます。

## キャンバス

<!-- script: advance:50,dump -->
```ruby
require "wakakusa"

PALETTE = ["#11111b", "#89b4fa", "#f38ba8", "#eeeeee", "#a6e3a1"].freeze

class Sky
  def initialize
    @frame = 0
  end

  def tick
    @frame += 1
  end

  def view
    canvas(64, 40, scale: 6, background: 0, palette: PALETTE) {
      rect(2, 2, 12, 6, 1)
      rect_outline(16, 2, 12, 6, 2)
      circle_outline(34, 5, 4, 3)
      line(2, 11, 61, 11, 2)
      triangle(3, 37, 8, 28, 13, 37, 4)
      circle(30, 18, 3, 3)
      pixel_text(2, 14, "FRAME #{@frame}", 3)
    }
  end
end

app = Sky.new
every(0.05) { app.tick }
run(app, title: "sky")
```

`canvas(width, height, scale:, background:, palette:)` が格子を開きます。
最初の二つの数が格子の大きさで、単位は仮想的な画素です。
`scale` は、その 1 画素を画面上で何ピクセル分にするかです。
64×40 の格子を `scale: 6` で描けば、画面上では 384×240 になります。
`background` と、命令に渡す色は、どれも `palette` の何番目かという番号です。

![キャンバスのデモ。仮想的な画素の格子を、命令をひとつずつ並べて塗る](images/demos/canvas.png)

## 命令

| 命令 | 描くもの |
|---|---|
| `pixel(x, y, color)` | 画素を一つ |
| `line(x1, y1, x2, y2, color)` | 直線 |
| `rect(x, y, w, h, color)` | 塗りつぶした矩形 |
| `rect_outline(x, y, w, h, color)` | その輪郭 |
| `circle(x, y, r, color)` | 塗りつぶした円 |
| `circle_outline(x, y, r, color)` | その輪郭 |
| `triangle(x1, y1, x2, y2, x3, y3, color)` | 塗りつぶした三角形 |
| `triangle_outline(…)` | その輪郭 |
| `sprite(x, y, source, u, v, w, h, colkey:, flip_x:, flip_y:)` | 画像から切り出した矩形 |
| `pixel_text(x, y, text, color)` | キャンバスが持つ 4×6 ドットの文字で書いた一行 |

これらは要素ではないので、押すことも、テーマを変えることも、大きさを指定することも、アニメーションをつけることもできません。
すべての要素が取るキーワードも取りません。
書いたキャンバスの外では意味を持ちません。
キャンバスの中に書いた繰り返しはただの繰り返しで、その中で描いたものは、書いた場所にそのまま加わります。

`sprite` は、キャンバスと palette を共有する画像から矩形を切り出して写します。
`colkey:` に渡した番号の色だけは写りません。
それが絵の背景にあたる色で、`-1` を渡すとすべての画素を写します。

```ruby
  sprite(@px, @py, SHEET, 0, 0, 16, 16, colkey: SKY)
```

## キーボード

ゲームでは、キーが押されたと知らせてもらうのを待たずに、今どのキーが押されているかをアプリの側から尋ねます。

```ruby
  def tick
    @x -= 2 if key_down("left")
    @x += 2 if key_down("right")
    fire if key_pressed("space")
    quit if key_pressed("q")
  end
```

`key_down` は今押されているかどうかを、`key_pressed` は前のティック以降に押されたかどうかを答えます（押しっぱなしでも、答えるのは一度だけです）。
`key_released` はその逆で、離されたかどうかを答えます。
キーの名前はそのままで、`"left"`、`"right"`、`"up"`、`"down"`、`"space"`、`"enter"` があり、文字のキーはその文字です。

これらを読むのはタイマーの中だけで、ビューの中では読みません。
ビューでキーボードを読むと、ウィンドウとスクリプトで違う絵を描くことになり、ゲートは違うプログラムを比べることになります。

`quit` はウィンドウを閉じます。
ゲームが自分で終わるときに使います。

## 移植した二つのゲーム

`demo/jump.rb` と `demo/shooter.rb` は、Pyxel 自身の例（Takashi Kitao、MIT）を若草に移植して、ゲートに通したものです。
どちらも、その例が持っている画像から切り出したスプライトで描いています。
移植は原作を一行ずつ追っていて、`pyxel.blt` は `sprite` に、`pyxel.btn` は `key_down` に、`pyxel.cls(12)` はキャンバスの背景になりました。
12 が同じ色を指したままなのは、キャンバスの中では色がどちらでも番号だからです。

<p align="center">
  <img src="images/demos/shooter.gif" width="240" align="middle">
  <img src="images/demos/jump.gif" width="320" align="middle">
</p>

効果音は、原作のチップチューンではなく WAV ファイルです。
エンジンが鳴らすのはファイルなので、`tools/gen_sounds.rb` が算術で書き出しています。
スクリプトの下では無音になるので、ゲートが突き合わせるのは無音どうしです。
原作が生成器に種を与えているところは、それぞれ算術だけの六行に置き換えてあります。
種を与えた `Random` は二つの実行で同じ生成器にならないので、そのままではコマを比べられず、ゲートに通す意味がなくなります。

## ウィンドウなしでキャンバスを見る

`WAKAKUSA_FRAMES=<dir>` を与えると、スクリプトの一手ごとに、最初のキャンバスを PNG に書き出します。
絵を描くのは、ウィンドウが使うのと同じ raster です。
`WAKAKUSA_FRAME_SCALE` を与えると、アプリを書き換えずに格子を大きく描けます。
dump がその一コマの中身を示すのに対して、こちらはそれがどう見えるかを残します。
ssh 越しでも、CI の中でも、画面がロックされていても書き出せます。

```console
$ WAKAKUSA_FRAMES=frames PIXIE_SCRIPT="advance:34,advance:34" ./demo/.gate/jump
$ ls frames
0000.png  0001.png
```

上の二つの GIF も、ゲームを遊ぶスクリプトからそうやって録りました。

## 次に読むもの

- [見た目とウィンドウ](tour-ui.md)：すべての要素が取るキーワード、テーマ、ウィンドウそのものが渡すもの。
- [Ruby、データ、ジョブ](tour-lib.md)：Ruby 自身のライブラリ、データベース、タイマー、ウィンドウの外のジョブ。
