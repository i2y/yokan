<!-- Written by website/tools/tour_pages.pl from the tour. Edit the tour. -->
# Perl とデータとタスク

Perl 自身のライブラリ、フレームワークのライブラリ、失敗したときの書き方、ウィンドウを固めない処理の出し方。

## Perl 自身の標準ライブラリ

名前が Perl のものであるかぎり、仕様を決めるのは perl です。
`length`、`substr`（四引数の形も）、`index`、`rindex`、`uc`、`lc`、`ucfirst`、`lcfirst`、`reverse`、`join`、`split`、`sprintf`、`chomp`、`chop`、`ord`、`chr`、`hex`、`oct`、`trim`、`tr///`、`abs`、`int`、`sqrt`、`sin`、`cos`、`atan2`、`exp`、`log`、`**`、`x`、`time`、`sort`（`sort { lc($a) cmp lc($b) }` のように両側に鍵を書く形も）、`grep`、`map`、`splice`、値としての `shift` と `pop`、スライス（`@xs[1 .. 3]`）、`scalar`、`exists`、`defined`、`keys`、`values`、`rand`、`srand`、`List::Util` の `sum`、`sum0`、`max`、`min`、`maxstr`、`minstr`、`first`、`any`、`all`、`none`、`reduce`、`uniq`、`shuffle`、`POSIX` の `floor`、`ceil`、`fmod`、`strftime` は、Rakugan のものではなく言語自身のものです。
ヒアドキュメント（`<<~EOT`）、文字列の中の `\U…\E` と `\u`、`until`、`do { … } while`、リストから作るリスト（`(@a, @b)`、`push @xs, @ys`）も同じです。

乱数も perl 自身のものです。
5.20 以降の perl はどのプラットフォームでも同じ生成器を持つので、`srand(42)` のあとは二つの実行が同じ `rand` を引き、同じ `shuffle` を配り、ゲートはそれもほかと同じように比べます。
一度も種を蒔かないアプリは断ります。
実行のたびに別の数列になるからです。

<!-- script: click:run,dump -->
```perl
use Rakugan;
use List::Util qw(sum max min shuffle);
use POSIX qw(floor);

class Stats {
    use Rakugan;
    use List::Util qw(sum max min shuffle);
    use POSIX qw(floor);
    field @scores = (3, 5, 8, 13, 21);
    field $line   = "-";

    method summarize {
        my @sorted = sort { $a <=> $b } @scores;
        my $mean = sum(@scores) / scalar @scores;
        my @big  = grep { $_ > 5 } @scores;
        my @text = map { "$_" } @big;
        srand(3);
        my @dealt = shuffle(@scores);
        my @hand  = map { "$_" } @dealt;
        $line = sprintf("mean %.1f median %d min %d max %d floor %d big %s dealt %s",
                        $mean, $sorted[int(scalar(@sorted) / 2)],
                        min(@scores), max(@scores), floor(2.7),
                        join(",", @text), join(",", @hand));
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
どれも、perl 自身が出力した表（`crates/rakugan-stdlib/tests/expected/`、1000 行あまり）に照らされます。
突き合わせるのは `cargo test -p rakugan-stdlib` です。
答えがマシンの地域設定や時計で変わる関数は、どちらの実行でも UTC を読みます。


## フレームワークの標準ライブラリ

ファイル、データベース、ネットワークなど、オペレーティングシステムが持っているものは、Perl ではなくフレームワークから来ます。
そこでは、一つの実装が両方の実行に答えます。
コンパイルした実行はそれをリンクし、解釈実行はエンジンの C API を通して同じ Rust に触ります。
どの層のものかは、名前でわかります。

```perl
    fs_write_text($path, $body);       fs_read_text($path);
    fs_read_text_or($path, "(none)");  fs_exists($path);
    fs_list_dir($dir);                 fs_make_dir($dir);
    fs_append_text($path, $more);      fs_remove($path);
    fs_app_dir("myapp");
    fs_size($path);                    fs_is_dir($dir);
    fs_modified_ms($path);             fs_read_text_from($path, $offset);

    http_get_text($url);               http_get_text_or($url, "");
    http_post_text($url, $body);       http_status($url);

    jsondoc_get_text($doc, "user.name");   jsondoc_get_int($doc, "items.0.qty");
    jsondoc_length($doc, "items");         jsondoc_has($doc, "user.email");
    jsondoc_get_texts($doc, ["user.name", "items.0.qty"], "");

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


