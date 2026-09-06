#!/usr/bin/env perl
# What perl answers, printed by perl.
#
# The gate proves the two runs AGREE. It cannot prove they agree with
# perl: a twin that is wrong the same way in both runs still passes it.
# That is what these tables are for. The interpreted run IS perl, so
# holding a twin to what perl printed holds it to the other run too.
#
#   tools/gen_expected.pl           write the tables
#   tools/gen_expected.pl --check   fail when what is on disk is not
#                                   what perl says now
#
# Run it under the perl an app runs under (`tools/perl_setup.sh
# --path`), which is the one the sweep measures against. A row is
#
#   name arg… -> result
#
# with each value tagged: `i:` a whole number, `f:` a double in hex,
# `b:` 0 or 1, `s:` a string with everything outside a safe set
# percent-escaped, and `[a,b]` a list. `~>` in place of `->` allows one
# ulp, for an answer the platform's own libm decides.
use v5.36;
use utf8;
use open qw(:std :encoding(UTF-8));
use List::Util qw(sum max min uniq);
use POSIX qw(floor ceil fmod strftime);
use Encode qw(encode);
use File::Basename qw(dirname);
use File::Path qw(make_path);
use File::Spec;

my $ROOT = File::Spec->rel2abs(File::Spec->catdir(dirname(__FILE__), '..'));
my $OUT  = File::Spec->rel2abs(File::Spec->catdir($ROOT, '..', 'crates', 'rakugan-stdlib', 'tests', 'expected'));

# --- how a value is written --------------------------------------------------

sub tag_int { "i:$_[0]" }
sub tag_bool { 'b:' . ($_[0] ? 1 : 0) }
sub tag_float { 'f:' . unpack('H*', pack('d>', $_[0])) }

sub tag_str {
    my ($s) = @_;
    my $bytes = encode('UTF-8', $s);
    $bytes =~ s/([^A-Za-z0-9_.\/:+\-])/sprintf '%%%02x', ord $1/ge;
    return "s:$bytes";
}

sub tag_list { '[' . join(',', @_) . ']' }

# --- the cases ---------------------------------------------------------------
#
# Each row: the twin's name, the arguments as perl values with the tag
# that writes them, and how the answer is written.

my @rows;
sub row {
    my ($name, $args, $answer, $slack) = @_;
    push @rows, "$name " . join(' ', @$args) . ' ' . ($slack ? '~>' : '->') . " $answer";
}

sub strings {
    @rows = ();
    for my $s ('', 'hello', '日本', 'ΣΑΣ', 'σας', 'ß', 'aBc', 'a b') {
        row('length_of', [tag_str($s)], tag_int(length $s));
        row('uc', [tag_str($s)], tag_str(uc $s));
        row('lc', [tag_str($s)], tag_str(lc $s));
        row('ucfirst', [tag_str($s)], tag_str(ucfirst $s));
        row('lcfirst', [tag_str($s)], tag_str(lcfirst $s));
        row('reverse_str', [tag_str($s)], tag_str(scalar reverse $s));
    }
    for my $c (['hello', 0], ['hello', 1], ['hello', 5], ['hello', -2], ['日本語', 1]) {
        row('substr_from', [tag_str($c->[0]), tag_int($c->[1])], tag_str(substr $c->[0], $c->[1]));
    }
    for my $c (['hello', 1, 3], ['hello', 1, -1], ['hello', 0, 0], ['hello', -3, 2], ['日本語', 0, 2]) {
        row('substr_len', [tag_str($c->[0]), tag_int($c->[1]), tag_int($c->[2])],
            tag_str(substr $c->[0], $c->[1], $c->[2]));
    }
    for my $c (['hello', 'l'], ['hello', 'z'], ['hello', ''], ['日本語', '本'], ['banana', 'na']) {
        row('index_of', [tag_str($c->[0]), tag_str($c->[1])], tag_int(index $c->[0], $c->[1]));
        row('rindex_of', [tag_str($c->[0]), tag_str($c->[1])], tag_int(rindex $c->[0], $c->[1]));
    }
    for my $c (['hello', 'l', 3], ['hello', 'l', 4], ['banana', 'na', 3], ['banana', 'na', 0]) {
        row('index_from', [tag_str($c->[0]), tag_str($c->[1]), tag_int($c->[2])],
            tag_int(index $c->[0], $c->[1], $c->[2]));
        row('rindex_from', [tag_str($c->[0]), tag_str($c->[1]), tag_int($c->[2])],
            tag_int(rindex $c->[0], $c->[1], $c->[2]));
    }
    for my $c ([',', ['a', 'b', 'c']], ['', ['a', 'b']], ['-', []], [', ', ['x']]) {
        row('join', [tag_str($c->[0]), tag_list(map { tag_str($_) } @{ $c->[1] })],
            tag_str(join $c->[0], @{ $c->[1] }));
    }
    for my $c ([',', 'a,b,c'], [',', 'a,,b,,'], [',', ''], ['', 'abc'], [',', ',a'], ['--', 'a--b']) {
        my @parts = split /\Q$c->[0]\E/, $c->[1], -1;
        pop @parts while @parts && $parts[-1] eq '';
        @parts = split //, $c->[1] if $c->[0] eq '';
        row('split_on', [tag_str($c->[0]), tag_str($c->[1])], tag_list(map { tag_str($_) } @parts));
    }
    for my $s ('  the quick  brown fox  ', '', 'one', "a\tb\nc") {
        row('split_words', [tag_str($s)], tag_list(map { tag_str($_) } split ' ', $s));
    }
    return join('', map { "$_\n" } @rows);
}

