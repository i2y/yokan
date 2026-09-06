#!/usr/bin/env perl
# The site's Demos page, in both languages, written from demo/.
#
# A gallery is a page of usage samples, so the sample has to be on it:
# each demo gets its screenshot and, under it, its whole source in a
# collapsed block. The gloss for each one is written here, once per
# language, and a demo with no entry stops this program, so a new demo
# cannot quietly go missing from the gallery.
#
#   tools/demos_page.pl            write docs/demos.md and docs-ja/
#   tools/demos_page.pl --check    fail if either is behind demo/
use strict;
use warnings;
use utf8;
use open qw(:std :encoding(UTF-8));
use File::Basename qw(basename dirname);
use File::Spec;

my $SITE  = File::Spec->rel2abs(File::Spec->catdir(dirname(__FILE__), File::Spec->updir));
my $ROOT  = File::Spec->rel2abs(File::Spec->catdir($SITE, File::Spec->updir));
my $DEMOS = File::Spec->catdir($ROOT, 'demo');
my $SHOTS = File::Spec->catdir($DEMOS, 'screenshots');

my @GROUPS = qw(start state look lists canvas perl window work);

# group, name, the English gloss, the Japanese one.
my @CATALOGUE = (
['start', 'counter',
 'the reference: an app is a class, its state is its fields, and a handler is an anonymous sub that closes over them',
 '基本形。アプリはクラスで、状態はそのフィールド、ハンドラはフィールドがそのまま見える無名サブルーチン'],
['start', 'control',
 'ordinary Perl inside a view: `if`, `unless`, a conditional expression, a loop, and a method that answers part of the screen',
 'ビューの中はただの Perl。`if`、`unless`、条件演算子、ループ、画面の一部を返すメソッド'],
['start', 'todo',
 'a list whose rows are built on demand, and a field that submits with enter',
 '行を必要なぶんだけ作るリストと、enter で確定する入力欄'],
['start', 'calc',
 'a calculator: one accumulator, one pending operation, and a look kept in a hash and handed to each key',
 '電卓。途中の値ひとつと待っている演算ひとつを持ち、見た目はハッシュにまとめて各キーに渡す'],
['start', 'calcgrid',
 'the same calculator on a grid instead of five rows; `col_span` is what makes the zero key twice as wide',
 '同じ電卓を 5 行ではなくグリッドで。0 キーが 2 つぶんの幅になるのは `col_span`'],

['state', 'mixer',
 'state an app keeps, and a field that writes into it; one part of the screen is only there some of the time',
 'アプリが持つ状態と、そこへ書き込む入力欄。画面の一部は、あるときだけ現れる'],
['state', 'lookup',
 'a hash on the app: reading with a fallback, asking whether a key is there, and adding one while the window is open',
 'アプリが持つハッシュ。既定値つきの読み出し、鍵があるかどうかの確認、ウィンドウを開けたままの追加'],
['state', 'points',
 "a small class of values, carried on the app's own state",
 '値のための小さなクラスを、アプリの状態として持つ'],

['look', 'forms',
 'the controls a person changes: a box, a switch, a track, and the four choosers',
 '人が動かす部品。チェックボックス、スイッチ、スライダ、そして 4 種類の選択'],
['look', 'quantities',
 'the two fields that hold a number rather than text: enter commits, text that is not a number is dropped',
 '文字ではなく数を持つ 2 つの入力欄。enter で確定し、数でない文字は捨てられる'],
['look', 'layout',
 'spacer and divider: a filler that pushes what follows to the edge, and a rule',
 'spacer と divider。続きを端まで押しやる余白と、区切り線'],
['look', 'cards',
 'a piece of screen with a name is a method, and one that wraps other elements takes them after its own values',
 '名前のついた画面の一部はメソッド。ほかの要素を包むものは、自分の値のあとにそれらを取る'],
['look', 'styled',
 'a look kept in one place: a hash of keywords handed to an element, and `theme` flipping a whole panel',
 '見た目をひとところに。キーワードのハッシュを要素に渡し、`theme` でパネルを丸ごと切り替える'],
['look', 'badges',
 'text as a pill, and the rest of what a run of text can be: monospace, underlined, italic, clipped, clamped',
 '文字を丸いラベルに。等幅、下線、斜体、省略記号での打ち切り、行数の制限'],
['look', 'panels',
 'the elements that arrange or cover: tracks, layers, panes that scroll, and a panel over the rest of the window',
 '並べる要素と覆う要素。グリッド、重ね、スクロールする面、ウィンドウの上に出る一枚'],
['look', 'dialog',
 'a panel over the rest of the window, opened and closed by the app',
 'ウィンドウの上に出る一枚を、アプリが開いて閉じる'],
['look', 'labels',
 'what a screen reader is told and what the pointer shows; `role` takes a value, so a line is a heading until it is not',
 '画面読み上げに伝える名前と、ポインタが見せる説明。`role` は値を取るので、行が見出しであるかどうかを切り替えられる'],
['look', 'shared',
 'the keywords every element takes, on elements that have nothing else in common',
 '共通のキーワードを、種類の違う要素それぞれに付けてみる'],
['look', 'loading',
 'the bar that fills, in its three forms, and the sweep for work with no known length',
 '満ちていくバーの 3 つの形と、終わりの見えない仕事のために行き来する表示'],
['look', 'filter',
 'a chooser that changes what a list shows, with the rows built on demand',
 'リストが見せるものを変える選択。行は必要なぶんだけ作られる'],

['lists', 'table',
 '`data_table` draws the table itself: the first row is the header, the later ones are shaded in alternation',
 '`data_table` が表そのものを描く。最初の row が見出しで、以降は交互に色の変わるデータ行'],
['lists', 'roster',
 'the table that builds its rows on demand, with row selection and header sort the app performs itself',
 '行を必要なぶんだけ作る表。行の選択と見出しでの並べ替えは、アプリ自身が行う'],
['lists', 'csv_viewer',
 'a hundred thousand rows, filtered as you type; only the rows in the window are ever built',
 '10 万行を、打ちながら絞り込む。作られるのは画面に入っている行だけ'],
['lists', 'trend',
 'one list of numbers, drawn twice',
 'ひとつの数のリストを、2 通りに描く'],
['lists', 'charts',
 'losses below the zero line, a pinned range, an axis with gridlines, and two series with their own colors',
 '0 の線より下に伸びる損失、固定した範囲、目盛りと補助線のある軸、色を持つ 2 本の系列'],

['canvas', 'canvas',
 'a grid of virtual pixels painted command by command, colors by palette index, and the keyboard read from the tick',
 '仮想的な画素の格子を、命令をひとつずつ並べて塗る。色は配色の番号、キーボードはタイマーから読む'],
['canvas', 'jump',
 "Pyxel's jump game, ported: gravity, floors that fall away when you land on them, fruit, and scenery scrolling at its own speed",
 'Pyxel のジャンプゲームの移植。重力、乗ると落ちる床、果物、それぞれの速さで流れる背景'],
['canvas', 'shooter',
 "Pyxel's shoot-'em-up, ported: scenes, parallax stars, enemies that sway as they fall, collisions and expanding blasts",
 'Pyxel のシューティングの移植。場面の切り替え、視差のある星、揺れながら落ちてくる敵、当たり判定と広がる爆発'],

['perl', 'stdlib',
 "Perl's own under the gate: `sprintf`, `sort`, `grep`, `map`, `List::Util`, `POSIX`, regular expressions",
 'Perl 自身のものをゲートにかける。`sprintf`、`sort`、`grep`、`map`、`List::Util`、`POSIX`、正規表現'],
['perl', 'files',
 "files through the framework's own library: one implementation answers both runs",
 '枠組みのライブラリでファイルを扱う。一つの実装が両方の実行に答える'],
['perl', 'reader',
 'nested JSON, reached by path, written to a file and read back so both runs read the same bytes',
 '入れ子の JSON をドットパスで読む。ファイルに書いて読み直すので、両方の実行が同じバイト列を読む'],
['perl', 'dbnotes',
 'a database reached through the engine, with the values bound rather than spliced',
 'エンジン越しに触るデータベース。値は文に埋め込まず、束縛して渡す'],
['perl', 'ledger',
 "money kept in sqlite: an item called o'brien is an apostrophe and never a piece of SQL",
 "sqlite に置いた家計簿。o'brien という品目はアポストロフィであって、SQL の一部にはならない"],
['perl', 'edges',
 'the edges: an index past the end of a list, and a number far past what a machine word holds',
 '端の話。リストの終わりを越えた添字と、64 ビットをはるかに超えた数'],
['perl', 'flow',
 'control flow in the handlers: a loop that skips, a loop that stops, a while, and a method that answers a value',
 'ハンドラの中の制御構造。飛ばすループ、止まるループ、while、値を返すメソッド'],

['window', 'keys',
 'the keyboard as chords and the same handlers in the menu bar, driven with `key:cmd+s` and `menu:Save`',
 'キーの組み合わせにハンドラを結び付け、同じものをメニューバーにも置く。`key:cmd+s` と `menu:Save` で動かせる'],
['window', 'picker',
 "the platform's own file panels, asked for off the window's thread, and a file dragged onto the window",
 'OS 自身のファイル選択と、ウィンドウへ落とされたファイル。選択は人を待つので、ウィンドウのスレッドの外で頼む'],
['window', 'about',
 'links that open a page, and the system clipboard',
 'ページを開くリンクと、システムのクリップボード'],
['window', 'sound',
 'a WAV file played from a handler; a run under a script is silent, so the gate compares two silent runs',
 'ハンドラから WAV ファイルを鳴らす。スクリプトの下では無音になるので、ゲートが突き合わせるのは画面だけ'],

['work', 'dashboard',
 'a timer declared before the app runs, ticking in both runs (the gate steps it with `advance:`)',
 'アプリを走らせる前に宣言するタイマー。両方の実行で同じだけ時を刻む（ゲートは `advance:` で進める）'],
['work', 'tasks',
 "work that takes a while, done off the window's thread; the answer comes back through `on_done`",
 '時間のかかる仕事をウィンドウのスレッドの外へ。答えは `on_done` で受け取る'],
);