## 失敗したとき

失敗しうる呼び出しには二つの形があり、先に手に取るのは `_or` の形です。
`fs_read_text_or($path, "")` は失敗を既定値に畳み込み、それ以上は何も求めません。
理由が要るときは捕まえます。
`try` と `catch` は perl 5.40 が書くとおりの perl 自身のもので、`$e` には perl が渡すものがそのまま入ります。
文面と、失敗した文のファイル名と行番号です。

<!-- script: click:read,dump,click:halve,dump,click:save,dump -->
```perl
use Rakugan;

class Notes {
    use Rakugan;
    field $body  = "(none)";
    field $share = 0.0;
    field $count = 0;
    field $note  = "-";

    method read {
        try {
            $body = fs_read_text("demo/.gate/absent.txt");
            $note = "read";
        } catch ($e) {
            $note = "no file: $e";
        }
    }

    method halve {
        try {
            $share = 100 / $count;
        } catch ($e) {
            $note = $e;
        }
    }

    method save {
        try {
            fs_write_text("/nonexistent/dir/notes.txt", $body);
            $note = "saved";
        } catch ($e) {
            $note = "not saved: $e";
        }
    }

    method view {
        return column(
            text("body: $body"),
            text("share: $share"),
            text("note: $note"),
            row(
                button("read",  on_click => sub { $self->read }),
                button("halve", on_click => sub { $self->halve }),
                button("save",  on_click => sub { $self->save }),
                spacing => 6,
            ),
            spacing => 8,
            padding => 12,
        );
    }
}

run(Notes->new, title => "failing");
```

捕まえなかった失敗は、そのハンドラを止めます。
手前の行は効いたまま、後ろの行は走らず、アプリは開いたままで、文面は標準エラーに出ます。
これは二つの実行で同じです。
`die "…"` はわざと失敗させる書き方で、`warn "…"` は標準エラーに書いてそのまま進みます。
どちらも perl と同じく、文面が改行で終わっていなければ ` at FILE line N.` を付け足します。
コンパイルした実行も、同じファイル名と行番号を言います。

Perl 自身の失敗も同じ扱いです。
ゼロでの割り算、ゼロでの `%`、負の数の平方根は、二つの実行で perl の文面のとおりに die し、`try` で囲めば捕まります。
フレームワークの呼び出しも、失敗しうるものはすべて捕まえられます。
書き込み、問い合わせ、JSON の経路による読み取り、時刻の整形のどれにも、`try` が受け取れる形がライブラリにあるからです。
理由が要らないところでは、`_or` の形が短い書き方のままです。
`try` がまだ届かない書き方が三つあります。
ループ（`try` をループの中に入れ、失敗しうる行を囲みます）、失敗しうるメソッド（`try` をメソッドの中に入れます）、そして `finally` です。
コンパイルした実行は失敗した文でハンドラを止めるだけで、どちらの道でも必ず動く場所を持たないからです。


## タイマーと、ウィンドウの外でする処理

繰り返したい処理は `every` に渡します。
宣言するのは `run` より前です。

```perl
my $app = Clock->new;
every(1.0, sub { $app->tick });
run($app, title => "clock");
```

二つの実行は一つの時計で刻みます。
その時計を進めるのは、ウィンドウでは 1 フレーム、スクリプトでは `advance:` の 1 手順です。

待ち続けるハンドラは、ウィンドウを固めます。
時間のかかる処理は `task` に渡し、その答えをどうするかは `on_done` に書きます。

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
`task` はタスクを始め、`on_done` はタスクが答えたあとにウィンドウのスレッドで走ります。
ハンドラはそこで終わり、ウィンドウは動き続けます。
タスクの中からアプリの状態や画面に触ることはできません。
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