sub numbers {
    @rows = ();
    for my $v (0.0, -1.5, 1.5, 0.1, -0.0, 1e21, 1 / 3) {
        # `int(1e21)` is past a machine word, and perl answers a number
        # with a fraction there; the dialect holds 64 bits and says so.
        row('int_of', [tag_float($v)], tag_int(int $v)) if abs($v) < 2**62;
        row('abs_num', [tag_float($v)], tag_float(abs $v));
        row('num_text', [tag_float($v)], tag_str("$v"));
        row('floor_of', [tag_float($v)], tag_float(floor $v));
        row('ceil_of', [tag_float($v)], tag_float(ceil $v));
    }
    for my $v (0, -7, 7, 9223372036854775807) {
        row('abs_int', [tag_int($v)], tag_int(abs $v));
    }
    for my $v (0.0, 2.0, 25.0, 1e10, 0.5) {
        row('sqrt_of', [tag_float($v)], tag_float(sqrt $v));
    }
    for my $c ([7.0, 3.0], [-7.0, 3.0], [7.0, -3.0], [7.5, 2.5]) {
        row('fmod_of', [tag_float($c->[0]), tag_float($c->[1])], tag_float(fmod($c->[0], $c->[1])));
    }
    for my $c ([-7, 3], [7, -3], [7, 3], [-7, -3], [10, 5]) {
        row('mod_int', [tag_int($c->[0]), tag_int($c->[1])], tag_int($c->[0] % $c->[1]));
        row('div_int', [tag_int($c->[0]), tag_int($c->[1])], tag_float($c->[0] / $c->[1]));
    }
    {
        # perl warns when it reads a number off something that is not
        # one, and answering that is exactly what the twin is for.
        no warnings 'numeric';
        for my $s ('10', '3abc', 'abc', '  12  ', '-2.5', '1e3', '1e', '', '0.', '.5', '0x10') {
            row('num_of', [tag_str($s)], tag_float(0 + $s));
        }
    }
    row('bool_text', [tag_bool(1)], tag_str(!!1 . ''));
    row('bool_text', [tag_bool(0)], tag_str(!!0 . ''));
    return join('', map { "$_\n" } @rows);
}

