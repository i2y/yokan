#!/usr/bin/env perl
# The site's Refusals page, in both languages, written from the fixtures.
#
# Every refusal under `test/refuse/` is a pair: an app outside the
# dialect, and the message `rakugan check` must print for it, word for
# word. That message is the page, quoted from the file the sweep holds
# the translator to, so the page cannot describe a refusal in wording
# the command no longer uses. What is written here is the order, the
# grouping, and one line of why.
#
#   tools/refusals_page.pl            write docs/refusals.md and docs-ja/
#   tools/refusals_page.pl --check    fail if either is behind the fixtures
use strict;
use warnings;
use utf8;
use open qw(:std :encoding(UTF-8));
use File::Basename qw(basename dirname);
use File::Spec;

my $SITE = File::Spec->rel2abs(File::Spec->catdir(dirname(__FILE__), File::Spec->updir));
my $ROOT = File::Spec->rel2abs(File::Spec->catdir($SITE, File::Spec->updir));
my $FIX  = File::Spec->catdir($ROOT, 'test', 'refuse');

my @GROUPS = qw(class types views perl);

# group, fixture, the English line of why, the Japanese one.
my @CATALOGUE = (
['class', 'class_pragma',
 'The elements, the type names and `empty` are brought in by an import, and a Perl import is per package.',
 '要素と型の名前と `empty` は import で入ってきますが、Perl の import はパッケージごとに効きます。'],
['class', 'field_initializer',
 "A field's type is read from what it starts as, so a field with no initializer has no type to read.",
 'フィールドの型は初期値から読むので、初期値がなければ読むものがありません。'],
['class', 'field_attribute',
 "The app's own fields are the app's alone: nothing hands them in and nothing reads them from outside.",
 "アプリ自身のフィールドはアプリだけのものです。\n外から渡すものも、外から読むものもありません。"],
['class', 'method_without_sig',
 'The types of a method\'s parameters cannot be read off anything, so they are written down.',
 'メソッドの引数の型はどこからも読めないので、メソッドに書いて示します。'],
['class', 'quote_method',
 'perl reads the file first, and there a bare `s` begins a substitution.',
 'ファイルを先に読むのは perl で、そこでは裸の `s` は置換の始まりです。'],
['class', 'top_statement',
 'The top of the file is where the app is made, not a second place to keep state.',
 'ファイルの最上位はアプリを作る場所であって、状態を置く二つ目の場所ではありません。'],

['types', 'empty_list',
 'A container that starts empty has nothing in it to read a type from.',
 '空で始まる入れ物には、型を読むものが入っていません。'],
['types', 'mixed_list',
 'A list is one type, because the compiled run holds it as one.',
 "リストは一つの型です。\nコンパイルした実行がそう持つからです。"],
['types', 'wrong_type',
 'A keyword takes the type the table gives it, and the table is what both sides of the engine count with.',
 "キーワードの型は表が決めます。\nエンジンの両側が数えているのはその表です。"],
['types', 'unknown_keyword',
 'The message lists what that element does take, so the name is one lookup away.',
 '文面はその要素が取るものを並べるので、正しい名前はその場で分かります。'],
['types', 'truthiness',
 "Perl's truthiness of a number or a string is not in the dialect: a condition is a `Bool`.",
 "数や文字列の真偽は方言に入っていません。\n条件は `Bool` です。"],
['types', 'string_and_number',
 'Reading a string as a number is written out, the way perl would do it silently.',
 '文字列を数として読むことは、perl なら黙ってしますが、ここでは書いて示します。'],
['types', 'string_increment',
 'Perl counts letters there, and the compiled run has no such counting in it.',
 'perl はそこで文字を数えますが、コンパイルした実行にその数え方は入っていません。'],

['views', 'view_calls_method',
 'Building a screen twice has to build the same screen, so building it only reads.',
 '同じ画面を二度組み立てたら同じ画面になる必要があるので、組み立てるときは読むだけです。'],
['views', 'negative_index',
 'A row reads its own number; anything else is worked out where it can be checked.',
 "行は自分の番号を読みます。\nそれ以外は、確かめられる場所で計算します。"],
['views', 'handler_arity',
 'A handler is called with what the event carries, and nothing else.',
 'ハンドラは、その出来事が運ぶものだけを受け取って呼ばれます。'],
['views', 'bare_hash_read',
 'The two runs would have to agree about a key that is not there, so the app says what to answer.',
 '鍵がないときに何が起きるかで両方の実行が一致する必要があるので、答えをアプリが決めます。'],
['views', 'unsorted_keys',
 "perl's order for a hash changes every time perl starts, and a screen cannot depend on that.",
 "ハッシュの順序は perl を起動するたびに変わります。\n画面がそれに依存するわけにはいきません。"],

['perl', 'say_to_stdout',
 "A compiled app writes its screen, which is where the gate reads the tree from.",
 'コンパイルしたアプリが書くのは画面で、ゲートが木を読むのもそこからです。'],
['perl', 'string_eval',
 'A shipped app carries no compiler.',
 '配ったアプリはコンパイラを積んでいません。'],
['perl', 'pattern_built',
 'The pattern is compiled when the app is translated, so it has to be there to compile.',
 'パターンは翻訳のときにコンパイルするので、そのときにはもうファイルに書かれていなければなりません。'],
['perl', 'pattern_code',
 'A pattern that runs code needs perl, and the compiled run has none.',
 'コードを走らせるパターンには perl が要りますが、コンパイルした実行に perl はありません。'],
['perl', 'substitute_eval',
 'The same reason: the replacement would be perl, run while the app runs.',
 "同じ理由です。\n置換の中身は、アプリが動いている最中に走る perl になります。"],
['perl', 'capture_unguarded',
 'Outside that `if` it would be whatever the last successful match anywhere had left.',
 'その `if` の外では、どこかで最後に成功した一致が残したものになります。'],
['perl', 'try_finally',
 'The compiled run stops the handler at the failure, so nothing of it runs after that on either path.',
 'コンパイルした実行は失敗した文でハンドラを止めるので、どちらの道でも必ず動く場所がありません。'],
['perl', 'try_loop',
 'Inside a loop the catch would run once per failure and the loop would go on; a `try` inside the loop says what happens then.',
 "ループの中では失敗ごとに catch が走り、ループは続くことになります。\nループの中に `try` を置けば、そのあと何をするかを言えます。"],
['perl', 'try_method',
 'A method is compiled once, and a `try` outside it cannot reach in; inside it, the line that can fail is right there.',
 "メソッドは一度だけコンパイルされ、外の `try` はその中まで届きません。\n中に置けば、失敗しうる行はすぐそこにあります。"],
['perl', 'after_die',
 'perl never runs those lines, so the compiled run must not either; the refusal spares reading code that does nothing.',
 "perl はその行を走らせないので、コンパイルした実行も走らせてはなりません。\n動かないコードを読ませないために断ります。"],
['perl', 'after_task',
 'perl runs those lines at once and the compiled run when the work is done; before the `task`, both run them at once.',
 "perl はその行をすぐに走らせ、コンパイルした実行は処理が終わってから走らせます。\n`task` の前に置けば、どちらもすぐに走らせます。"],
['types', 'undef_alone',
 'A field\'s type is read from its initializer, and `undef` alone names none; `maybe(Int)` says what it may hold.',
 'フィールドの型は初期値から読みますが、`undef` だけでは型が決まりません。`maybe(Int)` が何を持ちうるかを言います。'],
['types', 'maybe_in_text',
 'Nothing has no text; the app says what to print then, or reads the value inside `defined`.',
 "なにもないものには文面がありません。\nそのとき何を出すかをアプリが言うか、`defined` の中で値を読みます。"],
['types', 'maybe_member',
 'A member of nothing is where perl dies at run time; inside `if (defined $x)` the object is there in both runs.',
 "なにもないもののメンバーは、perl が実行時に die する場所です。\n`if (defined \$x)` の中なら、どちらの実行でもオブジェクトがそこにあります。"],
['types', 'defined_in_while',
 'The branch of that `if` is where the value is read as a value; a `while` or an `&&` has no such branch.',
 "その `if` の分岐が、値を値として読む場所です。\n`while` や `&&` にはそのような分岐がありません。"],
['types', 'write_inside_defined',
 'Inside the branch the name is a copy of the value; a write there would change the copy in one run and the field in the other.',
 "分岐の中では、その名前は値の写しです。\nそこで書けば、片方の実行では写しが、もう片方ではフィールドが変わります。"],
['types', 'constant_twice',
 'perl keeps a constant per package, and two classes may share the name — as long as they mean one thing by it.',
 "perl は定数をパッケージごとに持つので、二つのクラスが同じ名前を使えます。\nただし、同じ値を指しているときだけです。"],
['class', 'self_in_class',
 'Inside a class with methods a field is reached by its name; calling another method of the same class from there is not carried yet.',
 "メソッドを持つクラスの中では、フィールドは名前で触ります。\n同じクラスの別のメソッドをそこから呼ぶ形は、まだ運びません。"],
['class', 'new_with_values_in_field',
 'A field starts at what `new` gives with no values, so the type is known before anything runs; values go in `ADJUST` or a handler.',
 "フィールドは値なしの `new` から始まり、何も動く前に型が決まります。\n値は `ADJUST` かハンドラで入れます。"],
['class', 'method_named_like_field',
 'The compiled run gives every field a reader and a writer of its own name, and a method cannot take those names.',
 "コンパイルした実行はすべてのフィールドに、その名前の読み手と書き手を与えます。\nメソッドはその名前を取れません。"],
['perl', 'rand_unseeded',
 'perl seeds itself from the clock and the process when nothing else does, so an unseeded app draws a different sequence every start, in either run.',
 "何も種を蒔かないと、perl は時計とプロセスから自分で種を取ります。\nだから種のないアプリは、どちらの実行でも起動ごとに別の数列を引きます。"],
['perl', 'srand_bare',
 'The same reason: `srand()` with nothing picks a seed of its own.',
 "同じ理由です。\n引数のない `srand()` は、自分で種を選びます。"],
['views', 'rand_in_view',
 'Building a screen twice has to build the same screen, and a number drawn while building it would not be.',
 '同じ画面を二度組み立てたら同じ画面になる必要がありますが、組み立てながら引いた数は二度目には別の数です。'],
['perl', 'pow_variable',
 'perl answers a fraction for a negative power and a whole number otherwise, and a type is one or the other.',
 "perl は負の指数に小数を、そうでなければ整数を答えますが、型はどちらか一方です。"],
['perl', 'sleep_builtin',
 'A Perl function the translator has no twin for yet is said to be Perl\'s own, with what to write instead where there is something.',
 "翻訳器にまだ双子のない Perl の関数は、Perl 自身のものだと言います。\n代わりがあれば、それも言います。"],
['perl', 'module_call',
 'A shipped app is one file; a module of your own would have to be translated with it.',
 "配るアプリは一つのファイルで、自分のモジュールはそれと一緒に翻訳されなければなりません。"],
['perl', 'eval_block',
 '`eval { … }` is the older spelling of `try` / `catch`, which the dialect takes.',
 "`eval { … }` は `try` と `catch` の古い綴りで、方言は新しいほうを受け取ります。"],
['perl', 'sub_value',
 'A sub held in a variable is a closure, and the compiled run has no shape for one; a method of the app is what it calls.',
 "変数に持ったサブルーチンはクロージャで、コンパイルした実行にその形はありません。\nアプリのメソッドを呼びます。"],
['perl', 'label_loop',
 'A jump to a labeled loop has no shape in the compiled run; a flag the inner loop sets does the same.',
 "ラベルつきのループへの飛び越しは、コンパイルした実行に形がありません。\n内側のループが立てるフラグで同じことができます。"],
['perl', 'state_var',
 'A `state` variable is a field the method keeps for itself, and the app has fields for that.',
 "`state` 変数はメソッドが自分のために持つフィールドで、アプリにはそのためのフィールドがあります。"],
['perl', 'match_in_loop',
 'A match walked one step at a time keeps its place in perl\'s own bookkeeping; taking every match first is the same list.',
 "一歩ずつ歩く一致は、どこまで来たかを perl 自身が覚えています。\n先に全部取っても同じ並びです。"],
['perl', 'list_match',
 'The names come from `$1` and `$2` once the match is known to have happened, which is inside the `if`.',
 "名前は、一致したと分かってから `$1` と `$2` から取ります。\nそれは `if` の中です。"],
['types', 'literal_overflow',
 'perl would grow the number into one with a fraction; 64 bits is the edge the dialect holds, and two written numbers are worked out before anything is built.',
 "perl はその数を小数のある数に育てますが、方言が持つのは 64 bit までです。\n書き下した二つの数はビルドの前に計算し、そこで断ります。"],
);

