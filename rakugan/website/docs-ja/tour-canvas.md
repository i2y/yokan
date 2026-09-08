<!-- Written by website/tools/tour_pages.pl from the tour. Edit the tour. -->
# キャンバスとキーボード

仮想的な画素の格子と、知らせを待つのではなくこちらから尋ねるキーボード。

## キャンバス

`canvas` は仮想的な画素の格子で、`paint` キーワードに書いたサブルーチンの中の命令がそこに絵を描きます。
この中では、色は番号です。
アプリが宣言した配色の何番目か、という意味の番号です。
ドット絵の道具はどれもこの作りなので、そうした道具のために書かれた描画のコードは、数字を書き換えずにそのまま移せます。

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
これらは要素ではないので、押すことも、テーマを変えることも、大きさを与えることも、動かすこともできません。
キャンバスの中の繰り返しは、ただの繰り返しです。
その本体が塗ったものは、書いた場所でそのまま絵に加わります。

`sprite` は、キャンバスと同じ配色を持つ画像から矩形を写します。
`colkey` に番号を渡すと、その色だけは写りません。

```perl
    sprite($px, $py, "demo/assets/sheet.png", 0, 0, 16, 16, colkey => 0);
```


## キーボード

ゲームでは、キーが押されたと知らせてもらうのを待たずに、今どのキーが押されているかをアプリの側から尋ねます。

```perl
    method tick {
        $x -= 2 if keys_down("left");
        $x += 2 if keys_down("right");
        $self->fire if keys_pressed("space");
        quit() if keys_pressed("q");
    }
```

`keys_down` は今押されているかどうかを、`keys_pressed` は前のティック以降に押されたかどうかを答えます（押しっぱなしでも、答えるのは一度だけです）。
`keys_released` はその逆で、離されたかどうかを答えます。
これらを読むのはタイマーの中だけで、ビューの中では読みません。
ビューでキーボードを読むと、ウィンドウとスクリプトで違う画面を描くことになります。

`demo/jump.pl` と `demo/shooter.pl` は、Pyxel 自身の例（Takashi Kitao、MIT）をこの描画命令に移植し、ゲートに通したものです。

キャンバスはウィンドウなしでも見られます。
`PIXIE_FRAMES=<dir>` を与えると、スクリプトの一手ごとに、最初のキャンバスを PNG に書き出します。
絵を描くのは、ウィンドウが使うのと同じラスタライザです。
`PIXIE_FRAME_SCALE` を与えると、アプリが指定した倍率より大きく描けます。
dump がその一コマの中身を示すのに対して、こちらはそれがどう見えるかを残します。
ssh 越しでも、CI の中でも、画面がロックされていても書き出せます。

```console
$ PIXIE_FRAMES=frames PIXIE_SCRIPT="advance:34,advance:34" ./demo/.gate/jump
$ ls frames
0000.png  0001.png
```