sub list_util {
    @rows = ();
    for my $xs ([1, 2, 3], [-1, 1], [5], [3, 1, 2]) {
        row('sum_int', [tag_list(map { tag_int($_) } @$xs)], tag_int(sum @$xs));
        row('max_int', [tag_list(map { tag_int($_) } @$xs)], tag_int(max @$xs));
        row('min_int', [tag_list(map { tag_int($_) } @$xs)], tag_int(min @$xs));
        row('uniq_int', [tag_list(map { tag_int($_) } @$xs)], tag_list(map { tag_int($_) } uniq @$xs));
    }
    for my $xs ([0.1, 0.2], [1.5], [1.5, -2.5, 0.5]) {
        row('sum_num', [tag_list(map { tag_float($_) } @$xs)], tag_float(sum @$xs));
        row('max_num', [tag_list(map { tag_float($_) } @$xs)], tag_float(max @$xs));
        row('min_num', [tag_list(map { tag_float($_) } @$xs)], tag_float(min @$xs));
    }
    for my $xs ([qw(a b a c)], [qw(x)], [qw(ivy momo ivy ada)]) {
        row('uniq_str', [tag_list(map { tag_str($_) } @$xs)], tag_list(map { tag_str($_) } uniq @$xs));
    }
    return join('', map { "$_\n" } @rows);
}

sub sprintf_table {
    @rows = ();
    for my $c (['%.1f', 5.0], ['%.2f', 0.255], ['total %.2f', 12.345], ['%.0f', 2.5], ['%6.2f', 1.5],
               ['%-6.2f|', 1.5], ['%+.1f', 1.25], ['%e', 1234.5], ['%.3e', 0.000123], ['%g', 0.0001],
               ['%g', 100000.0], ['%s', 0.1], ['%08.3f', -1.5]) {
        row('fmt_num', [tag_str($c->[0]), tag_float($c->[1])], tag_str(sprintf $c->[0], $c->[1]));
    }
    for my $c (['%d items', 4], ['%04d', 7], ['%x', 255], ['%X', 255], ['%o', 8], ['%b', 5],
               ['100%% of %d', 3], ['%5d|', -42], ['%-5d|', -42], ['%+d', 42], ['%s', 42]) {
        row('fmt_int', [tag_str($c->[0]), tag_int($c->[1])], tag_str(sprintf $c->[0], $c->[1]));
    }
    for my $c (['[%s]', 'hi'], ['%10s|', 'hi'], ['%-10s|', 'hi'], ['%.2s', 'hello'], ['%s', ''],
               ['%s', '日本']) {
        row('fmt_str', [tag_str($c->[0]), tag_str($c->[1])], tag_str(sprintf $c->[0], $c->[1]));
    }
    return join('', map { "$_\n" } @rows);
}

sub time_table {
    @rows = ();
    my @epochs = (0, 1_700_000_000, 951_782_400, -1, 2_147_483_647);
    my @formats = ('%Y-%m-%d %H:%M:%S UTC', '%F %T', '%Y/%j', '%y%m%d', '%A %B %e', '%a %b', '100%%');
    for my $t (@epochs) {
        for my $f (@formats) {
            row('strftime_utc', [tag_str($f), tag_int($t)], tag_str(strftime($f, gmtime($t))));
        }
    }
    return join('', map { "$_\n" } @rows);
}