my %WORDS = (
    en => {
        title => 'What Rakugan refuses',
        intro => <<'MD',
The dialect is a subset, and `rakugan check` is where you meet its edge.
It reads the app and names what it cannot take, with the file, the line
and the column, the line itself, and what to write instead. perl reads
the file first, so a shape perl rejects never reaches the translator.
`check` runs before every build and every gate, needs no compiler and no
window, and prints nothing at all when there is nothing to say.

Each of the %d below has a file under `test/refuse/` that triggers it and
the message it must print, word for word. The sweep runs them, so a
refusal cannot quietly change its wording, and this page is quoted from
those same files.
MD
        groups => {
            class => 'The class',
            types => 'Types',
            views => 'Views and handlers',
            perl  => "Perl the compiled run has no perl for",
        },
    },
    ja => {
        title => 'Rakugan が断る書き方',
        intro => <<'MD',
方言は部分集合で、その境目に出会う場所が `rakugan check` です。
アプリを読み、受け取れない書き方があれば、ファイルと行と桁、その行そのもの、そして代わりの書き方を示します。
ファイルを先に読むのは perl なので、perl が撥ねた書き方が翻訳器に届くことはありません。
`check` はビルドの前にもゲートの前にも走ります。
コンパイラもウィンドウも要らず、言うことがなければ何も出力しません。

下の %d 個には、それを起こすファイルと、出力されるべき文面が、`test/refuse/` にそのまま置いてあります。
`tools/gate_all.sh` がそれを回すので、断りの文面が黙って変わることはありません。
このページも、その同じファイルから引いています。
MD
        groups => {
            class => 'クラス',
            types => '型',
            views => 'ビューとハンドラ',
            perl  => 'コンパイルした実行に perl がないこと',
        },
    },
);

