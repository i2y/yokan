#!/usr/bin/env perl
# The framework's own standard library, on the substrate's C face.
#
# `crates/yokan-stdlib` is one implementation that both of Yokan's runs
# call. Rakugan's compiled run reaches it the same way Yokan's does,
# through the `.rpi` the project carries. Its interpreted run reaches it
# through the C face, which is what this file generates: a small,
# generic calling convention (push the arguments, name the row, read the
# answer) and one arm per row of the manifest.
#
#   tools/gen_capi.pl           write the files
#   tools/gen_capi.pl --check   fail if what is on disk is not what the
#                               manifest says (the sweep runs this)
#
# The manifest is `crates/yokan-stdlib/stdlib.toml`, which no language
# owns. Only its framework layer crosses here: the rows whose names are
# Python's are twins of CPython, and Perl answers those for itself.
use strict;
use warnings;
use utf8;
use open qw(:std :encoding(UTF-8));
use FindBin;
use File::Spec;

my $ROOT = File::Spec->rel2abs(File::Spec->catdir($FindBin::Bin, '..'));
my $REPO = File::Spec->rel2abs(File::Spec->catdir($ROOT, '..'));

# --- the manifest, in the TOML subset this repository writes -----------------

sub toml_str { my ($s) = @_; $s =~ s/\\"/"/g; $s =~ s/\\\\/\\/g; return $s }

sub parse_manifest {
    my ($text) = @_;
    my (@groups, $cur, $in_rows);
    for my $raw (split /\n/, $text) {
        (my $line = $raw) =~ s/\A\s+|\s+\z//g;
        next if $line eq '' || $line =~ /\A#/;
        if ($in_rows) {
            if ($line =~ /\A\]/) { $in_rows = 0; next }
            my %row;
            while ($line =~ /(\w+)\s*=\s*"((?:[^"\\]|\\.)*)"/g) { $row{$1} = toml_str($2) }
            push @{ $cur->{rows} }, \%row;
            next;
        }
        if ($line eq '[[group]]') { $cur = { rows => [] }; push @groups, $cur; next }
        if ($line eq 'rows = [') { $in_rows = 1; next }
        $cur->{$1} = toml_str($2) if $line =~ /\A(\w+)\s*=\s*"((?:[^"\\]|\\.)*)"/;
    }
    return @groups;
}

