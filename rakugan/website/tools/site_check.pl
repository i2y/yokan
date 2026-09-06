#!/usr/bin/env perl
# What the site claims, checked against the tree it claims it about.
#
# Four of the site's pages are written from somewhere else: the tour
# from `TOUR.md`, the elements from the table, the gallery from
# `demo/`, the refusals from the fixtures that hold their wording. Each
# writer has a `--check`, and this program runs all of them, then reads
# what is left: that both languages carry the same pages, that the nav
# and the tree agree, and that every image a page points at is there. It
# reads only; nothing here rewrites a page.
#
#   tools/site_check.pl
use strict;
use warnings;
use utf8;
use open qw(:std :encoding(UTF-8));
use File::Basename qw(basename dirname);
use File::Spec;

my $SITE = File::Spec->rel2abs(File::Spec->catdir(dirname(__FILE__), File::Spec->updir));
my $ROOT = File::Spec->rel2abs(File::Spec->catdir($SITE, File::Spec->updir));

my @fails;
sub ok { print "OK   $_[0]\n" }

sub slurp {
    my ($path) = @_;
    open my $fh, '<:encoding(UTF-8)', $path or die "$path: $!\n";
    local $/;
    my $text = <$fh>;
    close $fh;
    return $text;
}

sub pages_in {
    my ($dir) = @_;
    opendir my $dh, File::Spec->catdir($SITE, $dir) or die "$dir: $!\n";
    my @md = sort grep { /\.md\z/ } readdir $dh;
    closedir $dh;
    return @md;
}

# --- the four generated pages -----------------------------------------------

for my $gen (qw(tour_pages elements_page demos_page refusals_page)) {
    my $prog = File::Spec->catfile($SITE, 'tools', "$gen.pl");
    if (system($^X, $prog, '--check') == 0) {
        ok $gen;
    } else {
        push @fails, "$gen is behind its source (run tools/$gen.pl)";
    }
}

# --- both languages carry the same pages ------------------------------------

my @en = pages_in('docs');
my @ja = pages_in('docs-ja');
if ("@en" eq "@ja") {
    ok 'both languages: ' . scalar(@en) . ' pages';
} else {
    my %ja = map { $_ => 1 } @ja;
    my %en = map { $_ => 1 } @en;
    push @fails, 'the two languages differ: only EN '
        . join(', ', grep { !$ja{$_} } @en) . '; only JA '
        . join(', ', grep { !$en{$_} } @ja);
}

# --- every page in the nav exists, and every page is in the nav -------------

for my $pair (['zensical.toml', 'docs'], ['zensical.ja.toml', 'docs-ja']) {
    my ($config, $dir) = @$pair;
    my $text = slurp(File::Spec->catfile($SITE, $config));
    my ($nav) = $text =~ /^nav = \[(.*?)^\]/ms;
    my @in_nav = $nav =~ /"([\w-]+\.md)"/g;
    my @files = pages_in($dir);
    my %files = map { $_ => 1 } @files;
    my %in_nav = map { $_ => 1 } @in_nav;
    my @gone = grep { !$files{$_} } @in_nav;
    my @unlisted = grep { !$in_nav{$_} } @files;
    push @fails, "$config lists a page that is not there: @gone" if @gone;
    push @fails, "$dir has a page the nav never shows: @unlisted" if @unlisted;
    ok "$config: " . scalar(@in_nav) . ' pages in the nav' if !@gone && !@unlisted;
}

# --- images: every page's images exist --------------------------------------

for my $dir (qw(docs docs-ja)) {
    my @missing;
    for my $page (pages_in($dir)) {
        my $text = slurp(File::Spec->catfile($SITE, $dir, $page));
        my %seen;
        while ($text =~ /(?:!\[[^\]]*\]\(|<img src=")(images\/[^)"#]+)/g) {
            next if $seen{$1}++;
            push @missing, "$page → $1"
                unless -f File::Spec->catfile($SITE, $dir, $1);
        }
    }
    if (@missing) {
        push @fails, "$dir points at images that are not there: @missing";
    } else {
        ok "$dir: every image is there";
    }
}

# --- the gallery has a picture of every demo --------------------------------

opendir my $dh, File::Spec->catdir($ROOT, 'demo') or die "demo: $!\n";
my @apps = sort map { s/\.pl\z//r } grep { /\.pl\z/ } readdir $dh;
closedir $dh;
my @unshot = grep {
    my $n = $_;
    !grep { -f File::Spec->catfile($SITE, 'docs', 'images', 'demos', "$n.$_") } qw(png gif);
} @apps;
if (@unshot) {
    push @fails, "the gallery has no picture of: @unshot";
} else {
    ok 'the gallery: ' . scalar(@apps) . ' demos, all pictured';
}

if (@fails) {
    warn "FAIL $_\n" for @fails;
    exit 1;
}
print "SITE OK\n";
