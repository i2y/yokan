#!/usr/bin/env perl
# Read the engine's element table (crates/pixie-capi/elements.toml) and
# write the two Perl files that have to agree about it: the table as
# Perl data, which the door's runtime and the translator both read, and
# one sub per element for an app to call. Nothing here is written by
# hand, so an element cannot mean one thing to the interpreted run and
# another to the compiled one.
#
#   tools/gen.pl            write the files
#   tools/gen.pl --check    fail if what is on disk is not what this
#                           would write (the sweep runs this)
#
# The TOML this reads is the TOML we write: array-of-tables headers,
# scalars, and arrays of one-line inline tables — the same subset
# wakakusa/tools/gen.rb speaks. The Rust test beside the table parses
# it with a real parser and fails if a key never reached an arm, so a
# mistake in the reader below cannot pass quietly.
use strict;
use warnings;
use utf8;
use open qw(:std :encoding(UTF-8));
use FindBin;
use File::Spec;

my $ROOT = File::Spec->rel2abs(File::Spec->catdir($FindBin::Bin, '..'));   # rakugan/
my $REPO = File::Spec->rel2abs(File::Spec->catdir($ROOT, '..'));
my $TABLE_PATH = "$REPO/crates/pixie-capi/elements.toml";

# --- the TOML this file speaks ----------------------------------------------

sub toml_value {
    my ($s) = @_;
    return $1 =~ s/\\"/"/gr =~ s/\\\\/\\/gr if $s =~ /\A"(.*)"\z/s;
    return 1 if $s eq 'true';
    return 0 if $s eq 'false';
    return [] if $s eq '[]';
    return $s + 0 if $s =~ /\A-?\d+\z/;
    return $s + 0.0;
}

sub toml_inline {
    my ($s) = @_;
    my %h;
    while ($s =~ /(\w+)\s*=\s*("(?:[^"\\]|\\.)*"|\[\]|[^,}\s]+)/g) {
        $h{$1} = toml_value($2);
    }
    # A boolean has to stay apart from a number when it is written back.
    $h{"_bool_$1"} = 1 while $s =~ /(\w+)\s*=\s*(?:true|false)\b/g;
    return \%h;
}

sub parse_toml {
    my ($text) = @_;
    my (%doc, $cur, $key, $arr);
    for my $raw (split /\n/, $text) {
        (my $line = $raw) =~ s/\A\s+|\s+\z//g;
        next if $line eq '' || $line =~ /\A#/;
        if ($arr) {
            if ($line =~ /\A\]/) { $cur->{$key} = $arr; undef $arr }
            elsif ($line =~ /\A\{/) { push @$arr, toml_inline($line) }
            next;
        }
        if ($line =~ /\A\[\[(\w+)\]\]\z/) {
            $cur = {};
            push @{ $doc{$1} }, $cur;
            next;
        }
        my ($k, $v) = split /=/, $line, 2;
        $k =~ s/\s+\z//;
        $v =~ s/\A\s+//;
        if ($v eq '[') { $key = $k; $arr = [] }
        else {
            $cur->{$k} = toml_value($v);
            $cur->{"_bool_$k"} = 1 if $v =~ /\A(?:true|false)\z/;
        }
    }
    return \%doc;
}

open my $tfh, '<:encoding(UTF-8)', $TABLE_PATH or die "$TABLE_PATH: $!\n";
my $TABLE = parse_toml(do { local $/; <$tfh> });
close $tfh;
my @RIDERS = @{ $TABLE->{rider} };
my @ELEMENTS = @{ $TABLE->{element} };
my @OPS = @{ $TABLE->{op} };

# A drawing command's values, in the order the C face takes them: the
# name an app writes, the name the `.pix` writes, and the type.
for my $op (@OPS) {
    $op->{params} = [map {
        my ($lhs, $rhs) = split /:\s*/, $_, 2;
        my ($name, $pix) = split /=/, $lhs, 2;
        my ($ty, $default) = split /\s*=\s*/, $rhs, 2;
        { name => $name, pix => $pix // $name, type => $ty,
          (defined $default ? (default => $default) : ()) }
    } split /,\s*/, $op->{args}];
}

# --- the numbers ------------------------------------------------------------
# A keyword is numbered once, by its name, however many elements take
# it — the same numbering gen.rb gives, since both read one table in
# one order.
my (%KEY_ID, @KEY_ORDER);
sub key_id {
    my ($name) = @_;
    return $KEY_ID{$name} //= do { push @KEY_ORDER, $name; scalar @KEY_ORDER };
}
key_id($_->{name}) for @RIDERS;
for my $e (@ELEMENTS) { key_id($_->{name}) for @{ $e->{props} } }
my %KIND_ID;
$KIND_ID{ $ELEMENTS[$_]{name} } = $_ + 1 for 0 .. $#ELEMENTS;

my %PAYLOADS = (none => 0, text => 1, bool => 2, int => 3, float => 4);
my @PAYLOAD_ORDER = qw(none text bool int float);
# What a handler of each kind is called with: the .pix type of the
# parameter, and what the payload is called at a .pix call site.
my %PAYLOAD_TYPE = (text => 'String', bool => 'Bool', int => 'Int', float => 'Float');
my %PAYLOAD_NAME = (none => '', text => 'text', bool => 'checked', int => 'index', float => 'value');

