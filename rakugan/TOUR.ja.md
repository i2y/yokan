# 落雁 言語ツアー

<!-- 冒頭の一段落は所有者が書きます。以下は、ほかの二つのツアーと同じ
     かたちで置いた下書きです。 -->

落雁は、Perl で書いたデスクトップアプリを 1 本のネイティブバイナリにして配ります。
`rakugan build` は、書いた Perl を [pixie](../docs/PIXIE.md) に翻訳し、Zed エディタを支える **gpui** の上に組んだ pixie のエンジンと結びます。
書いているあいだは、同じファイルを perl が動かします。
このとき perl は小さな XS の門を通して同じエンジンに触ります。
配るバイナリと、書いているあいだの実行が同じプログラムかどうかは、`rakugan gate` で確かめます。
一つのスクリプトで両方を動かし、描いた画面を 1 バイトずつ突き合わせます。
画面を組み立てるサブルーチン（`text`、`button`、`column` など 30 あまり）は落雁が用意しますが、アプリそのものは普通の Perl のクラスです。
書ける Perl の範囲は、このページが説明する方言です。
そこから外れる書き方は[落雁が断る書き方](#落雁が断る書き方)に挙げてあります。

このページには、言語そのものが読者の出会う順に並んでいます。
ここに書いたものはすべて実際に動きます。
`tools/tour_check.pl` がこのファイルから完全なアプリをすべて取り出し、デモと同じコマンドに通すからです。
要素やキーワードの名前を変えれば、読者がそれを目にする前にこのページが壊れます。
英語版は [TOUR.md](TOUR.md) です。

## 目次

- [いちばん小さいアプリ](#いちばん小さいアプリ)
- [状態の持ち方](#状態の持ち方)
- [型はどこから来るか](#型はどこから来るか)
- [ビューの書き方](#ビューの書き方)
- [ビューの中の制御構造](#ビューの中の制御構造)
- [入力の部品](#入力の部品)
- [ハンドラ](#ハンドラ)
- [一覧、グラフ、必要な行だけ作る一覧](#一覧グラフ必要な行だけ作る一覧)
- [ハッシュ](#ハッシュ)
- [値のクラス](#値のクラス)
- [正規表現](#正規表現)
- [キャンバス](#キャンバス)
- [キーボード](#キーボード)
- [すべての要素が取るキーワード](#すべての要素が取るキーワード)
- [テーマとアニメーション](#テーマとアニメーション)
- [ウィンドウまわり](#ウィンドウまわり)
- [Perl 自身の標準ライブラリ](#perl-自身の標準ライブラリ)
- [枠組みの標準ライブラリ](#枠組みの標準ライブラリ)
- [タイマーと、ウィンドウの外でする仕事](#タイマーとウィンドウの外でする仕事)
- [書いているあいだ](#書いているあいだ)
- [ウィンドウなしの実行とゲート](#ウィンドウなしの実行とゲート)
- [落雁が断る書き方](#落雁が断る書き方)
- [リリース](#リリース)
- [まだできないこと](#まだできないこと)

## いちばん小さいアプリ

アプリは `view` メソッドを持つクラスです。
`view` は要素を一つ返し、そのクラスのインスタンスを `run` に渡すとウィンドウが開きます。
`use Rakugan;` はファイルの先頭と、クラスの中の 1 行目に書きます。
Perl の import はパッケージごとに効き、`class App { ... }` はそれ自体が一つのパッケージだからです。

<!-- script: dump -->
```perl
use Rakugan;

class Hello {
    use Rakugan;

    method view {
        return text("hello", size => 28);
    }
}

run(Hello->new, title => "hello");
```

```console
$ rakugan run  demo/hello.pl      # perl でウィンドウが開く
$ rakugan gate demo/hello.pl --script "dump"
GATE OK — 1 dump line identical in both runs
```

アプリを動かす perl は 5.40 以降です。
`class` を機能として当てにできるのは、そこからだからです。
コマンド自身は、パスの先頭にある perl で動きます。

## 状態の持ち方

状態はクラスのフィールドです。
ハンドラは無名サブルーチンで、その中からはフィールドがそのまま見えます。
だから書き換え方は、普通のメソッドと変わりません。
ハンドラを抜けると、そのときのフィールドからビュー全体が組み直されます。

<!-- script: click:+1,click:+1,dump,input:Momo,dump -->
```perl
use Rakugan;

class Counter {
    use Rakugan;
    field $count = 0;
    field $name  = "";

    method view {
        return column(
            text("count: $count", size => 34),
            row(
                button("+1",    on_click => sub { $count += 1 }),
                button("+10",   on_click => sub { $count += 10 }),
                button("reset", on_click => sub { $count = 0 }),
                spacing => 8,
            ),
            text_field($name, placeholder => "your name",
                       on_change => sub ($s) { $name = $s }),
            text("hello, $name"),
            spacing => 12,
            padding => 16,
        );
    }
}

run(Counter->new, title => "counter");
```

ストアを別に用意することはありません。
監視の設定を書く必要も、`new` を自分で書く必要もありません。
状態はフィールドの初期値から始まります。

## 型はどこから来るか

落雁には型がありますが、そのほとんどはどこにも書きません。
フィールドの型は初期値から読みます。
`= 0` なら `Int`、`= 0.0` なら `Num`、`= ""` なら `Str`、`= false` なら `Bool` です。
空で始まる入れ物には読むものがないので、何を入れるかをそこで言います。

```perl
    field @names  = empty(Str);
    field %counts = empty(Int);
```

引数を取るメソッドは、その引数が何かを属性で言います。

```perl
    method add :Sig(Int) ($by) { $count += $by }
    method label :Sig(Int, Str => Str) ($n, $unit) { return "$n $unit" }
```

`:Sig(A, B => R)` は、引数の型と、そのメソッドが返すものです。
何も返さないメソッドは矢印を書きません。
引数のないメソッドには属性そのものが要りません。
型の名前は Types::Standard のつづりで、`Int`、`Num`、`Str`、`Bool`、`ArrayRef[Int]`、`HashRef[Str]`、それに自分で書いたクラスの名前です。

残りはすべて落雁が計算します。
`my` で作った名前は、そこに入れたものの型になり、式の型はその部品から決まります。
食い違いは、実行して初めて分かるのではなく、行番号とともに示されます。

## ビューの書き方

コンテナは子を引数として取り、キーワードはそのあとに続きます。
要素を返すメソッドは画面の一部にあたり、それを呼ぶことでビューを分けて書けます。

<!-- script: click:+1,dump -->
```perl
use Rakugan;

class Two {
    use Rakugan;
    field $count = 0;
    field $name  = "Ada";

    method field_line :Sig(Str, Str) ($label, $value) {
        return row(
            text($label, width => 90),
            text($value, bold => true),
            spacing => 6,
        );
    }

    method view {
        return column(
            text("count: $count", size => 34),
            $self->field_line("name", $name),
            $self->field_line("count", "$count"),
            row(
                button("+1",    on_click => sub { $count += 1 }),
                button("reset", on_click => sub { $count = 0 }),
                spacing => 8,
            ),
            spacing => 12,
            padding => 16,
        );
    }
}

run(Two->new, title => "two");
```

ビューから呼ばれるメソッドは、読むだけです。
アプリのフィールドも自分の引数も使えますが、何も書き換えられません。
二度組み立てても同じ画面になる必要があるからです。
フィールドを書き換えるメソッドはハンドラのもので、断りの文面もそう言います。

## ビューの中の制御構造

ビューの中に書くのも、普通の Perl です。
`if`、`unless`、条件演算子、`for`、`my`、メソッド呼び出しがそのまま使えます。
部品はリストに集め、そのリストをコンテナに渡します。

<!-- script: dump,click:hint,dump,click:pick 1,dump -->
```perl
use Rakugan;

class Control {
    use Rakugan;
    field @items     = ("milk", "eggs", "rice");
    field $picked    = -1;
    field $show_hint = true;

    method hint {
        return text("pick one", size => 12, color => "#8a8f98");
    }

    method line :Sig(Str, Int) ($name, $i) {
        return row(
            text($i == $picked ? "▸ $name" : "  $name"),
            button("pick $i", on_click => sub { $picked = $i }),
            spacing => 8,
        );
    }

    method view {
        my @cells = (text("control flow", size => 18, bold => true));
        if ($show_hint) {
            push @cells, $self->hint;
        } else {
            push @cells, text("hidden", size => 12);
        }
        for my $i (0 .. $#items) {
            push @cells, $self->line($items[$i], $i);
        }
        push @cells, text("picked $items[$picked]")
            unless $picked < 0;
        push @cells, button("hint", on_click => sub { $show_hint = !$show_hint });
        return column(@cells, spacing => 10, padding => 14);
    }
}

run(Control->new, title => "control");
```

この例について、二つ言うことがあります。
要素のハンドラをこの中に直接書くと `$i` を閉じ込めてしまうので、`line` はメソッドにしてあります。
必要な番号を引数として受け取るためです。
もう一つは条件のことで、条件に書けるのは `Bool` だけです。
数や文字列の真偽は方言に入っていないので、`if ($picked)` は `if ($picked >= 0)` と書きます。

## 入力の部品

```perl
    text_field($name, placeholder => "name", on_change => sub ($s) { $name = $s });
    int_field($qty, min => 0, max => 99, on_change => sub ($n) { $qty = $n });
    number_field($rate, min => 0, max => 1, step => 0.05,
                 on_change => sub ($v) { $rate = $v });
    checkbox("ready", checked => $ready, on_change => sub ($on) { $ready = $on });
    switch("dark", checked => $dark, on_change => sub ($on) { $dark = $on });
    slider(value => $vol, min => 0, max => 10, on_change => sub ($v) { $vol = $v });
    select(options => \@colors, selected => $pick, on_change => sub ($i) { $pick = $i });
    radio_group(options => \@sizes, selected => $size, on_change => sub ($i) { $size = $i });
    segmented(options => ["day", "week"], selected => $span, on_change => sub ($i) { $span = $i });
    tab_bar(labels => \@tabs, active => $tab, on_change => sub ($i) { $tab = $i });
```

どれも、表示する値を受け取ります。
変わった値は、書いたサブルーチンが受け取ります。
裏で結ばれているものは何もありません。
欄が `$name` を表示するのは `$name` と書いたからで、書き戻すのはそのサブルーチンです。
選択肢のリストは、リファレンスで渡すか（`\@colors`）、その場に書きます（`["day", "week"]`）。

## ハンドラ

ハンドラは、要素がそのために用意したキーワードに書く無名サブルーチンです。
受け取る引数は、そのイベントが運ぶものだけです。
ボタンなら何もなく、値の変わる部品なら一つです。

```perl
    button("save", on_click => sub { $self->save });
    text_field($draft, on_change => sub ($s) { $draft = $s },
               on_submit  => sub ($s) { $self->add($s) });
```

書き方は `sub { ... }` と `sub ($v) { ... }` の二つです。
ほかのものを渡したときや、呼ばれ方と引数の数が合わないときは、いくつであるべきかを示して断ります。

## 一覧、グラフ、必要な行だけ作る一覧

10 万行の一覧は、10 万個の要素ではありません。
`list_view` と `table` が受け取るのは、行数と、`$i` 行目を組み立てるサブルーチンです。
実際に組み立てられるのは、画面に見えている行だけです。

<!-- script: dump,click:add,dump -->
```perl
use Rakugan;

class Ledger {
    use Rakugan;
    field @names  = ("rent", "coffee", "books");
    field @totals = (1200, 4, 36);

    method add {
        push @names, "misc";
        push @totals, 12;
    }

    method entry :Sig(Int) ($i) {
        return row(
            text($names[$i], grow => 2),
            text("$totals[$i]", grow => 1, align => "right"),
            spacing => 8,
        );
    }

    method view {
        return column(
            text("spending", size => 18, bold => true),
            list_view(scalar @names, sub ($i) { $self->entry($i) },
                      item_height => 24, height => 120),
            bar_chart(\@totals, labels => \@names, axis => true, height => 90),
            button("add", on_click => sub { $self->add }),
            spacing => 10,
            padding => 14,
        );
    }
}

run(Ledger->new, title => "ledger");
```

`table` は同じかたちに見出しと並べ替えを加えたもので、`data_table` は行を自分で組み立てる表です。
行のサブルーチンは画面を描いている最中に呼ばれるので、状態を読むだけで、書き換えることはありません。
使える添字も自分の番号だけです。
リストの範囲に収まっていることをビューが示せない添字は断られ、計算はハンドラの側ですることになります。

## ハッシュ

アプリの持つハッシュは、必ず既定値とともに読みます。

```perl
    field %prices = (apple => 120, banana => 80);

    method pick {
        $picked = $prices{"apple"} // -1;
        $label  = exists $prices{"cherry"} ? "cherry known" : "no cherry";
        $prices{"cherry"} = 200;
    }
```

`$prices{$k}` とだけ書く読み方は断ります。
その鍵はないかもしれず、ないときに何が起きるかは二つの実行で一致していなければならないからです。
`// 0` が答えを決め、あるかどうかは `exists` が答えます。
`keys` は `sort keys %prices` と書きます。
perl が鍵を返す順序は、そのとき鍵を保持している順序のままであり、perl を起動するたびに変わります。
画面がその順序に依存するわけにはいきません。

## 値のクラス

`view` を持たない二つ目のクラスは値です。
`:param` が `new` に何を渡すかを、`:reader` が何を読み返せるかを言います。
コンパイルした実行では、二つの名前が一つの実体を共有することはなく、アプリはそれを値として持ちます。

<!-- script: dump,click:right,dump,click:measure,dump -->
```perl
use Rakugan;

class Point {
    use Rakugan;
    field $x :param :reader = 0;
    field $y :param :reader = 0;
}

class Points {
    use Rakugan;
    field $sel  = Point->new(x => 3, y => 4);
    field $dist = 0;

    method view {
        return column(
            text("p=(@{[ $sel->x ]}, @{[ $sel->y ]}) d2=$dist"),
            row(
                button("right",   on_click => sub { $sel = Point->new(x => $sel->x + 5, y => $sel->y) }),
                button("measure", on_click => sub { $dist = $sel->x * $sel->x + $sel->y * $sel->y }),
                spacing => 6,
            ),
            spacing => 8,
            padding => 12,
        );
    }
}

run(Points->new, title => "points");
```

値は書き換えるのではなく、置き換えます。
上のボタンが新しい `Point` を作っているのはそのためです。
値のリストは `field @seen = empty(Point);` で、二つのゲームが星や弾を入れているのもこれです。

## 正規表現

パターンはファイルにそのまま書き、捕まえたものはその一致を作った `if` の中で読みます。

```perl
    if ($line =~ /^(\w+)\s*=\s*(\d+)$/) {
        $key = $1;
        $val = 0 + $2;
    }
    $count = () = $text =~ /\bfoo\b/g;
    $clean = $line =~ s/\s+/ /gr;
    my @parts = split /,\s*/, $line;
```

パターン自体は翻訳のときにコンパイルされるので、二つの実行は一つのエンジンで一致を探します。
ここから二つのことが決まります。
変数から組み立てたパターンは断ります。
配ったアプリには、それをコンパイルするものが入っていないからです。
置換の `/e` も同じ理由で断ります。
`if` の外の `$1` も断ります。
どこかで最後に成功した一致が残したものになり、それで二つの実行をそろえることはできないからです。
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

`demo/jump.pl` と `demo/shooter.pl` は、Pyxel 自身の例（Takashi Kitao、MIT）をこの命令の語彙に移し、ゲートに通したものです。

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

## すべての要素が取るキーワード

どの要素も、次の 15 の性質を同じ名前と同じ意味で取ります。
`width`、`height`、`min_width`、`max_width`、`disabled`、`theme`、`animate`、`easing`、`enter`、`exit`、`col_span`、`row_span`、`role`、`a11y_label`、`tooltip` です。

```perl
    button("save", disabled => $busy, tooltip => "write the file", width => 120);
    text("total", role => "heading", a11y_label => "the running total");
```

同じ名前をもともと自分の意味で持っている要素では、要素側の意味がそのまま通ります。
`text` の `width` は文字の幅を指し、外側のコンテナが上書きすることはありません。

## テーマとアニメーション

配色も動きも、キーワードで指定します。

```perl
    column(@cells, theme => "dark");
    text("saved", animate => 150, easing => "out", enter => true);
```

`theme` は、書いた要素から下だけ配色を差し替えます。
`animate` には変化にかける時間をミリ秒で渡し、`easing` にはその曲がり方（`linear`、`in`、`out`、`inOut`）を渡します。
現れるときと消えるときの動きは、`enter` と `exit` で指定します。
色は 16 進の文字列か、配色自身の名前（`accent`、`muted`、`danger`）です。
名前で書けば、一度書いた画面がそのまま明暗どちらの配色にも従います。

## ウィンドウまわり

ウィンドウ自身から来るものが、次の四つです。
どれもアプリを作ったあと、`run` より前に宣言します。

<!-- script: click:+1,key:cmd+s,dump,menu:Clear,dump -->
```perl
use Rakugan;

class Keys {
    use Rakugan;
    field $count = 0;
    field $saved = 0;
    field $last  = "-";

    method save  { $saved = $count }
    method clear { $count = 0; $saved = 0 }

    method typed :Sig(Str) ($key) {
        $last = $key;
    }

    method view {
        return column(
            text("count: $count  saved: $saved"),
            text("last key: $last"),
            button("+1", on_click => sub { $count += 1 }),
            spacing => 8,
            padding => 12,
        );
    }
}

my $app = Keys->new;

menu_item("Count", "Clear", sub { $app->clear });
shortcut("cmd+s", sub { $app->save });
on_key(sub ($chord) { $app->typed($chord) });

run($app, title => "keys");
```

四つ目は `on_file_drop(sub ($path) { ... })` です。
これらは、アプリが動いているあいだずっと有効です。

クリップボードは、二つの呼び出しで読み書きします。

```perl
    clipboard_set_text($text);
    $text = clipboard_get_text();
```

ファイルダイアログは人の操作を待つので、ウィンドウのスレッドの外に出します。
そのためにあるのが [`task`](#タイマーとウィンドウの外でする仕事) です。

```perl
    task(sub { fs_open_dialog("Choose a file") },
         on_done => sub ($path) { $self->took($path) });
    task(sub { fs_save_dialog("notes.txt") },
         on_done => sub ($path) { $self->write_to($path) });
```

ウィンドウなしのスクリプトは、ダイアログに `file:<path>` の手順で答えます。
だからダイアログも、ほかの操作と同じように確かめられます。

音は、ファイルを鳴らして、あとは放っておく形です。

```perl
    audio_play("demo/assets/sound/blip.wav");        # 録音されたままの大きさで
    audio_play("demo/assets/sound/blast.wav", 0.4);  # 0.0 から 1.0 の大きさで
    audio_stop();
```

呼び出しはすぐ返り、鳴り終わるのを待ちません。
スクリプトで走らせるときは無音になります。
ゲートがスピーカーのあるマシンを要求するわけにはいかないからです。
音の出ないマシンや、読めないファイルでは、アプリを止めずに何も鳴りません。
エンジンが読めるのは WAV です。

## Perl 自身の標準ライブラリ

名前が Perl のものであるかぎり、仕様を決めるのは perl です。
`length`、`substr`、`index`、`rindex`、`uc`、`lc`、`ucfirst`、`lcfirst`、`reverse`、`join`、`split`、`sprintf`、`abs`、`int`、`sqrt`、`sort`、`grep`、`map`、`scalar`、`exists`、`defined`、`keys`、`values`、`List::Util` の `sum`、`max`、`min`、`first`、`uniq`、`POSIX` の `floor`、`ceil`、`fmod`、`strftime` は、落雁のものではなく言語自身のものです。

<!-- script: click:run,dump -->
```perl
use Rakugan;
use List::Util qw(sum max min);
use POSIX qw(floor);

class Stats {
    use Rakugan;
    use List::Util qw(sum max min);
    use POSIX qw(floor);
    field @scores = (3, 5, 8, 13, 21);
    field $line   = "-";

    method summarize {
        my @sorted = sort { $a <=> $b } @scores;
        my $mean = sum(@scores) / scalar @scores;
        my @big  = grep { $_ > 5 } @scores;
        my @text = map { "$_" } @big;
        $line = sprintf("mean %.1f median %d min %d max %d floor %d big %s",
                        $mean, $sorted[int(scalar(@sorted) / 2)],
                        min(@scores), max(@scores), floor(2.7),
                        join(",", @text));
    }

    method view {
        return column(
            text($line),
            button("run", on_click => sub { $self->summarize }),
            spacing => 8,
            padding => 12,
        );
    }
}

run(Stats->new, title => "stats");
```

コンパイルした実行に perl は入っていないので、これらは Rust で一度ずつ書いてリンクしてあります。
解釈実行のほうは perl のものを呼びます。
その二つが一致することは、願いではありません。
どれも、perl 自身が印字した表（`crates/rakugan-stdlib/tests/expected/`、1000 行あまり）に照らされます。
突き合わせるのは `cargo test -p rakugan-stdlib` です。
答えがマシンの地域設定や時計で変わる関数は、どちらの実行でも UTC を読みます。

## 枠組みの標準ライブラリ

ファイル、データベース、ネットワークなど、オペレーティングシステムが持っているものは、Perl ではなく枠組みから来ます。
そこでは、一つの実装が両方の実行に答えます。
コンパイルした実行はそれをリンクし、解釈実行はエンジンの C 面を通して同じ Rust に触ります。
どの層のものかは、名前でわかります。

```perl
    fs_write_text($path, $body);       fs_read_text($path);
    fs_read_text_or($path, "(none)");  fs_exists($path);
    fs_list_dir($dir);                 fs_make_dir($dir);
    fs_append_text($path, $more);      fs_remove($path);
    fs_app_dir("myapp");

    http_get_text($url);               http_get_text_or($url, "");
    http_post_text($url, $body);       http_status($url);

    jsondoc_get_text($doc, "user.name");   jsondoc_get_int($doc, "items.0.qty");
    jsondoc_length($doc, "items");         jsondoc_has($doc, "user.email");

    strings_to_int($s);                strings_to_float($s);
    clock_format_ms($ms, "%Y-%m-%d");  clock_local_offset_minutes();
    notify_send("done", "the file is written");
```

失敗しうる呼び出しには二つの形があります。
素の形は、失敗したときにそのハンドラを止めます。
`_or` の形は、代わりに答えるものを受け取ります。
どちらを書くかは、ファイルがないことを誤りと見るか、既定値と見るか、という問いです。

データベースも同じ作りです。
両方の実行が一つのファイルを同じように読めなければ、意味がないからです。

```perl
    sqlite_exec($db, "CREATE TABLE IF NOT EXISTS notes(t TEXT)");
    sqlite_exec($db, "INSERT INTO notes VALUES (?)", [$draft]);
    my @rows  = sqlite_query_text($db, "SELECT t FROM notes ORDER BY t");
    my $count = sqlite_query_int_or($db, "SELECT COUNT(*) FROM notes", 0);
```

文には `?` を書き、値は別の引数として渡します。
そうすれば、人が打った文字が文の一部になることはありません。

## タイマーと、ウィンドウの外でする仕事

繰り返したい仕事は `every` に渡します。
宣言するのは `run` より前です。

```perl
my $app = Clock->new;
every(1.0, sub { $app->tick });
run($app, title => "clock");
```

二つの実行は一つの時計で刻みます。
その時計を進めるのは、ウィンドウでは 1 フレーム、スクリプトでは `advance:` の 1 手順です。

待ち続けるハンドラは、ウィンドウを固めます。
時間のかかる仕事は `task` に渡し、その答えをどうするかは `on_done` に書きます。

<!-- script: click:start,advance:2000,dump -->
```perl
use Rakugan;

class Jobs {
    use Rakugan;
    field $status = "idle";
    field $answer = 0;

    method start {
        $status = "working";
        task(sub {
            my $total = 0;
            my $i = 0;
            while ($i < 300000) {
                $total += $i % 7;
                $i += 1;
            }
            $total;
        }, on_done => sub ($v) {
            $answer = $v;
            $status = "done";
        });
    }

    method view {
        return column(
            text("status: $status  answer: $answer"),
            button("start", on_click => sub { $self->start }),
            spacing => 10,
            padding => 14,
        );
    }
}

run(Jobs->new, title => "tasks");
```

どちらの呼び出しも待ちません。
`task` は仕事を始め、`on_done` は仕事が答えたあとにウィンドウのスレッドで走ります。
ハンドラはそこで終わり、ウィンドウは動き続けます。
仕事の中からアプリの状態や画面に触ることはできません。
それをするのは `on_done` のサブルーチンで、答えが値として返ってくるのもそのためです。

## 書いているあいだ

`rakugan run` はアプリのファイルを監視しています。
保存すると、その変更がウィンドウに反映されます。
ファイルが読み直され、クラスも定義し直されて、ウィンドウの持っているインスタンスが新しい `view` で答えます。
コンパイルできないファイルを保存しても、ウィンドウは直前の表示のままで、端末にそのことが出ます。
次にコンパイルできるファイルを保存すれば、それが効きます。

保存をまたいでもフィールドの値は残ります。
インスタンスがそのまま残るからです。
フィールドを足したり、初期値を変えたりしたときは、次に起動したときから効きます。
ウィンドウの中のインスタンスは、その変更より前に作られたものだからです。

## ウィンドウなしの実行とゲート

`PIXIE_SCRIPT` が人の操作の代わりになります。
エンジンが木を組み、そこに書かれた手順のとおりに動かして、dump を印字します。

```
click:<label>      表示されている文字でボタンを押す
input:<text>       欄に打つ            submit    その欄で改行する
slide / select     つまみを動かす、選択肢を選ぶ
key:<chord>        shortcut に結ばれた打鍵
keydown:<key> / keyup:<key>    キーを押したままにする、離す
menu:<item>        メニュー項目を選ぶ  file:<path>   ダイアログの答え
drop:<path>        ウィンドウにファイルを落とす
advance:<ms>       時計を進める        theme:dark|light
dump               木を印字            a11y   読み上げが読むものを印字
```

`rakugan gate` は、一つのスクリプトでアプリを二回走らせます。
片方は perl が XS の門越しに走らせるもので、もう片方は翻訳した `.pix` から作ったバイナリです。
最後に、二つの記録を一バイトずつ比べます。

```console
$ rakugan gate demo/counter.pl --script "click:+1,dump,input:Momo"
GATE OK — 3 dump lines identical in both runs
```

ゲートが、落雁の約束そのものです。
このページのほかの部分では、そのゲートを通るものをどう書くかを説明しています。

## 落雁が断る書き方

`rakugan check` はアプリを読み、受け取れない書き方があれば、その行番号と書き換え方を示します。
先に読むのは perl です。
構文の可否は perl が決めるので、perl が撥ねた書き方が翻訳器に届くことはありません。
そのあとで翻訳器が、方言から外れた最初のものを指します。
ビルドの前にもゲートの前にも毎回走り、言うことがなければ何も印字しません。

```console
$ rakugan check demo/broken.pl
demo/broken.pl:9:26: Rakugan cannot take this — a hash may not have that
key, so say what to answer when it does not: `$prices{$k} // 0`
        $picked = $prices{"apple"};
                  ^
```

断るものと、代わりの書き方は次のとおりです。

- 初期値のないフィールドと、アプリ自身のクラスのフィールドに付けた `:param`。
  型は初期値から来ます。
- 何を入れるか言わずに空で始まるリストやハッシュ。
  `empty(Str)` と書きます。
- 二つの型が混ざったリスト。
- 引数があるのに `:Sig` のないメソッド。
- `Bool` ではない条件。
  `!= 0` や `ne ""` のように比べます。
- 片側が文字列の `+`。
  文字列を数として読むのは `0 + $s` です。
- 文字列への `++`。
  perl はそこで文字を数え上げますが（`"az"++` は `"ba"`）、コンパイルした実行はそれをしません。
- `//` のないハッシュの読みと、`sort` のない `keys`。
- リストの中に収まっていることをビューが示せない添字。
- ビューを組み立てている最中に呼ばれる、フィールドを書き換えるメソッド。
- 一致を作った `if` の外の `$1`、実行時に組み立てたパターン、置換の `/e`。
- `print`、`printf`、`say`。
  コンパイルしたアプリが書くのは画面で、標準出力ではありません。
  標準エラーに出す `warn` は受け取ります。
- 文字列の `eval`、`goto`、`local`、`wantarray`、`each`、`tie`、`bless`、`ref`、`AUTOLOAD`。
- サブルーチンでないハンドラと、呼ばれ方に引数の数が合わないハンドラ。
- 要素の知らないキーワードと、型の合わないキーワード。
  文面は、その要素が取るものを並べます。

どれも、印字される文面ごと `test/refuse/` に置いてあります。
だから、断りの文面が黙って変わることはありません。

## リリース

```console
$ rakugan build demo/todo.pl --release --app
built: ~/.cache/pixie/target/release/main (11.9 MB)
bundle: demo/dist/todo.app (11.9 MB)
```

`--release` は symbol table を落とします。
`--app` はバイナリを macOS のアプリケーションバンドルに包み、ad-hoc 署名をつけます。
アプリと同じ場所に `<stem>.png` か `<stem>.icns` を置いておくと、それがアイコンになります。
バイナリはエンジンと翻訳したアプリを含んでいて、システム自身のライブラリ以外は何もリンクしません。
だから、このバンドルだけでプログラムが完結します。
perl 5.40 もツールチェインも入っていないマシンで、そのまま開きます。

## まだできないこと

- 方言は部分集合で、[落雁が断る書き方](#落雁が断る書き方)がその外側です。
  どれも、翻訳器がまだコンパイルした実行まで運べない書き方であって、Perl についての評価ではありません。
- リファレンスは、次のものだけです。
  要素に渡すリストとハッシュ、それに自分で書いたクラスです。
  ハンドラ以外のコードリファレンス、リファレンスへのリファレンス、`ref` はありません。
- 自分で書くモジュールはありません。
  アプリは 1 ファイルで、翻訳器が知っているモジュールの関数は `List::Util` と `POSIX` のものだけです。
  ほかのモジュールの `use` は読んで無視され、その中の関数を呼べば名前を挙げて断られます。
- `sprintf` は `%s %d %i %f %F %e %E %g %G %x %X %o %b %%` までです。
  幅と精度、`-`、`+`、空白、`0`、`#` の指定は付けられます。
- 乱数は、二つの実行で同じ生成器になりません。
  どちらの実行でも同じ数列がほしいなら、生成器を自分で書きます。
  二つのゲームがそうしていて、算術だけの数行で足ります。
- `check` が見つけるのは上に挙げたものです。
  翻訳器が取りこぼすものをすべて見ているわけではないので、残りを見つけるのは今もゲートです。
- macOS だけです。
  バイナリが描画に使うエンジンを含むので、小さなアプリでも 12 MB ほどになります。