# Regular expressions. perl's own engine cannot be lifted out of the
# interpreter, so the compiled run runs PCRE2; these rows are where
# the two have to agree. Each case is real perl, run by perl.
sub regexp {
    @rows = ();
    my @cases = (
        ['\\d+', ''],
        ['(\\d+)', ''],
        ['[a-z]+', 'i'],
        ['^a', ''],
        ['b$', ''],
        ['(?<num>\\d+)', ''],
        ['\\s+', ''],
        ['a|b', ''],
        ['(\\w)(\\d)', ''],
        ['x*', ''],
        ['\\bfox\\b', ''],
        ['(\\d+)-(\\d+)', ''],
    );
    my @inputs = ('a1b22c333', 'ABCdef', 'abc', 'x42y', 'a  b   c', 'the quick fox', '12-34', '', '日本1');
    for my $c (@cases) {
        my ($pat, $flags) = @$c;
        for my $s (@inputs) {
            my $ok = perl_re("\$s =~ /$pat/$flags ? 1 : 0", $s);
            row('re_matches', [tag_str($pat), tag_str($flags), tag_str($s)], tag_bool($ok));
            for my $n (0, 1, 2) {
                my $got = perl_re("\$s =~ /$pat/$flags; defined \$-[$n] ? substr(\$s, \$-[$n], \$+[$n] - \$-[$n]) : ''", $s);
                row('re_capture', [tag_str($pat), tag_str($flags), tag_str($s), tag_int($n)], tag_str($got));
            }
            my @parts = @{ perl_re("[ split /$pat/$flags, \$s ]", $s) };
            row('re_split', [tag_str($pat), tag_str($flags), tag_str($s)], tag_list(map { tag_str($_) } @parts));
            my @all = @{ perl_re("[ \$s =~ /$pat/${flags}g ]", $s) };
            row('re_all', [tag_str($pat), tag_str($flags), tag_str($s)], tag_list(map { tag_str($_ // '') } @all));
            my $n = perl_re("scalar(() = \$s =~ /$pat/${flags}g)", $s);
            row('re_count', [tag_str($pat), tag_str($flags), tag_str($s)], tag_int($n));
        }
    }
    my @named = ('x42y', 'no digits');
    for my $s (@named) {
        my $got = perl_re("\$s =~ /(?<num>\\d+)/; \$+{num} // ''", $s);
        row('re_capture_named', [tag_str('(?<num>\\d+)'), tag_str(''), tag_str($s), tag_str('num')], tag_str($got));
    }
    for my $c (['(\\d+)', '', 'a1b22', '[$1]'], ['(\\d+)', 'g', 'a1b22', '[$1]'],
               ['\\s+', 'g', 'a  b c', '-'], ['x*', 'g', 'abc', '-'],
               ['(\\w)(\\d)', 'g', 'a1 b2', '$2$1'], ['o', 'g', 'foo', '0'],
               ['[aeiou]', 'gi', 'Hello There', '_']) {
        my ($pat, $flags, $s, $repl) = @$c;
        my $got = perl_re("my \$t = \$s; \$t =~ s/$pat/$repl/$flags; \$t", $s);
        row('re_subst', [tag_str($pat), tag_str($flags), tag_str($s), tag_str($repl)], tag_str($got));
    }
    return join('', map { "$_\n" } @rows);
}

# One case, run by perl itself. This file is a tool, so a string eval
# here is perl doing the work rather than this file guessing at it.
sub perl_re {
    my ($code, $s) = @_;
    my $out = eval "no warnings; my \$s = \$_[1]; $code";
    die "perl could not run `$code`: $@" if $@;
    return $out;
}

# --- write, or check ---------------------------------------------------------

my $banner = "# perl $^V — printed by rakugan/tools/gen_expected.pl, not by hand.\n";
my %TABLES = (
    'strings.txt'   => $banner . strings(),
    'numbers.txt'   => $banner . numbers(),
    'list_util.txt' => $banner . list_util(),
    'sprintf.txt'   => $banner . sprintf_table(),
    'time.txt'      => $banner . time_table(),
    'regexp.txt'    => $banner . regexp(),
);

my $check = grep { $_ eq '--check' } @ARGV;
make_path($OUT) unless $check;
my @stale;
for my $name (sort keys %TABLES) {
    my $path = "$OUT/$name";
    if ($check) {
        my $on_disk = -f $path ? do { open my $fh, '<:encoding(UTF-8)', $path or die; local $/; <$fh> } : undef;
        push @stale, $name unless defined $on_disk && $on_disk eq $TABLES{$name};
    } else {
        open my $fh, '>:encoding(UTF-8)', $path or die "$path: $!\n";
        print {$fh} $TABLES{$name};
        close $fh;
        my $n = () = $TABLES{$name} =~ /\n/g;
        print "wrote crates/rakugan-stdlib/tests/expected/$name (", $n - 1, " rows)\n";
    }
}
if ($check && @stale) {
    warn "gen_expected: these are not what perl says now — run rakugan/tools/gen_expected.pl\n";
    warn "  $_\n" for @stale;
    exit 1;
}