sub fixture {
    my ($name) = @_;
    my $path = File::Spec->catfile($FIX, "$name.txt");
    open my $fh, '<:encoding(UTF-8)', $path or die "$path: $!\n";
    local $/;
    my $text = <$fh>;
    close $fh;
    $text =~ s/\s+\z//;
    return $text;
}

# Every fixture is on the page, or the page is not the whole of it.
opendir my $dh, $FIX or die "$FIX: $!\n";
my @have = sort map { s/\.txt\z//r } grep { /\.txt\z/ } readdir $dh;
closedir $dh;
my %listed = map { $_->[1] => 1 } @CATALOGUE;
my @absent = grep { !$listed{$_} } @have;
die "the page has no entry for: @absent\n" if @absent;
my @gone = grep { my $n = $_->[1]; !grep { $_ eq $n } @have } @CATALOGUE;
die "the page lists refusals that have no fixture: "
  . join(', ', map { $_->[1] } @gone) . "\n" if @gone;

sub page {
    my ($lang) = @_;
    my $w = $WORDS{$lang};
    my @out = ('<!-- Written by website/tools/refusals_page.pl from test/refuse/. Edit the fixtures. -->',
               "# $w->{title}", '', sprintf($w->{intro}, scalar @have) =~ s/\s+\z//r, '');
    for my $group (@GROUPS) {
        my @in = grep { $_->[0] eq $group } @CATALOGUE;
        next unless @in;
        push @out, "## $w->{groups}{$group}", '';
        for my $e (@in) {
            my (undef, $name, $en, $ja) = @$e;
            push @out, ($lang eq 'ja' ? $ja : $en), '';
            push @out, '```console', fixture($name), '```', '';
        }
    }
    my $text = join("\n", @out);
    $text =~ s/\n{3,}/\n\n/g;
    return $text;
}

my $check = grep { $_ eq '--check' } @ARGV;
my $stale = 0;
for my $lang (qw(en ja)) {
    my $path = File::Spec->catfile($SITE, $lang eq 'ja' ? 'docs-ja' : 'docs', 'refusals.md');
    my $text = page($lang);
    if ($check) {
        my $have = -e $path ? do { open my $f, '<:encoding(UTF-8)', $path; local $/; <$f> } : '';
        if ($have ne $text) {
            warn "FAIL refusals.md ($lang) is behind the fixtures\n";
            $stale++;
        }
    } else {
        open my $out, '>:encoding(UTF-8)', $path or die "$path: $!\n";
        print {$out} $text;
        close $out;
        print "wrote $path\n";
    }
}
exit($stale ? 1 : 0);