my %WORDS = (
    en => {
        title => 'Demos',
        intro => <<'MD',
%d apps, every one of them gated: the interpreted run and the compiled
one, driven by the same script, compared byte for byte. Each runs as-is
from `rakugan/` in the repository.

```console
$ ./bin/rakugan run demo/counter.pl     # substitute any demo's name
$ ./tools/gate_all.sh                   # gate every demo at once
```

Every screenshot shows the state right after launch, except the two
games, which show a recording of play. The source under each one is the
whole file.
MD
        groups => {
            start  => 'Start here',
            state  => 'State',
            look   => 'Look and layout',
            lists  => 'Lists, tables, charts',
            canvas => 'The canvas',
            perl   => 'Perl, files, data',
            window => 'The window',
            work   => 'Timers and work',
        },
    },
    ja => {
        title => 'デモ',
        intro => <<'MD',
アプリが %d 本あり、すべてがゲートを通っています。
解釈実行とコンパイルした実行を同じスクリプトで動かし、1 バイトずつ突き合わせています。
どれもリポジトリの `rakugan/` からそのまま動きます。

```console
$ ./bin/rakugan run demo/counter.pl     # 名前はどのデモでもよい
$ ./tools/gate_all.sh                   # すべてのデモをまとめてゲートにかける
```

画面写真はどれも起動直後の状態です。
ただし 2 つのゲームだけは、遊んでいるところの録画です。
その下にあるのは、そのデモのファイル全体です。
MD
        groups => {
            start  => 'まずはここから',
            state  => '状態',
            look   => '見た目と配置',
            lists  => 'リストと表とグラフ',
            canvas => 'キャンバス',
            perl   => 'Perl とファイルとデータ',
            window => 'ウィンドウまわり',
            work   => 'タイマーと仕事',
        },
    },
);

