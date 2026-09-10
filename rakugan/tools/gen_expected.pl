#!/usr/bin/env perl
# What perl answers, printed by perl.
#
# Pinned to the C locale, because `strftime`'s %A/%a/%B/%b read
# LC_TIME and the twin reads it too: a table printed on a Japanese
# machine would say 木曜日 and be false everywhere else. The table is a
# claim about the C locale, and the test that reads it pins the same.
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
use List::Util qw(sum max min uniq shuffle maxstr minstr);
use POSIX qw(floor ceil fmod strftime);
use builtin qw(trim);
use Encode qw(encode);
use File::Basename qw(dirname);
use File::Path qw(make_path);
use File::Spec;

# See the note above: the table is a claim about the C locale. Setting
# %ENV here would be too late — perl fixes its locale before this file
# runs — so the locale itself is set, which is what strftime reads.
POSIX::setlocale(POSIX::LC_ALL(), 'C');

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
    # An offset that counts past the start clamps to it; one past the end
    # answers undef, which the twin writes as "" (perl warns there).
    for my $c (['hello', 0], ['hello', 1], ['hello', 5], ['hello', -2], ['日本語', 1], ['hello', -10], ['hello', 6]) {
        no warnings 'substr';
        row('substr_from', [tag_str($c->[0]), tag_int($c->[1])], tag_str(substr($c->[0], $c->[1]) // ''));
    }
    for my $c (['hello', 1, 3], ['hello', 1, -1], ['hello', 0, 0], ['hello', -3, 2], ['日本語', 0, 2],
               ['hello', -10, 7], ['hello', -10, -1], ['hello', -10, 2], ['hello', 2, -10], ['hello', 6, 1]) {
        no warnings 'substr';
        row('substr_len', [tag_str($c->[0]), tag_int($c->[1]), tag_int($c->[2])],
            tag_str(substr($c->[0], $c->[1], $c->[2]) // ''));
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
    {
        # A negative repeat count warns and does nothing, and chr of a
        # negative number warns and answers U+FFFD; both are what the
        # twin is asked for.
        no warnings;
        for my $c (['ab', 3], ['ab', 0], ['ab', -1], ['日本', 2], ['', 3], ['ß', 1]) {
            row('repeat_str', [tag_str($c->[0]), tag_int($c->[1])], tag_str($c->[0] x $c->[1]));
        }
        for my $n (65, 0, 223, 255, 26085, 0x1F600, -1) {
            row('chr_of', [tag_int($n)], tag_str(chr $n));
        }
    }
    for my $s ('', 'a', '日本', 'ß', "\n", '😀') {
        row('ord_of', [tag_str($s)], tag_int(ord $s));
    }
    for my $s ("a\n", "a\n\n", "a\r\n", '', 'abc', "\n", "日本\n", "a\r") {
        my $t = $s;
        chomp $t;
        row('chomp_str', [tag_str($s)], tag_str($t));
    }
    for my $s ('日本', '', "a\n", 'abc', "\r\n", 'ß') {
        my $t = $s;
        chop $t;
        row('chop_str', [tag_str($s)], tag_str($t));
    }
    for my $c ([',', [1, 2, 3]], ['-', [-1, 0, 9223372036854775807, -9223372036854775808]], [',', []], [', ', [5]]) {
        row('join_int', [tag_str($c->[0]), tag_list(map { tag_int($_) } @{ $c->[1] })],
            tag_str(join $c->[0], @{ $c->[1] }));
    }
    for my $c ([', ', [0.1, 1e21, -0.0, 1 / 3]], [' ', [2.0, 1e15, 1e16, 0.1 + 0.2]], ['', [1.5]], [',', []]) {
        row('join_num', [tag_str($c->[0]), tag_list(map { tag_float($_) } @{ $c->[1] })],
            tag_str(join $c->[0], @{ $c->[1] }));
    }
    # The four-argument substr, as the string after. perl's rules for a
    # negative offset or length have corners, and these are them; the
    # cases that die are held in the crate's own tests.
    for my $c (['hello', 1, 3, 'X'], ['hello', 0, 0, 'X'], ['hello', 5, 0, 'X'], ['hello', 5, 3, 'X'],
               ['hello', -2, 1, 'X'], ['hello', -2, -1, 'X'], ['hello', 1, -1, 'X'], ['hello', 2, 10, 'X'],
               ['hello', -10, 7, 'X'], ['hello', -10, -1, 'X'], ['hello', 2, -10, 'X'], ['hello', -3, -4, 'X'],
               ['hello', 0, 5, ''], ['abc', -5, 5, 'X'], ['abc', -5, 2, 'X'], ['abc', -5, 3, 'X'],
               ['abc', -4, -1, 'X'], ['abc', -4, -3, 'X'], ['abc', 3, -1, 'X'], ['', 0, 0, 'X'],
               ['', -1, 1, 'X'], ['', 0, -1, 'X'], ['日本語', 1, 1, 'ß'], ['abc', 0, 0, 'ßß']) {
        my ($s, $off, $len, $repl) = @$c;
        my $t = $s;
        substr($t, $off, $len, $repl);
        row('substr_replace', [tag_str($s), tag_int($off), tag_int($len), tag_str($repl)], tag_str($t));
    }
    for my $s ("\x{3000}x\x{3000}", "\x{85}x\x{85}", "\t x \n", '', '   ', 'x', "\x{a0}\x{2028}x\x{200b}y\x{feff}",
               "\x{180e}x\x{180e}", "\x0bx\x0c", ' 日本 ', "\x1cx\x1f") {
        row('trim_str', [tag_str($s)], tag_str(trim $s));
    }
    # tr///, as a value. FROM and TO are the text between the delimiters,
    # as the translator hands them over: ranges and escapes still in.
    for my $c (['hello', 'a-y', 'b-z', ''], ['hello', 'lo', '0', ''], ['hello', 'lo', '0', 'd'],
               ['hello', 'l', '', ''], ['hello', 'l', '', 'd'], ['hello', 'a-z', '', 's'],
               ['aabbccdd', 'a-c', 'x', 's'], ['aabbaa', 'ab', 'xy', 's'], ['aXXbb', 'ab', 'x', 's'],
               ['a-b', '\\-', '_', ''], ['a-b', 'a-', '_', ''], ["a\nb\tc", '\\n\\t', '  ', ''],
               ["a\\b", '\\\\', '\\/', ''], ['a/b', '\\/', '-', ''], ['日本語', '日', '月', ''], ['hello', 'lhz', 'xy', ''],
               ['aabb', 'aa', 'xy', ''], ['hello', 'olleh', '12345', ''], ['aXXaXXa', 'aX', 'b', 'ds'],
               ['aXXa', 'X', 'a', 'ds'], ['xaXXaXXaz', 'X', '', 'ds'], ['aQa', 'a', 'b', 's'],
               ['abc', 'abc', 'xyzw', ''], ['日日本', '日', '-', 's'], ['abc', 'a-c', '日-本', ''],
               ['ab-', 'a\\-b', 'xyz', ''], ['abc', 'a-c', '\\n\\t\\\\', ''], ['HeLLo World', 'A-Z', 'a-z', ''],
               ['hello', 'a-z', 'A-C', 'd'], ['', 'a-z', 'A-Z', ''], ['hello', 'a-c', '', 'd']) {
        my ($s, $from, $to, $flags) = @$c;
        row('tr_str', [tag_str($s), tag_str($from), tag_str($to), tag_str($flags)],
            tag_str(perl_tr($s, "tr/$from/$to/${flags}r")));
    }
    for my $c (['hello', 'lo'], ['hello', 'a-z'], ['aaa', 'a'], ['日本', 'a-z'], ['', 'a'], ["a\nb\n", '\\n'],
               ['a-b-c', '\\-']) {
        row('tr_count', [tag_str($c->[0]), tag_str($c->[1])], tag_int(perl_tr($c->[0], "tr/$c->[1]//")));
    }
    return join('', map { "$_\n" } @rows);
}

# One tr///, run by perl itself on a copy of the string, the way perl_re
# below runs a match.
sub perl_tr {
    my ($s, $code) = @_;
    my $out = eval "no warnings; my \$t = \$_[0]; \$t =~ $code";
    die "perl could not run `$code`: $@" if $@;
    return $out;
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
    for my $c ([7.5, 2.5], [-7.0, 2.0], [1.0, 3.0], [0.1, 0.3], [1e300, 1e-10]) {
        row('div_num', [tag_float($c->[0]), tag_float($c->[1])], tag_float($c->[0] / $c->[1]));
    }
    # What `die` hands a `catch`: the place is appended unless the text
    # ends its own line. perl names its own place here, so the row hands
    # the twin the suffix perl used and asks for the same text back.
    for my $m ('boom', "boom\n", 'two words', '') {
        eval { die $m };
        my $got = $@;
        my $said = length $m ? $m : 'Died';
        my $at = $m =~ /\n\z/ ? " at nowhere.pl line 1.\n" : substr($got, length $said);
        row('die_text', [tag_str($m), tag_str($at)], tag_str($got));
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
    # `**` on two whole numbers. perl computes in floating point whenever
    # the base is a power of two or the answer might not fit 64 bits, and
    # keeps the answer as a fraction — `2 ** 50` prints as
    # 1.12589990684262e+15. The twin answers a whole number, so the rows
    # are the ones perl prints as digits, and a case that is not is a
    # mistake in this list rather than a row.
    for my $c ([2, 10], [-2, 3], [0, 0], [7, 0], [-3, 3], [3, 20], [10, 15], [10, 16], [7, 21], [-7, 21],
               [2, 49], [-1, 7], [1, 200], [0, 5]) {
        my $r = $c->[0] ** $c->[1];
        die "pow_int: perl writes $c->[0] ** $c->[1] as $r, which is not digits\n" unless $r =~ /\A-?\d+\z/;
        row('pow_int', [tag_int($c->[0]), tag_int($c->[1])], tag_int($r));
    }
    # `**` with a fraction on either side is C's pow, so an ulp is allowed;
    # Inf and NaN are what perl answers for 0 ** -1 and (-8) ** (1/3).
    for my $c ([0.0, -1.0], [-8.0, 1 / 3], [2.0, 0.5], [2.5, 2.0], [-2.0, -1.0], [-2.0, 2.0], [10.0, -2.0],
               [0.0, 0.0], [1e300, 2.0]) {
        row('pow_num', [tag_float($c->[0]), tag_float($c->[1])], tag_float($c->[0] ** $c->[1]), 1);
    }
    {
        # hex and oct warn at the first character that is not a digit and
        # answer what they have, and that answer is what the twin is for.
        # hex skips no leading space; oct does. An underscore is skipped
        # when a digit follows it.
        no warnings;
        for my $s ('ff', '0xff', '0Xff', 'xff', 'FF', '', '1_000', '1__0', '1_', '_1', 'ffg', '0b101', ' ff',
                   '-ff', '0x', '7fffffffffffffff', 'ffÿ', '0') {
            row('hex_of', [tag_str($s)], tag_int(hex $s));
        }
        for my $s ('755', '0755', '00755', '0x1f', '0X1F', 'x1f', '0b101', 'b101', '0o17', 'o17', '', '789',
                   ' 755', "\t0x1f", '-7', '7_7', '0_7', '0b', '1_000_000', '777777777777777777777', '12 3',
                   '0 12', '0o0o7', '8') {
            row('oct_of', [tag_str($s)], tag_int(oct $s));
        }
    }
    # The libm functions: the platform decides the last bit, so an ulp.
    for my $v (0.0, 1.0, -2.5, 1e10, 3.141592653589793) {
        row('sin_of', [tag_float($v)], tag_float(sin $v), 1);
        row('cos_of', [tag_float($v)], tag_float(cos $v), 1);
    }
    for my $v (0.0, 1.0, -1.0, 710.0, -1000.0, 0.5) {
        row('exp_of', [tag_float($v)], tag_float(exp $v), 1);
    }
    for my $c ([0.0, 0.0], [1.0, 0.0], [-1.0, -1.0], [0.0, -1.0], [-0.0, -1.0], [1.0, 1.0], [0.0, -0.0]) {
        row('atan2_of', [tag_float($c->[0]), tag_float($c->[1])], tag_float(atan2($c->[0], $c->[1])), 1);
    }
    for my $v (1.0, 10.0, 0.5, 2.718281828459045, 1e-320, 1e300) {
        row('log_of', [tag_float($v)], tag_float(log $v), 1);
    }
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
    for my $xs ([qw(apple pear fig)], ['日本', 'z', 'ß'], ['10', '9'], ['B', 'a'], ['x'], ['', 'a'], ['b', 'a', 'b']) {
        row('max_str', [tag_list(map { tag_str($_) } @$xs)], tag_str(maxstr @$xs));
        row('min_str', [tag_list(map { tag_str($_) } @$xs)], tag_str(minstr @$xs));
    }
    {
        # A negative repeat count warns and does nothing, and a splice
        # offset past the end warns and appends; both are what the twin
        # is asked for. The splices that die are held in the crate's own
        # tests.
        no warnings;
        for my $c ([[1, 2], 2], [[1, 2], 0], [[1, 2], -1], [[], 3], [[7], 3]) {
            row('repeat_int', [tag_list(map { tag_int($_) } @{ $c->[0] }), tag_int($c->[1])],
                tag_list(map { tag_int($_) } (@{ $c->[0] }) x $c->[1]));
        }
        for my $c ([[0.5, 1.5], 2], [[0.1], 0], [[0.1], 3]) {
            row('repeat_num', [tag_list(map { tag_float($_) } @{ $c->[0] }), tag_int($c->[1])],
                tag_list(map { tag_float($_) } (@{ $c->[0] }) x $c->[1]));
        }
        for my $c ([['a', 'b'], 2], [['日本'], 3], [['a'], 0], [[], 2]) {
            row('repeat_strs', [tag_list(map { tag_str($_) } @{ $c->[0] }), tag_int($c->[1])],
                tag_list(map { tag_str($_) } (@{ $c->[0] }) x $c->[1]));
        }
        for my $c ([[1, 2, 3, 4, 5], 1, 2, []], [[1, 2, 3, 4, 5], -2, 1, [9]], [[1, 2, 3, 4, 5], 1, -1, [7, 8]],
                   [[1, 2, 3, 4, 5], 10, 0, [6]], [[1, 2, 3], 0, 0, [0]], [[1, 2, 3], 3, 0, [4]],
                   [[1, 2, 3], 1, 10, []], [[1, 2, 3], -1, -1, [9]], [[1, 2, 3], 2, -5, [9]], [[], 0, 0, [1]],
                   [[], 0, 1, []], [[1, 2, 3], -3, 3, []], [[1, 2, 3], 1, 0, [8, 9]], [[1, 2, 3], -1, 5, [9]],
                   [[], 2, 0, [1]], [[1, 2, 3], 0, -3, [9]], [[1, 2, 3], 5, -1, [9]]) {
            my ($xs, $off, $len, $repl) = @$c;
            my @t = @$xs;
            splice @t, $off, $len, @$repl;
            row('splice_int', [tag_list(map { tag_int($_) } @$xs), tag_int($off), tag_int($len),
                               tag_list(map { tag_int($_) } @$repl)], tag_list(map { tag_int($_) } @t));
        }
        for my $c ([[0.5, 1.5, 2.5], 1, 1, [9.5]], [[0.5, 1.5], -1, 1, []], [[0.5], 5, 0, [1.5, 2.5]]) {
            my ($xs, $off, $len, $repl) = @$c;
            my @t = @$xs;
            splice @t, $off, $len, @$repl;
            row('splice_num', [tag_list(map { tag_float($_) } @$xs), tag_int($off), tag_int($len),
                               tag_list(map { tag_float($_) } @$repl)], tag_list(map { tag_float($_) } @t));
        }
        for my $c ([[qw(a b c)], 1, 1, ['日本']], [[qw(a b c)], -1, 0, ['x', 'y']], [['ß'], 0, 1, []]) {
            my ($xs, $off, $len, $repl) = @$c;
            my @t = @$xs;
            splice @t, $off, $len, @$repl;
            row('splice_strs', [tag_list(map { tag_str($_) } @$xs), tag_int($off), tag_int($len),
                                tag_list(map { tag_str($_) } @$repl)], tag_list(map { tag_str($_) } @t));
        }
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

# perl's own random numbers, from its own drand48: the sequence after a
# seed, and List::Util's shuffle drawing from the same state.
sub random_table {
    @rows = ();
    # The sequence after a seed: the first values, and `rand(10)` over
    # them. A seed past 32 bits and a negative one show what perl keeps.
    for my $seed (0, 1, 42, 12345, 4294967295, 4294967296, -1, 2**40 + 7) {
        srand($seed);
        my @seq = map { rand() } 1 .. 5;
        row('rand_seq', [tag_int($seed), tag_int(5)], tag_list(map { tag_float($_) } @seq));
        srand($seed);
        my @tens = map { rand(10) } 1 .. 4;
        row('rand_ten', [tag_int($seed), tag_int(4)], tag_list(map { tag_float($_) } @tens));
    }
    for my $seed (7, 2024) {
        srand($seed);
        my @s = shuffle(1 .. 6);
        row('shuffle_int_after', [tag_int($seed), tag_list(map { tag_int($_) } 1 .. 6)], tag_list(map { tag_int($_) } @s));
        srand($seed);
        my @t = shuffle(qw(a b c d e));
        row('shuffle_str_after', [tag_int($seed), tag_list(map { tag_str($_) } qw(a b c d e))], tag_list(map { tag_str($_) } @t));
    }
    return join('', map { "$_\n" } @rows);
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
    'random.txt'    => $banner . random_table(),
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
