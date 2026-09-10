<!-- Written by website/tools/refusals_page.pl from test/refuse/. Edit the fixtures. -->
# Rakugan が断る書き方

方言は部分集合で、その境目に出会う場所が `rakugan check` です。
アプリを読み、受け取れない書き方があれば、ファイルと行と桁、その行そのもの、そして代わりの書き方を示します。
ファイルを先に読むのは perl なので、perl が撥ねた書き方が翻訳器に届くことはありません。
`check` はビルドの前にもゲートの前にも走ります。
コンパイラもウィンドウも要らず、言うことがなければ何も出力しません。

下の 31 個には、それを起こすファイルと、出力されるべき文面が、`test/refuse/` にそのまま置いてあります。
`tools/gate_all.sh` がそれを回すので、断りの文面が黙って変わることはありません。
このページも、その同じファイルから引いています。

## クラス

要素と型の名前と `empty` は import で入ってきますが、Perl の import はパッケージごとに効きます。

```console
test/refuse/class_pragma.pl:3:11: Rakugan cannot take this — `class App` needs `use Rakugan;` as its first line: a Perl import is per package, and the elements, `empty` and the type names have to be in this one
    class App {
              ^
```

フィールドの型は初期値から読むので、初期値がなければ読むものがありません。

```console
test/refuse/field_initializer.pl:5:11: Rakugan cannot take this — a field needs an initializer (`= 0`, `= ""`, `= empty(Str)`) — that is where its type comes from
        field $n;
              ^
```

アプリ自身のフィールドはアプリだけのものです。
外から渡すものも、外から読むものもありません。

```console
test/refuse/field_attribute.pl:5:14: Rakugan cannot take this — a field takes no attributes here (`:param`, `:reader`); its type is read from the initializer
        field $n :param = 0;
                 ^
```

メソッドの引数の型はどこからも読めないので、メソッドに書いて示します。

```console
test/refuse/method_without_sig.pl:7:12: Rakugan cannot take this — a method with parameters says what they are: `method add :Sig(Int) ($by) { ... }`
        method add ($by) {
               ^
```

ファイルを先に読むのは perl で、そこでは裸の `s` は置換の始まりです。

```console
test/refuse/quote_method.pl:7:12: Rakugan cannot take this — a method named `s` reads as a regular expression to the parser; pick another name
        method s { $n += 1 }
               ^
```

ファイルの最上位はアプリを作る場所であって、状態を置く二つ目の場所ではありません。

```console
test/refuse/top_statement.pl:3:1: Rakugan cannot take this — a declaration at the top of the file is a hash of keywords (`my %PILL = (...)`), a name for a literal (`my $WIDTH = 120;`) or the app itself (`my $app = Counter->new;`)
    my @greetings = ("hello", "goodbye");
    ^
```

## 型

空で始まる入れ物には、型を読むものが入っていません。

```console
test/refuse/empty_list.pl:5:20: Rakugan cannot take this — a list that starts empty says what it will hold: `field @items = empty(Str);`
        field @items = ();
                       ^
```

リストは一つの型です。
コンパイルした実行がそう持つからです。

```console
test/refuse/mixed_list.pl:5:24: Rakugan cannot take this — a list holds one type: this one started with Int and this is String
        field @items = (1, "two");
                           ^
```

キーワードの型は表が決めます。
エンジンの両側が数えているのはその表です。

```console
test/refuse/wrong_type.pl:7:30: Rakugan cannot take this — `size =>` takes a number (got String)
            return text("hello", size => "large");
                                 ^
```

文面はその要素が取るものを並べるので、正しい名前はその場で分かります。

```console
test/refuse/unknown_keyword.pl:7:30: Rakugan cannot take this — `text` has no `weight =>`; it takes `a11y_label`, `align`, `animate`, `background`, `bold`, `border_color`, `border_radius`, `border_width`, `col_span`, `color`, `disabled`, `easing`, `enter`, `exit`, `grow`, `height`, `italic`, `max_lines`, `max_width`, `min_width`, `mono`, `padding`, `role`, `row_span`, `size`, `theme`, `tooltip`, `underline`, `width`, `wrap`
            return text("hello", weight => 700);
                                 ^
```

数や文字列の真偽は方言に入っていません。
条件は `Bool` です。

```console
test/refuse/truthiness.pl:9:35: Rakugan cannot take this — a condition is a bool (got Int); Perl's truthiness of a number or a string is not in the translator — compare it (`!= 0`, `ne ""`)
            push @cells, text("some") if $n;
                                      ^
```

文字列を数として読むことは、perl なら黙ってしますが、ここでは書いて示します。

```console
test/refuse/string_and_number.pl:8:19: Rakugan cannot take this — `+` needs a number on both sides (got String and Int); `0 + $s` reads a string as a number, the way perl does
            $n = "10" + 5;
                      ^
```

perl はそこで文字を数えますが、コンパイルした実行にその数え方は入っていません。

```console
test/refuse/string_increment.pl:8:13: Rakugan cannot take this — `$tag` holds a String, and Perl's `++` on a string counts letters (`"az"++` is `"ba"`), which the compiled run does not do; join or replace the string instead
            $tag++;
                ^
```

perl はその数を小数のある数に育てますが、方言が持つのは 64 bit までです。
書き下した二つの数はビルドの前に計算し、そこで断ります。

```console
test/refuse/literal_overflow.pl:8:34: Rakugan cannot take this — this comes to 18446744073709551616, and a whole number here holds 64 bits — perl would grow it into a number with a fraction, which the compiled run cannot follow; write it with a `.0` to mean that number
            $n = 4611686018427387904 * 4;
                                     ^
```

## ビューとハンドラ

同じ画面を二度組み立てたら同じ画面になる必要があるので、組み立てるときは読むだけです。

