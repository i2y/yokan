<!-- Written by website/tools/tour_pages.pl from the tour. Edit the tour. -->
# Perl とデータと仕事

Perl 自身のライブラリ、枠組みのライブラリ、ウィンドウを固めない仕事の出し方。

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