# --- the .pix spelling ------------------------------------------------------

sub camel { my ($s) = @_; $s =~ s/_(\w)/\u$1/g; return $s }
sub pix_element { my ($el) = @_; return $el->{pix} // ucfirst camel($el->{name}) }
sub pix_prop { my ($p) = @_; return $p->{pix} // camel($p->{name}) }

# --- Perl text --------------------------------------------------------------

sub q_str { my ($s) = @_; $s =~ s/([\\'])/\\$1/g; return "'$s'" }

sub perl_default {
    my ($p) = @_;
    return 'undef' unless exists $p->{default};
    my $d = $p->{default};
    my $t = $p->{type};
    return q_str($d) if $t eq 'str';
    return ($d ? 1 : 0) if $t eq 'bool';
    return sprintf('%.1f', $d) if $t eq 'num';
    return "$d" if $t eq 'int';
    return '[]';
}

sub banner {
    return "# Generated from crates/pixie-capi/elements.toml by tools/gen.pl. Do not\n"
         . "# edit by hand; edit the table and run `tools/gen.pl`.\n";
}

# --- lib/Rakugan/Vocab.pm ---------------------------------------------------
# The table as Perl data. Read by the door's runtime under the app's
# perl and by the translator under the tooling's, so it is plain data
# and nothing newer than perl 5.34 is written here.

sub gen_vocab {
    my $out = "package Rakugan::Vocab;\n" . banner() . <<'HEAD';
#
# The engine's vocabulary as data: the numbers both sides of the C face
# count with, every element with its keywords, and the keywords every
# element takes. The door's runtime writes an element from this; the
# translator reads the same rows to write the .pix.
use strict;
use warnings;
use utf8;

HEAD
    $out .= "# What a handler is called with.\n";
    $out .= "our %PAY = (" . join(', ', map { "$_ => $PAYLOADS{$_}" } @PAYLOAD_ORDER) . ");\n";
    $out .= "# The .pix type of a handler's parameter, and the payload's name at a .pix call site.\n";
    $out .= "our %PAYLOAD_TYPE = (" . join(', ', map { "$_ => '$PAYLOAD_TYPE{$_}'" } qw(text bool int float)) . ");\n";
    $out .= "our %PAYLOAD_NAME = (" . join(', ', map { "$_ => '$PAYLOAD_NAME{$_}'" } qw(text bool int float)) . ");\n\n";
    $out .= "# The elements.\n";
    $out .= "our %KIND = (\n" . join('', map { "    $_->{name} => $KIND_ID{$_->{name}},\n" } @ELEMENTS) . ");\n\n";
    $out .= "# The keyword arguments, numbered once across every element.\n";
    $out .= "our %KEY = (\n" . join('', map { "    $_ => $KEY_ID{$_},\n" } @KEY_ORDER) . ");\n\n";

    $out .= "# The keywords every element takes. `presence` marks the four whose box\n";
    $out .= "# is built because somebody wrote the property; `owned` names the element\n";
    $out .= "# props that take the same word first.\n";
    $out .= "our \@RIDERS = (\n";
    for my $r (@RIDERS) {
        my @f = ("name => " . q_str($r->{name}), "type => " . q_str($r->{type}), "key => $KEY_ID{$r->{name}}",
                 "pix => " . q_str($r->{pix} // camel($r->{name})));
        push @f, "presence => 1" if $r->{presence};
        push @f, "default => " . perl_default($r) if exists $r->{default};
        push @f, "owned => " . q_str($r->{owned}) if defined $r->{owned};
        $out .= "    { " . join(', ', @f) . " },\n";
    }
    $out .= ");\nour %RIDER = map { \$_->{name} => \$_ } \@RIDERS;\n\n";

    $out .= "# The elements, in the table's order: the kind, the .pix element, what\n";
    $out .= "# it is (a container, a painter, one that sizes a side of its own, one\n";
    $out .= "# whose own label is its accessible name), and its keywords in the\n";
    $out .= "# order they are written to the engine.\n";
    $out .= "our \@ELEMENTS = (\n";
    for my $el (@ELEMENTS) {
        my @f = ("name => " . q_str($el->{name}), "kind => $KIND_ID{$el->{name}}", "pix => " . q_str(pix_element($el)));
        push @f, "children => 1" if $el->{children};
        push @f, "paints => 1" if $el->{paints};
        push @f, "native => " . q_str($el->{native}) if defined $el->{native};
        push @f, "owns_label => 1" if $el->{owns_label};
        push @f, "primary => " . q_str($el->{primary}) if defined $el->{primary};
        push @f, "doc => " . q_str($el->{doc}) if defined $el->{doc};
        $out .= "    {\n        " . join(",\n        ", @f) . ",\n        props => [\n";
        for my $p (@{ $el->{props} }) {
            my @g = ("name => " . q_str($p->{name}), "key => $KEY_ID{$p->{name}}");
            if (defined $p->{handler}) {
                push @g, "handler => " . q_str($p->{handler}),
                         "pix => " . q_str(pix_prop($p)),
                         "payload => " . q_str($p->{payload} // $PAYLOAD_NAME{ $p->{handler} });
            } else {
                push @g, "type => " . q_str($p->{type}), "pix => " . q_str(pix_prop($p));
                push @g, "pos => 1" if $p->{pos};
                push @g, "default => " . perl_default($p) if exists $p->{default};
            }
            $out .= "            { " . join(', ', @g) . " },\n";
        }
        $out .= "        ],\n    },\n";
    }
    $out .= ");\nour %ELEMENT = map { \$_->{name} => \$_ } \@ELEMENTS;\n\n";

    $out .= "# The canvas's drawing commands. Not elements: no kind, no handle, no\n";
    $out .= "# keywords that every element takes, and no meaning outside the canvas\n";
    $out .= "# they are written in.\n";
    $out .= "our \@OPS = (\n";
    for my $op (@OPS) {
        my @f = ("name => " . q_str($op->{name}), "pix => " . q_str($op->{pix}),
                 'params => [' . join(', ', map {
                     '{ ' . join(', ', "name => " . q_str($_->{name}), "pix => " . q_str($_->{pix}),
                                       "type => " . q_str($_->{type}),
                                       (exists $_->{default} ? ('default => ' . q_str($_->{default})) : ())) . ' }'
                 } @{ $op->{params} }) . ']');
        $out .= "    { " . join(', ', @f) . " },\n";
    }
    $out .= ");\nour %OP = map { \$_->{name} => \$_ } \@OPS;\n\n1;\n";
    return $out;
}

# --- lib/Rakugan/Elements.pm ------------------------------------------------
# One sub per element. The work is in the runtime, read off the table;
# the subs are the names an app calls, each with its sentence.

sub wrap_doc {
    my ($doc) = @_;
    my @lines;
    my $line = '#';
    for my $w (split ' ', $doc) {
        if (length($line) + length($w) + 1 > 74) { push @lines, $line; $line = '#' }
        $line .= " $w";
    }
    push @lines, $line;
    return join("\n", @lines) . "\n";
}

sub gen_elements {
    my $out = "package Rakugan::Elements;\n" . banner() . <<'HEAD';
#
# The elements an app writes its screen with: one sub per row of the
# table. Positional arguments come first, then keywords and children in
# any order (`column(text(...), row(...), spacing => 8)`); the keywords
# every element takes ride along with the element's own. What each one
# does with what it was given is in Rakugan::Runtime, read off the
# table, so the subs here are the names and their sentences.
use v5.40;
use Rakugan::Runtime;
use Rakugan::Vocab;

HEAD
    $out .= "our \@ELEMENTS = qw(" . join(' ', map { $_->{name} } @ELEMENTS) . ");\n";
    for my $el (@ELEMENTS) {
        $out .= "\n";
        $out .= wrap_doc($el->{doc}) if defined $el->{doc};
        $out .= "sub $el->{name} { Rakugan::Runtime::element(\$Rakugan::Vocab::ELEMENT{$el->{name}}, \@_) }\n";
    }
    $out .= "\n";
    $out .= <<'PAINT';
# --- the canvas's drawing commands ------------------------------------------
#
# A command joins the canvas that is open where it stands. A canvas
# cannot hold another, so one place to remember the open one is enough.

our @OPS = qw(PAINTNAMES);

PAINT
    $out =~ s/PAINTNAMES/join ' ', map { $_->{name} } @OPS/e;
    for my $op (@OPS) {
        $out .= '# ' . join(', ', map { exists $_->{default} ? "$_->{name} => …" : $_->{name} }
                                  @{ $op->{params} }) . ".\n";
        $out .= "sub $op->{name} { Rakugan::Runtime::paint_op('$op->{name}', \@_) }\n";
    }
    $out .= "\n1;\n";
    return $out;
}

# --- write, or check --------------------------------------------------------

my %FILES = (
    "$ROOT/lib/Rakugan/Vocab.pm"    => gen_vocab(),
    "$ROOT/lib/Rakugan/Elements.pm" => gen_elements(),
);

my $check = grep { $_ eq '--check' } @ARGV;
my @stale;
for my $path (sort keys %FILES) {
    my $body = $FILES{$path};
    if ($check) {
        my $on_disk = -f $path ? do { open my $fh, '<:encoding(UTF-8)', $path or die; local $/; <$fh> } : undef;
        push @stale, $path unless defined $on_disk && $on_disk eq $body;
    } else {
        open my $fh, '>:encoding(UTF-8)', $path or die "$path: $!\n";
        print {$fh} $body;
        close $fh;
        (my $rel = $path) =~ s{^\Q$REPO\E/}{};
        print "wrote $rel\n";
    }
}
if ($check && @stale) {
    warn "gen: these are not what elements.toml says — run rakugan/tools/gen.pl\n";
    for my $path (@stale) {
        (my $rel = $path) =~ s{^\Q$REPO\E/}{};
        warn "  $rel\n";
    }
    exit 1;
}
