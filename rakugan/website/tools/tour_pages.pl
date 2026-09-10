#!/usr/bin/env perl
# The site's six tour pages, in both languages, cut out of TOUR.md and
# TOUR.ja.md.
#
# The repository's tour is the one text. The site shows it in six
# pages because a single page that long is not read, and cutting it
# here rather than keeping a second copy is what stops the two from
# drifting: a paragraph written once appears in both places or in
# neither. Links between sections are rewritten to point at the page
# the section landed on.
#
#   tools/tour_pages.pl            write docs/tour*.md and docs-ja/
#   tools/tour_pages.pl --check    fail if either is behind the tour
use strict;
use warnings;
use utf8;
use open qw(:std :encoding(UTF-8));
use File::Basename qw(dirname);
use File::Spec;

my $SITE = File::Spec->rel2abs(File::Spec->catdir(dirname(__FILE__), File::Spec->updir));
my $ROOT = File::Spec->rel2abs(File::Spec->catdir($SITE, File::Spec->updir));

# The six pages: the file, the heading each language gives it, the lede
# under that heading, and the sections it takes from the tour, in order.
my @PAGES = (
    {
        file => 'tour.md',
        en   => ['The first app',
                 'An app is a class, its state is its fields, and types are written in only two places.'],
        ja   => ['はじめてのアプリ',
                 "アプリはクラスで、状態はそのフィールドです。\n型を書く場所は二か所だけです。"],
        take => ['The smallest app', 'Holding state', 'Types, and where they come from'],
    },
    {
        file => 'tour-logic.md',
        en   => ['Views and control flow',
                 'How a screen is built, broken into pieces, and driven by what the app holds.'],
        ja   => ['ビューと制御構造',
                 '画面をどう組み立て、どう分け、アプリの持つもので動かすか。'],
        take => ['Writing views', 'Control flow in a view', 'Form controls', 'Handlers',
                 'Lists, charts, and rows built on demand', 'Hashes', 'Value classes',
                 'Regular expressions'],
    },
    {
        file => 'tour-canvas.md',
        en   => ['The canvas and the keyboard',
                 'A grid of virtual pixels, and keys read as a device rather than waited for.'],
        ja   => ['キャンバスとキーボード',
                 '仮想的な画素の格子と、知らせを待つのではなくこちらから尋ねるキーボード。'],
        take => ['The canvas', 'The keyboard'],
    },
    {
        file => 'tour-ui.md',
        en   => ['Look, and the window',
                 'The keywords every element takes, the palette, and what the window itself brings.'],
        ja   => ['見た目とウィンドウ',
                 'すべての要素が取るキーワード、配色、そしてウィンドウ自身が持ってくるもの。'],
        take => ['The keywords every element takes', 'Themes and animation', 'The window'],
    },
    {
        file => 'tour-lib.md',
        en   => ['Perl, data, and work',
                 "Perl's own library, the framework's, what to do when a call fails, and work that must not freeze the window."],
        ja   => ['Perl とデータとタスク',
                 'Perl 自身のライブラリ、フレームワークのライブラリ、失敗したときの書き方、ウィンドウを固めない処理の出し方。'],
        take => ["Perl's own standard library", "The framework's standard library", 'When something fails',
                 "Timers and work off the window's thread", 'While you are writing it'],
    },
    {
        file => 'tour-ship.md',
        en   => ['Verify and ship',
                 'The gate, what the dialect refuses, the bundle, and what does not work yet.'],
        ja   => ['確かめて配る',
                 'ゲート、方言が断る書き方、バンドル、そしてまだできないこと。'],
        take => ['Headless runs and the gate', 'What Rakugan refuses', 'Shipping',
                 'What does not work yet'],
    },
);

# --- reading the tour -------------------------------------------------------

# GitHub's anchor for a heading, which is also the site's (both configs
# ask pymdownx for a unicode-preserving slug). `\w` is Unicode here, so
# it already holds every Japanese letter; naming the scripts as well
# would keep the punctuation they share (`\p{Han}` matches `、`, which
# is Common with Han among its script extensions).
sub slug {
    my ($h) = @_;
    my $s = lc $h;
    $s =~ s/[^\w\s-]//g;
    $s =~ s/\s+/-/g;
    return $s;
}

