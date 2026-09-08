#!/usr/bin/env perl
# The site's Elements page, in both languages, written from the table.
#
# `Rakugan::Vocab` is the table as Perl data, generated from
# `crates/pixie-capi/elements.toml` by `tools/gen.pl` and kept honest by
# that program's `--check`. It is what the interpreted run writes
# elements from and what the translator reads, so a reference page
# written from anything else would be a second opinion. Nothing here is
# typed by hand except the prose that frames it.
#
#   tools/elements_page.pl            write docs/elements.md and docs-ja/
#   tools/elements_page.pl --check    fail if either is behind the table
use strict;
use warnings;
use utf8;
use open qw(:std :encoding(UTF-8));
use File::Basename qw(dirname);
use File::Spec;

my $SITE = File::Spec->rel2abs(File::Spec->catdir(dirname(__FILE__), File::Spec->updir));
my $ROOT = File::Spec->rel2abs(File::Spec->catdir($SITE, File::Spec->updir));
BEGIN {
    my $site = File::Spec->rel2abs(File::Spec->catdir(dirname(__FILE__), File::Spec->updir));
    unshift @INC, File::Spec->catdir($site, File::Spec->updir, 'lib');
}
use Rakugan::Vocab;

# The order a reader meets them, which is not the order the table is
# written in: the table is ordered by the numbers the two sides count
# with, and those may never be reordered.
my @GROUPS = (
    ['text',      [qw(text button link)]],
    ['arrange',   [qw(column row grid grid_cell stack scroll_view h_scroll_view modal)]],
    ['fields',    [qw(text_field number_field int_field checkbox switch slider
                      select radio_group segmented tab_bar)]],
    ['rows',      [qw(list_view table data_table)]],
    ['charts',    [qw(bar_chart line_chart progress)]],
    ['pictures',  [qw(image svg canvas)]],
    ['small',     [qw(spacer divider spinner)]],
);

my $N_EL    = scalar @Rakugan::Vocab::ELEMENTS;
my $N_RIDER = scalar @Rakugan::Vocab::RIDERS;
my $N_OP    = scalar @Rakugan::Vocab::OPS;