```console
test/refuse/view_calls_method.pl:13:21: Rakugan cannot take this — `bumped` touches the app's state, and building a view only reads; give it what it needs as parameters, or read a field
            return text("n=@{[ $self->bumped ]}");
                        ^
```

行は自分の番号を読みます。
それ以外は、確かめられる場所で計算します。

```console
test/refuse/negative_index.pl:9:21: Rakugan cannot take this — an index a view cannot prove is not negative; a row reads its own index, and anything else is worked out in a handler
            return text("at: $items[$at]");
                        ^
```

ハンドラは、その出来事が運ぶものだけを受け取って呼ばれます。

```console
test/refuse/handler_arity.pl:8:45: Rakugan cannot take this — this handler is called with nothing; drop the parameter
            return button("go", on_click => sub ($x) { $n += 1 });
                                                ^
```

鍵がないときに何が起きるかで両方の実行が一致する必要があるので、答えをアプリが決めます。

```console
test/refuse/bare_hash_read.pl:9:26: Rakugan cannot take this — a hash may not have that key, so say what to answer when it does not: `$prices{$k} // 0`
            $picked = $prices{"apple"};
                             ^
```

ハッシュの順序は perl を起動するたびに変わります。
画面がそれに依存するわけにはいきません。

```console
test/refuse/unsorted_keys.pl:10:20: Rakugan cannot take this — a hash hands `keys` back in the order perl happens to hold it, which is a different order every time perl starts; write `sort keys %h`
            for my $k (keys %prices) {
                       ^
```

## コンパイルした実行に perl がないこと

コンパイルしたアプリが書くのは画面で、ゲートが木を読むのもそこからです。

```console
test/refuse/say_to_stdout.pl:8:9: Rakugan cannot take this — a compiled app writes its screen, not its standard output; `warn` goes to standard error
            say "n is $n";
            ^
```

配ったアプリはコンパイラを積んでいません。

```console
test/refuse/string_eval.pl:8:14: Rakugan cannot take this — a string `eval` compiles Perl while the app runs, and a shipped app carries no compiler; catch a failure with `try` / `catch`
            $n = eval "1 + 1";
                 ^
```

パターンは翻訳のときにコンパイルするので、そのときにはもうファイルに書かれていなければなりません。

```console
test/refuse/pattern_built.pl:9:27: Rakugan cannot take this — a pattern here is written out; one built while the app runs would have to be compiled by something the shipped app does not carry
            $found = "abc" =~ /$needle/;
                              ^
```

コードを走らせるパターンには perl が要りますが、コンパイルした実行に perl はありません。

```console
test/refuse/pattern_code.pl:8:27: Rakugan cannot take this — a pattern that runs code (`(?{ … })`) is perl's own; the compiled run has no perl in it
            $found = "abc" =~ /a(?{ print "hi" })b/;
                              ^
```

同じ理由です。
置換の中身は、アプリが動いている最中に走る perl になります。

```console
test/refuse/substitute_eval.pl:8:18: Rakugan cannot take this — a replacement here is text, with `$1` … `$9` for what the pattern caught; `/e` runs perl and the compiled run has none
            $line =~ s/(\d+)/$1 + 1/e;
                     ^
```

その `if` の外では、どこかで最後に成功した一致が残したものになります。

```console
test/refuse/capture_unguarded.pl:10:16: Rakugan cannot take this — what a pattern caught is read where the match is known to have happened: inside the `if` that made it
            $got = $1;
                   ^
```

コンパイルした実行は失敗した文でハンドラを止めるので、どちらの道でも必ず動く場所がありません。

```console
test/refuse/try_finally.pl:9:50: Rakugan cannot take this — `finally` runs after either path, and the compiled run has no unwinding to hang it on; write the line after the `try`
            try { $n = 1 } catch ($e) { $note = $e } finally { $n = 2 }
                                                     ^
```

ループの中では失敗ごとに catch が走り、ループは続くことになります。
ループの中に `try` を置けば、そのあと何をするかを言えます。

```console
test/refuse/try_loop.pl:12:44: Rakugan cannot take this — a `try` does not reach into a loop yet; put the `try` inside the loop, around the line that can fail
                for my $x (@xs) { $total += $x / $n }
                                               ^
```

メソッドは一度だけコンパイルされ、外の `try` はその中まで届きません。
中に置けば、失敗しうる行はすぐそこにあります。

```console
test/refuse/try_method.pl:12:22: Rakugan cannot take this — `halve` can fail — it divides, takes a root, writes `die` or calls the library — and a `try` here does not reach into it yet; put the `try` inside `halve`, around the line that can fail
            try { $self->halve } catch ($e) { $note = $e }
                         ^
```

コンパイルした実行が catch に渡せない失敗は、片方の実行だけで捕まることになります。

```console
test/refuse/try_plain_library.pl:8:15: Rakugan cannot take this — `fs_write_text` can fail, and the library has no form of it a `try` can take yet; call it before the `try`, or write its `_or` twin
            try { fs_write_text("/nonexistent/dir/x.txt", "a") } catch ($e) { $note = $e }
                  ^
```

perl はその行を走らせないので、コンパイルした実行も走らせてはなりません。
動かないコードを読ませないために断ります。

```console
test/refuse/after_die.pl:9:9: Rakugan cannot take this — nothing after `die` runs; drop these lines, or put the `die` under an `if`
            $n = 1;
            ^
```

perl はその行をすぐに走らせ、コンパイルした実行は処理が終わってから走らせます。
`task` の前に置けば、どちらもすぐに走らせます。

```console
test/refuse/after_task.pl:10:9: Rakugan cannot take this — `task` is the last thing a handler does: the compiled run reaches these lines when the work is done, and perl reaches them at once; write them before the `task`
            $status = "working";
            ^
```