# --- the tree the page claims things about ----------------------------------

opendir my $dh, $DEMOS or die "$DEMOS: $!\n";
my @apps = sort map { s/\.pl\z//r } grep { /\.pl\z/ } readdir $dh;
closedir $dh;

my %entry = map { $_->[1] => $_ } @CATALOGUE;
my @missing = grep { !$entry{$_} } @apps;
die "demo/ has apps the gallery has no entry for: @missing\n" if @missing;
my @gone = grep { my $n = $_->[1]; !grep { $_ eq $n } @apps } @CATALOGUE;
die "the gallery lists apps demo/ does not have: "
  . join(', ', map { $_->[1] } @gone) . "\n" if @gone;

sub shot {
    my ($name) = @_;
    for my $ext (qw(gif png)) {
        return "$name.$ext" if -f File::Spec->catfile($SHOTS, "$name.$ext");
    }
    die "demo/screenshots has no picture of $name\n";
}

sub source {
    my ($name) = @_;
    open my $fh, '<:encoding(UTF-8)', File::Spec->catfile($DEMOS, "$name.pl") or die "$name: $!\n";
    local $/;
    my $text = <$fh>;
    close $fh;
    $text =~ s/\s+\z//;
    return $text;
}

sub page {
    my ($lang) = @_;
    my $w = $WORDS{$lang};
    my @out = ('<!-- Written by website/tools/demos_page.pl from demo/. Edit the demos. -->',
               "# $w->{title}", '', sprintf($w->{intro}, scalar @apps) =~ s/\s+\z//r, '');
    for my $group (@GROUPS) {
        my @in = grep { $_->[0] eq $group } @CATALOGUE;
        next unless @in;
        push @out, "## $w->{groups}{$group}", '';
        for my $e (@in) {
            my (undef, $name, $en, $ja) = @$e;
            my $gloss = $lang eq 'ja' ? $ja : $en;
            my $pic = shot($name);
            my $width = $pic =~ /\.gif\z/ ? 300 : 360;
            push @out, "#### $name — $gloss";
            push @out, qq{<img src="images/demos/$pic" width="$width">}, '';
            push @out, qq{??? note "$name.pl"}, '';
            push @out, '    ```perl';
            push @out, map { $_ eq '' ? '' : "    $_" } split /\n/, source($name);
            push @out, '    ```', '';
        }
    }
    my $text = join("\n", @out);
    $text =~ s/\n{3,}/\n\n/g;
    return $text . "\n";
}

my $check = grep { $_ eq '--check' } @ARGV;
my $stale = 0;
for my $lang (qw(en ja)) {
    my $path = File::Spec->catfile($SITE, $lang eq 'ja' ? 'docs-ja' : 'docs', 'demos.md');
    my $text = page($lang);
    if ($check) {
        my $have = -e $path ? do { open my $f, '<:encoding(UTF-8)', $path; local $/; <$f> } : '';
        if ($have ne $text) {
            warn "FAIL demos.md ($lang) is behind demo/\n";
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
