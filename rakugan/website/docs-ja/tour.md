<!-- Written by website/tools/tour_pages.pl from the tour. Edit the tour. -->
# はじめてのアプリ

アプリはクラスで、状態はそのフィールドです。
型を書く場所は二か所だけです。

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