my %WORDS = (
    en => {
        title => 'Elements',
        intro => <<"MD",
$N_EL elements, and the $N_RIDER keywords every one of them takes. This
page is written from `elements.toml`, the one table the Perl subs an app
calls, the numbers both sides of the engine's C face count with, and the
engine's own constants are generated from. A keyword that is not here is
one an app cannot write, and `rakugan check` says so by name.

Types on this page: **text** is a string, **number** a float, **whole
number** an integer, **true/false** a boolean, and a **list of** either.
A **handler** is an anonymous sub written on the keyword the element
names for it, taking what the event carries; see
[Handlers](tour-logic.md#handlers).
MD
        riders_head  => 'The keywords every element takes',
        riders_intro => <<'MD',
These fifteen ride on every element under one name and one meaning. They
are wrappers around the element rather than fields repeated on thirty of
them, which is why an element that owns one of the names under its own
meaning keeps it: a `text`'s `width` is the text's, and the box leaves
it alone.
MD
        groups => {
            text     => 'Text, buttons, links',
            arrange  => 'The boxes that arrange',
            fields   => 'Fields and choosers',
            rows     => 'Lists and tables',
            charts   => 'Charts and progress',
            pictures => 'Pictures and the canvas',
            small    => 'The small pieces',
        },
        th         => ['Keyword', 'Type', 'Default'],
        op_th      => ['Command', 'Written as'],
        positional => 'written first, in order',
        handler    => 'a handler',
        children   => 'Takes elements as its children, written as its arguments.',
        owns       => 'Sizes its own %s: those are the element\'s, and the shared keyword leaves them alone.',
        owns_label => 'Its own `label` is what a screen reader reads, so the shared `a11y_label` is not offered here.',
        none       => 'No keywords of its own.',
        ops_head   => "The canvas's drawing commands",
        ops_intro  => <<"MD",
The $N_OP commands below are written inside a `canvas`'s `paint` sub.
They are not elements: they take none of the keywords above, nothing can click them,
and they mean nothing outside the canvas they are written in. Every
coordinate is a whole virtual pixel and every color is a number, the
index of a color in the canvas's palette. A value with a default may be
left out, and is then given by name.
MD
        footer => <<'MD',
## Adding one

An element is a row in `elements.toml` and an arm in the engine's
`materialize`. `tools/gen.pl` writes the Perl subs and the table as Perl
data from it, and the sweep fails when either is behind the table, so an
element cannot come to mean one thing in Perl and another where it is
drawn. The same table is read by the other three languages on this engine.
MD
    },
    ja => {
        title => '要素',
        intro => <<"MD",
要素は $N_EL 個、そのすべてが受け取る共通のキーワードが $N_RIDER 個あります。
このページは `elements.toml` から生成しています。
アプリが呼ぶ Perl のサブルーチンも、エンジンの C API で両側が数える番号も、エンジン側の定数も、同じ表から書き出されます。
ここにないキーワードは、アプリには書けません。
書いた場合は `rakugan check` が、その要素が受け取るキーワードを並べて断ります。

このページでの型の読み方です。
**文字列** は string、**数** は float、**整数** は integer、**真偽** は boolean、**〜のリスト** はそれぞれのリストです。
**ハンドラ** は、要素がそのために用意したキーワードに書く無名サブルーチンで、その出来事が運ぶものを受け取ります（[ハンドラ](tour-logic.md#ハンドラ)）。
MD
        riders_head  => 'すべての要素が受け取るキーワード',
        riders_intro => <<'MD',
どの要素も、この 15 個を同じ名前と同じ意味で受け取ります。
30 個の要素にそれぞれフィールドを足すのではなく、要素を包む形で実装してあります。
同じ名前を要素自身が別の意味で持っている場合は、要素のものが優先されます。
`text` の `width` は文字列そのものの幅で、外側の箱は手を出しません。
MD
        groups => {
            text     => '文字とボタンとリンク',
            arrange  => '並べる箱',
            fields   => '入力と選択',
            rows     => 'リストと表',
            charts   => 'グラフと進捗',
            pictures => '画像とキャンバス',
            small    => '小さな要素',
        },
        th         => ['キーワード', '型', '既定値'],
        op_th      => ['命令', '書き方'],
        positional => '先頭に、この順で書く',
        handler    => 'ハンドラ',
        children   => "要素を子に取ります。\n子は引数として書きます。",
        owns       => "%s は自分で決めます。\n共通キーワードは手を出しません。",
        owns_label => '自身の `label` が画面読み上げの読む名前なので、共通の `a11y_label` はここでは受け取りません。',
        none       => '固有のキーワードはありません。',
        ops_head   => 'キャンバスの描画命令',
        ops_intro  => <<"MD",
次の $N_OP 個の命令は、`canvas` の `paint` に渡すサブルーチンの中に書きます。
これらは要素ではありません。
上のキーワードをどれも取らず、押すこともできず、書かれたキャンバスの外では意味を持ちません。
座標はすべて仮想的な画素の整数で、色はすべて番号です。
番号はそのキャンバスの配色の何番目か、というだけのものです。
既定値のある値は省略できます。
書くときは名前を添えます。
MD
        footer => <<'MD',
## 要素を足すとき

要素を足す作業は、`elements.toml` に 1 行足すことと、エンジンの `materialize` に分岐を 1 つ足すことです。
`tools/gen.pl` が、表から Perl のサブルーチンと、表そのものを写した Perl のデータを書き出します。
どちらかが表より古ければ、`tools/gate_all.sh` が落ちます。
だから、ある要素が Perl では 1 つの意味を持ち、描かれる側では別の意味を持つ、ということが起きません。
このエンジンの上にあるほかの三つの言語も、同じ表を読んでいます。
MD
    },
);

my %TYPES = (
    en => { str => 'text', num => 'number', int => 'whole number', bool => 'true/false',
            strs => 'list of text', nums => 'list of numbers',
            nums2 => 'list of lists of numbers',
            rows => 'a count, and the sub building row i' },
    ja => { str => '文字列', num => '数', int => '整数', bool => '真偽',
            strs => '文字列のリスト', nums => '数のリスト',
            nums2 => '数のリストのリスト',
            rows => '個数と、i 行目を作るサブルーチン' },
);

# The table holds a Perl value; the page shows what an app writes, so a
# bool is `true` / `false` rather than the 1 and 0 Perl keeps it as.
sub show_default {
    my ($p) = @_;
    return undef unless exists $p->{default};
    my $v = $p->{default};
    return '`[]`' if ref $v eq 'ARRAY';
    return $v ? '`true`' : '`false`' if ($p->{type} // '') eq 'bool';
    return '`""`' if $v eq '';
    return "`$v`";
}

sub prop_rows {
    my ($el, $lang) = @_;
    my @rows;
    for my $p (@{ $el->{props} || [] }) {
        my $type = exists $p->{handler} ? $WORDS{$lang}{handler}
                 : ($TYPES{$lang}{ $p->{type} } // die "no name for type $p->{type}\n");
        my $default = $p->{pos} ? $WORDS{$lang}{positional} : (show_default($p) // '—');
        push @rows, "| `$p->{name}` | $type | $default |";
    }
    return @rows;
}

sub element_section {
    my ($el, $lang) = @_;
    my $w = $WORDS{$lang};
    my @out = ("### `$el->{name}`", '', $el->{doc}, '');
    push @out, "$w->{children}\n" if $el->{children};
    my @owned = map { "`$_->{name}`" }
                grep { my $r = $_; grep { $_ eq ($el->{native} // '') } split /,/, ($r->{owned} // '') }
                @Rakugan::Vocab::RIDERS;
    push @out, sprintf($w->{owns}, join(' / ', @owned)) . "\n" if @owned;
    push @out, "$w->{owns_label}\n" if $el->{owns_label};
    my @rows = prop_rows($el, $lang);
    if (!@rows) {
        push @out, "$w->{none}\n";
    } else {
        push @out, '| ' . join(' | ', @{ $w->{th} }) . ' |', '|---|---|---|', @rows, '';
    }
    return join("\n", @out);
}

sub op_section {
    my ($lang) = @_;
    my $w = $WORDS{$lang};
    my @out = ("## $w->{ops_head}", '', $w->{ops_intro} =~ s/\s+\z//r, '',
               '| ' . join(' | ', @{ $w->{op_th} }) . ' |', '|---|---|');
    for my $op (@Rakugan::Vocab::OPS) {
        my @args = map {
            exists $_->{default} ? "$_->{name} => $_->{default}" : $_->{name}
        } @{ $op->{params} };
        push @out, "| `$op->{name}` | `$op->{name}(" . join(', ', @args) . ')` |';
    }
    push @out, '';
    return join("\n", @out);
}

sub page {
    my ($lang) = @_;
    my $w = $WORDS{$lang};
    my @out = ('<!-- Written by website/tools/elements_page.pl from the table. Edit the table. -->',
               "# $w->{title}", '', $w->{intro} =~ s/\s+\z//r, '');
    push @out, "## $w->{riders_head}", '', $w->{riders_intro} =~ s/\s+\z//r, '',
               '| ' . join(' | ', @{ $w->{th} }) . ' |', '|---|---|---|';
    for my $r (@Rakugan::Vocab::RIDERS) {
        push @out, "| `$r->{name}` | $TYPES{$lang}{ $r->{type} } | "
                 . ($r->{presence} ? '—' : (show_default($r) // '—')) . ' |';
    }
    push @out, '';
    for my $g (@GROUPS) {
        my ($key, $names) = @$g;
        push @out, "## $w->{groups}{$key}", '';
        push @out, element_section($Rakugan::Vocab::ELEMENT{$_}, $lang) for @$names;
    }
    push @out, op_section($lang);
    push @out, $w->{footer} =~ s/\s+\z//r, '';
    my $text = join("\n", @out);
    $text =~ s/\n{3,}/\n\n/g;
    return $text;
}

# Every element in the table is on the page, or the page is a lie.
my %listed = map { $_ => 1 } map { @{ $_->[1] } } @GROUPS;
my @absent = grep { !$listed{ $_->{name} } } @Rakugan::Vocab::ELEMENTS;
die "the page has no place for: " . join(', ', map { $_->{name} } @absent) . "\n" if @absent;

my $check = grep { $_ eq '--check' } @ARGV;
my $stale = 0;
for my $lang (qw(en ja)) {
    my $path = File::Spec->catfile($SITE, $lang eq 'ja' ? 'docs-ja' : 'docs', 'elements.md');
    my $text = page($lang);
    if ($check) {
        my $have = -e $path ? do { open my $f, '<:encoding(UTF-8)', $path; local $/; <$f> } : '';
        if ($have ne $text) {
            warn "FAIL elements.md ($lang) is behind the table\n";
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