open my $fh, '<:encoding(UTF-8)', "$REPO/crates/yokan-stdlib/stdlib.toml" or die "$!\n";
my @ALL = parse_manifest(do { local $/; <$fh> });
my @GROUPS = grep { ($_->{layer} // '') eq 'yokan' } @ALL;
close $fh;

# --- the rows that can cross -------------------------------------------------
#
# A row crosses when every one of its parameters and its answer is a
# shape the face carries. The one that is not — a map of headers — waits
# for a reason to carry one.

my %ARG_KIND = ('String' => 'str', 'Int' => 'int', 'Float' => 'num', 'List<String>' => 'list');
my %RET_KIND = ('String' => 'str', 'Int' => 'int', 'Bool' => 'bool', 'Float' => 'num',
                'List<String>' => 'strs', 'List<List<String>>' => 'rows', '' => 'void');

my (@rows, %skipped);
for my $g (@GROUPS) {
    for my $r (@{ $g->{rows} }) {
        next unless defined $r->{rust};
        my @params = grep { length } split /,\s*/, $r->{params} // '';
        my @kinds = map { my ($n, $t) = split /:\s*/, $_, 2; $ARG_KIND{ $t // '' } } @params;
        my $ret = $RET_KIND{ $r->{ret} // '' };
        if (grep { !defined } @kinds or !defined $ret) {
            $skipped{"$g->{module}.$r->{py}"} = $r->{params} . ' -> ' . $r->{ret} if defined $r->{py};
            next;
        }
        push @rows, {
            id     => scalar(@rows) + 1,
            module => $g->{module},
            class  => $g->{class},
            py     => $r->{py},
            fn     => $r->{fn},
            rust   => $r->{rust},
            ret    => $ret,
            ret_ty => $r->{ret},
            kinds  => \@kinds,
            sig    => $r->{params} // '',
            names  => [map { (split /:\s*/, $_, 2)[0] } @params],
            pure   => (($r->{flags} // '') =~ /\bpure\b/) ? 1 : 0,
        };
    }
}

my $banner = "// Generated from crates/yokan-stdlib/stdlib.toml by rakugan/tools/gen_capi.pl.\n"
           . "// Do not edit by hand; edit the manifest and run the generator.\n";

# --- crates/pixie-capi/src/stdlib.rs -----------------------------------------

sub gen_rust {
    my $out = $banner . <<'HEAD';
//! The framework's own standard library, reachable from a door written
//! in another language.
//!
//! `yokan-stdlib` is one implementation that a translated language's
//! COMPILED run links through pixie's binding door. Its interpreted run
//! has to land on the same code or the gate compares two libraries, so
//! this is the same functions through the C face.
//!
//! The convention is generic, the way the element builder's is: the
//! caller pushes the arguments, names the row by number, and reads the
//! answer back. Adding a function to the manifest therefore adds an arm
//! here and nothing to the ABI.

use std::cell::{Cell, RefCell};
use std::ffi::{CStr, c_char};

enum Arg {
    S(String),
    I(i64),
    N(f64),
    L(Vec<String>),
}

thread_local! {
    static ARGS: RefCell<Vec<Arg>> = const { RefCell::new(Vec::new()) };
    static BUILDING: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
    /// What the last call answered, when it answered text: one row of
    /// one cell for a string, one row per element for a list, and the
    /// rows themselves for a query.
    static ROWS: RefCell<Vec<Vec<String>>> = const { RefCell::new(Vec::new()) };
    static NUM: Cell<f64> = const { Cell::new(0.0) };
}

/// # Safety
/// `v` is NULL or NUL-terminated.
unsafe fn text(v: *const c_char) -> String {
    if v.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(v) }.to_string_lossy().into_owned()
}

fn push(a: Arg) {
    BUILDING.with(|b| {
        let mut b = b.borrow_mut();
        match (&mut *b, a) {
            (Some(list), Arg::S(s)) => list.push(s),
            (_, a) => ARGS.with(|args| args.borrow_mut().push(a)),
        }
    });
}

/// Start a call: the arguments pushed after this one are its own.
#[unsafe(no_mangle)]
pub extern "C" fn pixie_std_reset() {
    ARGS.with(|a| a.borrow_mut().clear());
    BUILDING.with(|b| *b.borrow_mut() = None);
    ROWS.with(|r| r.borrow_mut().clear());
}

/// # Safety
/// `v` is NULL or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pixie_std_arg_str(v: *const c_char) {
    push(Arg::S(unsafe { text(v) }));
}

#[unsafe(no_mangle)]
pub extern "C" fn pixie_std_arg_int(v: i64) {
    push(Arg::I(v));
}

#[unsafe(no_mangle)]
pub extern "C" fn pixie_std_arg_num(v: f64) {
    push(Arg::N(v));
}

/// The strings pushed until the matching end are one list argument.
#[unsafe(no_mangle)]
pub extern "C" fn pixie_std_arg_list_begin() {
    BUILDING.with(|b| *b.borrow_mut() = Some(Vec::new()));
}

#[unsafe(no_mangle)]
pub extern "C" fn pixie_std_arg_list_end() {
    let list = BUILDING.with(|b| b.borrow_mut().take()).unwrap_or_default();
    ARGS.with(|a| a.borrow_mut().push(Arg::L(list)));
}

/// How many rows the last call's answer holds.
#[unsafe(no_mangle)]
pub extern "C" fn pixie_std_rows() -> i64 {
    ROWS.with(|r| r.borrow().len() as i64)
}

/// How many cells one of those rows holds.
#[unsafe(no_mangle)]
pub extern "C" fn pixie_std_cells(row: i64) -> i64 {
    ROWS.with(|r| r.borrow().get(row as usize).map_or(0, |c| c.len() as i64))
}

/// Put one cell where the character reader can pick it up.
#[unsafe(no_mangle)]
pub extern "C" fn pixie_std_pick(row: i64, col: i64) {
    let cell = ROWS.with(|r| {
        r.borrow()
            .get(row as usize)
            .and_then(|c| c.get(col as usize))
            .cloned()
            .unwrap_or_default()
    });
    crate::answer_with(&cell);
}

/// What the last call answered when it answered a number with a
/// fraction.
#[unsafe(no_mangle)]
pub extern "C" fn pixie_std_answer_num() -> f64 {
    NUM.with(|n| n.get())
}

fn s(args: &[Arg], i: usize) -> &str {
    match args.get(i) {
        Some(Arg::S(v)) => v,
        _ => panic!("the standard library was given the wrong shape of argument {i}"),
    }
}

fn i(args: &[Arg], i: usize) -> i64 {
    match args.get(i) {
        Some(Arg::I(v)) => *v,
        _ => panic!("the standard library was given the wrong shape of argument {i}"),
    }
}

fn n(args: &[Arg], i: usize) -> f64 {
    match args.get(i) {
        Some(Arg::N(v)) => *v,
        Some(Arg::I(v)) => *v as f64,
        _ => panic!("the standard library was given the wrong shape of argument {i}"),
    }
}

fn l(args: &[Arg], i: usize) -> Vec<String> {
    match args.get(i) {
        Some(Arg::L(v)) => v.clone(),
        _ => panic!("the standard library was given the wrong shape of argument {i}"),
    }
}

fn text_answer(v: String) -> i64 {
    ROWS.with(|r| *r.borrow_mut() = vec![vec![v]]);
    0
}

fn list_answer(v: Vec<String>) -> i64 {
    ROWS.with(|r| *r.borrow_mut() = v.into_iter().map(|s| vec![s]).collect());
    0
}

fn rows_answer(v: Vec<Vec<String>>) -> i64 {
    ROWS.with(|r| *r.borrow_mut() = v);
    0
}

fn num_answer(v: f64) -> i64 {
    NUM.with(|n| n.set(v));
    0
}

/// One row of the manifest, by the number the generator gave it.
#[unsafe(no_mangle)]
pub extern "C" fn pixie_std_call(id: i32) -> i64 {
    let args = ARGS.with(|a| a.take());
    call(id, &args)
}

HEAD
    $out .= "fn call(id: i32, args: &[Arg]) -> i64 {\n    match id {\n";
    for my $r (@rows) {
        my @get = map { my $k = $r->{kinds}[$_];
                        $k eq 'str'  ? "s(args, $_)"
                      : $k eq 'int'  ? "i(args, $_)"
                      : $k eq 'num'  ? "n(args, $_)"
                      :                "l(args, $_)" } 0 .. $#{ $r->{kinds} };
        my $callee = "yokan_stdlib::$r->{rust}(" . join(', ', @get) . ')';
        my $arm = $r->{ret} eq 'int'  ? $callee
                : $r->{ret} eq 'bool' ? "($callee) as i64"
                : $r->{ret} eq 'num'  ? "num_answer($callee)"
                : $r->{ret} eq 'str'  ? "text_answer($callee)"
                : $r->{ret} eq 'strs' ? "list_answer($callee)"
                : $r->{ret} eq 'rows' ? "rows_answer($callee)"
                :                       "{ $callee; 0 }";
        $out .= "        $r->{id} => $arm,  // $r->{module}.$r->{py}\n" if defined $r->{py};
        $out .= "        $r->{id} => $arm,  // $r->{class}.$r->{fn}\n" unless defined $r->{py};
    }
    $out .= "        other => panic!(\"the standard library has no row {other}\"),\n    }\n}\n";
    return $out;
}

# --- rakugan/lib/Rakugan/Stdlib.pm -------------------------------------------

sub q_str { my ($s) = @_; $s =~ s/([\\'])/\\$1/g; return "'$s'" }

# The rows as data, in a file the tooling's perl can read: the
# translator runs under whichever perl is first on the path, and only
# the app's perl has the door.
sub gen_manifest {
    my $out = "package Rakugan::Manifest;\n"
            . "# Generated from crates/yokan-stdlib/stdlib.toml by tools/gen_capi.pl.\n"
            . "# Do not edit by hand; edit the manifest and run the generator.\n" . <<'HEAD';
#
# The framework's own standard library as data: one row per function
# that both runs land on. The translator reads this to write the
# `.pix`; Rakugan::Stdlib reads the same rows to make the call through
# the C face. Plain data, and nothing newer than perl 5.34 is written
# here.
use strict;
use warnings;

HEAD
    $out .= "our \@ROWS = (\n";
    for my $r (@rows) {
        my @f = ("id => $r->{id}", 'module => ' . q_str($r->{module}), 'name => ' . q_str($r->{py} // $r->{fn}),
                 'class => ' . q_str($r->{class}), 'fn => ' . q_str($r->{fn}),
                 'ret => ' . q_str($r->{ret}), 'ret_ty => ' . q_str($r->{ret_ty}),
                 'sig => ' . q_str($r->{sig}),
                 'kinds => [' . join(', ', map { q_str($_) } @{ $r->{kinds} }) . ']');
        push @f, 'pure => 1' if $r->{pure};
        $out .= "    { " . join(', ', @f) . " },\n";
    }
    $out .= ");\n\n";
    $out .= <<'BODY';
# By the name an app writes and how many values it wrote: two rows of
# one name are two arities, the way `sqlite_exec` takes a statement
# with or without values to bind.
our %BY_CALL = map { ("$_->{module}_$_->{name}/" . scalar @{ $_->{kinds} }) => $_ } @ROWS;
our %NAMES = map { ("$_->{module}_$_->{name}" => 1) } @ROWS;

1;
BODY
    return $out;
}

sub gen_perl {
    my $out = "package Rakugan::Stdlib;\n"
            . "# Generated from crates/yokan-stdlib/stdlib.toml by tools/gen_capi.pl.\n"
            . "# Do not edit by hand; edit the manifest and run the generator.\n" . <<'BODY';
#
# The framework's own standard library, as one Perl sub per row. Each
# one pushes its arguments through the C face and reads the answer back
# — the same Rust the compiled run links through the binding door, so
# the two runs cannot be answering different libraries.
#
# The rows themselves are Rakugan::Manifest's, which the translator
# reads too.
use v5.40;
use Rakugan::Door;
use Rakugan::Manifest;

our @ROWS = @Rakugan::Manifest::ROWS;
our %BY_CALL = %Rakugan::Manifest::BY_CALL;
our %NAMES = %Rakugan::Manifest::NAMES;

# One call: the arguments in, the row's number, the answer out.
sub call ($row, @args) {
    Rakugan::Door::std_reset();
    for my $i (0 .. $#args) {
        my $kind = $row->{kinds}[$i];
        if    ($kind eq 'str')  { Rakugan::Door::std_arg_str("$args[$i]") }
        elsif ($kind eq 'int')  { Rakugan::Door::std_arg_int(int $args[$i]) }
        elsif ($kind eq 'num')  { Rakugan::Door::std_arg_num(0 + $args[$i]) }
        else {
            Rakugan::Door::std_arg_list_begin();
            Rakugan::Door::std_arg_str("$_") for @{ $args[$i] };
            Rakugan::Door::std_arg_list_end();
        }
    }
    my $n = Rakugan::Door::std_call($row->{id});
    return $n                                 if $row->{ret} eq 'int';
    return $n != 0 ? true : false             if $row->{ret} eq 'bool';
    return Rakugan::Door::std_answer_num()    if $row->{ret} eq 'num';
    return cell(0, 0)                         if $row->{ret} eq 'str';
    # A list answer comes back as a list, which is what perl hands a
    # caller; a query's rows come back as a list of rows.
    return map { cell($_, 0) } 0 .. Rakugan::Door::std_rows() - 1 if $row->{ret} eq 'strs';
    return map { my $r = $_; [map { cell($r, $_) } 0 .. Rakugan::Door::std_cells($r) - 1] }
           0 .. Rakugan::Door::std_rows() - 1 if $row->{ret} eq 'rows';
    return;
}

sub cell ($row, $col) {
    Rakugan::Door::std_pick($row, $col);
    return Rakugan::Door::answer_text();
}

BODY
    my %seen;
    for my $r (@rows) {
        my $name = "$r->{module}_" . ($r->{py} // $r->{fn});
        next if $seen{$name}++;
        my @sig = grep { $_->{module} eq $r->{module} && ($_->{py} // $_->{fn}) eq ($r->{py} // $r->{fn}) } @rows;
        my $doc = "# `$r->{module}.$r->{py}` in the manifest: " . join(' or ', map {
            '(' . join(', ', @{ $_->{names} }) . ')' } @sig) . ".\n";
        $out .= "\n$doc";
        $out .= "sub $name { Rakugan::Stdlib::dispatch('$name', \@_) }\n";
    }
    $out .= <<'TAIL';

# Which row a call means: the name it was written with, and how many
# values came with it.
sub dispatch ($name, @args) {
    my $row = $BY_CALL{"$name/" . scalar @args};
    unless ($row) {
        my @arities;
        for my $r (@ROWS) {
            push @arities, scalar @{ $r->{kinds} } if "$r->{module}_$r->{name}" eq $name;
        }
        die "$name takes " . join(' or ', sort @arities) . " values, and got " . scalar(@args) . "\n";
    }
    return call($row, @args);
}

our @EXPORT = sort keys %NAMES;

1;
TAIL
    return $out;
}

# --- lib/Rakugan/yokan-stdlib.rpi --------------------------------------------
#
# The binding door the compiled run reaches the same library through.
# Written from the same manifest, so the two runs cannot be reading
# different signatures; the first line is the key pixie invalidates a
# derived binding on, and this one is not derived.

sub gen_rpi {
    my $out = "# pixie-cache-key: version=0.1.0; features=audio; format=61\n"
            . "# Generated by rakugan/tools/gen_capi.pl from crates/yokan-stdlib/stdlib.toml.\n";
    for my $g (@ALL) {
        $out .= "\nclass $g->{class} {\n";
        for my $r (@{ $g->{rows} }) {
            next unless defined $r->{rust};
            my $sig = "  static fn $r->{fn}(" . ($r->{params} // '') . ')';
            my $mark = (($r->{params} // '') =~ /Map</ || ($r->{ret} // '') =~ /Map</) ? 'stdmap:' : '';
            $sig .= " $r->{ret}" if length($r->{ret} // '');
            $out .= qq{$sig \@rust("${mark}yokan_stdlib::$r->{rust}")\n};
            if ((($r->{flags} // '') =~ /\btry\b/)) {
                my $t = 'try' . ucfirst $r->{fn};
                my $sig2 = "  static fn $t(" . ($r->{params} // '') . ") !$r->{ret}";
                $out .= qq{$sig2 \@rust("${mark}yokan_stdlib::$r->{rust}_result")\n};
            }
        }
        $out .= "}\n";
    }
    return $out;
}

# --- write, or check ---------------------------------------------------------

my %FILES = (
    "$REPO/crates/pixie-capi/src/stdlib.rs" => gen_rust(),
    "$ROOT/lib/Rakugan/Manifest.pm"         => gen_manifest(),
    "$ROOT/lib/Rakugan/Stdlib.pm"           => gen_perl(),
    "$ROOT/lib/Rakugan/yokan-stdlib.rpi"    => gen_rpi(),
);

my $check = grep { $_ eq '--check' } @ARGV;
my @stale;
for my $path (sort keys %FILES) {
    if ($check) {
        my $on_disk = -f $path ? do { open my $f, '<:encoding(UTF-8)', $path or die; local $/; <$f> } : undef;
        push @stale, $path unless defined $on_disk && $on_disk eq $FILES{$path};
    } else {
        open my $f, '>:encoding(UTF-8)', $path or die "$path: $!\n";
        print {$f} $FILES{$path};
        close $f;
        (my $rel = $path) =~ s{^\Q$REPO\E/}{};
        print "wrote $rel\n";
    }
}
if (%skipped && !$check) {
    print "not on the face (a shape it does not carry):\n";
    print "  $_: $skipped{$_}\n" for sort keys %skipped;
}
if ($check && @stale) {
    warn "gen_capi: these are not what the manifest says — run rakugan/tools/gen_capi.pl\n";
    for my $path (@stale) {
        (my $rel = $path) =~ s{^\Q$REPO\E/}{};
        warn "  $rel\n";
    }
    exit 1;
}
