<!-- Written by website/tools/tour_pages.pl from the tour. Edit the tour. -->
# ビューと制御構造

画面をどう組み立て、どう分け、アプリの持つもので動かすか。

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
要素はリストに集め、そのリストをコンテナに渡します。

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


## 入力の要素

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
ボタンなら何もなく、値の変わる要素なら一つです。

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

チャートにポインタを載せると、その下の値が読めます。
棒や点のラベルと、系列ごとの数値です。
スクリプトでは `hover:<i>` でポインタを載せ、`hover:` で外します。
ダンプにその読み取りが載るので、ホバーで見えるものもクリックと同じように確かめられます。


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


## なにもないかもしれない値

Perl には `undef` があり、アプリにも使いどころがあります。
まだ誰も選んでいない選択、いない親などです。
なにもない状態で始まるフィールドは、何を持ちうるかを `field $sel = maybe(Int);` と言います。
型はそこから来ます（`empty` と同じです）。
なにもない状態に戻すときにハンドラが書くのが `undef` で、なにも答えないことのあるメソッドは、Types::Standard の綴りで `:Sig(Int => Maybe[Int])` と言います。

二つの実行が一致していなければならないのは読むところなので、読みは問いの中に置きます。
`if (defined $sel) { … }` の分岐の中では `$sel` はその値で、分岐の外では値として読みません。
もう一つの書き方が `//` で、一つの式で済みます。
`$sel // 0` は、値があればその値、なければ後ろのものです。
`$sel` を裸で文字列や算術や引数に置くことは断ります。
なにもないものには文面も和もないからです。

<!-- script: dump,click:pick,dump,click:clear,dump -->
```perl
use Rakugan;

class Choice {
    use Rakugan;
    field $sel  = maybe(Int);
    field $note = "-";

    method pick :Sig(Int => Maybe[Int]) ($v) {
        return undef if $v < 0;
        return $v;
    }

    method choose :Sig(Int) ($v) {
        $sel = $self->pick($v);
        if (defined $sel) {
            $note = "chose $sel";
        } else {
            $note = "nothing to choose";
        }
    }

    method view {
        my @cells = (text("note: $note"), text("or zero: @{[ $sel // 0 ]}"));
        if (defined $sel) {
            push @cells, text("selection: $sel");
        } else {
            push @cells, text("(no selection)");
        }
        return column(
            @cells,
            row(
                button("pick",  on_click => sub { $self->choose(7) }),
                button("clear", on_click => sub { $self->choose(-1) }),
                spacing => 6,
            ),
            spacing => 8,
            padding => 12,
        );
    }
}

run(Choice->new, title => "choice");
```

`unless (defined $sel)` と `if (!defined $sel)` は、同じ分岐を逆から書いたものです。
そこでは `defined` が条件の全部で、`&&` の片側にはなりません。
その分岐の中では `$sel` を読むだけで、書きません。


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


## メソッドを持つクラス

メソッドを持つ二つ目のクラスは、値ではなくオブジェクトです。
二つの名前が同じ一つを持て、どちらを通した変更ももう一方から見えます。
perl 自身のオブジェクトがそう振る舞い、コンパイルした実行もそう持ちます。
`:param` は `new` に渡せるものを、`:reader` は外から読めるものを言い、メソッドはフィールドを名前で触ります。
perl 5.42 で加わった `:writer` はフィールドに `set_name` を与え、方言はそれも受け取ります。

<!-- script: click:bump,click:bump,dump -->
```perl
use Rakugan;

class Tally {
    use Rakugan;
    field $count :reader = 0;
    field $label :param :reader = "clicks";

    method bump :Sig(Int) ($by) {
        $count += $by;
    }

    method rename :Sig(Str) ($to) {
        $label = $to;
    }
}

class Board {
    use Rakugan;
    field $tally = Tally->new;
    field $note  = "-";

    method bump {
        $tally->bump(2);
        $tally->rename("clicks so far") if $tally->count > 2;
        $note = "@{[ $tally->label ]}: @{[ $tally->count ]}";
    }

    method view {
        return column(
            text("note: $note"),
            text("count: @{[ $tally->count ]}"),
            button("bump", on_click => sub { $self->bump }),
            spacing => 8,
            padding => 12,
        );
    }
}

run(Board->new, title => "tally");
```

オブジェクトは、なにもないかもしれないフィールド `field $kid = maybe(Node);` を通して互いを指します。
メソッドから代入し、`defined` の中で読みます。
戻りの参照は perl の作法どおり弱くします。
代入したメソッドの中で、そのあとに `weaken($parent)` と書きます。
そうすれば親と子が互いを生かし続けることはなく、コンパイルした実行もそのフィールドを弱い参照として宣言し、perl と同じ文で鎖を解放します。
`demo/links.pl` がその形で、`demo/moods.pl` はアプリが持って問いかけるクラスです。

コンパイルした実行がこのクラスに自前で与えるものがいくつかあり、方言はそれらを断ります。
フィールドと同じ名前のメソッドと `set_<field>` という名前のメソッド（そこではフィールド自身の読み手と書き手の名前です）、そのメソッドの中の `$self`、フィールドとしてのリストやハッシュ、`ADJUST`、そしてフィールドの初期値としての値つきの `Name->new` です。
`Name->new` で始めて、`ADJUST` かハンドラで値を入れます。


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