# What the tour says before its first section: the paragraph that names
# the language. The site's first tour page carries it, the way the other
# two languages' first pages do, so a reader who lands there from a
# search meets the language before the vocabulary. Only the first
# paragraph travels — what follows it in the file is about the file (the
# checker, the other language's copy) and the site says both elsewhere.
sub identity {
    my ($path) = @_;
    open my $fh, '<:encoding(UTF-8)', $path or die "$path: $!\n";
    local $/;
    my $text = <$fh>;
    close $fh;
    my ($intro) = $text =~ /\A\#\ [^\n]*\n(.*?)^\#\# /ms
        or die "$path: no intro before the first section\n";
    $intro =~ s/<!--.*?-->//gs;                    # the draft marker is the owner's, not a reader's
    my ($first) = grep { /\S/ } split /\n\s*\n/, $intro;
    die "$path: the intro is empty\n" unless defined $first;
    $first =~ s/\A\s+//;
    $first =~ s/\s+\z//;
    return $first;
}

sub sections {
    my ($path) = @_;
    open my $fh, '<:encoding(UTF-8)', $path or die "$path: $!\n";
    my (@order, %body, $cur);
    while (my $line = <$fh>) {
        if ($line =~ /\A## (.+?)\s*\z/) {
            $cur = $1;
            push @order, $cur;
            $body{$cur} = '';
            next;
        }
        $body{$cur} .= $line if defined $cur;
    }
    close $fh;
    return (\@order, \%body);
}

# --- writing a page ---------------------------------------------------------

sub render {
    my ($lang, $i, $order, $body, $where) = @_;
    my $page = $PAGES[$i];
    my ($title, $lede) = @{ $page->{$lang} };
    my $out = "<!-- Written by website/tools/tour_pages.pl from the tour. Edit the tour. -->\n"
             . "# $title\n\n";
    $out .= $where->{identity} . "\n\n" if $i == 0;
    $out .= "$lede\n";
    for my $head (@{ $page->{take} }) {
        my $key = $lang eq 'ja' ? $where->{ja_of}{$head} : $head;
        die "$page->{file}: the tour has no section \"$head\"\n" unless exists $body->{$key};
        my $text = $body->{$key};
        $out .= "\n## $key\n" . $text;
    }
    # A link to a section is rewritten to the page it landed on, and
    # dropped to a bare anchor when that is this page.
    $out =~ s{\]\(#([^)]+)\)}{
        my $anchor = $1;
        my $file = $where->{page}{$anchor};
        defined $file ? ($file eq $page->{file} ? "](#$anchor)" : "]($file#$anchor)")
                      : "](#$anchor)";
    }ge;
    # The tour sits beside docs/PIXIE.md in the repository; the site does not.
    $out =~ s{\]\(\.\./docs/PIXIE\.md\)}{](https://github.com/i2y/yokan/blob/main/docs/PIXIE.md)}g;
    $out =~ s/\n{3,}\z/\n/;
    return $out;
}

# --- main -------------------------------------------------------------------

my $check = grep { $_ eq '--check' } @ARGV;
my $stale = 0;

for my $lang (qw(en ja)) {
    my $tour = File::Spec->catfile($ROOT, $lang eq 'ja' ? 'TOUR.ja.md' : 'TOUR.md');
    my ($order, $body) = sections($tour);
    my ($en_order) = sections(File::Spec->catfile($ROOT, 'TOUR.md'));

    # The two tours carry the same sections in the same order, so the
    # Japanese heading for an English one is the heading in that place.
    my %ja_of;
    if ($lang eq 'ja') {
        die "the two tours have a different number of sections\n"
            unless @$order == @$en_order;
        @ja_of{@$en_order} = @$order;
    }
    my %where;
    $where{ja_of} = \%ja_of;
    $where{identity} = identity($tour);
    for my $i (0 .. $#PAGES) {
        for my $head (@{ $PAGES[$i]{take} }) {
            my $key = $lang eq 'ja' ? $ja_of{$head} : $head;
            $where{page}{ slug($key) } = $PAGES[$i]{file};
        }
    }

    my $dir = File::Spec->catdir($SITE, $lang eq 'ja' ? 'docs-ja' : 'docs');
    for my $i (0 .. $#PAGES) {
        my $text = render($lang, $i, $order, $body, \%where);
        my $path = File::Spec->catfile($dir, $PAGES[$i]{file});
        if ($check) {
            my $have = -e $path ? do { open my $f, '<:encoding(UTF-8)', $path; local $/; <$f> } : '';
            if ($have ne $text) {
                warn "FAIL $lang/$PAGES[$i]{file} is behind the tour\n";
                $stale++;
            }
        } else {
            open my $out, '>:encoding(UTF-8)', $path or die "$path: $!\n";
            print {$out} $text;
            close $out;
            print "wrote $path\n";
        }
    }
}

exit($stale ? 1 : 0);
