<!-- Written by website/tools/tour_pages.pl from the tour. Edit the tour. -->
# キャンバスとキーボード

仮想的な画素の格子と、知らせを待つのではなく尋ねるキーボード。

## キャンバス

`canvas` は仮想的な画素の格子で、`paint` キーワードに書いたサブルーチンの中の命令で塗ります。
この中では、色は番号です。
アプリが宣言した配色の何番目か、というだけの番号です。
ドット絵の道具はそういう作りなので、そのために書かれた描画は数字を変えずにそのまま移せます。

<!-- script: advance:50,dump -->
```perl
use Rakugan;

class Sky {
    use Rakugan;
    field @palette = ("#11111b", "#89b4fa", "#f38ba8", "#eeeeee", "#a6e3a1");
    field $frame = 0;

    method tick {
        $frame += 1;
    }

    method view {
        return canvas(64, 40, scale => 6, background => 0, palette => \@palette,
                      paint => sub {
            rect(2, 2, 12, 6, 1);
            rect_outline(16, 2, 12, 6, 2);
            circle_outline(34, 5, 4, 3);
            line(2, 11, 61, 11, 2);
            triangle(3, 37, 8, 28, 13, 37, 4);
            circle(30, 18, 3, 3);
            pixel_text(2, 14, "FRAME $frame", 3);
        });
    }
}

my $app = Sky->new;
every(0.05, sub { $app->tick });
run($app, title => "sky");
```

命令は `pixel`、`line`、`rect`、`rect_outline`、`circle`、`circle_outline`、`triangle`、`triangle_outline`、`sprite`、`pixel_text` です。
これらは要素ではありません。
押すことも、テーマを変えることも、大きさを与えることも、動かすこともできません。
キャンバスの中の繰り返しは普通の繰り返しで、その本体が塗ったものは書かれた場所でそのまま絵に加わります。

`sprite` は、キャンバスと同じ配色を持つ画像から矩形を写します。
`colkey` は、写さない番号です。

```perl
    sprite($px, $py, "demo/assets/sheet.png", 0, 0, 16, 16, colkey => 0);
```


## キーボード

ゲームは、知らせが来るのを待つのではなく、手がいま何をしているかを尋ねます。

```perl
    method tick {
        $x -= 2 if keys_down("left");
        $x += 2 if keys_down("right");
        $self->fire if keys_pressed("space");
        quit() if keys_pressed("q");
    }
```

`keys_down` は「いま押されている」、`keys_pressed` は「前の tick から今までに押された」（押しっぱなしなら一度だけ答えます）、`keys_released` はその反対の縁です。
読むのはタイマーの中で、ビューの中では読みません。
キーボードを読むビューは、ウィンドウとスクリプトとで違う画面を描くことになります。

`demo/jump.pl` と `demo/shooter.pl` は、Pyxel 自身の例（Takashi Kitao、MIT）をこの語彙に移してゲートに通したものです。

キャンバスはウィンドウなしでも見られます。
`PIXIE_FRAMES=<dir>` を与えると、スクリプトの各手順のあとで最初のキャンバスを PNG に書きます。
描くのはウィンドウと同じラスタライザです。
`PIXIE_FRAME_SCALE` は、アプリの言う倍率より大きく描きます。
dump が言うのはその画面が何であるかで、こちらが言うのはどう見えるかです。
ssh の向こうでも、CI の中でも、画面がロックされていても使えます。

```console
$ PIXIE_FRAMES=frames PIXIE_SCRIPT="advance:34,advance:34" ./demo/.gate/jump
$ ls frames
0000.png  0001.png
```

