package Rakugan::Translator;
# Perl to .pix: the compiled run is built from what this writes.
#
# It reads the app with PPI and answers the `.pix` text and the window
# the app asked for, or stops at the first thing it cannot take, with
# the file, line and column, the line itself, a caret, and what to write
# instead. Nothing here executes the app: this module runs under the
# tooling's perl, which needs PPI and nothing else, while the app runs
# under a perl with the class feature.
#
# The element vocabulary is not written here. It comes from
# Rakugan::Vocab, which tools/gen.pl writes from the engine's table, so
# the .pix this emits and the calls the interpreted run makes are read
# off the same rows.
#
# Types are not written in the app: a field's type is read from its
# initializer (`empty(Str)` for a container that starts empty), a
# method's parameters come with `:Sig(...)`, a handler's parameter type
# from the element it is written on, and everything else is checked
# against those.
#
# What a method becomes depends on what it does. One that answers an
# element is a `.pix` component (`view Name(params)`), unless it CHOOSES
# between elements, which a component's body cannot do — that one is
# written out at the place it is called. One that touches a field is a
# store fn, and a view cannot call it, because building a view only
# reads. One that touches no field is a static on `Helpers`, which a
# view can call.
#
# PPI does not know `class`, `field` or `method`. Their tokens still
# come through, in three shapes this file allows for: a `class` block
# and whatever follows it up to the next `;` arrive as one statement;
# a method whose name is a quote-like operator (`m`, `s`, `y`, `tr`,
# `q`, `qq`, `qw`, `qr`) is read as a regular expression, so those
# names are refused; and `method name :Sig(...)` mis-tokenizes the name
# and its colon as one Label. How PPI splits an attribute list has
# changed between its own versions, so `attr_names` reads either shape
# rather than pinning the dialect to one PPI.
use strict;
use warnings;
use utf8;
use PPI;
use File::Spec;
use Math::BigInt;
use Rakugan::Vocab;
use Rakugan::Manifest;

my %ROLES = map { $_ => 1 } qw(button label heading textInput image list listItem table dialog
                               progress slider group checkbox switch comboBox radioGroup tabList);
my %EASINGS = map { $_ => 1 } qw(linear in out inOut);
my %TYPE_WORD = (Int => 'Int', Str => 'String', Num => 'Float', Bool => 'Bool');
# What Perl has that a compiled app cannot be given, each with what to
# write instead. These are decisions, not gaps: the reason is in the
# message, and the tour lists them.
# Perl's named unary operators the dialect takes: what each is given,
# what it answers, and the twin that answers it.
my %UNARY = (
    length  => ['String', 'Int',    'lengthOf'],
    uc      => ['String', 'String', 'uc'],
    lc      => ['String', 'String', 'lc'],
    ucfirst => ['String', 'String', 'ucfirst'],
    lcfirst => ['String', 'String', 'lcfirst'],
    sqrt    => ['Float',  'Float',  'sqrtOf'],
    floor   => ['Float',  'Float',  'floorOf'],
    ceil    => ['Float',  'Float',  'ceilOf'],
);
my %NOT_TAKEN = (
    print   => 'a compiled app writes its screen, not its standard output — that is where the '
             . 'gate reads the tree from; `warn` goes to standard error and is taken',
    printf  => 'a compiled app writes its screen, not its standard output; `warn` goes to standard error',
    say     => 'a compiled app writes its screen, not its standard output; `warn` goes to standard error',
    eval    => 'a string `eval` compiles Perl while the app runs, and a shipped app carries no compiler; '
             . 'catch a failure with `try` / `catch`',
    goto    => '`goto` has no shape in the compiled run; write the call itself',
    local   => '`local` gives a value back when the scope ends, which the compiled run has nowhere to '
             . 'keep; a field or a `my` name holds a value here',
    wantarray => 'the translator settles what every expression is read as while it reads it, so there '
             . 'is nothing to ask at run time',
    each    => '`each` hands a hash back in the order perl happens to hold it, which is a different '
             . 'order every time perl starts; write `for my $k (sort keys %h)`',
    tie     => '`tie` hides a call behind a variable, and the compiled run reads the variable itself',
    untie   => '`tie` hides a call behind a variable, and the compiled run reads the variable itself',
    bless   => 'an app is one class, written with `class`; `bless` is the older way and is not read here',
    AUTOLOAD => 'a method that does not exist is a refusal here, not something answered at run time',
    ref     => '`ref` asks what something is while the app runs; in the compiled run every value '
             . 'already has one type, and the translator knows it',
);
# The framework's functions that cannot fail: the `_or` twins, and the
# rest that answer a default or a yes-or-no. Anything else inside a
# `try` is either the `!T` form the manifest marks or refused by name,
# because a failure the compiled run cannot hand to the catch would be
# caught in one run and not the other.
my %CANNOT_FAIL = map { $_ => 1 } qw(fs_exists fs_app_dir clipboard_set_text clipboard_get_text
                                     keys_down keys_pressed keys_released audio_play audio_stop
                                     notify_send strings_to_int strings_to_float http_status);
sub cannot_fail { my ($name) = @_; return $CANNOT_FAIL{$name} || $name =~ /_or\z/ }
my %ELEMENT = %Rakugan::Vocab::ELEMENT;
my %RIDER = %Rakugan::Vocab::RIDER;
my @RIDERS = @Rakugan::Vocab::RIDERS;
my %OP = %Rakugan::Vocab::OP;
my %PIX_ELEMENT = map { $_->{pix} => 1 } @Rakugan::Vocab::ELEMENTS;

# --- state for one translation ---------------------------------------------
my ($path, @lines);
my ($class_name, $class_block, $run_word, $run_list, $view_block);
my (@fields, %field);       # name => { ty, init, kind }
my %bag;                    # a hash of keywords at the top of the file: name => [pieces]
my %const;                  # a named literal at the top of the file: name => { ty, pix }
my (@methods, %method);     # name => { name, block, node, params, ret, kind, ... }
my @handlers;               # { id, params => [[name, ty]], body => [lines], async }
my @timers;                 # { ms, body => [lines] }
my @binds;                  # { kind, args, param, body => [lines] }
my @lifted;                 # the statics a conditional expression became
my (@values, %value);       # the value classes: name => [{ name, ty, init }]
my (@models, %model);       # the classes with methods: name => { fields => [...], by => {...}, methods => {...}, order => [...] }
my %constant;               # `use constant NAME => literal`: name => { ty, pix, lit }
my $app_class;              # the class `run` was handed
my $app_var;                # `my $app = Class->new` at the top of the file
my @app_stmts;              # every top-level statement, in order
my $uses_pl;                # the twins of Perl's own functions were called
my $uses_std;               # the framework's own standard library was called
our $re_at;                 # the token a piece of Perl inside a string came from
my $tmp;                    # a counter for the names this file makes up
my $where;                  # the app's path as perl names it in a message: absolute, because that is how the command starts perl

# The one entry point: the .pix text and the window, or a refusal thrown
# as a string that already says everything.
sub translate {
    my ($file) = @_;
    ($path, $class_name, $class_block, $run_word, $run_list, $view_block) = ($file);
    (@fields, %field, %bag, %const, @methods, %method, @handlers, @timers, @binds, @app_stmts,
     @lifted, @values, %value, @models, %model, %constant) = ();
    $app_class = undef;
    ($app_var, $uses_pl, $uses_std, $tmp) = (undef, 0, 0, 0);
    $where = File::Spec->rel2abs($path);
    open my $fh, '<:encoding(UTF-8)', $path or die "$path: $!\n";
    my $src = do { local $/; <$fh> };
    close $fh;
    @lines = split /^/m, $src;
    my $doc = PPI::Document->new(\$src) or die PPI::Document->errstr . "\n";

    declarations($doc);
    class_body();
    classify();
    my $tree = view();
    my $window = run_line();
    bodies();
    return { pix => emit($tree), window => $window, stdlib => $uses_pl, framework => $uses_std };
}

# The pixie.toml of the project a translation is built as.
sub project_toml {
    my ($stem, $window, $stdlib_dir, $framework_dir) = @_;
    my $toml = qq{[package]\nname = "$stem"\nversion = "0.1.0"\n};
    if (%$window) {
        $toml .= "\n[window]\n";
        $toml .= qq{title = $window->{title}\n} if defined $window->{title};
        $toml .= "width = $window->{width}\nheight = $window->{height}\n" if defined $window->{width};
        $toml .= "padding = $window->{padding}\n" if defined $window->{padding};
    }
    $toml .= "\n[crates]\n";
    $toml .= qq{rakugan-stdlib = { path = "$stdlib_dir" }\n} if defined $stdlib_dir;
    # The framework's own library, with sound: the same crate and the
    # same feature the C face carries, so both runs are one library.
    $toml .= qq{yokan-stdlib = { path = "$framework_dir", features = ["audio"] }\n}
        if defined $framework_dir;
    return $toml;
}

# The binding door of the twins, written out rather than derived: the
# compiled run reaches the same Rust the interpreted run gets from perl
# itself. One row per function, in the order the crate declares them.
sub stdlib_rpi {
    return <<'RPI';
# pixie-cache-key: version=0.1.0; features=; format=61
# Generated by rakugan — the twins of Perl's own answers.

class Pl {
  static fn modInt(a: Int, b: Int) Int @rust("rakugan_stdlib::mod_int")
  static fn divInt(a: Int, b: Int) Float @rust("rakugan_stdlib::div_int")
  static fn intOf(v: Float) Int @rust("rakugan_stdlib::int_of")
  static fn numOf(s: String) Float @rust("rakugan_stdlib::num_of")
  static fn numText(v: Float) String @rust("rakugan_stdlib::num_text")
  static fn boolText(v: Bool) String @rust("rakugan_stdlib::bool_text")
  static fn fmtNum(fmt: String, v: Float) String @rust("rakugan_stdlib::fmt_num")
  static fn fmtInt(fmt: String, v: Int) String @rust("rakugan_stdlib::fmt_int")
  static fn fmtStr(fmt: String, v: String) String @rust("rakugan_stdlib::fmt_str")
  static fn lengthOf(s: String) Int @rust("rakugan_stdlib::length_of")
  static fn substrFrom(s: String, off: Int) String @rust("rakugan_stdlib::substr_from")
  static fn substrLen(s: String, off: Int, len: Int) String @rust("rakugan_stdlib::substr_len")
  static fn indexOf(s: String, sub: String) Int @rust("rakugan_stdlib::index_of")
  static fn indexFrom(s: String, sub: String, pos: Int) Int @rust("rakugan_stdlib::index_from")
  static fn rindexOf(s: String, sub: String) Int @rust("rakugan_stdlib::rindex_of")
  static fn rindexFrom(s: String, sub: String, pos: Int) Int @rust("rakugan_stdlib::rindex_from")
  static fn uc(s: String) String @rust("rakugan_stdlib::uc")
  static fn lc(s: String) String @rust("rakugan_stdlib::lc")
  static fn ucfirst(s: String) String @rust("rakugan_stdlib::ucfirst")
  static fn lcfirst(s: String) String @rust("rakugan_stdlib::lcfirst")
  static fn reverseStr(s: String) String @rust("rakugan_stdlib::reverse_str")
  static fn join(sep: String, parts: List<String>) String @rust("rakugan_stdlib::join")
  static fn splitOn(sep: String, s: String) List<String> @rust("rakugan_stdlib::split_on")
  static fn splitWords(s: String) List<String> @rust("rakugan_stdlib::split_words")
  static fn absNum(v: Float) Float @rust("rakugan_stdlib::abs_num")
  static fn absInt(v: Int) Int @rust("rakugan_stdlib::abs_int")
  static fn sqrtOf(v: Float) Float @rust("rakugan_stdlib::sqrt_of")
  static fn floorOf(v: Float) Float @rust("rakugan_stdlib::floor_of")
  static fn ceilOf(v: Float) Float @rust("rakugan_stdlib::ceil_of")
  static fn fmodOf(a: Float, b: Float) Float @rust("rakugan_stdlib::fmod_of")
  static fn sumInt(xs: List<Int>) Int @rust("rakugan_stdlib::sum_int")
  static fn sumNum(xs: List<Float>) Float @rust("rakugan_stdlib::sum_num")
  static fn maxInt(xs: List<Int>) Int @rust("rakugan_stdlib::max_int")
  static fn minInt(xs: List<Int>) Int @rust("rakugan_stdlib::min_int")
  static fn maxNum(xs: List<Float>) Float @rust("rakugan_stdlib::max_num")
  static fn minNum(xs: List<Float>) Float @rust("rakugan_stdlib::min_num")
  static fn uniqStr(xs: List<String>) List<String> @rust("rakugan_stdlib::uniq_str")
  static fn uniqInt(xs: List<Int>) List<Int> @rust("rakugan_stdlib::uniq_int")
  static fn strftimeUtc(fmt: String, epoch: Int) String @rust("rakugan_stdlib::strftime_utc")
  static fn strftimeLocal(fmt: String, epoch: Int) String @rust("rakugan_stdlib::strftime_local")
  static fn reMatches(pat: String, mods: String, s: String) Bool @rust("rakugan_stdlib::re_matches")
  static fn reCapture(pat: String, mods: String, s: String, n: Int) String @rust("rakugan_stdlib::re_capture")
  static fn reCaptureNamed(pat: String, mods: String, s: String, name: String) String @rust("rakugan_stdlib::re_capture_named")
  static fn reSubst(pat: String, mods: String, s: String, repl: String) String @rust("rakugan_stdlib::re_subst")
  static fn reSplit(pat: String, mods: String, s: String) List<String> @rust("rakugan_stdlib::re_split")
  static fn reAll(pat: String, mods: String, s: String) List<String> @rust("rakugan_stdlib::re_all")
  static fn reCount(pat: String, mods: String, s: String) Int @rust("rakugan_stdlib::re_count")
  static fn divNum(a: Float, b: Float) Float @rust("rakugan_stdlib::div_num")
  static fn dieText(text: String, at: String) String @rust("rakugan_stdlib::die_text")
  static fn warnAt(text: String, at: String) Int @rust("rakugan_stdlib::warn_at")
  static fn dieAt(text: String, at: String) Int @rust("rakugan_stdlib::die_at")
  static fn tryDivInt(a: Int, b: Int, at: String) !Float @rust("rakugan_stdlib::try_div_int")
  static fn tryDivNum(a: Float, b: Float, at: String) !Float @rust("rakugan_stdlib::try_div_num")
  static fn tryModInt(a: Int, b: Int, at: String) !Int @rust("rakugan_stdlib::try_mod_int")
  static fn trySqrt(v: Float, at: String) !Float @rust("rakugan_stdlib::try_sqrt")
}
RPI
}

# --- refusals -------------------------------------------------------------
sub refuse {
    my ($node, $msg) = @_;
    # A hole in a string is read as Perl of its own, whose lines are its
    # own; the string is where the reader has to look.
    $node = $re_at if $re_at;
    $node = $node->{_rakugan_at} if $node && ref $node && $node->{_rakugan_at};
    my ($line, $col) = $node && $node->can('line_number') ? ($node->line_number, $node->column_number) : (1, 1);
    my $src = $lines[$line - 1] // '';
    chomp $src;
    die sprintf "%s:%d:%d: Rakugan cannot take this — %s\n    %s\n    %s^\n",
        $path, $line, $col, $msg, $src, ' ' x ($col - 1);
}

sub sig { grep { $_->significant } $_[0]->children }

# PPI reads a bare word followed by a colon as a label — `HAPPY ? SAD :
# HAPPY` arrives with `SAD :` as one token. Where an expression is
# read, such a token is cut back into the word and the colon; the two
# pieces point at the token they came from, so a refusal still names
# the right place.
sub unlabel {
    my ($toks) = @_;
    return @$toks unless grep { $_->isa('PPI::Token::Label') } @$toks;
    my @out;
    for my $t (@$toks) {
        if ($t->isa('PPI::Token::Label') && $t->content =~ /\A(\w+)\s*:\z/) {
            my $w = PPI::Token::Word->new($1);
            my $c = PPI::Token::Operator->new(':');
            $w->{_rakugan_at} = $c->{_rakugan_at} = $t;
            push @out, $w, $c;
        } else {
            push @out, $t;
        }
    }
    return @out;
}
sub is_word { my ($t, $w) = @_; $t && $t->isa('PPI::Token::Word') && (!defined $w || $t->content eq $w) }
sub is_op   { my ($t, $o) = @_; $t && $t->isa('PPI::Token::Operator') && (!defined $o || $t->content eq $o) }
sub is_sym  { my ($t, $kind) = @_; $t && $t->isa('PPI::Token::Symbol') && (!defined $kind || $t->raw_type eq $kind) }
sub is_sub  { my ($t, $b) = @_; $t && $t->isa('PPI::Structure::Subscript') && $t->start->content eq $b }
sub strip_semicolon { my @t = @_; pop @t if @t && $t[-1]->isa('PPI::Token::Structure') && $t[-1]->content eq ';'; @t }

# The attribute names after a field, and where the list ends. PPI has
# tokenized `:param :reader` two ways across its own versions: older
# ones fold `param :` into a single Label, swallowing the next colon,
# and 1.28x gives the colon and the word apart. Reading both is what
# keeps the dialect from depending on which PPI a machine happens to
# ship — a field that is written correctly must not be refused because
# the parser underneath was upgraded.
sub attr_names {
    my ($toks, $i) = @_;
    my @names;
    while (is_op($toks->[$i], ':')) {
        my $t = $toks->[$i + 1] or return (undef, $i);
        if ($t->isa('PPI::Token::Label')) {
            (my $n = $t->content) =~ s/\s*:\s*\z//;
            push @names, $n;
            $i += 2;
            # the Label ate the NEXT attribute's colon, so what follows
            # is that attribute: another label when there is a third
            # (`:param :reader :writer`), a word for the last
            while ($toks->[$i] && $toks->[$i]->isa('PPI::Token::Label')) {
                (my $more = $toks->[$i]->content) =~ s/\s*:\s*\z//;
                push @names, $more;
                $i++;
            }
            if (is_word($toks->[$i])) { push @names, $toks->[$i]->content; $i++ }
        } elsif ($t->isa('PPI::Token::Word')) {
            push @names, $t->content;
            $i += 2;
        } else {
            return (undef, $i);
        }
    }
    return (\@names, $i);
}

# --- the file's declarations ---------------------------------------------
# `use` lines, hashes of keywords, one class, the app, its timers and
# the `run(...)` line. PPI reads `class Name { ... }` and whatever
# follows it up to the next `;` as one statement, so the tokens are
# walked in runs, and a run ends where a declaration's shape ends.
sub declarations {
    my ($doc) = @_;
    # A method named like a quote-like operator (`method s { ... }`) is
    # read by PPI as a regular expression that runs to some later brace,
    # taking the rest of the file with it; it has to be caught before
    # anything else is looked for.
    my $eaten = $doc->find(sub {
        my $t = $_[1];
        $t->isa('PPI::Token::Regexp') && is_word($t->sprevious_sibling, 'method');
    }) || [];
    if (@$eaten) {
        (my $name = $eaten->[0]->content) =~ s/\W.*//s;
        refuse($eaten->[0], "a method named `$name` reads as a regular expression to the parser; pick another name");
    }
    my ($pragma, %seen, @order);
    for my $st ($doc->schildren) {
        next if $st->isa('PPI::Statement::End') || $st->isa('PPI::Statement::Null');
        if ($st->isa('PPI::Statement::Include')) {
            $pragma = $st if ($st->module // '') eq 'Rakugan';
            constants($st) if ($st->module // '') eq 'constant';
            next;
        }
        my @t = strip_semicolon(sig($st));
        while (@t) {
            if (is_word($t[0], 'class')) {
                refuse($t[0], 'a class is written `class Name { ... }`')
                    unless @t >= 3 && is_word($t[1]) && $t[2]->isa('PPI::Structure::Block');
                refuse($t[1], '`' . $t[1]->content . '` is declared twice') if $seen{ $t[1]->content };
                $seen{ $t[1]->content } = $t[2];
                push @order, $t[1]->content;
                splice @t, 0, 3;
            } elsif (is_word($t[0], 'my')) {
                my $n = app_decl(\@t);
                splice @t, 0, $n;
            } elsif (is_word($t[0], 'run')) {
                refuse($t[0], '`run` is called with parentheses: `run(Counter->new, title => "...")`')
                    unless @t >= 2 && $t[1]->isa('PPI::Structure::List');
                ($run_word, $run_list) = @t[0, 1];
                splice @t, 0, 2;
            } elsif (is_word($t[0]) && $t[0]->content =~ /\A(?:every|task|shortcut|menu_item|on_key|on_file_drop)\z/) {
                refuse($t[0], '`' . $t[0]->content . '` is called with parentheses')
                    unless @t >= 2 && $t[1]->isa('PPI::Structure::List');
                push @app_stmts, { what => $t[0]->content, word => $t[0], list => $t[1] };
                splice @t, 0, 2;
            } else {
                refuse($t[0], 'a statement at the top of the file — the compiled app reads the declarations '
                            . '(`use`, a hash of keywords, `class`, the app, `every`, `run`) and never executes the file');
            }
        }
    }
    die "$path: no `class` found — an app is a class with a `view` method, handed to `run`\n" unless @order;
    die "$path: no `run(...)` found — the last line hands the app to `run`\n" unless defined $run_word;
    die "$path: the file starts with `use Rakugan;` — it turns on what the dialect assumes and brings `run`\n" unless $pragma;
    # The class `run` was handed is the app; any other is a value the
    # app holds, and has no screen of its own.
    if (!defined $app_class) {
        my @a = split_args($run_list);
        my $first = @a ? join('', map { $_->content } @{ $a[0]{toks} }) : '';
        ($app_class) = $first =~ /\A(\w+)->new\z/;
    }
    refuse($run_word, 'the app is one of the classes in this file, handed to `run` as `Name->new`')
        unless defined $app_class && $seen{$app_class};
    ($class_name, $class_block) = ($app_class, $seen{$app_class});
    # The other classes: one with methods is an object the app points
    # at, one with fields alone is a value it holds. Every name is known
    # before any body is read, so a class can name another — or itself.
    my @others = grep { $_ ne $app_class } @order;
    for my $name (@others) {
        $model{$name} = { name => $name, fields => [], by => {}, methods => {}, order => [] } if is_model($seen{$name});
    }
    for my $name (@others) {
        if ($model{$name}) { model_class($name, $seen{$name}) } else { value_class($name, $seen{$name}) }
    }
}

# `use constant NAME => value;` or `use constant { A => 1, B => 2 };` —
# a name for a literal, which is what it becomes wherever it is read.
sub constants {
    my ($st) = @_;
    my @t = strip_semicolon(sig($st));
    splice @t, 0, 2;
    my @pairs;
    if (@t == 1 && $t[0]->isa('PPI::Structure::Constructor')) {
        @pairs = split_args($t[0]);
        refuse($t[0], '`use constant { ... }` holds `NAME => value` pairs') if grep { !defined $_->{key} } @pairs;
    } else {
        refuse($st, 'a constant is `use constant NAME => value;` or `use constant { A => 1, B => 2 };`')
            unless @t >= 3 && is_word($t[0]) && is_op($t[1], '=>');
        @pairs = ({ key => $t[0]->content, node => $t[0], toks => [@t[2 .. $#t]] });
    }
    for my $p (@pairs) {
        my ($ty, $pix) = literal_run($p->{toks});
        # perl keeps a constant per package, so two classes may each
        # declare it — as long as they mean the same thing by it.
        if (my $had = $constant{ $p->{key} }) {
            refuse($p->{node}, "`$p->{key}` is declared twice, and not the same way") unless $had->{pix} eq $pix;
            next;
        }
        my $tok = $p->{toks}[0];
        my $lit = $ty ne 'String' ? undef
                : $tok->isa('PPI::Token::Quote::Double') ? unescape($tok, $tok->string) : $tok->literal;
        $constant{ $p->{key} } = { ty => $ty, pix => $pix, lit => $lit };
    }
}

# Whether a class other than the app is one with methods: it has a
# `method`, or a field that is not the `:param :reader` pair a value's
# fields are.
sub is_model {
    my ($block) = @_;
    for my $st ($block->schildren) {
        next if $st->isa('PPI::Statement::Null') || $st->isa('PPI::Statement::Include');
        my @t = sig($st);
        next unless @t;
        return 1 if is_word($t[0], 'method') || is_word($t[0], 'ADJUST');
        if (is_word($t[0], 'field')) {
            my ($attrs, $j) = attr_names(\@t, 2);
            my @a = @{ $attrs // [] };
            return 1 unless @a == 2 && $a[0] eq 'param' && $a[1] eq 'reader';
        }
    }
    return 0;
}

# A class with methods: an object, which two names can share, the way
# perl's objects are shared. Its fields are its own, read by name inside
# its methods and through `:reader` outside; `:param` says `new` may be
# given one, `:writer` gives it `set_name`. It crosses to the compiled
# run as a class of its own, with the same sharing.
sub model_class {
    my ($name, $block) = @_;
    my $m = $model{$name};
    my $pragma;
    for my $st ($block->schildren) {
        next if $st->isa('PPI::Statement::Null');
        if ($st->isa('PPI::Statement::Include')) {
            $pragma = 1 if ($st->module // '') eq 'Rakugan';
            constants($st) if ($st->module // '') eq 'constant';
            next;
        }
        my @t = strip_semicolon(sig($st));
        next unless @t;
        while (@t) {
            if (is_word($t[0], 'field')) {
                my $sym = $t[1];
                refuse($sym // $t[0], "a class with methods holds scalars here; a list or a hash belongs to the app")
                    unless is_sym($sym, '$');
                my ($attrs, $j) = attr_names(\@t, 2);
                my %a = map { $_ => 1 } @{ $attrs // [] };
                refuse($sym, 'a field here takes `:param`, `:reader` and `:writer`, and nothing else')
                    if grep { !/\A(?:param|reader|writer)\z/ } keys %a;
                refuse($sym, 'a field needs an initializer (`= 0`, `= ""`, `= maybe(Node)`) — that is where its type comes from')
                    unless is_op($t[$j], '=') && @t > $j + 1;
                my @init = @t[$j + 1 .. $#t];
                my ($ty, $pix) = (is_word($init[0], 'maybe') || is_word($init[0], 'undef'))
                                ? maybe_init(\@init, $sym) : literal_run(\@init);
                my $fname = substr $sym->content, 1;
                refuse($sym, "`$fname` is declared twice") if $m->{by}{$fname};
                my $fd = { name => $fname, ty => $ty, init => $pix, param => $a{param} ? 1 : 0,
                           reader => $a{reader} ? 1 : 0, writer => $a{writer} ? 1 : 0, weak => 0 };
                push @{ $m->{fields} }, $fd;
                $m->{by}{$fname} = $fd;
                @t = ();
            } elsif (is_word($t[0], 'method')) {
                my $n = model_method($m, \@t);
                splice @t, 0, $n;
            } elsif (is_word($t[0], 'ADJUST')) {
                refuse($t[0], '`ADJUST` in a class with methods is not taken yet; give the fields their values with `:param`');
            } else {
                refuse($t[0], "inside `class $name`: `use Rakugan;`, `field` and `method` only");
            }
        }
    }
    refuse($block, "`class $name` needs `use Rakugan;` as its first line") unless $pragma;
    push @models, $name;
}

# `method name :Sig(...) (...) { ... }` inside a class with methods:
# the same shape as the app's, kept on the class.
sub model_method {
    my ($m, $toks) = @_;
    my @t = @$toks;
    my ($name, $sig, $taken) = (undef, undef, 0);
    if ($t[1] && $t[1]->isa('PPI::Token::Label')) {
        (my $label = $t[1]->content) =~ s/\s*:\s*\z//;
        refuse($t[1], "`$label` takes `:Sig(...)`: the types of what it is called with, and after `=>` what it answers")
            unless is_word($t[2], 'Sig') && $t[3] && $t[3]->isa('PPI::Structure::List');
        ($name, $sig, $taken) = ({ content => $label, node => $t[1] }, $t[3], 4);
    } else {
        refuse($t[0], 'a method is written `method name { ... }`') unless is_word($t[1]);
        ($name, $taken) = ({ content => $t[1]->content, node => $t[1] }, 2);
    }
    my @names;
    if ($t[$taken] && $t[$taken]->isa('PPI::Structure::List')) { @names = param_names($t[$taken]); $taken++ }
    refuse($name->{node}, "a class with methods has no screen of its own; `view` belongs to the app")
        if $name->{content} eq 'view';
    my $block = $t[$taken];
    refuse($name->{node}, 'a method needs a block') unless $block && $block->isa('PPI::Structure::Block');
    $taken++;
    my ($ptys, $ret) = sig_types($sig, scalar @names, $name->{node});
    refuse($name->{node}, "a method with parameters says what they are: `method $name->{content} :Sig("
                        . join(', ', ('Int') x @names) . ") (" . join(', ', @names) . ') { ... }`')
        if @names && !$sig;
    refuse($name->{node}, "`$name->{content}` is called with " . scalar(@names) . ' value'
                        . (@names == 1 ? '' : 's') . ", and `:Sig` gives " . scalar(@$ptys) . ' type'
                        . (@$ptys == 1 ? '' : 's'))
        if @$ptys != @names;
    refuse($name->{node}, "`$name->{content}` is declared twice") if $m->{methods}{ $name->{content} };
    refuse($name->{node}, '`@kids` belongs to a method of the app that answers part of the screen')
        if grep { /\A\@/ } @names;
    # The compiled run gives every field a reader and a writer of its
    # own name (`label`, `set_label`), so a method cannot take those.
    if ($name->{content} =~ /\A(?:set_)?(\w+)\z/ && $m->{by}{$1}) {
        refuse($name->{node}, "`$name->{content}` is the name the compiled run gives `$1`'s own "
                            . ($name->{content} =~ /\Aset_/ ? 'writer; put `:writer` on the field, or name the method for what it does' : 'reader; rename the method'));
    }
    my $rec = { name => $name->{content}, block => $block, node => $name->{node},
                params => [map { [$names[$_], $ptys->[$_]] } 0 .. $#names], ret => $ret };
    # What could stop it, for a `try` around a call to it.
    $rec->{may_fail} = grep({ $_->content =~ m{\A[/%]=?\z} } @{ $block->find('PPI::Token::Operator') || [] })
        || grep({ my $n = $_->content; $n eq 'die' || $n eq 'sqrt' || ($Rakugan::Manifest::NAMES{$n} && !cannot_fail($n)) }
                @{ $block->find('PPI::Token::Word') || [] });
    $m->{methods}{ $name->{content} } = $rec;
    push @{ $m->{order} }, $rec;
    return $taken;
}

# `Node->new(label => "alpha")` — an object made where it is written:
# the compiled run makes it with its defaults and sets what was given.
sub build_object {
    my ($word, $list, $env) = @_;
    my $name = $word->content;
    my $m = $model{$name};
    refuse($word, "`$name->new` makes an object, and building a view only reads; make it in a handler and keep it in a field")
        if $env->{ctx} eq 'view';
    refuse($word, "`$name->new` needs a line to stand on; give it to a name first: `my \$n = $name->new(...)`")
        unless $env->{pre};
    my $o = '__o' . ++$tmp;
    push @{ $env->{pre} }, "var $o = $name()";
    my %given;
    for my $a ($list ? split_args($list) : ()) {
        refuse($a->{node}, "`$name` is built by naming its fields: `$name->new(label => \"x\")`") unless defined $a->{key};
        my $fd = $m->{by}{ $a->{key} } or refuse($a->{node}, "`$name` has no field `$a->{key}`");
        refuse($a->{node}, "`$a->{key}` is not a `:param` field of $name, so `new` cannot be given it") unless $fd->{param};
        refuse($a->{node}, "`$name` was given `$a->{key}` twice") if $given{ $a->{key} }++;
        my $v = parse_expr($a->{toks}, { %$env, at => $a->{node} });
        refuse($a->{node}, "`$name`'s `$a->{key}` holds a $fd->{ty}, and this is a $v->{ty}") unless fits($fd->{ty}, $v->{ty});
        push @{ $env->{pre} }, "$o.$a->{key} = $v->{pix}";
    }
    return { ty => $name, pix => $o };
}

# `$node->label`, `$node->set_label("x")`, `$node->grow(0.5)`: a field
# read through its `:reader`, written through its `:writer`, or a method.
sub member {
    my ($obj, $word, $ip, $toks, $env) = @_;
    my $m = $model{ $obj->{ty} };
    my $what = $word->content;
    my $args = $toks->[$$ip];
    my $has_args = $args && $args->isa('PPI::Structure::List');
    $$ip++ if $has_args;
    if (my $mm = $m->{methods}{$what}) {
        refuse($word, "`$what` is a method of $obj->{ty}, and building a view only reads; give the view a field with a `:reader`")
            if $env->{ctx} eq 'view';
        refuse($word, "`$what` can fail — it divides, takes a root, writes `die` or calls the library — and a "
                    . "`try` here does not reach into it yet; put the `try` inside `$what`, around the line that can fail")
            if $env->{try} && $mm->{may_fail};
        my @vals = call_args($word, $has_args ? $args : undef, $mm, $env);
        refuse($word, "`$what` does not say what it answers; write `:Sig(... => Str)`")
            if !defined $mm->{ret} && !$env->{as_statement};
        my $o = '__o' . ++$tmp;
        push @{ $env->{pre} }, "var $o = $obj->{pix}";
        return { ty => $mm->{ret} // 'Void', pix => "$o.$what(" . join(', ', @vals) . ')' };
    }
    if ($what =~ /\Aset_(\w+)\z/ && $m->{by}{$1}) {
        my ($fname, $fd) = ($1, $m->{by}{$1});
        refuse($word, "`$fname` has no `:writer`; put it on the field, or write a method that sets it") unless $fd->{writer};
        refuse($word, "`$what` writes an object, and building a view only reads") if $env->{ctx} eq 'view';
        refuse($word, "`$what` sets a field and answers nothing worth reading; it stands on a line of its own")
            unless $env->{as_statement};
        my @a = $has_args ? split_args($args) : ();
        refuse($word, "`$what` takes the new value: `\$n->$what(...)`") unless @a == 1 && !defined $a[0]{key};
        my $v = parse_expr($a[0]{toks}, { %$env, at => $a[0]{node} });
        refuse($a[0]{node}, "`$fname` holds a $fd->{ty}, and this is a $v->{ty}") unless fits($fd->{ty}, $v->{ty});
        my $o = '__o' . ++$tmp;
        push @{ $env->{pre} }, "var $o = $obj->{pix}";
        return { ty => 'Void', pix => "$o.$fname = $v->{pix}" };
    }
    if (my $fd = $m->{by}{$what}) {
        refuse($word, "`$what` has no `:reader`, so it is the object's own; add `:reader` to the field") unless $fd->{reader};
        refuse($args, 'a field is read, not called') if $has_args;
        my $pix = "$obj->{pix}.$what";
        if ($env->{narrowed} && (my $nw = $env->{narrowed}{$pix})) {
            return { ty => $nw->{ty}, pix => $nw->{pix}, narrowed => $what };
        }
        return { ty => $fd->{ty}, pix => $pix };
    }
    refuse($word, "`$obj->{ty}` has no `$what`; its fields are " . join(', ', map { "`$_->{name}`" } @{ $m->{fields} })
               . (@{ $m->{order} } ? ' and its methods ' . join(', ', map { "`$_->{name}`" } @{ $m->{order} }) : ''));
}

# `$root->kid->parent`: what a member answered may be an object with
# members of its own.
sub chain {
    my ($r, $ip, $toks, $env) = @_;
    while (is_op($toks->[$$ip], '->') && is_word($toks->[$$ip + 1])) {
        if ($model{ $r->{ty} }) {
            $$ip += 2;
            $r = member($r, $toks->[$$ip - 1], $ip, $toks, $env);
            next;
        }
        refuse($toks->[$$ip], "this may be nothing; read it inside `if (defined ...)`, where it is the object")
            if $r->{ty} =~ /\A(\w+)\?\z/ && $model{$1};
        refuse($toks->[$$ip], "`->` reads a member of an object, and this is a $r->{ty}");
    }
    return $r;
}

# `weaken($parent);` — inside a class with methods, on a field that
# points back at another object. perl's collector and the compiled
# run's both count references, so both need the back pointer not to
# count; the field is declared weak, and the line itself is the
# declaration.
sub weaken_stmt {
    my ($toks, $env) = @_;
    my ($w, @rest) = @$toks;
    refuse($w, '`weaken` breaks a cycle between two objects, and belongs in the class that holds the pointer back: '
             . '`method set_parent :Sig(Node) ($p) { $parent = $p; weaken($parent) }`')
        unless $env->{model};
    @rest = sig(($rest[0]->schildren)[0]) if @rest == 1 && $rest[0]->isa('PPI::Structure::List') && $rest[0]->schildren;
    refuse($w, '`weaken` takes one field: `weaken($parent)`') unless @rest == 1 && is_sym($rest[0], '$');
    my $fname = substr $rest[0]->content, 1;
    my $fd = $model{ $env->{model} }{by}{$fname} or refuse($rest[0], "`\$$fname` is not a field of $env->{model}");
    refuse($rest[0], "`\$$fname` holds a $fd->{ty}; a pointer back at an object is a `maybe(Node)` field")
        unless $fd->{ty} =~ /\A(\w+)\?\z/ && $model{$1};
    $fd->{weak} = 1;
    return ();
}

# A class the app holds values of: fields and nothing else, each one
# written where `new` can be given it and read back. It crosses to the
# compiled run as a struct, which is a VALUE — two names for one of
# these are two copies, where in perl they would be one object. A
# method on one waits for the phase that brings them.
sub value_class {
    my ($name, $block) = @_;
    my $pragma;
    my @fields;
    for my $st ($block->schildren) {
        next if $st->isa('PPI::Statement::Null');
        if ($st->isa('PPI::Statement::Include')) {
            $pragma = 1 if ($st->module // '') eq 'Rakugan';
            next;
        }
        my @t = strip_semicolon(sig($st));
        next unless @t;
        refuse($t[0], "`$name` holds values, so it is fields and nothing else; a class with a `view` "
                    . 'is the app, and there is one of those')
            unless is_word($t[0], 'field');
        my $sym = $t[1];
        my ($attrs, $j) = attr_names(\@t, 2);
        refuse($t[0], 'a field of a value is written `field $x :param :reader = 0;`')
            unless is_sym($sym, '$') && $attrs && @$attrs == 2
                && $attrs->[0] eq 'param' && $attrs->[1] eq 'reader'
                && is_op($t[$j], '=') && @t > $j + 1;
        my ($ty, $pix) = literal_run([@t[$j + 1 .. $#t]]);
        push @fields, { name => substr($sym->content, 1), ty => $ty, init => $pix };
    }
    refuse($block, "`class $name` needs `use Rakugan;` as its first line") unless $pragma;
    refuse($block, "`$name` holds nothing; a value class is one or more fields") unless @fields;
    push @values, $name;
    $value{$name} = \@fields;
}

# `my %PILL = (...)` — a hash of keywords — or `my $app = Class->new;`,
# which is how an app that declares a timer names itself. Answers how
# many tokens the declaration took.
sub app_decl {
    my ($toks) = @_;
    my ($my, $sym) = @$toks;
    refuse($my, 'a declaration at the top of the file is a hash of keywords (`my %PILL = (...)`), '
              . 'a name for a literal (`my $WIDTH = 120;`) or the app itself '
              . '(`my $app = Counter->new;`)')
        unless is_sym($sym, '%') || is_sym($sym, '$');
    if (is_sym($sym, '%')) {
        bag($toks);
        return scalar @$toks;
    }
    # `my $WIDTH = 120;` — a name for a literal. The compiled run has
    # no place at the top of a file to keep a value, so the value is
    # written wherever the name is read.
    my ($eq0, @rest0) = @{$toks}[2 .. $#$toks];
    if (is_op($eq0, '=') && @rest0 && !is_word($rest0[0])) {
        my $name = substr $sym->content, 1;
        refuse($sym, "`\$$name` is declared twice") if exists $const{$name};
        my ($ty, $pix) = literal_run(\@rest0);
        my $lit = $ty eq 'String' ? (@rest0 == 1 ? $rest0[0]->string : undef) : undef;
        $const{$name} = { ty => $ty, pix => $pix, lit => $lit };
        return scalar @$toks;
    }
    refuse($sym, '`$' . substr($sym->content, 1) . '` is declared twice') if defined $app_var;
    my ($eq, $cls, $arrow, $new, @rest) = @{$toks}[2 .. $#$toks];
    $app_class = $cls->content if is_word($cls);
    refuse($sym, 'a `my $...` at the top of the file is a name for a literal (`my $WIDTH = 120;`) '
               . 'or the app itself (`my $app = ' . ($class_name // 'Counter') . '->new;`), which is '
               . 'what a timer reaches its methods through')
        unless is_op($eq, '=') && is_word($cls) && is_op($arrow, '->') && is_word($new, 'new');
    $app_var = substr $sym->content, 1;
    return 6;
}

# `my %PILL = (size => 11, color => "#11111b");` — a bag of keywords an
# app writes once and hands to elements with `%PILL`. Another bag inside
# the list is merged in place, so `(%PILL, background => "#2fa84f")` is
# the pill with a background.
sub bag {
    my ($toks) = @_;
    my @t = @$toks;
    my ($my, $sym, $eq, $list) = @t;
    refuse($t[0], 'a declaration at the top of the file is a hash of keywords for elements: `my %PILL = (size => 11, ...);`')
        unless is_word($my, 'my') && is_sym($sym, '%') && is_op($eq, '=') && $list && $list->isa('PPI::Structure::List') && @t == 4;
    my $name = substr $sym->content, 1;
    refuse($sym, "`%$name` is declared twice") if exists $bag{$name};
    my @pieces;
    for my $a (split_args($list)) {
        if (!defined $a->{key}) {
            my ($tok) = @{ $a->{toks} };
            refuse($a->{node}, 'a hash of keywords holds `name => value` pairs, or another such hash to merge')
                unless @{ $a->{toks} } == 1 && is_sym($tok, '%');
            my $other = substr $tok->content, 1;
            refuse($tok, "`%$other` is not a hash of keywords declared above this one") unless exists $bag{$other};
            push @pieces, @{ $bag{$other} };
            next;
        }
        push @pieces, $a;
    }
    # A hash keeps one value per key, and the last one written wins.
    my (%seen, @kept);
    for my $p (reverse @pieces) {
        next if $seen{ $p->{key} }++;
        unshift @kept, $p;
    }
    $bag{$name} = \@kept;
}

# --- fields and methods --------------------------------------------------
sub class_body {
    my $pragma;
    for my $st ($class_block->schildren) {
        next if $st->isa('PPI::Statement::Null');
        if ($st->isa('PPI::Statement::Include')) {
            $pragma = 1 if ($st->module // '') eq 'Rakugan';
            constants($st) if ($st->module // '') eq 'constant';
            next;
        }
        my @t = strip_semicolon(sig($st));
        next unless @t;
        # A `method` block and whatever follows it up to the next `;`
        # arrive as one statement, like a `class` block does.
        while (@t) {
            if (is_word($t[0], 'field')) {
                my $n = field(\@t);
                splice @t, 0, $n;
            } elsif (is_word($t[0], 'method')) {
                my $n = method_decl(\@t);
                splice @t, 0, $n;
            } elsif (is_word($t[0], 'ADJUST')) {
                refuse($t[0], '`ADJUST` takes a block') unless $t[1] && $t[1]->isa('PPI::Structure::Block');
                refuse($t[0], 'one `ADJUST` block per class here') if exists $method{'__start'};
                push @methods, $method{'__start'} = { name => '__start', block => $t[1], node => $t[0],
                                                      params => [], ret => undef };
                splice @t, 0, 2;
            } else {
                refuse($t[0], 'inside the class: `use Rakugan;`, `field`, `ADJUST` and `method` only');
            }
        }
    }
    refuse($class_block, "$class_name has no `method view`") unless $view_block;
    refuse($class_block, "`class $class_name` needs `use Rakugan;` as its first line: a Perl import is per "
                       . 'package, and the elements, `empty` and the type names have to be in this one')
        unless $pragma;
}

# `method name { ... }`, `method name :Sig(Int => Str) ($n) { ... }`.
# PPI reads the name and the attribute's colon as one Label, so the two
# shapes are told apart by what the second token is.
sub method_decl {
    my ($toks) = @_;
    my @t = @$toks;
    my ($name, $sig, $taken) = (undef, undef, 0);
    if ($t[1] && $t[1]->isa('PPI::Token::Label')) {
        (my $label = $t[1]->content) =~ s/\s*:\s*\z//;
        refuse($t[1], 'a method is written `method name { ... }` or `method name :Sig(...) ($a) { ... }`')
            unless $label =~ /\A\w+\z/;
        refuse($t[1], "`$label` takes `:Sig(...)`: the types of what it is called with, and after `=>` what it answers")
            unless is_word($t[2], 'Sig') && $t[3] && $t[3]->isa('PPI::Structure::List');
        $name = { content => $label, node => $t[1] };
        $sig = $t[3];
        $taken = 4;
    } else {
        refuse($t[0], 'a method is written `method name { ... }`') unless is_word($t[1]);
        $name = { content => $t[1]->content, node => $t[1] };
        $taken = 2;
    }
    my $params = $t[$taken];
    my @names;
    if ($params && $params->isa('PPI::Structure::List')) {
        @names = param_names($params);
        $taken++;
    }
    my $block = $t[$taken];
    refuse($name->{node}, 'a method needs a block') unless $block && $block->isa('PPI::Structure::Block');
    $taken++;
    my ($ptys, $ret) = sig_types($sig, scalar @names, $name->{node});
    refuse($name->{node}, "a method with parameters says what they are: `method $name->{content} :Sig("
                        . join(', ', ('Int') x @names) . ") (" . join(', ', @names) . ') { ... }`')
        if @names && !$sig;
    refuse($name->{node}, "`$name->{content}` is called with " . scalar(@names) . ' value'
                        . (@names == 1 ? '' : 's') . ", and `:Sig` gives " . scalar(@$ptys) . ' type'
                        . (@$ptys == 1 ? '' : 's'))
        if @$ptys != @names;
    my @pairs = map { [$names[$_], $ptys->[$_]] } 0 .. $#names;
    if ($name->{content} eq 'view') {
        refuse($name->{node}, '`view` is declared twice') if $view_block;
        refuse($name->{node}, '`view` is called with nothing: it answers the whole screen') if @names;
        $view_block = $block;
        return $taken;
    }
    refuse($name->{node}, "`$name->{content}` is declared twice") if exists $method{ $name->{content} };
    push @methods, $method{ $name->{content} } = {
        name => $name->{content}, block => $block, node => $name->{node},
        params => \@pairs, ret => $ret, kids => (grep { $_->[0] =~ /\A\@/ } @pairs) ? 1 : 0,
    };
    return $taken;
}

# The names in a parameter list, `($a, $b)` or `($title, @kids)`.
sub param_names {
    my ($list) = @_;
    my @t = map { sig($_) } $list->schildren;
    my @out;
    for my $t (@t) {
        next if is_op($t, ',');
        refuse($t, 'a parameter here is a scalar (`$n`), and a last one may be `@kids`')
            unless is_sym($t, '$') || is_sym($t, '@');
        push @out, $t->content;
    }
    refuse($list, '`@kids` is the last parameter, and there is one of it')
        if grep { $out[$_] =~ /\A\@/ && $_ != $#out } 0 .. $#out;
    return @out;
}

# `:Sig(Int, Str => Bool)` — the types of the parameters, and after the
# arrow what the method answers.
sub sig_types {
    my ($sig, $want, $at) = @_;
    return ([], undef) unless $sig;
    my (@ptys, $ret, @cur, $after);
    for my $t (map { sig($_) } $sig->schildren) {
        if (is_op($t, ',')) { push @ptys, type_of_words([@cur], $sig) if @cur; @cur = (); next }
        if (is_op($t, '=>')) {
            refuse($t, '`:Sig` has one `=>`, and what it answers is after it') if $after;
            push @ptys, type_of_words([@cur], $sig) if @cur;
            @cur = ();
            $after = 1;
            next;
        }
        push @cur, $t;
    }
    if (@cur) {
        if ($after) { $ret = type_of_words([@cur], $sig) }
        else        { push @ptys, type_of_words([@cur], $sig) }
    }
    # `@kids` carries no type: a slot holds elements, not values.
    push @ptys, 'Slot' while @ptys < $want;
    return (\@ptys, $ret);
}

sub field {
    my ($toks) = @_;
    my @t = @$toks;
    my $sym = $t[1];
    refuse($t[0], 'a field is written `field $name = <literal>;`') unless $sym && $sym->isa('PPI::Token::Symbol');
    my $name = substr $sym->content, 1;
    refuse($sym, "`$name` is declared twice") if exists $field{$name};
    my $eq = $t[2];
    refuse($eq // $sym, 'a field takes no attributes here (`:param`, `:reader`); its type is read from the initializer')
        if $eq && !is_op($eq, '=');
    refuse($sym, 'a field needs an initializer (`= 0`, `= ""`, `= empty(Str)`) — that is where its type comes from')
        unless $eq && @t >= 4;
    my @init = @t[3 .. $#t];
    my ($ty, $pix);
    if ($sym->raw_type eq '@') {
        ($ty, $pix) = list_init(\@init, $sym);
    } elsif ($sym->raw_type eq '%') {
        ($ty, $pix) = hash_init(\@init, $sym);
    } elsif (is_word($init[0]) && $value{ $init[0]->content }) {
        my $env = { ctx => 'fn', vars => {}, nonneg => {}, at => $sym };
        my $i = 0;
        my $v = primary(\$i, \@init, $env);
        refuse($init[$i], 'this does not continue the value') if $i < @init;
        ($ty, $pix) = ($v->{ty}, $v->{pix});
    } elsif (is_word($init[0], 'maybe') || is_word($init[0], 'undef')) {
        ($ty, $pix) = maybe_init(\@init, $sym);
    } elsif (is_word($init[0]) && $model{ $init[0]->content }) {
        # An object the app holds from the start: made with nothing, so
        # its fields start at their defaults; values go in `ADJUST`.
        refuse($init[0], "a field starts as `$init[0]->{content}->new` with no values; give it values in `ADJUST`, "
                       . 'or make it in a handler')
            unless @init == 3 && is_op($init[1], '->') && is_word($init[2], 'new');
        ($ty, $pix) = ($init[0]->content, $init[0]->content . '()');
    } else {
        refuse($init[0], 'a scalar field starts as one literal, or as one of this file\'s own values '
                       . '(`Point->new(x => 3, y => 4)`)')
            if @init > 2 || (@init == 2 && !is_op($init[0], '-'));
        ($ty, $pix) = literal_run(\@init);
    }
    push @fields, $name;
    $field{$name} = { ty => $ty, init => $pix, kind => $sym->raw_type };
    return scalar @t;
}

# `field $sel = maybe(Int);` — a scalar that starts as nothing, and says
# what it may hold. `undef` alone is refused: it says nothing about the type.
sub maybe_init {
    my ($toks, $sym) = @_;
    my ($w, $list) = @$toks;
    refuse($w, '`undef` alone says nothing about the type; say what this may hold: `maybe(Int)`, `maybe(Str)`')
        if is_word($w, 'undef');
    refuse($w, '`maybe` takes the type it may hold: `maybe(Int)`, `maybe(Str)`, `maybe(Node)`')
        unless $list && $list->isa('PPI::Structure::List') && @$toks == 2;
    my @st = $list->schildren;
    refuse($list, '`maybe` takes one type') unless @st == 1;
    return (type_of_words([sig($st[0])], $list) . '?', 'nil');
}

# `field @items = empty(Str);` or `field @xs = ("a", "b");`
sub list_init {
    my ($toks, $sym) = @_;
    my ($w, $list) = @$toks;
    if (is_word($w, 'empty')) {
        refuse($w, '`empty` takes the type it will hold: `empty(Str)`, `empty(Int)`, `empty(ArrayRef[Int])`')
            unless $list && $list->isa('PPI::Structure::List') && @$toks == 2;
        my @st = $list->schildren;
        refuse($list, '`empty` takes one type') unless @st == 1;
        return ('List<' . type_of_words([sig($st[0])], $list) . '>', '[]');
    }
    if ($w && $w->isa('PPI::Structure::List') && @$toks == 1) {
        my @items = split_args($w);
        refuse($w, 'a list that starts empty says what it will hold: `field @items = empty(Str);`') unless @items;
        my ($ty, @pix);
        for my $a (@items) {
            refuse($a->{node}, 'a list literal holds literals') if defined $a->{key};
            my ($t, $p) = list_item($a);
            refuse($a->{node}, "a list holds one type: this one started with $ty and this is $t") if defined $ty && $t ne $ty;
            $ty = $t;
            push @pix, $p;
        }
        return ("List<$ty>", '[' . join(', ', @pix) . ']');
    }
    refuse($sym, 'a list field starts as a literal list or as `empty(Str)`');
}

# One item of a literal list: a literal, or a list of literals for a
# list of lists (`([1, 2], [3, 4])`).
sub list_item {
    my ($a) = @_;
    my @t = @{ $a->{toks} };
    # One of the file's own values, built where it is written.
    if (is_word($t[0]) && $value{ $t[0]->content }) {
        my $i = 0;
        my $v = primary(\$i, \@t, { ctx => 'fn', vars => {}, nonneg => {}, at => $a->{node} });
        refuse($t[$i], 'this does not continue the value') if $i < @t;
        return ($v->{ty}, $v->{pix});
    }
    if (@t == 1 && $t[0]->isa('PPI::Structure::Constructor') && $t[0]->start->content eq '[') {
        my ($ty, @pix);
        for my $inner (split_args($t[0])) {
            my ($t2, $p) = literal_run($inner->{toks});
            refuse($inner->{node}, "a list holds one type: this one started with $ty and this is $t2") if defined $ty && $t2 ne $ty;
            $ty = $t2;
            push @pix, $p;
        }
        refuse($t[0], 'a list inside a list says what it holds; an empty one cannot') unless defined $ty;
        return ("List<$ty>", '[' . join(', ', @pix) . ']');
    }
    return literal_run(\@t);
}

# `field %prices = (apple => 120);` or `field %h = empty(Int);`
sub hash_init {
    my ($toks, $sym) = @_;
    my ($w, $list) = @$toks;
    if (is_word($w, 'empty')) {
        refuse($w, '`empty` takes the type it will hold: `empty(Int)`')
            unless $list && $list->isa('PPI::Structure::List') && @$toks == 2;
        my @st = $list->schildren;
        refuse($list, '`empty` takes one type') unless @st == 1;
        return ('Map<String, ' . type_of_words([sig($st[0])], $list) . '>', '{}');
    }
    refuse($sym, 'a hash field starts as a literal hash or as `empty(Int)`')
        unless $w && $w->isa('PPI::Structure::List') && @$toks == 1;
    my @items = split_args($w);
    refuse($w, 'a hash that starts empty says what it will hold: `field %h = empty(Int);`') unless @items;
    my ($ty, @pix);
    for my $a (@items) {
        refuse($a->{node}, 'a hash holds `key => value` pairs') unless defined $a->{key};
        my ($t, $p) = literal_run($a->{toks});
        refuse($a->{node}, "a hash holds one type: this one started with $ty and this is $t") if defined $ty && $t ne $ty;
        $ty = $t;
        push @pix, map_key($a->{key}, $a->{node}) . ": $p";
    }
    return ("Map<String, $ty>", '{ ' . join(', ', @pix) . ' }');
}

# A map's key in a .pix literal: a plain word stands for itself, and
# anything else is quoted.
sub map_key {
    my ($key, $node) = @_;
    return $key if $key =~ /\A[A-Za-z_]\w*\z/;
    return '"' . pix_text($node, $key) . '"';
}

# `Str`, `Int`, `Num`, `Bool`, `ArrayRef[Int]` — a type written where perl
# reads it as a value.
sub type_of_words {
    my ($toks, $at) = @_;
    my ($w, $param) = @$toks;
    refuse($at, 'a type here is `Int`, `Str`, `Num`, `Bool`, `ArrayRef[...]` or `HashRef[...]`') unless is_word($w);
    if ($w->content eq 'ArrayRef' || $w->content eq 'HashRef') {
        my $outer = $w->content eq 'ArrayRef' ? 'List<%s>' : 'Map<String, %s>';
        refuse($w, '`' . $w->content . '` takes its element type in brackets: `' . $w->content . '[Int]`')
            unless $param && $param->isa('PPI::Structure::Constructor') && @$toks == 2;
        my @st = $param->schildren;
        refuse($param, '`' . $w->content . '` takes one type') unless @st == 1;
        return sprintf $outer, type_of_words([sig($st[0])], $param);
    }
    # `Maybe[Int]`: a value that may be nothing.
    if ($w->content eq 'Maybe') {
        refuse($w, '`Maybe` takes the type it may hold in brackets: `Maybe[Int]`')
            unless $param && $param->isa('PPI::Structure::Constructor') && @$toks == 2;
        my @st = $param->schildren;
        refuse($param, '`Maybe` takes one type') unless @st == 1;
        return type_of_words([sig($st[0])], $param) . '?';
    }
    # A class in this file is a type name too.
    return $w->content if ($value{ $w->content } || $model{ $w->content }) && @$toks == 1;
    refuse($w, 'a type here is `Int`, `Str`, `Num`, `Bool`, `Maybe[...]`, `ArrayRef[...]` or one of this '
             . 'file\'s own classes; `' . $w->content . '` is not one')
        unless $TYPE_WORD{ $w->content } && @$toks == 1;
    return $TYPE_WORD{ $w->content };
}

# A literal, possibly with a minus in front: its type and .pix spelling.
sub literal_run {
    my ($toks) = @_;
    my @t = @$toks;
    if (@t == 2 && is_op($t[0], '-')) {
        refuse($t[0], 'a minus here needs a number after it') unless $t[1]->isa('PPI::Token::Number');
        my ($ty, $pix) = literal($t[1]);
        return ($ty, "-$pix");
    }
    refuse($t[0], 'one literal was expected here') unless @t == 1;
    return literal($t[0]);
}

sub literal {
    my ($tok) = @_;
    if ($tok->isa('PPI::Token::Number')) {
        my $s = $tok->content;
        $s =~ s/_//g;
        refuse($tok, 'a number literal here is a plain decimal') unless $s =~ /\A-?\d+(\.\d+)?\z/;
        return ('Float', $s) if $s =~ /\./;
        # perl grows a whole number past a machine word into one with a
        # fraction; the compiled run holds 64 bits and would wrap.
        refuse($tok, 'a whole number here holds 64 bits, and this one is past that — perl would grow '
                   . 'it into a number with a fraction, which the compiled run cannot follow; write it '
                   . 'with a `.0` to mean that number')
            if length($s =~ s/\A-//r) > 18 && !eval { my $n = $s + 0; "$n" eq $s && $n == int($n) };
        return ('Int', $s);
    }
    if ($tok->isa('PPI::Token::Quote::Single')) {
        return ('String', '"' . pix_text($tok, $tok->literal) . '"');
    }
    if ($tok->isa('PPI::Token::Quote::Double')) {
        my $s = $tok->string;
        refuse($tok, 'a literal here is a plain string; a string with holes goes where a view or a handler reads it') if $s =~ /(?<!\\)[\$\@]/;
        return ('String', '"' . pix_text($tok, unescape($tok, $s)) . '"');
    }
    return ('Bool', $tok->content) if is_word($tok, 'true') || is_word($tok, 'false');
    # A constant's name stands for its literal.
    if (is_word($tok) && (my $c = $constant{ $tok->content })) { return ($c->{ty}, $c->{pix}) }
    refuse($tok, 'a literal (a number, a string, `true` or `false`) was expected here');
}

sub unescape {
    my ($tok, $s) = @_;
    $s =~ s{\\(.)}{
        $1 eq 'n' ? "\n" : $1 eq 't' ? "\t" : $1 =~ /[\\"\$\@]/ ? $1
            : refuse($tok, "the escape `\\$1` is not in the translator")
    }gse;
    return $s;
}

# Text on its way into a .pix string literal.
sub pix_text {
    my ($tok, $s) = @_;
    refuse($tok, 'a string with `#{` in it: that opens a hole in the compiled run') if $s =~ /#\{/;
    $s =~ s/\\/\\\\/g;
    $s =~ s/"/\\"/g;
    $s =~ s/\n/\\n/g;
    $s =~ s/\t/\\t/g;
    return $s;
}

# --- expressions ----------------------------------------------------------
# An expression is a run of significant tokens. `$env` says where it is
# read: in the view (a field reads as `App.name`) or in a handler
# (bare), what is in scope, and what a handler reaching outward
# captures. Answers { ty, pix, lit? } — `lit` is the text of a string
# literal, kept so two of them can be joined and a keyword checked.
my %BP = ('or' => 1, '||' => 1, '//' => 1, 'and' => 2, '&&' => 2,
          '=~' => 7, '!~' => 7,
          '==' => 5, '!=' => 5, '<' => 5, '>' => 5, '<=' => 5, '>=' => 5, 'eq' => 5, 'ne' => 5,
          'lt' => 5, 'gt' => 5, 'le' => 5, 'ge' => 5,
          '+' => 10, '-' => 10, '.' => 10, '*' => 20, '/' => 20, '%' => 20);

sub parse_expr {
    my ($toks, $env) = @_;
    $toks = [unlabel($toks)];
    refuse($env->{at}, 'an expression is missing here') unless @$toks;
    if (my @cut = ternary_cut($toks)) { return conditional($toks, @cut, $env) }
    my $i = 0;
    my $r = expr_bp(\$i, $toks, $env, 0);
    refuse($toks->[$i], 'this does not continue the expression: `' . $toks->[$i]->content . '`') if $i < @$toks;
    return $r;
}

# Where a conditional expression's `?` and its own `:` are, if there is
# one at this level.
sub ternary_cut {
    my ($toks) = @_;
    my ($q) = grep { is_op($toks->[$_], '?') } 0 .. $#$toks;
    return () unless defined $q;
    my $depth = 0;
    for my $i ($q + 1 .. $#$toks) {
        $depth++ if is_op($toks->[$i], '?');
        if (is_op($toks->[$i], ':')) {
            return ($q, $i) if $depth == 0;
            $depth--;
        }
    }
    refuse($toks->[$q], 'a `?` needs its `:`');
}

# `$c ? $a : $b` where a value is wanted. The compiled run has no
# conditional expression, so this becomes a static of its own, called
# with everything the three parts read.
sub conditional {
    my ($toks, $q, $c, $env) = @_;
    my $lift = { by => {}, order => [] };
    my $lenv = { %$env, lift => $lift, no_try => '`?:`' };
    my $cond = parse_expr([@{$toks}[0 .. $q - 1]], { %$lenv, at => $toks->[$q] });
    refuse($toks->[$q], "a `?` asks a bool (got $cond->{ty}); Perl's truthiness of a number or a string "
                      . 'is not in the translator')
        unless $cond->{ty} eq 'Bool';
    my $a = parse_expr([@{$toks}[$q + 1 .. $c - 1]], { %$lenv, at => $toks->[$q] });
    my $b = parse_expr([@{$toks}[$c + 1 .. $#$toks]], { %$lenv, at => $toks->[$c] });
    my $ty = $a->{ty};
    $ty = 'Float' if num_ty($a->{ty}) && num_ty($b->{ty}) && ($a->{ty} eq 'Float' || $b->{ty} eq 'Float');
    refuse($toks->[$c], "both sides of a `?` answer one type (got $a->{ty} and $b->{ty})")
        unless $ty eq $b->{ty} || (num_ty($ty) && num_ty($b->{ty}));
    my $id = '__c' . scalar @lifted;
    push @lifted, {
        name => $id, ret => $ty, async => 0,
        params => [map { [$_, $lift->{by}{$_}{ty}] } @{ $lift->{order} }],
        body => ["if $cond->{pix} {", "  return $a->{pix}", '}', $b->{pix}],
    };
    return { ty => $ty, pix => "Helpers.$id(" . join(', ', map { $lift->{by}{$_}{arg} } @{ $lift->{order} }) . ')' };
}

# Inside a lifted conditional every name it reads becomes one of the
# static's parameters, because a static reaches nothing else.
sub lifted {
    my ($env, $name, $v) = @_;
    return $v unless $env->{lift};
    unless ($env->{lift}{by}{$name}) {
        $env->{lift}{by}{$name} = { ty => $v->{ty}, arg => $v->{pix} };
        push @{ $env->{lift}{order} }, $name;
    }
    return { %$v, pix => $name };
}

sub op_of {
    my ($t) = @_;
    return $t->content if is_op($t) && exists $BP{ $t->content };
    return $t->content if is_word($t) && exists $BP{ $t->content };
    return undef;
}

sub expr_bp {
    my ($ip, $toks, $env, $min) = @_;
    my $lhs = primary($ip, $toks, $env);
    # `$s =~ /pat/` — what follows the operator is a pattern, not an
    # expression, so it is read here rather than by `primary`.
    while ($$ip < @$toks && (is_op($toks->[$$ip], '=~') || is_op($toks->[$$ip], '!~'))) {
        my $op = $toks->[$$ip];
        $$ip += 2;
        $lhs = regexp_op($op, $lhs, $toks->[$$ip - 1], $env);
    }
    # A hash may not hold the key it was asked for. Perl answers undef;
    # the app says what it wants instead, and both runs answer that.
    refuse($toks->[$$ip - 1], 'a hash may not have that key, so say what to answer when it does not: '
                            . '`$prices{$k} // 0`')
        if $lhs->{maybe} && !$env->{allow_maybe} && !($$ip < @$toks && is_op($toks->[$$ip], '//'));
    while ($$ip < @$toks) {
        my $op = $toks->[$$ip];
        my $o = op_of($op);
        last unless defined $o;
        my $bp = $BP{$o};
        last if $bp < $min;
        $$ip++;
        my $rhs = expr_bp($ip, $toks, $env, $bp + 1);
        $lhs = binop($op, $o, $lhs, $rhs, $env);
        # a chain of comparisons (`a < b < c`) is Perl's own mistake; refuse it
        if ($bp == 5 && $$ip < @$toks) {
            my $next = op_of($toks->[$$ip]);
            refuse($toks->[$$ip], 'a comparison does not chain; write `a < b && b < c`') if defined $next && $BP{$next} == 5;
        }
    }
    return $lhs;
}

sub num_ty { my ($t) = @_; $t eq 'Int' || $t eq 'Float' }

# Whether a value of one type can be put where another is wanted: the
# same type, an Int where a Float is, and `undef` or a T where a
# `Maybe[T]` is.
sub fits {
    my ($want, $have) = @_;
    return 1 if $have eq $want;
    return 1 if $want eq 'Float' && $have eq 'Int';
    if ($want =~ /\A(.+)\?\z/) {
        my $inner = $1;
        return 1 if $have eq 'Nil' || $have eq $inner || ($inner eq 'Float' && $have eq 'Int');
    }
    return 0;
}

# What a message adds when the value in hand may be nothing.
sub maybe_hint {
    my ($v, $what) = @_;
    return $v->{ty} =~ /\?\z/ ? "; `defined $what` first, or `$what // <value>` to say what to use instead" : '';
}

sub binop {
    my ($node, $o, $l, $r, $env) = @_;
    if ($o eq '.') {
        if (defined $l->{lit} && defined $r->{lit}) {
            my $lit = $l->{lit} . $r->{lit};
            return { ty => 'String', pix => '"' . pix_text($node, $lit) . '"', lit => $lit, str => 1 };
        }
        refuse($node, "`.` joins strings (got $l->{ty} and $r->{ty}); a number is written into a string with a hole")
            unless $l->{ty} eq 'String' && $r->{ty} eq 'String';
        return { ty => 'String', pix => group($l->{pix}) . ' + ' . group($r->{pix}) };
    }
    if ($o eq '//') {
        # A value that may be nothing: the value, or what stands after
        # `//`. The compiled run has no `??`, so this is a static that
        # looks inside — callable from a view, where a hole has nowhere
        # to put a line of its own.
        if ($l->{ty} =~ /\A(.+)\?\z/ && !$l->{maybe} && !$l->{soft}) {
            my $inner = $1;
            refuse($node, "`//` answers a $inner here, and this is a $r->{ty}") unless fits($inner, $r->{ty});
            my $id = '__m' . scalar @lifted;
            push @lifted, { name => $id, ret => $inner, async => 0, params => [['x', "$inner?"], ['d', $inner]],
                            body => ['if let some(v) = x {', '  return v', '}', 'd'] };
            return { ty => $inner, pix => "Helpers.$id($l->{pix}, $r->{pix})" };
        }
        refuse($node, '`//` answers what a hash or a list holds there, or the value after it: '
                    . '`$prices{$k} // 0`')
            unless ($l->{maybe} || $l->{soft})
                && ($l->{ty} eq $r->{ty} || ($l->{ty} eq 'Float' && $r->{ty} eq 'Int'));
        return { ty => $l->{ty}, pix => "$l->{of}.getOr($l->{key}, $r->{pix})" };
    }
    if ($o eq '%') {
        refuse($node, "`%` needs whole numbers on both sides (got $l->{ty} and $r->{ty})")
            unless $l->{ty} eq 'Int' && $r->{ty} eq 'Int';
        $uses_pl = 1;
        return fallible($env, $node, 'Int', "Pl.modInt($l->{pix}, $r->{pix})",
                        "Pl.tryModInt($l->{pix}, $r->{pix}, " . at_pix($env, $node) . ')');
    }
    if ($o eq '/') {
        refuse($node, "`/` needs numbers on both sides (got $l->{ty} and $r->{ty})") unless num_ty($l->{ty}) && num_ty($r->{ty});
        $uses_pl = 1;
        # Perl's `/` answers a Num even for two whole numbers, and dies on
        # a zero divisor whatever the numbers are; a raw division would
        # answer Inf, so both go through a twin.
        my $fn = $l->{ty} eq 'Int' && $r->{ty} eq 'Int' ? 'DivInt' : 'DivNum';
        return fallible($env, $node, 'Float', 'Pl.' . lcfirst($fn) . "($l->{pix}, $r->{pix})",
                        "Pl.try$fn($l->{pix}, $r->{pix}, " . at_pix($env, $node) . ')');
    }
    if ($o =~ /\A(?:\+|-|\*)\z/) {
        # Two whole numbers written out are worked out here, because the
        # compiled run's own compiler refuses to build an overflow it can
        # see — and that is the dialect's refusal to make, by name.
        if ($l->{pix} =~ /\A-?\d+\z/ && $r->{pix} =~ /\A-?\d+\z/) {
            my ($a, $b) = (Math::BigInt->new($l->{pix}), Math::BigInt->new($r->{pix}));
            my $c = $o eq '+' ? $a + $b : $o eq '-' ? $a - $b : $a * $b;
            refuse($node, "this comes to $c, and a whole number here holds 64 bits — perl would grow it into "
                        . 'a number with a fraction, which the compiled run cannot follow; write it with a '
                        . '`.0` to mean that number')
                if $c > Math::BigInt->new('9223372036854775807') || $c < Math::BigInt->new('-9223372036854775808');
            return { ty => 'Int', pix => "$c" };
        }
        # `0 + $s` is how Perl says "this string as a number", and it is
        # the only place the two mix: perl reads as much of a number off
        # the front as it can, and the twin reads it the same way.
        if ($o eq '+' && $l->{ty} eq 'Int' && $l->{pix} eq '0' && $r->{ty} eq 'String') {
            $uses_pl = 1;
            return { ty => 'Float', pix => "Pl.numOf($r->{pix})" };
        }
        refuse($node, "`$o` needs a number on both sides (got $l->{ty} and $r->{ty}); "
                    . '`0 + $s` reads a string as a number, the way perl does')
            unless num_ty($l->{ty}) && num_ty($r->{ty});
        my $ty = ($l->{ty} eq 'Float' || $r->{ty} eq 'Float') ? 'Float' : 'Int';
        return { ty => $ty, pix => group($l->{pix}) . " $o " . group($r->{pix}) };
    }
    if ($o =~ /\A(?:eq|ne|lt|gt|le|ge)\z/) {
        refuse($node, "`$o` compares strings (got $l->{ty} and $r->{ty})") unless $l->{ty} eq 'String' && $r->{ty} eq 'String';
        refuse($node, "`$o` puts two strings in order, which the compiled run has no comparison for yet; "
                    . 'compare numbers, or ask `eq` / `ne` whether they are the same')
            if $o =~ /\A(?:lt|gt|le|ge)\z/;
        return { ty => 'Bool', pix => group($l->{pix}) . ($o eq 'eq' ? ' == ' : ' != ') . group($r->{pix}) };
    }
    if ($o eq '==' || $o eq '!=') {
        refuse($node, "`$o` compares numbers; for strings write `eq` or `ne`") if $l->{ty} eq 'String' || $r->{ty} eq 'String';
        refuse($node, "`$o` needs two values of one type (got $l->{ty} and $r->{ty})")
            unless $l->{ty} eq $r->{ty} || (num_ty($l->{ty}) && num_ty($r->{ty}));
        return { ty => 'Bool', pix => group($l->{pix}) . " $o " . group($r->{pix}) };
    }
    if ($o =~ /\A(?:<|>|<=|>=)\z/) {
        refuse($node, "`$o` compares numbers (got $l->{ty} and $r->{ty})") unless num_ty($l->{ty}) && num_ty($r->{ty});
        return { ty => 'Bool', pix => group($l->{pix}) . " $o " . group($r->{pix}) };
    }
    # and, or, &&, ||
    my $pix_op = ($o eq 'and' || $o eq '&&') ? '&&' : '||';
    refuse($node, "`$o` joins two bools (got $l->{ty} and $r->{ty}); Perl's truthiness of a number or a string is not in the translator")
        unless $l->{ty} eq 'Bool' && $r->{ty} eq 'Bool';
    return { ty => 'Bool', pix => group($l->{pix}) . " $pix_op " . group($r->{pix}) };
}

# `$s =~ /pat/`, `$s !~ /pat/`, `$s =~ s/pat/repl/r` — the shapes that
# answer a value. The one that writes back is a statement, and
# `substitution` below handles it.
sub regexp_op {
    my ($op, $subject, $tok, $env) = @_;
    refuse($op, 'a pattern is matched against a string (got ' . $subject->{ty} . ')')
        unless $subject->{ty} eq 'String';
    refuse($op, 'what follows `' . $op->content . '` is a pattern: `/\d+/`')
        unless $tok && $tok->isa('PPI::Token::Regexp');
    my ($pat, $flags) = regexp_of($tok, $env);
    $uses_pl = 1;
    if ($tok->isa('PPI::Token::Regexp::Substitute')) {
        refuse($tok, 'a substitution that writes back is a statement of its own; `s/…/…/r` answers a '
                   . 'new string and leaves the old one alone')
            unless $flags =~ /r/;
        return { ty => 'String', pix => subst_call($tok, $pat, $flags, $subject->{pix}, $env) };
    }
    if ($flags =~ /g/) {
        refuse($op, '`!~` asks whether a pattern is absent, and `g` finds every place it is present')
            if $op->content eq '!~';
        (my $keep = $flags) =~ s/g//;
        return { ty => 'List<String>',
                 pix => "Pl.reAll(\"" . pix_text($tok, $pat) . "\", \"$keep\", $subject->{pix})" };
    }
    remember_match($env, $pat, $flags, $subject->{pix});
    my $call = "Pl.reMatches(\"" . pix_text($tok, $pat) . "\", \"$flags\", $subject->{pix})";
    return { ty => 'Bool', pix => $op->content eq '!~' ? "!($call)" : $call };
}

# The pattern and the letters after it, with what PCRE2 does not do
# refused by name rather than run differently.
sub regexp_of {
    my ($tok, $env) = @_;
    my $pat = $tok->get_match_string;
    my $mods = $tok->get_modifiers || {};
    my $flags = join '', sort grep { $mods->{$_} } keys %$mods;
    refuse($tok, 'a pattern here is written out; one built while the app runs would have to be '
               . 'compiled by something the shipped app does not carry')
        if $pat =~ /(?<!\\)[\$\@]\w/;
    refuse($tok, 'a pattern that runs code (`(?{ … })`) is perl\'s own; the compiled run has no perl in it')
        if $pat =~ /\(\?\??\{/;
    refuse($tok, 'a character named with `\\N{…}` is not in the compiled run\'s engine; write the '
               . 'character itself')
        if $pat =~ /\\N\{/;
    refuse($tok, 'the case escapes (`\\l \\u \\L \\U \\F`) are perl\'s own; `uc` and `lc` do the '
               . 'same to a string')
        if $pat =~ /\\[luLUF]/;
    refuse($tok, 'a Unicode property written out in full is not in the compiled run\'s engine; the '
               . 'short name (`\\p{L}`) is')
        if $pat =~ /\\[pP]\{(\w{3,})\}/;
    refuse($tok, '`\\K` inside a look-around is refused by the compiled run\'s engine')
        if $pat =~ /\\K/ && $pat =~ /\(\?<?[=!]/;
    # `e` reaches `subst_call`, which has the better thing to say
    # about it.
    refuse($tok, "the letter `$1` after a pattern is not in the translator")
        if $flags =~ /([^imsxgre])/;
    return ($pat, $flags);
}

# What `$1` and `$+{name}` will be read against, until the next match.
sub remember_match {
    my ($env, $pat, $flags, $subject) = @_;
    return unless $env->{sink};
    %{ $env->{sink} } = (pat => $pat, flags => $flags, subject => $subject);
}

sub subst_call {
    my ($tok, $pat, $flags, $subject, $env) = @_;
    my $repl = $tok->get_substitute_string;
    refuse($tok, 'a replacement here is text, with `$1` … `$9` for what the pattern caught; `/e` runs '
               . 'perl and the compiled run has none')
        if $flags =~ /e/;
    refuse($tok, 'a replacement reads `$1` … `$9`; anything else in it would have to be worked out '
               . 'while the app runs')
        if $repl =~ /(?<!\\)\$(?![1-9])/ || $repl =~ /(?<!\\)\@/;
    (my $keep = $flags) =~ s/[r]//g;
    return "Pl.reSubst(\"" . pix_text($tok, $pat) . "\", \"$keep\", $subject, \"" . pix_text($tok, $repl) . "\")";
}

sub group { my ($s) = @_; return $s =~ / / && $s !~ /\A\(.*\)\z/ && $s !~ /\A"/ ? "($s)" : $s }

sub primary {
    my ($ip, $toks, $env) = @_;
    my $t = $toks->[$$ip];
    refuse($toks->[-1], 'the expression ends early') unless $t;
    if (is_op($t, '-')) {
        $$ip++;
        my $v = expr_bp($ip, $toks, $env, 25);
        refuse($t, "a minus here needs a number after it (got $v->{ty})") unless num_ty($v->{ty});
        return { ty => $v->{ty}, pix => '-' . group($v->{pix}) };
    }
    if (is_op($t, '!') || is_word($t, 'not')) {
        $$ip++;
        my $v = expr_bp($ip, $toks, $env, 30);
        refuse($t, "`" . $t->content . "` negates a bool (got $v->{ty}); Perl's truthiness of a number or a string is not in the translator") unless $v->{ty} eq 'Bool';
        return { ty => 'Bool', pix => "!($v->{pix})" };
    }
    if ($t->isa('PPI::Token::Number') || $t->isa('PPI::Token::Quote::Single')) {
        $$ip++;
        my ($ty, $pix) = literal($t);
        return { ty => $ty, pix => $pix, ($ty eq 'String' ? (lit => $t->literal, str => 1) : ()) };
    }
    if ($t->isa('PPI::Token::Quote::Double')) { $$ip++; return interpolate($t, $env) }
    # `$1` … `$9` and `$+{name}`. Every other magic name (`$_` among
    # them) is a symbol like any other and goes on below.
    if ($t->isa('PPI::Token::Magic') && ($t->content =~ /\A\$[1-9]\z/ || $t->content eq '$+')) {
        my $m = $env->{capture}
            or refuse($t, 'what a pattern caught is read where the match is known to have happened: '
                        . 'inside the `if` that made it');
        if ($t->content =~ /\A\$([1-9])\z/) {
            $$ip++;
            $uses_pl = 1;
            return { ty => 'String',
                     pix => "Pl.reCapture(\"" . pix_text($t, $m->{pat}) . "\", \"$m->{flags}\", "
                          . "$m->{subject}, $1)" };
        }
        if ($t->content eq '$+' && is_sub($toks->[$$ip + 1], '{')) {
            my $sub = $toks->[$$ip + 1];
            my @st = sig(($sub->schildren)[0]);
            refuse($sub, 'a named group is read `$+{name}`') unless @st == 1 && is_word($st[0]);
            $$ip += 2;
            $uses_pl = 1;
            return { ty => 'String',
                     pix => "Pl.reCaptureNamed(\"" . pix_text($t, $m->{pat}) . "\", \"$m->{flags}\", "
                          . "$m->{subject}, \"" . $st[0]->content . "\")" };
        }
        refuse($t, 'the only variables a pattern leaves behind that the translator reads are `$1` … `$9` '
                 . 'and `$+{name}`');
    }
    if ($t->isa('PPI::Token::ArrayIndex')) {
        $$ip++;
        my $name = substr $t->content, 2;
        my $v = read_list($t, $name, $env);
        return { ty => 'Int', pix => "$v->{pix}.length - 1" };
    }
    if (is_word($t, 'true') || is_word($t, 'false')) { $$ip++; return { ty => 'Bool', pix => $t->content } }
    # `undef`: nothing, which goes wherever a value may be nothing.
    if (is_word($t, 'undef')) { $$ip++; return { ty => 'Nil', pix => 'nil' } }
    if ($t->isa('PPI::Token::Word')) {
        # `use constant HAPPY => "happy"` — the name is the literal.
        if (my $c = $constant{ $t->content }) {
            $$ip++;
            return { ty => $c->{ty}, pix => $c->{pix}, ($c->{ty} eq 'String' ? (lit => $c->{lit}, str => 1) : ()) };
        }
        # `Node->new(label => "alpha")` — an object of one of the file's
        # classes with methods, made where it is written.
        if ($model{ $t->content } && is_op($toks->[$$ip + 1], '->') && is_word($toks->[$$ip + 2], 'new')) {
            my $list = $toks->[$$ip + 3];
            $$ip += 3;
            $$ip++ if $list && $list->isa('PPI::Structure::List');
            return build_object($t, ($list && $list->isa('PPI::Structure::List')) ? $list : undef, $env);
        }
        # `Point->new(x => 3, y => 4)` — a value of one of the file's
        # own classes, built where it is written.
        if ($value{ $t->content } && is_op($toks->[$$ip + 1], '->') && is_word($toks->[$$ip + 2], 'new')) {
            my $list = $toks->[$$ip + 3];
            refuse($t, "`$t->{content}` is built with the values of its fields: `"
                     . $t->content . '->new(' . join(', ', map { "$_->{name} => ..." } @{ $value{ $t->content } })
                     . ')`')
                unless $list && $list->isa('PPI::Structure::List');
            $$ip += 4;
            return build_value($t, $list, $env);
        }
        return word_expr($ip, $toks, $env);
    }
    if ($t->isa('PPI::Token::Symbol')) { return symbol_expr($ip, $toks, $env) }
    if ($t->isa('PPI::Structure::List')) {
        $$ip++;
        my @st = $t->schildren;
        refuse($t, 'one expression inside these parentheses') unless @st == 1;
        my $v = parse_expr([sig($st[0])], { %$env, at => $t });
        return { %$v, pix => "($v->{pix})" };
    }
    refuse($t, 'this is not an expression the translator takes: `' . $t->content . '`');
}

# `$x`, `$x[i]`, `$h{k}`, `@xs`, `%h`, `$self->name(...)`.
sub symbol_expr {
    my ($ip, $toks, $env) = @_;
    my $t = $toks->[$$ip];
    my $name = substr $t->content, 1;
    my $kind = $t->raw_type;
    $$ip++;
    # Inside the class the app is `$self`; at the top of the file it is
    # the name the app was given, and a timer reaches its methods there.
    if ($kind eq '$' && $name eq 'self' && $env->{model}) {
        refuse($t, "`\$self` inside `$env->{model}`: a method reaches its own fields by name, and another method of "
                 . 'the same class is not called from here yet');
    }
    if ($kind eq '$' && ($name eq 'self' || (defined $app_var && $name eq $app_var))) {
        my ($arrow, $word) = @{$toks}[$$ip, $$ip + 1];
        refuse($t, "`\$$name` is read for its methods (`\$$name->name`) and nothing else")
            unless is_op($arrow, '->') && is_word($word);
        $$ip += 2;
        my $args = $toks->[$$ip];
        if ($args && $args->isa('PPI::Structure::List')) { $$ip++ } else { $args = undef }
        return method_call($word, $args, $env);
    }
    my $next = $toks->[$$ip];
    # `$row->[0]` — one cell of a row, which is how perl reads a list
    # inside a list.
    if ($kind eq '$' && is_op($next, '->') && is_sub($toks->[$$ip + 1], '[')) {
        my $v = read_var($t, $name, $env);
        refuse($t, "`\$$name` holds a $v->{ty}; `->[...]` reads one of a list inside a list")
            unless $v->{ty} =~ /\AList</;
        $$ip += 2;
        return list_read($t, $v, $toks->[$$ip - 1], $env);
    }
    if ($kind eq '$' && is_op($next, '->') && is_word($toks->[$$ip + 1])) {
        my $v = read_var($t, $name, $env);
        if ($model{ $v->{ty} }) {
            $$ip += 2;
            return chain(member($v, $toks->[$$ip - 1], $ip, $toks, $env), $ip, $toks, $env);
        }
        refuse($t, "`\$$name` may be nothing; read it inside `if (defined \$$name)`, where it is the object")
            if $v->{ty} =~ /\A(\w+)\?\z/ && $model{$1};
        if (my $fields = $value{ $v->{ty} }) {
            my $f = $toks->[$$ip + 1];
            my ($fd) = grep { $_->{name} eq $f->content } @$fields;
            refuse($f, "`$v->{ty}` has no field `" . $f->content . '`') unless $fd;
            $$ip += 2;
            refuse($toks->[$$ip], 'a value is read, not called') if is_sub($toks->[$$ip], '(');
            return { ty => $fd->{ty}, pix => "$v->{pix}.$f->{content}" };
        }
    }
    if ($kind eq '$' && is_sub($next, '[')) {
        $$ip++;
        my $lv = read_list($t, $name, $env);
        return list_read($t, $lv, $next, $env);
    }
    if ($kind eq '$' && is_sub($next, '{')) {
        $$ip++;
        my $mv = read_map($t, $name, $env);
        my $key = subscript_key($next, $env);
        my $inner = $mv->{ty} =~ /\AMap<String, (.+)>\z/ ? $1 : refuse($t, "`%$name` is not a map");
        return { ty => $inner, pix => $mv->{pix} . "[$key]", maybe => 1, of => $mv->{pix}, key => $key };
    }
    if ($kind eq '@') { return read_list($t, $name, $env) }
    if ($kind eq '%') { return read_map($t, $name, $env) }
    return read_var($t, $name, $env);
}

# One element of a list. Perl counts a negative index from the end, so
# a statement can make that good with a line before it; an expression
# in a view has nowhere to put one.
sub list_read {
    my ($node, $lv, $sub, $env) = @_;
    my $inner = $lv->{ty} =~ /\AList<(.+)>\z/ ? $1 : refuse($node, "this is a $lv->{ty}, not a list");
    my $ix = parse_expr([sig(($sub->schildren)[0])], { %$env, at => $sub });
    refuse($sub, "a list is read with a whole number (got $ix->{ty})") unless $ix->{ty} eq 'Int';
    # The repeater's own element, which is what the .pix `for` hands over.
    my $rep = $env->{repeat};
    return { ty => $inner, pix => $rep->{it} }
        if $rep && $rep->{list} && $rep->{list} eq $lv->{pix} && $ix->{pix} eq $rep->{ix};
    # A list read stands on its own: an app indexes a list it knows the
    # length of. `// value` is how it says the row may not be there.
    my $at = guard_index($ix, $lv, $env, $sub);
    return { ty => $inner, pix => $lv->{pix} . "[$at]", soft => 1, of => $lv->{pix}, key => $at };
}

# The index as the compiled run must read it: as written when it cannot
# be negative, and otherwise through a line that adds the length first.
sub guard_index {
    my ($ix, $lv, $env, $node) = @_;
    return $ix->{pix} if $ix->{pix} =~ /\A\d+\z/;
    return $ix->{pix} if $env->{nonneg} && $env->{nonneg}{ $ix->{pix} };
    if ($env->{ctx} eq 'view') {
        refuse($node, 'an index a view cannot prove is not negative; a row reads its own index, '
                    . 'and anything else is worked out in a handler');
    }
    my $g = '__ix' . ++$tmp;
    push @{ $env->{pre} }, "var $g = $ix->{pix}", "if $g < 0 {", "  $g = $g + $lv->{pix}.length", '}';
    return $g;
}

sub subscript_key {
    my ($sub, $env) = @_;
    my @t = sig(($sub->schildren)[0]);
    return map_key($t[0]->content, $t[0]) if @t == 1 && is_word($t[0]);
    my $v = parse_expr(\@t, { %$env, at => $sub });
    refuse($sub, "a map here is read with a string (got $v->{ty})") unless $v->{ty} eq 'String';
    return $v->{pix};
}

# `Point->new(x => 3, y => 4)`, in the order the class declares them.
sub build_value {
    my ($word, $list, $env) = @_;
    my $name = $word->content;
    my %given;
    for my $a (split_args($list)) {
        refuse($a->{node}, "`$name` is built by naming its fields: `$name->new(x => 3)`") unless defined $a->{key};
        refuse($a->{node}, "`$name` has no field `$a->{key}`")
            unless grep { $_->{name} eq $a->{key} } @{ $value{$name} };
        refuse($a->{node}, "`$name` was given `$a->{key}` twice") if exists $given{ $a->{key} };
        $given{ $a->{key} } = $a;
    }
    my @vals;
    for my $f (@{ $value{$name} }) {
        my $a = $given{ $f->{name} }
            or refuse($word, "`$name` is built with every one of its fields; `$f->{name}` is missing");
        my $v = parse_expr($a->{toks}, { %$env, at => $a->{node} });
        refuse($a->{node}, "`$name`'s `$f->{name}` holds a $f->{ty}, and this is a $v->{ty}")
            unless fits($f->{ty}, $v->{ty});
        push @vals, $v->{pix};
    }
    return { ty => $name, pix => "$name(" . join(', ', @vals) . ')' };
}

sub read_var {
    my ($node, $name, $env) = @_;
    my $v = read_var_raw($node, $name, $env);
    # Inside `if (defined $x)` the name reads as the value it holds.
    if ($env->{narrowed} && (my $nw = $env->{narrowed}{ $v->{pix} })) {
        return { ty => $nw->{ty}, pix => $nw->{pix}, narrowed => $name };
    }
    return $v;
}

sub read_var_raw {
    my ($node, $name, $env) = @_;
    # A field of the class whose method this is, when that class is not
    # the app: its fields are the object's own, read by name.
    if (my $mname = $env->{model}) {
        my $m = $model{$mname};
        if (my $fd = $m->{by}{$name}) { return { ty => $fd->{ty}, pix => $name } }
        return { ty => $env->{vars}{$name}{ty}, pix => $env->{vars}{$name}{pix} } if $env->{vars}{$name};
        refuse($node, "`\$$name` is not a field of $mname, nor anything in scope here");
    }
    if (my $v = $env->{vars}{$name}) { return lifted($env, $name, { ty => $v->{ty}, pix => $v->{pix} }) }
    if ($env->{outer} && $env->{outer}{$name}) {
        my $v = $env->{outer}{$name};
        capture($env, $name, $v);
        return { ty => $v->{ty}, pix => $name };
    }
    if (exists $field{$name} && $field{$name}{kind} eq '$') {
        return lifted($env, $name, { ty => $field{$name}{ty}, pix => $env->{ctx} eq 'view' ? "App.$name" : $name });
    }
    # A name for a literal is the literal.
    if (my $c = $const{$name}) {
        return { ty => $c->{ty}, pix => $c->{pix},
                 ($c->{ty} eq 'String' ? (lit => $c->{lit}, str => 1) : ()) };
    }
    refuse($node, "`\$$name` is not a field of $class_name" . ($env->{ctx} eq 'fn' ? ' nor anything in scope here' : ''));
}

sub read_list {
    my ($node, $name, $env) = @_;
    refuse($node, "`\@$name` is not in scope here; a class with methods holds scalars, and a list belongs to the app")
        if $env->{model} && !$env->{vars}{$name} && !($env->{outer} && $env->{outer}{$name});
    if (my $v = $env->{vars}{$name}) {
        refuse($node, "`\@$name` holds a $v->{ty}") unless $v->{ty} =~ /\AList</;
        return lifted($env, $name, { ty => $v->{ty}, pix => $v->{pix} });
    }
    if ($env->{outer} && $env->{outer}{$name}) {
        my $v = $env->{outer}{$name};
        capture($env, $name, $v);
        return { ty => $v->{ty}, pix => $name };
    }
    refuse($node, "`\@$name` is not a list field of $class_name")
        unless exists $field{$name} && $field{$name}{kind} eq '@';
    return lifted($env, $name, { ty => $field{$name}{ty}, pix => $env->{ctx} eq 'view' ? "App.$name" : $name });
}

sub read_map {
    my ($node, $name, $env) = @_;
    refuse($node, "`%$name` is not in scope here; a class with methods holds scalars, and a hash belongs to the app")
        if $env->{model} && !$env->{vars}{$name};
    if (my $v = $env->{vars}{$name}) {
        refuse($node, "`%$name` holds a $v->{ty}") unless $v->{ty} =~ /\AMap</;
        return { ty => $v->{ty}, pix => $v->{pix} };
    }
    refuse($node, "`%$name` is not a hash field of $class_name")
        unless exists $field{$name} && $field{$name}{kind} eq '%';
    return lifted($env, $name, { ty => $field{$name}{ty}, pix => $env->{ctx} eq 'view' ? "App.$name" : $name });
}

# A handler reaching a name the view around it holds: the store fn it
# becomes takes that value as a parameter, and the call site hands it
# over.
sub capture {
    my ($env, $name, $v) = @_;
    return if $env->{captures}{$name};
    $env->{captures}{$name} = $v;
    push @{ $env->{capture_order} }, $name;
}

# `$self->name(...)`: a component is not a value, a store fn cannot be
# read while a view is being built, and everything else is a call.
sub method_call {
    my ($word, $args, $env) = @_;
    my $name = $word->content;
    my $m = $method{$name} or refuse($word, "`$name` is not a method of $class_name");
    refuse($word, "`$name` answers an element; it belongs where a child goes, not inside an expression") if $m->{element};
    refuse($word, "`$name` touches the app's state, and building a view only reads; "
                . 'give it what it needs as parameters, or read a field')
        if $m->{stateful} && $env->{ctx} eq 'view';
    refuse($word, "`$name` can fail — it divides, takes a root, writes `die` or calls the library — and a "
                . "`try` here does not reach into it yet; put the `try` inside `$name`, around the line that can fail")
        if $env->{try} && $m->{may_fail};
    my @vals = call_args($word, $args, $m, $env);
    refuse($word, "`$name` does not say what it answers; write `:Sig("
                . join(', ', map { $_->[1] } @{ $m->{params} }) . " => Str)`")
        if !defined $m->{ret} && !$env->{as_statement};
    my $recv = $m->{stateful} ? 'App' : 'Helpers';
    return { ty => $m->{ret} // 'Void', pix => "$recv.$name(" . join(', ', @vals) . ')' };
}

sub call_args {
    my ($word, $args, $m, $env) = @_;
    my @given = $args ? split_args($args) : ();
    refuse($word, "`$m->{name}` takes " . scalar(@{ $m->{params} }) . ' argument'
                . (@{ $m->{params} } == 1 ? '' : 's'))
        if @given != @{ $m->{params} };
    my @vals;
    for my $i (0 .. $#given) {
        refuse($given[$i]{node}, "`$m->{name}` takes its arguments in order, without names") if defined $given[$i]{key};
        my ($pname, $pty) = @{ $m->{params}[$i] };
        my $v = parse_expr($given[$i]{toks}, { %$env, at => $given[$i]{node} });
        refuse($given[$i]{node}, "`$m->{name}` takes a $pty here (got $v->{ty})" . maybe_hint($v, 'it'))
            unless fits($pty, $v->{ty});
        push @vals, $v->{pix};
    }
    return @vals;
}

# The words that are not variables: what Perl gives an app for nothing.
sub word_expr {
    my ($ip, $toks, $env) = @_;
    my $w = $toks->[$$ip];
    my $name = $w->content;
    my $next = $toks->[$$ip + 1];
    if ($name eq 'scalar') {
        $$ip++;
        my $v = expr_bp($ip, $toks, { %$env, sorted => 1 }, 30);
        refuse($w, "`scalar` here counts a list or a hash (got $v->{ty})") unless $v->{ty} =~ /\A(?:List|Map)</;
        # `scalar keys %h` is how many the hash holds.
        (my $of = $v->{pix}) =~ s/\.keys\z//;
        return { ty => 'Int', pix => "$of.length" };
    }
    if ($name eq 'keys' || $name eq 'values') {
        $$ip++;
        refuse($w, "a hash hands `$name` back in the order perl happens to hold it, which is a "
                 . 'different order every time perl starts; write `sort keys %h`')
            unless $env->{sorted};
        my $v = expr_bp($ip, $toks, $env, 30);
        refuse($w, "`$name` reads a hash (got $v->{ty})") unless $v->{ty} =~ /\AMap<String, (.+)>\z/;
        my $inner = $name eq 'keys' ? 'String' : $1;
        return { ty => "List<$inner>", pix => "$v->{pix}.$name" };
    }
    if ($name eq 'sort') {
        $$ip++;
        my $desc = 0;
        my $how;
        if ($next && $next->isa('PPI::Structure::Block')) {
            $$ip++;
            ($how, $desc) = sort_block($next);
        }
        my $v = expr_bp($ip, $toks, { %$env, sorted => 1 }, 30);
        my $inner = $v->{ty} =~ /\AList<(.+)>\z/ ? $1 : refuse($w, "`sort` puts a list in order (got $v->{ty})");
        refuse($w, 'a hash hands its keys back in a different order every time perl starts, so they are '
                 . 'read `sort keys %h`; sorting them again is the same list')
            if $v->{pix} =~ /\.keys\z/ && $how;
        return { ty => $v->{ty}, pix => $v->{pix} } if $v->{pix} =~ /\.keys\z/;
        if (!$how) {
            refuse($w, "a bare `sort` puts things in the order of their text, which for numbers is not "
                     . "their order (perl's `sort 10, 9` is `10, 9`); write `sort { \$a <=> \$b } \@xs`")
                unless $inner eq 'String';
        } elsif ($how eq 'num') {
            refuse($w, '`<=>` compares numbers, and this list holds ' . $inner) unless num_ty($inner);
        } else {
            refuse($w, '`cmp` compares text, and this list holds ' . $inner) unless $inner eq 'String';
        }
        return { ty => $v->{ty}, pix => "$v->{pix}.sortedBy($v->{pix}, " . ($desc ? 'true' : 'false') . ')' };
    }
    if ($name eq 'exists') {
        $$ip++;
        my $v = expr_bp($ip, $toks, { %$env, allow_maybe => 1 }, 30);
        refuse($w, '`exists` asks a hash whether it has a key: `exists $prices{$k}`') unless $v->{maybe};
        return { ty => 'Bool', pix => "$v->{of}.contains($v->{key})" };
    }
    if ($name eq 'defined') {
        $$ip++;
        my $v = expr_bp($ip, $toks, { %$env, allow_maybe => 1 }, 30);
        return { ty => 'Bool', pix => "$v->{of}.contains($v->{key})" } if $v->{maybe};
        # A value that may be nothing, asked as a bool rather than as
        # the condition of an `if`: the answer is worked out first.
        if ($v->{ty} =~ /\?\z/) {
            refuse($w, 'in a view, `defined` is the condition of an `if` or `unless`, whose branch reads the value')
                if $env->{ctx} eq 'view';
            my $b = '__b' . ++$tmp;
            push @{ $env->{pre} }, "var $b = false", "if let some(__v$tmp) = $v->{pix} {", "  $b = true", '}';
            return { ty => 'Bool', pix => $b };
        }
        refuse($w, '`defined` asks whether a value that may be nothing is there (`defined $sel`), or whether a '
                 . 'hash has a key (`defined $prices{$k}`); this can be neither');
    }
    if ($name eq 'join') {
        $$ip++;
        refuse($w, '`join` is called with parentheses: `join(", ", @xs)`')
            unless $next && $next->isa('PPI::Structure::List');
        $$ip++;
        my @a = split_args($next);
        refuse($w, '`join` takes what goes between, and a list: `join(", ", @xs)`') unless @a == 2;
        my $sep = parse_expr($a[0]{toks}, { %$env, at => $a[0]{node} });
        my $lst = parse_expr($a[1]{toks}, { %$env, at => $a[1]{node} });
        refuse($a[0]{node}, "`join` puts a string between (got $sep->{ty})") unless $sep->{ty} eq 'String';
        refuse($a[1]{node}, '`join` joins a list of strings') unless $lst->{ty} eq 'List<String>';
        $uses_pl = 1;
        return { ty => 'String', pix => "Pl.join($sep->{pix}, $lst->{pix})" };
    }
    if ($name eq 'sprintf') {
        $$ip++;
        refuse($w, '`sprintf` is called with parentheses') unless $next && $next->isa('PPI::Structure::List');
        $$ip++;
        return sprintf_call($w, $next, $env);
    }
    if ($name eq 'int') {
        $$ip++;
        refuse($w, '`int` is called with parentheses: `int($x)`') unless $next && $next->isa('PPI::Structure::List');
        $$ip++;
        my @a = split_args($next);
        refuse($w, '`int` takes one number') unless @a == 1;
        my $v = parse_expr($a[0]{toks}, { %$env, at => $a[0]{node} });
        return $v if $v->{ty} eq 'Int';
        refuse($a[0]{node}, "`int` takes a number (got $v->{ty})") unless $v->{ty} eq 'Float';
        $uses_pl = 1;
        return { ty => 'Int', pix => "Pl.intOf($v->{pix})" };
    }
    # The framework's own standard library: one implementation that
    # both runs land on, reached here by the name the manifest gives it.
    if ($Rakugan::Manifest::NAMES{$name}) {
        $$ip++;
        refuse($w, "`$name` is called with parentheses") unless $next && $next->isa('PPI::Structure::List');
        $$ip++;
        return framework_call($name, $w, $next, $env);
    }
    if ($name eq 'quit') {
        $$ip++;
        refuse($w, '`quit` is called with nothing: `quit()`')
            unless $next && $next->isa('PPI::Structure::List') && !$next->schildren;
        $$ip++;
        refuse($w, '`quit` closes the window, and building a view only reads') if $env->{ctx} eq 'view';
        $uses_std = 1;
        return { ty => 'Int', pix => 'Py.quit()' };
    }
    if (my $u = $UNARY{$name}) {
        $$ip++;
        my $v = one_arg($ip, $toks, $env, $w, $name);
        refuse($w, "`$name` takes a $u->[0] (got $v->{ty})")
            unless $v->{ty} eq $u->[0] || ($u->[0] eq 'Float' && $v->{ty} eq 'Int');
        $uses_pl = 1;
        # perl dies on the root of a negative number.
        return fallible($env, $w, 'Float', "Pl.sqrtOf($v->{pix})", "Pl.trySqrt($v->{pix}, " . at_pix($env, $w) . ')')
            if $name eq 'sqrt';
        return { ty => $u->[1], pix => "Pl.$u->[2]($v->{pix})" };
    }
    if ($name eq 'abs') {
        $$ip++;
        my $v = one_arg($ip, $toks, $env, $w, $name);
        refuse($w, "`abs` takes a number (got $v->{ty})") unless num_ty($v->{ty});
        $uses_pl = 1;
        return { ty => $v->{ty}, pix => 'Pl.' . ($v->{ty} eq 'Int' ? 'absInt' : 'absNum') . "($v->{pix})" };
    }
    if ($name eq 'reverse') {
        $$ip++;
        my $v = one_arg($ip, $toks, $env, $w, $name);
        $uses_pl = 1;
        return { ty => 'String', pix => "Pl.reverseStr($v->{pix})" } if $v->{ty} eq 'String';
        refuse($w, "`reverse` turns a string or a list around (got $v->{ty})") unless $v->{ty} =~ /\AList</;
        return { ty => $v->{ty}, pix => "$v->{pix}.reversed()" };
    }
    if ($name =~ /\A(?:sum|max|min|uniq)\z/) {
        $$ip++;
        my $v = one_arg($ip, $toks, $env, $w, $name, 1);
        my $inner = $v->{ty} =~ /\AList<(.+)>\z/
            ? $1 : refuse($w, "`$name` reads a list (got $v->{ty})");
        refuse($w, "`$name` reads a list of numbers or, for `uniq`, of strings; this one holds $inner")
            unless $inner eq 'Int' || $inner eq 'Float' || ($name eq 'uniq' && $inner eq 'String');
        my %by = (Int => 'Int', Float => 'Num', String => 'Str');
        my $fn = $name . $by{$inner};
        $fn = 'uniqStr' if $name eq 'uniq' && $inner eq 'String';
        $uses_pl = 1;
        return { ty => ($name eq 'uniq' ? $v->{ty} : $inner), pix => "Pl.$fn($v->{pix})" };
    }
    if ($name eq 'split' && $next && $next->isa('PPI::Token::Regexp::Match')) {
        $$ip += 2;
        my ($pat, $flags) = regexp_of($next, $env);
        refuse($next, '`split` takes a pattern and the string: `split /,\\s*/, $line`')
            unless is_op($toks->[$$ip], ',');
        $$ip++;
        my $v = expr_bp($ip, $toks, $env, 3);
        refuse($w, "`split` reads a string (got $v->{ty})") unless $v->{ty} eq 'String';
        $uses_pl = 1;
        return { ty => 'List<String>',
                 pix => "Pl.reSplit(\"" . pix_text($next, $pat) . "\", \"$flags\", $v->{pix})" };
    }
    if ($name =~ /\A(?:substr|index|rindex|split|fmod|strftime)\z/) {
        $$ip++;
        refuse($w, "`$name` is called with parentheses here") unless $next && $next->isa('PPI::Structure::List');
        $$ip++;
        return perl_call($name, $w, $next, $env);
    }
    refuse($w, $NOT_TAKEN{$name}) if $NOT_TAKEN{$name};
    refuse($w, "`$name` is not something the translator knows; a method of the app is called `\$self->$name`");
}

# `sort { $a <=> $b } @xs` and its three siblings: which way round, and
# whether the comparison is of numbers or of text.
sub sort_block {
    my ($block) = @_;
    my @st = $block->schildren;
    refuse($block, 'a `sort` block compares `$a` and `$b`') unless @st == 1;
    my @t = strip_semicolon(sig($st[0]));
    refuse($block, 'a `sort` block is `{ $a <=> $b }` or `{ $a cmp $b }`, either way round')
        unless @t == 3 && is_sym($t[0], '$') && is_sym($t[2], '$')
            && (is_op($t[1], '<=>') || is_word($t[1], 'cmp') || is_op($t[1], 'cmp'));
    my ($l, $r) = ($t[0]->content, $t[2]->content);
    refuse($block, 'a `sort` block compares `$a` and `$b`, and nothing else')
        unless ($l eq '$a' && $r eq '$b') || ($l eq '$b' && $r eq '$a');
    my $how = is_op($t[1], '<=>') ? 'num' : 'text';
    return ($how, $l eq '$b' ? 1 : 0);
}

# `grep { ... } @xs`, `map { ... } @xs` and `(first { ... } @xs) // $d`
# where a value is being given a name. Each becomes a loop, because
# that is what it is; the lines go in front of the statement.
sub pipeline_rhs {
    my ($toks, $env, $node) = @_;
    my @t = @$toks;
    # `(first { ... } @xs) // $default`
    if (@t >= 3 && $t[0]->isa('PPI::Structure::List') && is_op($t[1], '//')) {
        my @inner = map { sig($_) } $t[0]->schildren;
        return undef unless @inner && is_word($inner[0], 'first');
        return first_of(\@inner, [@t[2 .. $#t]], $env, $node);
    }
    # `my @found = ($line =~ /(\d+)/g);` — a match in the place a list
    # goes, which perl reads as every place the pattern matched.
    if (@t == 1 && $t[0]->isa('PPI::Structure::List')) {
        my @a = split_args($t[0]);
        if (@a == 1 && grep { is_op($_, '=~') } @{ $a[0]{toks} }) {
            return parse_expr($a[0]{toks}, { %$env, at => $a[0]{node} });
        }
        # `(0 .. $n)` where a list goes: the run of numbers, written out.
        if (@a == 1 && grep { is_op($_, '..') } @{ $a[0]{toks} }) {
            refuse($a[0]{node}, 'a run of numbers becomes a list where a handler can build one; a view '
                              . 'repeats over a list it already holds')
                if $env->{ctx} eq 'view';
            my $over = list_source($a[0]{toks}, $env, $a[0]{node});
            my $it = '__it' . ++$tmp;
            my $out = '__lc' . ++$tmp;
            push @{ $env->{pre} }, "var $out : List<Int> = []",
                "for $it in $over->{over} {", "  $out.push($it)", '}';
            return { ty => 'List<Int>', pix => $out };
        }
    }
    return undef unless is_word($t[0]) && $t[0]->content =~ /\A(?:grep|map)\z/;
    my ($w, $block, @rest) = @t;
    refuse($w, "`" . $w->content . '` takes a block and a list: `grep { $_ > 5 } @xs`')
        unless $block && $block->isa('PPI::Structure::Block') && @rest;
    refuse($w, "`" . $w->content . '` builds a list here, and a view has nowhere to build one; '
             . 'a view repeats over a list it already holds')
        if $env->{ctx} eq 'view';
    my $over = list_source(\@rest, $env, $w);
    my @st = $block->schildren;
    refuse($block, "`" . $w->content . '` takes one expression in its block') unless @st == 1;
    my $it = '__it' . ++$tmp;
    # What the block's own expression needs said first belongs INSIDE
    # the loop: it reads the row.
    my @first;
    my $inner = { %$env, pre => \@first, in_loop => 1,
                  vars => { %{ $env->{vars} }, '_' => { ty => $over->{ty}, pix => $it, fixed => 1 } } };
    my $out = '__lc' . ++$tmp;
    my @body = strip_semicolon(sig($st[0]));
    if ($w->content eq 'grep') {
        my $cond = cond_of(\@body, $inner, $block);
        my $ty = $over->{listty} // "List<$over->{ty}>";
        push @{ $env->{pre} }, "var $out : $ty = []", "for $it in $over->{over} {",
            (map { "  $_" } @first), "  if $cond {", "    $out.push($it)", '  }', '}';
        return { ty => $ty, pix => $out };
    }
    my $v = parse_expr(\@body, { %$inner, at => $block });
    push @{ $env->{pre} }, "var $out : List<$v->{ty}> = []", "for $it in $over->{over} {",
        (map { "  $_" } @first), "  $out.push($v->{pix})", '}';
    return { ty => "List<$v->{ty}>", pix => $out };
}

# `(first { ... } @xs) // $default` — perl answers undef when nothing
# matches, and the dialect has no undef, so the app says what to
# answer instead.
sub first_of {
    my ($toks, $default, $env, $node) = @_;
    my ($w, $block, @rest) = @$toks;
    refuse($w, '`first` takes a block and a list: `(first { $_ > 5 } @xs) // -1`')
        unless $block && $block->isa('PPI::Structure::Block') && @rest;
    refuse($w, '`first` looks through a list here, and a view has nowhere to do that')
        if $env->{ctx} eq 'view';
    my $over = list_source(\@rest, $env, $w);
    refuse($w, '`first` walks a list') unless $over->{list};
    my $d = parse_expr($default, { %$env, at => $node });
    refuse($node, "`first` answers a $over->{ty}, and after `//` this is a $d->{ty}")
        unless $d->{ty} eq $over->{ty} || ($over->{ty} eq 'Float' && $d->{ty} eq 'Int');
    my @st = $block->schildren;
    refuse($block, '`first` takes one expression in its block') unless @st == 1;
    my $it = '__it' . ++$tmp;
    my @first;
    my $inner = { %$env, pre => \@first, in_loop => 1,
                  vars => { %{ $env->{vars} }, '_' => { ty => $over->{ty}, pix => $it, fixed => 1 } } };
    my $out = '__f' . ++$tmp;
    my $cond = cond_of([strip_semicolon(sig($st[0]))], $inner, $block);
    push @{ $env->{pre} }, "var $out : $over->{ty} = $d->{pix}", "for $it in $over->{over} {",
        (map { "  $_" } @first), "  if $cond {", "    $out = $it", '    break', '  }', '}';
    return { ty => $over->{ty}, pix => $out };
}

# One of the framework's own. The row is picked by how many values
# came with the call, the way `sqlite_exec` takes a statement with or
# without values to bind.
sub framework_call {
    my ($name, $w, $list, $env) = @_;
    my @a = split_args($list);
    my $row = $Rakugan::Manifest::BY_CALL{"$name/" . scalar @a};
    unless ($row) {
        my %n = map { scalar(@{ $_->{kinds} }) => 1 }
                grep { "$_->{module}_$_->{name}" eq $name } @Rakugan::Manifest::ROWS;
        refuse($w, "`$name` takes " . join(' or ', sort keys %n) . ' values, and got ' . scalar(@a));
    }
    refuse($w, "`$name` reaches outside the app, and building a view only reads; call it from a "
             . 'handler and keep what it answered in a field')
        if $env->{ctx} eq 'view' && !$row->{pure};
    my %want = (str => 'String', int => 'Int', num => 'Float', list => 'List<String>');
    my @vals;
    for my $i (0 .. $#a) {
        refuse($a[$i]{node}, "`$name` takes its values in order, without names") if defined $a[$i]{key};
        my $ty = $want{ $row->{kinds}[$i] };
        my $v;
        if ($ty eq 'List<String>' && @{ $a[$i]{toks} } == 2 && $a[$i]{toks}[0]->isa('PPI::Token::Cast')) {
            my ($bs, $sym) = @{ $a[$i]{toks} };
            refuse($a[$i]{node}, "`$name` takes a list of strings here") unless is_sym($sym, '@');
            $v = read_list($sym, substr($sym->content, 1), $env);
        } elsif ($ty eq 'List<String>' && @{ $a[$i]{toks} } == 1
                 && $a[$i]{toks}[0]->isa('PPI::Structure::Constructor')
                 && $a[$i]{toks}[0]->start->content eq '[') {
            my ($lty, $pix) = list_literal($a[$i]{toks}[0], $env);
            $v = { ty => $lty, pix => $pix };
        } else {
            $v = parse_expr($a[$i]{toks}, { %$env, at => $a[$i]{node} });
        }
        refuse($a[$i]{node}, "`$name` takes a $ty here (got $v->{ty})")
            unless $v->{ty} eq $ty || ($ty eq 'Float' && $v->{ty} eq 'Int');
        push @vals, $v->{pix};
    }
    $uses_std = 1;
    my $ty = length $row->{ret_ty} ? $row->{ret_ty} : 'Void';
    # Inside work the app started, a call that waits is handed to the
    # engine's own pool rather than held on the window's thread.
    my $wait = $env->{awaiting} ? 'await ' : '';
    my $args = join(', ', @vals);
    if ($env->{try}) {
        # The `!T` twin the manifest marks, matched on the spot; a row
        # that can fail without one has nothing the catch could receive.
        return fallible($env, $w, $ty, "$wait$row->{class}.$row->{fn}($args)",
                        "$wait$row->{class}.try" . ucfirst($row->{fn}) . "($args)", at_pix($env, $w))
            if $row->{try};
        refuse($w, "`$name` can fail, and the library has no form of it a `try` can take yet; call it "
                 . 'before the `try`, or write its `_or` twin')
            unless cannot_fail($name);
    }
    return { ty => $ty, pix => "$wait$row->{class}.$row->{fn}($args)" };
}

# What a named unary operator was given: `uc $s` and `uc($s)` both.
sub one_arg {
    my ($ip, $toks, $env, $w, $name, $listy) = @_;
    my $t = $toks->[$$ip];
    if ($t && $t->isa('PPI::Structure::List')) {
        my @a = split_args($t);
        $$ip++;
        # `max($a, $b)` reads a list of two, the way perl reads it.
        if (@a > 1) {
            refuse($w, "`$name` takes one list, or the values themselves") unless $listy;
            my ($ty, $pix);
            for my $arg (@a) {
                my $v = parse_expr($arg->{toks}, { %$env, at => $arg->{node} });
                refuse($arg->{node}, "`$name` reads one type: this started with $ty and this is $v->{ty}")
                    if defined $ty && $v->{ty} ne $ty;
                $ty = $v->{ty};
                $pix = defined $pix ? "$pix, $v->{pix}" : $v->{pix};
            }
            return { ty => "List<$ty>", pix => "[$pix]" };
        }
        refuse($w, "`$name` takes one value") unless @a == 1;
        return parse_expr($a[0]{toks}, { %$env, at => $a[0]{node} });
    }
    return expr_bp($ip, $toks, $env, 30);
}

# The rest of Perl's own, each called with parentheses.
sub perl_call {
    my ($name, $w, $list, $env) = @_;
    my @a = split_args($list);
    return strftime_call($w, \@a, $env) if $name eq 'strftime';
    my @v = map { parse_expr($_->{toks}, { %$env, at => $_->{node} }) } @a;
    my $want = sub {
        my ($i, $ty) = @_;
        refuse($a[$i]{node}, "`$name` takes a $ty here (got $v[$i]{ty})")
            unless $v[$i]{ty} eq $ty || ($ty eq 'Float' && $v[$i]{ty} eq 'Int');
        return $v[$i]{pix};
    };
    $uses_pl = 1;
    if ($name eq 'substr') {
        refuse($w, '`substr` takes a string, where to start, and how much') unless @a == 2 || @a == 3;
        return { ty => 'String', pix => @a == 2 ? "Pl.substrFrom(" . $want->(0, 'String') . ', ' . $want->(1, 'Int') . ')'
                                                : "Pl.substrLen(" . $want->(0, 'String') . ', ' . $want->(1, 'Int')
                                                  . ', ' . $want->(2, 'Int') . ')' };
    }
    if ($name eq 'index' || $name eq 'rindex') {
        refuse($w, "`$name` takes a string, what to look for, and where to start from") unless @a == 2 || @a == 3;
        my $fn = $name eq 'index' ? 'index' : 'rindex';
        return { ty => 'Int', pix => @a == 2 ? "Pl.${fn}Of(" . $want->(0, 'String') . ', ' . $want->(1, 'String') . ')'
                                             : "Pl.${fn}From(" . $want->(0, 'String') . ', ' . $want->(1, 'String')
                                               . ', ' . $want->(2, 'Int') . ')' };
    }
    if ($name eq 'fmod') {
        refuse($w, '`fmod` takes two numbers') unless @a == 2;
        return { ty => 'Float', pix => 'Pl.fmodOf(' . $want->(0, 'Float') . ', ' . $want->(1, 'Float') . ')' };
    }
    if ($name eq 'split') {
        refuse($w, '`split` takes what to split on and the string') unless @a == 2;
        my $sep = $v[0];
        refuse($a[0]{node}, '`split` here takes a separator written out as a string; a pattern comes with '
                          . 'the regular expressions')
            unless defined $sep->{lit};
        return { ty => 'List<String>', pix => "Pl.splitWords(" . $want->(1, 'String') . ')' }
            if $sep->{lit} eq ' ';
        refuse($a[0]{node}, 'a separator here is plain text; `' . $sep->{lit} . '` reads as a pattern')
            if $sep->{lit} =~ /[\\^\$.\[\]|()?*+{}]/;
        return { ty => 'List<String>', pix => 'Pl.splitOn(' . $sep->{pix} . ', ' . $want->(1, 'String') . ')' };
    }
    refuse($w, "`$name` is not something the translator knows");
}

# `strftime($fmt, gmtime($epoch))` and the same with `localtime`. perl
# hands strftime the nine numbers a moment breaks into; the moment
# itself is what crosses here, and the twin breaks it apart again.
sub strftime_call {
    my ($w, $a, $env) = @_;
    my @a = @$a;
    refuse($w, '`strftime` is written `strftime($fmt, gmtime($epoch))` or with `localtime`') unless @a == 2;
    my $fmt = parse_expr($a[0]{toks}, { %$env, at => $a[0]{node} });
    refuse($a[0]{node}, "`strftime` takes a format (got $fmt->{ty})") unless $fmt->{ty} eq 'String';
    my @t = @{ $a[1]{toks} };
    refuse($a[1]{node}, '`strftime` takes the moment as `gmtime($epoch)` or `localtime($epoch)`')
        unless @t == 2 && is_word($t[0]) && $t[0]->content =~ /\A(?:gmtime|localtime)\z/
            && $t[1]->isa('PPI::Structure::List');
    my @e = split_args($t[1]);
    refuse($a[1]{node}, '`gmtime` here takes the moment in seconds') unless @e == 1;
    my $epoch = parse_expr($e[0]{toks}, { %$env, at => $e[0]{node} });
    refuse($e[0]{node}, "a moment is a whole number of seconds (got $epoch->{ty})") unless $epoch->{ty} eq 'Int';
    my $fn = $t[0]->content eq 'gmtime' ? 'strftimeUtc' : 'strftimeLocal';
    $uses_pl = 1;
    return { ty => 'String', pix => "Pl.$fn($fmt->{pix}, $epoch->{pix})" };
}

# `sprintf(FORMAT, value…)`. The format is written out, so it is cut
# apart here: the text between the conversions is text, and each
# conversion is one twin call with the one value it writes. The twin
# for one conversion is what the table holds to perl, so a format with
# five of them is held to perl five times over.
sub sprintf_call {
    my ($w, $list, $env) = @_;
    my @a = split_args($list);
    refuse($w, '`sprintf` takes a format and the values it writes') unless @a >= 1;
    my $fmt = parse_expr($a[0]{toks}, { %$env, at => $a[0]{node} });
    refuse($a[0]{node}, '`sprintf` takes a format written out as a string') unless defined $fmt->{lit};
    my @pieces = format_pieces($fmt->{lit}, $a[0]{node});
    my @specs = grep { $_->{spec} } @pieces;
    refuse($a[0]{node}, 'this format writes ' . scalar(@specs) . ' value'
                      . (@specs == 1 ? '' : 's') . ', and ' . (@a - 1) . ' '
                      . (@a - 1 == 1 ? 'was' : 'were') . ' written')
        unless @specs == @a - 1;
    my %fn = (Float => 'fmtNum', Int => 'fmtInt', String => 'fmtStr');
    my (@out, $i);
    for my $p (@pieces) {
        if (!$p->{spec}) {
            push @out, '"' . pix_text($a[0]{node}, $p->{text}) . '"' if length $p->{text};
            next;
        }
        my $arg = $a[++$i];
        my $v = parse_expr($arg->{toks}, { %$env, at => $arg->{node} });
        my $fn = $fn{ $v->{ty} }
            or refuse($arg->{node}, "`sprintf` writes a number or a string here (got $v->{ty})");
        $uses_pl = 1;
        push @out, "Pl.$fn(\"" . pix_text($arg->{node}, $p->{spec}) . "\", $v->{pix})";
    }
    return { ty => 'String', pix => '""' } unless @out;
    return { ty => 'String', pix => join(' + ', @out) };
}

# A format cut into the text between conversions and the conversions
# themselves. `%%` is a per cent sign in the text, not a conversion.
sub format_pieces {
    my ($fmt, $node) = @_;
    my (@out, $text);
    $text = '';
    my $s = $fmt;
    while (length $s) {
        if ($s =~ s/\A%%//) { $text .= '%'; next }
        if ($s =~ s/\A(%[-+ 0#]*\d*(?:\.\d+)?([A-Za-z]))//) {
            my ($spec, $conv) = ($1, $2);
            refuse($node, "the conversion `%$conv` is not in the translator")
                unless $conv =~ /\A[difFeEgGsxXob]\z/;
            push @out, { text => $text };
            $text = '';
            push @out, { spec => $spec };
            next;
        }
        refuse($node, 'a `%` here starts no conversion the translator knows') if $s =~ /\A%/;
        $s =~ s/\A(.)//s;
        $text .= $1;
    }
    push @out, { text => $text };
    return @out;
}

# "count: $count", "$xs[$i] and @{[ $i + 1 ]}" — literal text and holes.
sub interpolate {
    my ($tok, $env) = @_;
    my $s = $tok->string;
    my ($out, $buf, $plain) = ('', '', 1);
    my $lit = '';
    while (length $s) {
        if ($s =~ s/\A\\(.)//s) {
            my $c = $1;
            $buf .= $c eq 'n' ? "\n" : $c eq 't' ? "\t" : $c =~ /[\\"\$\@]/ ? $c
                  : refuse($tok, "the escape `\\$c` is not in the translator");
            next;
        }
        # @{[ ... ]} — Perl's way of writing an expression into a string.
        if ($s =~ /\A\@\{\[/) {
            my ($code, $rest) = split_block($tok, $s);
            $s = $rest;
            my $v = hole($tok, reparse($tok, $code, $env), $env);
            $out .= pix_text($tok, $buf) . '#{' . $v . '}';
            $lit .= $buf;
            ($buf, $plain) = ('', 0);
            next;
        }
        # What a pattern caught: `$1` … `$9` and `$+{name}`.
        if ($s =~ s/\A(\$[1-9])// || $s =~ s/\A(\$\+\{\w+\})//) {
            my $v = reparse($tok, $1, $env);
            $out .= pix_text($tok, $buf) . '#{' . hole($tok, $v, $env) . '}';
            $lit .= $buf;
            ($buf, $plain) = ('', 0);
            next;
        }
        if ($s =~ s/\A\$\{(\w+)\}// || $s =~ s/\A\$(\w+)((?:->)?(?:\[[^\[\]]*\]|\{[^{}]*\})?)//) {
            my ($name, $sub) = ($1, $2 // '');
            my $v = $name eq 'self' ? refuse($tok, '`$self` has no text')
                  : length $sub    ? reparse($tok, "\$$name$sub", $env)
                  :                  read_var($tok, $name, $env);
            $out .= pix_text($tok, $buf) . '#{' . hole($tok, $v, $env) . '}';
            $lit .= $buf;
            ($buf, $plain) = ('', 0);
            next;
        }
        refuse($tok, 'an array does not interpolate here; write `\@` for a literal at sign') if $s =~ /\A\@\w/;
        $s =~ s/\A(.)//s;
        $buf .= $1;
    }
    $out .= pix_text($tok, $buf);
    $lit .= $buf;
    return { ty => 'String', pix => qq{"$out"}, str => 1, ($plain ? (lit => $lit) : ()) };
}

# What a hole prints. Perl prints a number with %.15g and a bool as `1`
# or as nothing, and the compiled run is held to that by a twin.
sub hole {
    my ($tok, $v, $env) = @_;
    refuse($tok, "a list has no text; write what to put between it (`join(\", \", \@xs)`)") if $v->{ty} =~ /\A(?:List|Map)</;
    refuse($tok, 'this may be nothing, and nothing has no text; say what to print then: `$x // "-"`, or read it '
               . 'inside `if (defined $x)`')
        if $v->{ty} =~ /\?\z/;
    refuse($tok, "an object of `$v->{ty}` has no text; read one of its fields") if $model{ $v->{ty} };
    if ($v->{ty} eq 'Float') { $uses_pl = 1; return "Pl.numText($v->{pix})" }
    if ($v->{ty} eq 'Bool')  { $uses_pl = 1; return "Pl.boolText($v->{pix})" }
    return $v->{pix};
}

# `@{[ ... ]}` up to its matching bracket, and what follows it.
sub split_block {
    my ($tok, $s) = @_;
    my $i = 3;
    my $depth = 1;
    while ($i < length $s) {
        my $c = substr $s, $i, 1;
        if ($c eq '"' || $c eq "'") {
            my $q = $c;
            $i++;
            $i++ while $i < length($s) && substr($s, $i, 1) ne $q;
        }
        elsif ($c eq '[' || $c eq '{') { $depth++ }
        elsif ($c eq ']' || $c eq '}') { $depth--; last if $depth == 0 }
        $i++;
    }
    refuse($tok, '`@{[` in a string with no `]}` after it') if $depth != 0;
    my $code = substr $s, 3, $i - 3;
    my $rest = substr $s, $i + 1;
    refuse($tok, '`@{[ ... ]}` ends with `]}`') unless $rest =~ s/\A\}//;
    return ($code, $rest);
}

# A piece of Perl found inside a string, read as an expression.
sub reparse {
    my ($tok, $code, $env) = @_;
    # The string's own escapes are gone by the time perl reads the code.
    $code =~ s/\\(["\$\@\\])/$1/g;
    local $re_at = $tok;
    my $doc = PPI::Document->new(\$code) or refuse($tok, "this does not read as Perl: `$code`");
    my @st = $doc->schildren;
    refuse($tok, "one expression goes in here: `$code`") unless @st == 1;
    my @t = strip_semicolon(sig($st[0]));
    return parse_expr(\@t, { %$env, at => $tok });
}

# --- argument lists -------------------------------------------------------
# The pieces of a call's list, split at the commas PPI left at this level
# (parentheses and blocks are single nodes, so a comma inside one never
# shows up here). `word => value` is a keyword argument.
sub split_args {
    my ($list) = @_;
    my @st = $list->schildren;
    return () unless @st;
    refuse($st[1], 'a `;` inside an argument list') if @st > 1;
    my (@pieces, @cur);
    for my $t (sig($st[0])) {
        if (is_op($t, ',')) { push @pieces, [@cur]; @cur = () }
        else                { push @cur, $t }
    }
    push @pieces, [@cur] if @cur;
    my @args;
    for my $p (@pieces) {
        next unless @$p;
        if (@$p >= 3 && is_op($p->[1], '=>') && (is_word($p->[0]) || $p->[0]->isa('PPI::Token::Quote'))) {
            my $key = is_word($p->[0]) ? $p->[0]->content : $p->[0]->string;
            push @args, { key => $key, node => $p->[0], toks => [@$p[2 .. $#$p]] };
        } else {
            push @args, { node => $p->[0], toks => $p };
        }
    }
    return @args;
}

# --- elements -------------------------------------------------------------
# One element and everything under it, as a node the emitter walks:
#   { n => 'el',  pix, props => [[key, value]], kids => [nodes] }
#   { n => 'if',  cond, then => [nodes], else => [nodes] }
#   { n => 'for', it, ix, list | count, body => [nodes] }
#   { n => 'slot' }
sub parse_element {
    my ($toks, $at, $env) = @_;
    my ($w, $list, @more) = @$toks;
    refuse($at, 'a view is made of elements: ' . join(', ', map { "$_(...)" } sort keys %ELEMENT))
        unless is_word($w) && $ELEMENT{ $w->content };
    refuse($w, '`' . $w->content . '` is called with parentheses') unless $list && $list->isa('PPI::Structure::List') && !@more;
    my $spec = $ELEMENT{ $w->content };
    my $name = $spec->{name};
    my %own = map { $_->{name} => $_ } @{ $spec->{props} };

    # Expand the bags: `text("● OK", %PILL_OK)`.
    my @args;
    for my $a (split_args($list)) {
        if (!defined $a->{key} && @{ $a->{toks} } == 1 && is_sym($a->{toks}[0], '%')
                && exists $bag{ substr $a->{toks}[0]->content, 1 }) {
            push @args, @{ $bag{ substr $a->{toks}[0]->content, 1 } };
            next;
        }
        push @args, $a;
    }

    my (%given, @children);
    my @positional = grep { $_->{pos} } @{ $spec->{props} };
    for my $a (@args) {
        if (defined $a->{key}) {
            my $k = $a->{key};
            refuse($a->{node}, "`$name` was given `$k` twice") if exists $given{$k};
            if ($own{$k}) {
                refuse($a->{node}, "`$name`'s $k is written first, without its name") if $own{$k}{pos};
                $given{$k} = $a;
            } elsif ($k eq 'paint' && $spec->{paints}) {
                $given{$k} = $a;
            } elsif ($RIDER{$k}) {
                refuse($a->{node}, "`$name`'s own `label` is already the name a screen reader reads; there is no second one to give")
                    if $spec->{owns_label} && $k eq 'a11y_label';
                $given{$k} = $a;
            } else {
                my %takes = map { $_ => 1 } (map { $_->{name} } grep { !$_->{pos} } @{ $spec->{props} }),
                                            (map { $_->{name} } @RIDERS);
                refuse($a->{node}, "`$name` has no `$k =>`; it takes "
                                 . join(', ', map { "`$_`" } sort keys %takes));
            }
            next;
        }
        if (@positional) {
            my $p = shift @positional;
            $given{ $p->{name} } = $a;
            next;
        }
        refuse($a->{node}, "`$name` takes no children") unless $spec->{children};
        push @children, child_nodes($a, $env);
    }
    for my $p (@positional) {
        refuse($w, "`$name` needs its $p->{name}") unless exists $p->{default};
    }

    my @props;
    for my $r (@RIDERS) {
        next if $own{ $r->{name} } || !exists $given{ $r->{name} };
        push @props, [$r->{pix}, rider_value($r, $given{ $r->{name} }, $env)];
    }
    my ($rows) = grep { ($_->{type} // '') eq 'rows' } @{ $spec->{props} };
    for my $p (@{ $spec->{props} }) {
        next unless exists $given{ $p->{name} };
        next if $rows && (($p->{type} // '') eq 'rows' || $p->{name} eq 'count');
        my $a = $given{ $p->{name} };
        if (defined $p->{handler}) {
            push @props, [$p->{pix}, parse_handler($a->{toks}, $p, $a->{node}, $env)];
        } else {
            push @props, [$p->{pix}, prop_value($name, $p, $a, $env)];
        }
    }
    if ($spec->{paints}) {
        refuse($w, "`$name` is painted by a sub: `paint => sub { rect(...) }`")
            unless exists $given{paint};
        my ($sub, @rest) = @{ $given{paint}{toks} };
        my ($proto, $block) = @rest == 2 ? @rest : (undef, $rest[0]);
        refuse($given{paint}{node}, "`$name` is painted by a sub with nothing: `paint => sub { ... }`")
            unless is_word($sub, 'sub') && $block && $block->isa('PPI::Structure::Block') && !$proto;
        push @children, paint_nodes($block, $env);
    }
    if ($rows) {
        refuse($w, "`$name` builds its rows on demand: it takes how many, and a sub that builds row i")
            unless exists $given{count} && exists $given{ $rows->{name} };
        # Two keywords a list of rows starts with have one default in
        # the engine's table and another in the compiled run's element,
        # so they are always said out loud rather than left to whichever
        # side is reading.
        for my $d (grep { $_->{name} =~ /\A(?:virtualized|item_height)\z/ } @{ $spec->{props} }) {
            next if exists $given{ $d->{name} };
            push @props, [$d->{pix}, $d->{type} eq 'bool' ? ($d->{default} ? 'true' : 'false')
                                                          : $d->{default} + 0];
        }
        push @children, rows_repeater($given{count}, $given{ $rows->{name} }, $env);
    }
    # `grid_cell(child, col_span => 2)` is the child with the span
    # written on it; the engine's own face makes the same tree.
    if ($name eq 'grid_cell') {
        refuse($w, '`grid_cell` takes one element and what to say about it')
            unless @children == 1 && $children[0]{n} eq 'el';
        unshift @{ $children[0]{props} }, @props;
        return $children[0];
    }
    return { n => 'el', pix => $spec->{pix}, props => \@props, kids => \@children };
}

# A child of a container: an element, another element list spliced in,
# the slot a component's `@kids` stands for, a `map` over a list, or a
# `?:` that leaves the child out.
sub child_nodes {
    my ($a, $env) = @_;
    my @t = unlabel($a->{toks});
    # Parentheses around a child, which is how Perl writes a `map` into
    # an argument list.
    return map { child_nodes($_, $env) } split_args($t[0])
        if @t == 1 && $t[0]->isa('PPI::Structure::List') && $t[0]->schildren;
    if (@t == 1 && is_sym($t[0], '@')) {
        my $name = substr $t[0]->content, 1;
        return @{ $env->{elists}{$name} } if $env->{elists} && $env->{elists}{$name};
        return { n => 'slot' } if $env->{kids} && $name eq $env->{kids};
        refuse($t[0], "`\@$name` is not a list of elements built above this, nor this component's `\@kids`");
    }
    if (is_word($t[0], 'map')) { return map_repeater(\@t, $env) }
    my ($q) = grep { is_op($t[$_], '?') } 0 .. $#t;
    if (defined $q) {
        my ($c) = grep { is_op($t[$_], ':') && $_ > $q } 0 .. $#t;
        refuse($t[$q], 'a `?` needs its `:`') unless defined $c;
        my %sink;
        my $cond = cond_of([@t[0 .. $q - 1]], $env, $t[$q], \%sink);
        my ($tenv, $eenv, $swap) = branch_envs($env, \%sink, 0);
        my @then = child_nodes({ node => $t[$q + 1], toks => [@t[$q + 1 .. $c - 1]] }, $tenv);
        my @else = is_empty_list([@t[$c + 1 .. $#t]])
                 ? ()
                 : child_nodes({ node => $t[$c + 1], toks => [@t[$c + 1 .. $#t]] }, $eenv);
        return $swap ? { n => 'if', cond => $cond, then => \@else, else => \@then }
                     : { n => 'if', cond => $cond, then => \@then, else => \@else };
    }
    return elem_of(\@t, $a->{node}, $env);
}

sub is_empty_list {
    my ($toks) = @_;
    return @$toks == 1 && $toks->[0]->isa('PPI::Structure::List') && !$toks->[0]->schildren;
}

# An element expression: one of the vocabulary, or a method of the app
# that answers one.
sub elem_of {
    my ($toks, $at, $env) = @_;
    my @t = @$toks;
    if (is_sym($t[0], '$') && ($t[0]->content eq '$self' || (defined $app_var && $t[0]->content eq "\$$app_var"))) {
        refuse($t[0], 'a method of the app is called `$self->name(...)`') unless is_op($t[1], '->') && is_word($t[2]);
        my $args = $t[3];
        refuse($t[2], 'this does not continue the call') if @t > 4 || (@t == 4 && !$args->isa('PPI::Structure::List'));
        return component_call($t[2], $args, [], $env);
    }
    return parse_element(\@t, $at, $env);
}

# `map { ... } @xs` in a child's place: the .pix repeater.
sub map_repeater {
    my ($toks, $env) = @_;
    my ($w, $block, @rest) = @$toks;
    refuse($w, '`map` here takes a block and a list: `map { text($_) } @items`')
        unless $block && $block->isa('PPI::Structure::Block') && @rest;
    my $over = list_source(\@rest, $env, $w);
    my $inner = { %$env, vars => { %{ $env->{vars} }, '_' => { ty => $over->{ty}, pix => 'it' } },
                  nonneg => { %{ $env->{nonneg} // {} }, it => 1 },
                  repeat => { list => $over->{list}, it => 'it', ix => 'i' } };
    my @st = $block->schildren;
    refuse($block, '`map` in a view builds one element for each item') unless @st == 1;
    my $arg = { node => $st[0], toks => [strip_semicolon(sig($st[0]))] };
    return unrolled($over, '_', $arg, $env, $w) if $over->{counted};
    my @body = child_nodes($arg, $inner);
    return { n => 'for', over => $over->{over}, it => 'it', body => \@body };
}

# A run of whole numbers a view walks is written out: the .pix `for` in
# a view takes a list property and nothing else, and how many there are
# is known here anyway.
sub unrolled {
    my ($over, $name, $arg, $env, $at) = @_;
    refuse($at, 'a view repeats over a list, or over a run of numbers whose ends are written out')
        unless defined $over->{from};
    my @nodes;
    for my $n ($over->{from} .. $over->{to}) {
        my $inner = { %$env, vars => { %{ $env->{vars} }, $name => { ty => 'Int', pix => $n, fixed => 1 } },
                      nonneg => { %{ $env->{nonneg} // {} }, $n => 1 } };
        push @nodes, child_nodes($arg, $inner);
    }
    return @nodes;
}

# What a repeater walks: a list, or a run of whole numbers.
sub list_source {
    my ($toks, $env, $at) = @_;
    my @t = @$toks;
    @t = sig(($t[0]->schildren)[0]) if @t == 1 && $t[0]->isa('PPI::Structure::List') && $t[0]->schildren;
    my ($dd) = grep { is_op($t[$_], '..') } 0 .. $#t;
    if (defined $dd) {
        my $from = parse_expr([@t[0 .. $dd - 1]], { %$env, at => $at });
        # `0 .. $#items` walks the list itself, which is the only shape
        # a .pix view repeater takes — and the one that hands over the
        # row as well as its number.
        if ($from->{pix} eq '0' && $dd + 1 == $#t && $t[-1]->isa('PPI::Token::ArrayIndex')) {
            my $lv = read_list($t[-1], substr($t[-1]->content, 2), $env);
            return { ty => 'Int', over => "0..$lv->{pix}.length", counted => 1,
                     indexed => 1, list => $lv->{pix} };
        }
        my $to   = parse_expr([@t[$dd + 1 .. $#t]], { %$env, at => $at });
        refuse($at, 'a run of numbers goes from a whole number to a whole number')
            unless $from->{ty} eq 'Int' && $to->{ty} eq 'Int';
        # Perl's `..` takes both ends; the .pix `..=` says the same.
        my %ends = ($from->{pix} =~ /\A-?\d+\z/ && $to->{pix} =~ /\A-?\d+\z/)
                 ? (from => $from->{pix}, to => $to->{pix}) : ();
        return { ty => 'Int', over => "$from->{pix}..=$to->{pix}", counted => 1, %ends };
    }
    my $v = parse_expr(\@t, { %$env, at => $at });
    refuse($at, "a repeater walks a list (got $v->{ty})") unless $v->{ty} =~ /\AList<(.+)>\z/;
    return { ty => $1, listty => $v->{ty}, over => $v->{pix}, list => $v->{pix} };
}

# `list_view(scalar @items, sub ($i) { ... })` and the table's rows: the
# count names what is walked, and the builder is the repeater's body.
sub rows_repeater {
    my ($count, $builder, $env) = @_;
    my $cv = parse_expr($count->{toks}, { %$env, at => $count->{node} });
    refuse($count->{node}, "a list says how many rows it has as a whole number (got $cv->{ty})") unless $cv->{ty} eq 'Int';
    my ($sub, @rest) = @{ $builder->{toks} };
    refuse($builder->{node}, 'the rows are built by a sub taking the row number: `sub ($i) { ... }`')
        unless is_word($sub, 'sub');
    my ($proto, $block) = @rest == 2 ? @rest : (undef, $rest[0]);
    refuse($builder->{node}, 'the rows are built by a sub taking the row number: `sub ($i) { ... }`')
        unless $block && $block->isa('PPI::Structure::Block');
    my @params = sub_params($proto);
    refuse($proto // $sub, 'the row builder is called with the row number: `sub ($i) { ... }`') unless @params == 1;
    my $ivar = substr $params[0], 1;
    my %over = $cv->{pix} =~ /\A(.+)\.length\z/ ? (over => $1, list => $1, it => 'it')
                                                : (over => "0..$cv->{pix}");
    my $inner = { %$env,
                  vars => { %{ $env->{vars} }, $ivar => { ty => 'Int', pix => 'i' } },
                  nonneg => { %{ $env->{nonneg} // {} }, i => 1, it => 1 },
                  repeat => { list => $over{list}, it => 'it', ix => 'i' } };
    my @st = $block->schildren;
    refuse($block, 'a row builder answers one element') unless @st == 1;
    my $arg = { node => $st[0], toks => [strip_semicolon(sig($st[0]))] };
    return unrolled({ from => 0, to => $cv->{pix} - 1 }, $ivar, $arg, $env, $count->{node})
        if !defined $over{list} && $cv->{pix} =~ /\A\d+\z/;
    refuse($count->{node}, 'how many rows there are is `scalar @items` on a list field, or a number written out')
        unless defined $over{list};
    my @body = child_nodes($arg, $inner);
    return { n => 'for', %over, ix => 'i', body => \@body };
}

# The value of an element's own keyword, checked against its type.
sub prop_value {
    my ($name, $p, $a, $env) = @_;
    my $venv = { %$env, at => $a->{node} };
    my $t = $p->{type};
    if ($t eq 'strs' || $t eq 'nums' || $t eq 'nums2') {
        return list_prop($name, $p, $a, $venv);
    }
    my $v = parse_expr($a->{toks}, $venv);
    if ($t eq 'str')  { refuse($a->{node}, "`$p->{name} =>` takes a string (got $v->{ty})") unless $v->{ty} eq 'String'; return str_value($v) }
    if ($t eq 'num')  { refuse($a->{node}, "`$p->{name} =>` takes a number (got $v->{ty})") unless num_ty($v->{ty}); return $v->{pix} }
    if ($t eq 'int')  { refuse($a->{node}, "`$p->{name} =>` takes a whole number (got $v->{ty})") unless $v->{ty} eq 'Int'; return $v->{pix} }
    if ($t eq 'bool') { return bool_value($p->{name}, $v, $a->{node}) }
    refuse($a->{node}, "`$p->{name} =>` has a type the translator does not know: $t");
}

# A keyword that takes a list: a field the view re-reads (`\@items`), a
# literal (`["a", "b"]`), or a list of lists.
sub list_prop {
    my ($name, $p, $a, $env) = @_;
    my $t = $p->{type};
    my $want = $t eq 'strs' ? 'List<String>' : $t eq 'nums' ? 'List<Float>' : 'List<List<Float>>';
    my @tk = @{ $a->{toks} };
    my $v;
    if (@tk == 2 && $tk[0]->isa('PPI::Token::Cast') && $tk[0]->content eq '\\' && is_sym($tk[1], '@')) {
        $v = read_list($tk[1], substr($tk[1]->content, 1), $env);
    } elsif (@tk == 1 && $tk[0]->isa('PPI::Structure::Constructor') && $tk[0]->start->content eq '[') {
        my ($ty, $pix) = list_literal($tk[0], $env);
        $v = { ty => $ty, pix => $pix };
    } else {
        $v = parse_expr(\@tk, $env);
    }
    my $have = $v->{ty};
    my $ok = $have eq $want
          || ($t eq 'nums'  && $have eq 'List<Int>')
          || ($t eq 'nums2' && $have eq 'List<List<Int>>');
    refuse($a->{node}, "`$p->{name} =>` takes a $want; this is a $have") unless $ok;
    return $v->{pix};
}

# `["a", "b"]` or `[[1, 2], [3, 4]]` written where a list goes.
sub list_literal {
    my ($ctor, $env) = @_;
    my ($ty, @pix);
    for my $a (split_args($ctor)) {
        refuse($a->{node}, 'a list written out holds values, not `name => value`') if defined $a->{key};
        my @t = @{ $a->{toks} };
        my ($t2, $p);
        if (@t == 1 && $t[0]->isa('PPI::Structure::Constructor') && $t[0]->start->content eq '[') {
            ($t2, $p) = list_literal($t[0], $env);
        } elsif (@t == 2 && $t[0]->isa('PPI::Token::Cast') && $t[0]->content eq '\\' && is_sym($t[1], '@')) {
            my $lv = read_list($t[1], substr($t[1]->content, 1), $env);
            ($t2, $p) = ($lv->{ty}, $lv->{pix});
        } else {
            my $v = parse_expr(\@t, { %$env, at => $a->{node} });
            ($t2, $p) = ($v->{ty}, $v->{pix});
        }
        refuse($a->{node}, "a list holds one type: this one started with $ty and this is $t2") if defined $ty && $t2 ne $ty;
        $ty = $t2;
        push @pix, $p;
    }
    refuse($ctor, 'a list written out here says what it holds by having something in it') unless defined $ty;
    return ("List<$ty>", '[' . join(', ', @pix) . ']');
}

# The value of a keyword every element takes.
sub rider_value {
    my ($r, $a, $env) = @_;
    my $venv = { %$env, at => $a->{node} };
    my $v = parse_expr($a->{toks}, $venv);
    my $k = $r->{name};
    if ($r->{type} eq 'num') {
        refuse($a->{node}, "`$k =>` takes a number (got $v->{ty})") unless num_ty($v->{ty});
        return $v->{pix};
    }
    if ($r->{type} eq 'int') {
        refuse($a->{node}, "`$k =>` takes a whole number (got $v->{ty})") unless $v->{ty} eq 'Int';
        return $v->{pix};
    }
    if ($r->{type} eq 'bool') {
        my $b = bool_value($k, $v, $a->{node});
        refuse($a->{node}, "`$k =>` takes `true` (leave it out otherwise)") if ($k eq 'enter' || $k eq 'exit') && $b ne 'true';
        return $b;
    }
    refuse($a->{node}, "`$k =>` takes a string (got $v->{ty})") unless $v->{ty} eq 'String';
    return str_value($v) if !defined $v->{lit};
    if (defined $v->{lit}) {
        refuse($a->{node}, "`easing =>` is one of " . join(', ', map { "\"$_\"" } sort keys %EASINGS)) if $k eq 'easing' && !$EASINGS{ $v->{lit} };
        refuse($a->{node}, "unknown role `$v->{lit}`; one of " . join(', ', sort keys %ROLES)) if $k eq 'role' && !$ROLES{ $v->{lit} };
        refuse($a->{node}, '`theme =>` is "light", "dark" or a str field') if $k eq 'theme' && $v->{lit} !~ /\A(?:light|dark)\z/;
    }
    return $v->{pix};
}

# A string a property takes. One read out of a list is written into a
# hole, which is where the compiled run takes an element of a list.
sub str_value {
    my ($v) = @_;
    # A string an app wrote out stands as it is, and so does a name, a
    # property or one plain call. Anything else — a read out of a list,
    # two strings joined — is written into a hole, which is where the
    # compiled run takes an expression.
    return $v->{pix} if $v->{str};
    return $v->{pix} if $v->{pix} =~ /\A[A-Za-z_][\w.]*(?:\([^()]*\))?\z/;
    return '"#{' . $v->{pix} . '}"';
}

sub bool_value {
    my ($what, $v, $node) = @_;
    return 'true'  if $v->{ty} eq 'Int' && $v->{pix} eq '1';
    return 'false' if $v->{ty} eq 'Int' && $v->{pix} eq '0';
    refuse($node, "`$what =>` takes `true` or `false`, or a bool field (got $v->{ty})") unless $v->{ty} eq 'Bool';
    return $v->{pix};
}

# --- handlers -------------------------------------------------------------

# The parameter names of an anonymous sub. PPI gives them as a Prototype
# token (`($s)`), or, when it knows signatures are on, as a structure.
sub sub_params {
    my ($node) = @_;
    return () unless defined $node;
    if ($node->isa('PPI::Token::Prototype')) {
        (my $p = $node->content) =~ s/\A\(|\)\z//g;
        my @params = grep { length } map { s/\A\s+|\s+\z//gr } split /,/, $p;
        refuse($node, 'a parameter here is a plain scalar (`$s`)') if grep { !/\A\$\w+\z/ } @params;
        return @params;
    }
    if ($node->isa('PPI::Structure::List')) {
        my @t = map { sig($_) } $node->schildren;
        refuse($node, 'a parameter here is a plain scalar (`$s`)') if grep { !is_sym($_, '$') && !is_op($_, ',') } @t;
        return map { $_->content } grep { $_->isa('PPI::Token::Symbol') } @t;
    }
    refuse($node, 'a handler is `sub { ... }` or `sub ($x) { ... }`');
}

# `sub { ... }` or `sub ($x) { ... }` on a handler keyword. A body that
# is one call to a method of the app becomes that call; anything else
# becomes a store fn of its own, and the property reads `App.hN(...)`.
sub parse_handler {
    my ($toks, $p, $at, $env) = @_;
    my ($sub, @rest) = @$toks;
    refuse($at, "a handler is an anonymous sub written here: `$p->{name} => sub { ... }`") unless is_word($sub, 'sub');
    my ($proto, $block) = @rest == 2 ? @rest : (undef, $rest[0]);
    refuse($sub, 'a handler is `sub { ... }` or `sub ($x) { ... }`') unless $block && $block->isa('PPI::Structure::Block') && @rest <= 2;
    my @params = sub_params($proto);
    my $kind = $p->{handler};
    my $pty = $Rakugan::Vocab::PAYLOAD_TYPE{$kind};
    if ($kind eq 'none') {
        refuse($proto, 'this handler is called with nothing; drop the parameter') if @params;
    } else {
        refuse($proto // $sub, "this handler is called with one value, a $pty: write `sub (\$x) { ... }`") unless @params == 1;
    }
    my $payload = $p->{payload};
    # At the call site the event's value is named by the element's row;
    # inside a store fn of its own it keeps the name the app gave it.
    my %pay_call = map { substr($_, 1) => { ty => $pty, pix => $payload } } @params;
    my %pay_fn   = map { substr($_, 1) => { ty => $pty, pix => substr($_, 1), fixed => 1 } } @params;

    # `sub { $self->flip }` and `sub ($i) { $self->pick($i) }`: the
    # method itself is the handler, and the event's value is its argument.
    my @st = $block->schildren;
    refuse($block, 'an empty handler') unless @st;
    if (@st == 1) {
        my @t = strip_semicolon(sig($st[0]));
        if (@t >= 3 && is_sym($t[0], '$') && $t[0]->content eq '$self' && is_op($t[1], '->') && is_word($t[2])
                && (@t == 3 || (@t == 4 && $t[3]->isa('PPI::Structure::List')))) {
            my $m = $method{ $t[2]->content }
                or refuse($t[2], '`' . $t[2]->content . "` is not a method of $class_name");
            if ($m->{stateful} && !$m->{element}) {
                my $cenv = { %$env, vars => { %{ $env->{vars} }, %pay_call }, at => $t[2], as_statement => 1 };
                my @vals = call_args($t[2], $t[3], $m, $cenv);
                return "App.$m->{name}(" . join(', ', @vals) . ')';
            }
        }
    }

    my $id = 'h' . scalar @handlers;
    my $fenv = { ctx => 'fn', vars => { %pay_fn }, outer => $env->{vars}, captures => {}, capture_order => [],
                 nonneg => {}, at => $at };
    my @body = stmts($block, $fenv);
    my @caps = map { [$_, $fenv->{captures}{$_}{ty}] } @{ $fenv->{capture_order} };
    my @params_pix = (@caps, map { [substr($_, 1), $pty] } @params);
    push @handlers, { id => $id, params => \@params_pix, body => \@body, async => $fenv->{async} };
    my @args = ((map { $env->{vars}{ $_->[0] }{pix} } @caps), ($kind eq 'none' ? () : $payload));
    return "App.$id(" . join(', ', @args) . ')';
}

# --- what can fail ----------------------------------------------------------
# Where the statement is, the way perl says it: ` at FILE line N.`, on a
# line of its own. perl was started on the absolute path, so that is
# the path both runs name.
sub at_pix {
    my ($env, $node) = @_;
    return '"' . pix_text($node, " at $where line " . ($env->{line} // 0) . ".\n") . '"';
}

# A call that can fail: outside a `try` it is the plain form, which
# stops the handler in both runs; inside one it is the `!T` form,
# matched on the spot, so the catch runs where perl's would. The `case`
# opens before the statement and closes after it, and what follows the
# statement is wrapped by `stmt_list`. `$append` is the place to add to
# the message when the twin does not carry it already.
sub fallible {
    my ($env, $node, $ty, $plain, $try, $append) = @_;
    my $fr = $env->{try} or return { ty => $ty, pix => $plain };
    refuse($node, 'a `try` does not reach into a loop yet; put the `try` inside the loop, around the line that can fail')
        if $env->{in_loop};
    refuse($node, "a `try` does not reach into $env->{no_try} yet; write the `if` out") if $env->{no_try};
    $fr->{points}++;
    my $v = '__t' . ++$tmp;
    my $e = '__e' . $tmp;
    push @{ $env->{pre} }, "case $try {", "  when ok($v) {";
    unshift @{ $env->{post} }, '  }', "  when err($e) {", "    $fr->{ok} = false",
        "    var $fr->{evar} : String = $e" . (defined $append ? " + $append" : ''),
        (map { "    $_" } $fr->{catch}->()), '  }', '}';
    return { ty => $ty, pix => $v };
}

# What `die` or `warn` says: the pieces after the word, joined, each a
# string or something perl prints as one.
sub message_pix {
    my ($toks, $env, $w, $bare) = @_;
    my @t = @$toks;
    return '"' . pix_text($w, $bare) . '"' unless @t;
    my @pieces;
    if (@t == 1 && $t[0]->isa('PPI::Structure::List')) {
        my @a = split_args($t[0]);
        refuse($w, '`' . $w->content . '` takes what to say, in order') if grep { defined $_->{key} } @a;
        @pieces = map { $_->{toks} } @a;
    } else {
        my @cur;
        for my $t (@t) {
            if (is_op($t, ',')) { push @pieces, [@cur]; @cur = () } else { push @cur, $t }
        }
        push @pieces, [@cur] if @cur;
    }
    my @pix;
    for my $p (@pieces) {
        next unless @$p;
        my $v = parse_expr($p, { %$env, at => $p->[0] });
        push @pix, $v->{ty} eq 'String' ? $v->{pix} : hole($p->[0], $v, $env);
    }
    refuse($w, '`' . $w->content . '` says something: `' . $w->content . ' "…"`') unless @pix;
    return @pix == 1 ? $pix[0] : join(' + ', map { group($_) } @pix);
}

# `warn "…";` — the text to standard error in both runs, with the place
# appended the way perl appends it.
sub warn_stmt {
    my ($toks, $env) = @_;
    my ($w, @rest) = @$toks;
    refuse($w, '`warn` writes to standard error, and building a view only reads; warn from the handler that set the value')
        if $env->{ctx} eq 'view';
    my $msg = message_pix(\@rest, $env, $w, "Warning: something's wrong");
    $uses_pl = 1;
    return 'var __warn' . ++$tmp . " : Int = Pl.warnAt($msg, " . at_pix($env, $w) . ')';
}

# `die "…";` — the handler stops here in both runs, and each says so on
# standard error. Inside a `try` the catch runs instead, with `$e` the
# text perl would give it.
sub die_stmt {
    my ($toks, $env) = @_;
    my ($w, @rest) = @$toks;
    refuse($w, '`die` stops a handler, and building a view only reads; what a view shows was made good in the handler that set it')
        if $env->{ctx} eq 'view';
    my $msg = message_pix(\@rest, $env, $w, 'Died');
    my $at = at_pix($env, $w);
    $uses_pl = 1;
    $env->{dead} = 1 unless $env->{conditional};
    if (my $fr = $env->{try}) {
        refuse($w, 'a `try` does not reach into a loop yet; put the `try` inside the loop, around the line that can fail')
            if $env->{in_loop};
        $fr->{points}++;
        return ("$fr->{ok} = false", "var $fr->{evar} : String = Pl.dieText($msg, $at)", $fr->{catch}->());
    }
    return 'var __die' . ++$tmp . " : Int = Pl.dieAt($msg, $at)";
}

# `try { … } catch ($e) { … }`. The body runs until something fails;
# then the catch runs with the message, and the lines after the failure
# do not. Both runs do this; the compiled one has no unwinding, so each
# thing that can fail is matched where it stands (`fallible`) and the
# lines after it are wrapped in the flag the catch clears (`stmt_list`).
sub try_stmt {
    my ($st, $toks, $env) = @_;
    my @t = @$toks;
    my $w = $t[0];
    refuse($w, '`try` belongs in a handler or a method; building a view only reads') if $env->{ctx} eq 'view';
    my ($body, $cw, $clist, $cblock) = @t[1 .. 4];
    refuse($w, '`try` takes a block, then `catch ($e)` and its block')
        unless $body && $body->isa('PPI::Structure::Block') && is_word($cw, 'catch')
            && $clist && $clist->isa('PPI::Structure::List') && $cblock && $cblock->isa('PPI::Structure::Block');
    my @cv = map { sig($_) } $clist->schildren;
    refuse($clist, '`catch` names what it caught: `catch ($e)`') unless @cv == 1 && is_sym($cv[0], '$');
    my $evar = substr $cv[0]->content, 1;
    refuse($cv[0], "`\$$evar` is already in scope here") if $env->{vars}{$evar};
    refuse($cv[0], "`\$$evar` is a field of $class_name; a name declared here would hide it") if exists $field{$evar};
    refuse($t[5], '`finally` runs after either path, and the compiled run has no unwinding to hang it on; '
                . 'write the line after the `try`')
        if is_word($t[5], 'finally');
    my $ok = '__ok' . ++$tmp;
    my $frame = { ok => $ok, evar => $evar, points => 0 };
    # The catch runs outside the try: a failure inside it is the
    # enclosing code's to catch, or nobody's.
    $frame->{catch} = sub {
        my $cenv = scope($env);
        $cenv->{vars}{$evar} = { ty => 'String', pix => $evar, fixed => 1 };
        return stmts($cblock, $cenv);
    };
    my $tenv = scope($env);
    ($tenv->{try}, $tenv->{in_loop}) = ($frame, 0);
    my @lines = ("var $ok = true", stmts($body, $tenv));
    return (@lines, rest_stmts($st, $cblock, $env));
}

# PPI does not know `try`, so the statements after a `try` up to the
# next `;` arrive inside its statement. They are read again on their
# own, padded to where they stand in the file so a refusal still names
# the right line and column.
sub rest_stmts {
    my ($st, $last, $env) = @_;
    my @kids = $st->children;
    my ($idx) = grep { $kids[$_] == $last } 0 .. $#kids;
    my @rest = @kids[$idx + 1 .. $#kids];
    shift @rest while @rest && !$rest[0]->significant;
    my @sig = grep { $_->significant } @rest;
    return () unless @sig;
    return () if @sig == 1 && $sig[0]->isa('PPI::Token::Structure') && $sig[0]->content eq ';';
    my $first = $rest[0];
    my $text = ("\n" x ($first->line_number - 1)) . (' ' x ($first->column_number - 1))
             . join('', map { $_->content } @rest);
    my $doc = PPI::Document->new(\$text) or refuse($first, 'this does not read as Perl');
    return stmts($doc, $env);
}

# --- statements -----------------------------------------------------------
# The lines of a handler, a method or a loop's body.
sub stmts {
    my ($block, $env) = @_;
    my @st = grep { !$_->isa('PPI::Statement::Null') } $block->schildren;
    my ($saved_dead, $saved_task) = ($env->{dead}, $env->{after_task});
    ($env->{dead}, $env->{after_task}) = (0, 0);
    my @out = stmt_list(\@st, $env);
    ($env->{dead}, $env->{after_task}) = ($saved_dead, $saved_task);
    return @out;
}

sub stmt_list {
    my ($sts, $env) = @_;
    my @out;
    for my $i (0 .. $#$sts) {
        my $st = $sts->[$i];
        refuse((sig($st))[0], 'nothing after `die` runs; drop these lines, or put the `die` under an `if`')
            if $env->{dead};
        refuse((sig($st))[0], '`task` is the last thing a handler does: the compiled run reaches these lines '
                            . 'when the work is done, and perl reaches them at once; write them before the `task`')
            if $env->{after_task};
        my $fr = $env->{try};
        my $before = $fr ? $fr->{points} : 0;
        push @out, stmt($st, $env);
        # Something in that statement could have failed, and if it did
        # the catch has run: what follows runs only when it did not.
        if ($fr && $fr->{points} > $before && $i < $#$sts) {
            push @out, "if $fr->{ok} {", (map { "  $_" } stmt_list([@{$sts}[$i + 1 .. $#$sts]], $env)), '}';
            last;
        }
    }
    return @out;
}

# The lines a statement needs before it (an index made good, a `case`
# opened on something that can fail) and after it (that `case` closed).
sub with_pre {
    my ($env, $code) = @_;
    my ($saved, $saved_post) = ($env->{pre}, $env->{post});
    ($env->{pre}, $env->{post}) = ([], []);
    my @out = $code->();
    my @pre = @{ $env->{pre} };
    my @post = @{ $env->{post} };
    ($env->{pre}, $env->{post}) = ($saved, $saved_post);
    return (@pre, @out, @post);
}

sub stmt {
    my ($st, $env) = @_;
    # perl names a failure by the line its statement starts on, and the
    # compiled run says the same line.
    my $saved_line = $env->{line};
    my ($first) = sig($st);
    $env->{line} = $first->line_number if $first;
    my @out = with_pre($env, sub {
        return compound($st, $env) if $st->isa('PPI::Statement::Compound');
        my @t = strip_semicolon(sig($st));
        refuse($st, 'an empty statement') unless @t;
        return try_stmt($st, \@t, $env) if is_word($t[0], 'try');
        # a trailing `if` / `unless` / `while` / `for`
        for my $i (1 .. $#t) {
            next unless is_word($t[$i]) && $t[$i]->content =~ /\A(?:if|unless|while|for|foreach)\z/;
            my $kw = $t[$i]->content;
            my @head = @t[0 .. $i - 1];
            my @tail = @t[$i + 1 .. $#t];
            if ($kw eq 'for' || $kw eq 'foreach') {
                my $over = list_source(\@tail, $env, $t[$i]);
                my $inner = { %$env, in_loop => 1,
                              vars => { %{ $env->{vars} }, '_' => { ty => $over->{ty}, pix => '__it' } } };
                my @body = stmt_run(\@head, $inner, $st);
                return ("for __it in $over->{over} {", (map { "  $_" } @body), '}');
            }
            my %caught;
            my $loop = $kw eq 'while';
            my $cond = cond_of(\@tail, $loop ? { %$env, in_loop => 1 } : $env, $t[$i], $loop ? undef : \%caught);
            my ($tenv, $eenv, $swap) = branch_envs($env, \%caught, $kw eq 'unless');
            $cond = "!($cond)" if $kw eq 'unless' && !$caught{narrow};
            # What stands before the condition runs only sometimes, so a
            # `die` there does not make the lines after it dead.
            my $guarded = { %$tenv, conditional => 1, ($loop ? (in_loop => 1) : ()) };
            $guarded->{capture} = \%caught if $caught{pat} && $kw ne 'unless';
            my @body = stmt_run(\@head, $guarded, $st);
            $env->{async} = 1 if $guarded->{async};
            return ("$kw $cond {", (map { "  $_" } @body), '}') if $loop;
            return ("if $cond {", '} else {', (map { "  $_" } @body), '}') if $swap;
            return ("if $cond {", (map { "  $_" } @body), '}');
        }
        return stmt_run(\@t, $env, $st);
    });
    $env->{line} = $saved_line;
    return @out;
}

# The statement a run of tokens is, with the lines any index guard needs
# kept in front of it.
sub stmt_run {
    my ($toks, $env, $st) = @_;
    return with_pre($env, sub { simple_stmt($toks, $env, $st) });
}

sub simple_stmt {
    my ($toks, $env, $st) = @_;
    my @t = @$toks;
    my $head = $t[0];
    if (is_word($head, 'my'))     { return declare(\@t, $env) }
    if (is_word($head, 'return')) { return ret(\@t, $env) }
    if (is_word($head, 'last'))   { refuse($head, '`last` stands alone') if @t > 1; return 'break' }
    if (is_word($head, 'next'))   { refuse($head, '`next` stands alone') if @t > 1; return 'continue' }
    if (is_word($head, 'push') || is_word($head, 'unshift')) { return grow(\@t, $env) }
    if (is_word($head, 'pop') || is_word($head, 'shift'))    { return shrink(\@t, $env) }
    if (is_word($head, 'delete')) { return drop(\@t, $env) }
    if (is_word($head, 'task'))   { return task_stmt(\@t, $env) }
    if (is_word($head, 'warn'))   { return warn_stmt(\@t, $env) }
    if (is_word($head, 'die'))    { return die_stmt(\@t, $env) }
    if (is_sym($head, '$') && ($head->content eq '$self' || (defined $app_var && $head->content eq "\$$app_var"))) {
        my $i = 0;
        my $v = symbol_expr(\$i, \@t, { %$env, as_statement => 1, at => $head });
        refuse($t[$i], 'this does not continue the call') if $i < @t;
        return $v->{pix};
    }
    if (is_sym($head, '$') && is_op($t[1], '=~') && $t[2] && $t[2]->isa('PPI::Token::Regexp::Substitute')) {
        return substitution(\@t, $env);
    }
    # `$node->grow(0.5);`, `$node->set_label("x");` — an object's own.
    if (is_sym($head, '$') && is_op($t[1], '->') && is_word($t[2])) {
        my $rv = eval { read_var($head, substr($head->content, 1), $env) };
        if ($rv && ($model{ $rv->{ty} } || ($rv->{ty} =~ /\A(\w+)\?\z/ && $model{$1}))) {
            my $i = 0;
            my $v = symbol_expr(\$i, \@t, { %$env, as_statement => 1, at => $head });
            refuse($t[$i], 'this does not continue the call') if $i < @t;
            return $v->{pix};
        }
    }
    if (is_word($head, 'weaken')) { return weaken_stmt(\@t, $env) }
    if (is_sym($head, '@') || is_sym($head, '%')) { return whole_assign(\@t, $env) }
    refuse($head, $NOT_TAKEN{ $head->content }) if is_word($head) && $NOT_TAKEN{ $head->content };
    # A call whose answer nobody wants — the framework's own, and
    # nothing else, since a Perl builtin called for its own sake does
    # nothing here.
    if (is_word($head) && ($Rakugan::Manifest::NAMES{ $head->content } || $head->content eq 'quit')) {
        my $i = 0;
        my $v = word_expr(\$i, \@t, { %$env, as_statement => 1, at => $head });
        refuse($t[$i], 'this does not continue the call') if $i < @t;
        return $v->{pix};
    }
    refuse($head // $st, 'a statement here writes a field (`$count += 1`), a list or a hash, '
                       . 'declares a name (`my $x = ...`) or calls a method (`$self->flip`)')
        unless is_sym($head, '$');
    return assign(\@t, $env);
}

# `my $x = expr;`, `my @xs = empty(Str);`
sub declare {
    my ($toks, $env) = @_;
    my @t = @$toks;
    my $sym = $t[1];
    refuse($t[0], 'a `my` here declares one name (`my $x = ...`, `my @xs = ...`); a list of names on '
                . 'the left takes what is on the right apart, and the translator does not do that yet')
        if $sym && $sym->isa('PPI::Structure::List');
    refuse($t[0], 'a name is declared `my $x = ...;`') unless is_sym($sym);
    my $name = substr $sym->content, 1;
    refuse($sym, "`$name` is already in scope here") if $env->{vars}{$name};
    refuse($sym, "`$name` is a field of $class_name; a name declared here would hide it") if exists $field{$name};
    refuse($sym, 'a name declared here starts with a value: `my $x = 0;`') unless is_op($t[2], '=') && @t > 3;
    my @rhs = @t[3 .. $#t];
    my ($ty, $pix);
    if (my $p = pipeline_rhs(\@rhs, $env, $sym)) {
        $env->{vars}{$name} = { ty => $p->{ty}, pix => $name };
        return "var $name : $p->{ty} = $p->{pix}";
    }
    if ($sym->raw_type eq '$' && (is_word($rhs[0], 'maybe') || is_word($rhs[0], 'undef'))) {
        ($ty, $pix) = maybe_init(\@rhs, $sym);
    } elsif ($sym->raw_type eq '@' && is_word($rhs[0], 'empty')) {
        ($ty, $pix) = list_init(\@rhs, $sym);
    } elsif ($sym->raw_type eq '@' && @rhs == 1 && $rhs[0]->isa('PPI::Structure::List')) {
        ($ty, $pix) = list_init(\@rhs, $sym);
    } elsif ($sym->raw_type eq '%') {
        ($ty, $pix) = hash_init(\@rhs, $sym);
    } else {
        my $v = parse_expr(\@rhs, { %$env, at => $sym });
        refuse($sym, "`\@$name` holds a list; this is a $v->{ty}") if $sym->raw_type eq '@' && $v->{ty} !~ /\AList</;
        ($ty, $pix) = ($v->{ty}, $v->{pix});
    }
    $env->{vars}{$name} = { ty => $ty, pix => $name };
    return "var $name : $ty = $pix";
}

sub ret {
    my ($toks, $env) = @_;
    my @t = @$toks;
    return 'return' if @t == 1;
    my $v = parse_expr([@t[1 .. $#t]], { %$env, at => $t[0] });
    refuse($t[0], "this method answers a $env->{ret}, and this is a $v->{ty}" . maybe_hint($v, 'it'))
        if $env->{ret} && !fits($env->{ret}, $v->{ty});
    refuse($t[0], 'this method does not say what it answers; write `:Sig(... => ' .
                  ($v->{ty} eq 'String' ? 'Str' : $v->{ty} eq 'Float' ? 'Num' : $v->{ty}) . ')`')
        unless $env->{ret};
    return "return $v->{pix}";
}

# `push @xs, e;` / `unshift @xs, e;`
sub grow {
    my ($toks, $env) = @_;
    my @t = @$toks;
    my $what = $t[0]->content;
    refuse($t[0], "`$what` takes a list and what to add: `$what \@items, \$t`")
        unless is_sym($t[1], '@') && is_op($t[2], ',') && @t > 3;
    my $lv = read_list($t[1], substr($t[1]->content, 1), $env);
    my $inner = $lv->{ty} =~ /\AList<(.+)>\z/ ? $1 : refuse($t[1], 'this is not a list');
    my $v = parse_expr([@t[3 .. $#t]], { %$env, at => $t[2] });
    refuse($t[2], "`\@" . substr($t[1]->content, 1) . "` holds a $inner, and this is a $v->{ty}")
        unless $v->{ty} eq $inner || ($inner eq 'Float' && $v->{ty} eq 'Int');
    return "$lv->{pix}." . ($what eq 'push' ? 'push' : 'insert') . "($v->{pix})" if $what eq 'push';
    return "$lv->{pix}.insert(0, $v->{pix})";
}

# `pop @xs;` / `shift @xs;`
sub shrink {
    my ($toks, $env) = @_;
    my @t = @$toks;
    my $what = $t[0]->content;
    refuse($t[0], "`$what` takes a list: `$what \@items`") unless @t == 2 && is_sym($t[1], '@');
    my $lv = read_list($t[1], substr($t[1]->content, 1), $env);
    return "$lv->{pix}.pop()" if $what eq 'pop';
    return "$lv->{pix}.removeAt(0)";
}

# `delete $h{k};`
sub drop {
    my ($toks, $env) = @_;
    my @t = @$toks;
    refuse($t[0], '`delete` takes one key of a hash: `delete $prices{$k}`')
        unless @t == 3 && is_sym($t[1], '$') && is_sub($t[2], '{');
    my $mv = read_map($t[1], substr($t[1]->content, 1), $env);
    return "$mv->{pix}.remove(" . subscript_key($t[2], $env) . ')';
}

# `@xs = ();`, `@xs = (1, 2);`, `%h = ();`
sub whole_assign {
    my ($toks, $env) = @_;
    my @t = @$toks;
    my ($sym, $eq, @rhs) = @t;
    refuse($sym, 'a list or a hash is written whole with `=`') unless is_op($eq, '=') && @rhs;
    if ($sym->raw_type eq '%') {
        my $mv = read_map($sym, substr($sym->content, 1), $env);
        refuse($eq, 'a hash is emptied with `%h = ();`') unless is_empty_list(\@rhs);
        return "$mv->{pix} = {}";
    }
    my $lv = read_list($sym, substr($sym->content, 1), $env);
    return "$lv->{pix} = []" if is_empty_list(\@rhs);
    if (my $p = pipeline_rhs(\@rhs, $env, $sym)) {
        refuse($eq, "`\@" . substr($sym->content, 1) . "` holds a $lv->{ty}, and this is a $p->{ty}")
            unless $p->{ty} eq $lv->{ty};
        return "$lv->{pix} = $p->{pix}";
    }
    if (@rhs == 1 && $rhs[0]->isa('PPI::Structure::List')) {
        my ($ty, $pix) = list_literal($rhs[0], $env);
        refuse($eq, "`\@" . substr($sym->content, 1) . "` holds a $lv->{ty}, and this is a $ty") unless $ty eq $lv->{ty};
        return "$lv->{pix} = $pix";
    }
    my $v = parse_expr(\@rhs, { %$env, at => $eq });
    refuse($eq, "`\@" . substr($sym->content, 1) . "` holds a $lv->{ty}, and this is a $v->{ty}") unless $v->{ty} eq $lv->{ty};
    return "$lv->{pix} = $v->{pix}";
}

# `$x = e`, `$x += e`, `$xs[$i] = e`, `$h{k} = e`.
sub assign {
    my ($toks, $env) = @_;
    my @t = unlabel($toks);
    my $sym = shift @t;
    my $name = substr $sym->content, 1;
    my ($target, $tty);
    if (is_sub($t[0], '[')) {
        my $sub = shift @t;
        my $lv = read_list($sym, $name, $env);
        my $r = list_read($sym, $lv, $sub, $env);
        ($target, $tty) = ($r->{pix}, $r->{ty});
        refuse($sub, 'the row a repeater is showing cannot be written from a view') if $target eq 'it';
    } elsif (is_sub($t[0], '{')) {
        my $sub = shift @t;
        my $mv = read_map($sym, $name, $env);
        my $inner = $mv->{ty} =~ /\AMap<String, (.+)>\z/ ? $1 : refuse($sym, "`%$name` is not a hash");
        ($target, $tty) = ($mv->{pix} . "[" . subscript_key($sub, $env) . "]", $inner);
    } else {
        my $v = read_var($sym, $name, $env);
        refuse($sym, "`\$$name` is read as the value it holds inside `if (defined \$$name)`; write it outside that block")
            if $v->{narrowed};
        refuse($sym, "`\$$name` cannot be written here; it is what this is called with")
            if $env->{vars}{$name} && $env->{vars}{$name}{fixed};
        ($target, $tty) = ($v->{pix}, $v->{ty});
    }
    my $op = shift @t;
    if (is_op($op) && ($op->content eq '++' || $op->content eq '--')) {
        refuse($op, "`\$$name` holds a $tty, and Perl's `++` on a string counts letters (`\"az\"++` is "
                  . '`"ba"`), which the compiled run does not do; join or replace the string instead')
            if $tty eq 'String';
        refuse($op, "`" . $op->content . "` counts a number (`\$$name` holds a $tty)") unless num_ty($tty);
        refuse($op, 'nothing follows `' . $op->content . '`') if @t;
        my $by = $op->content eq '++' ? '+' : '-';
        return "$target = $target $by 1";
    }
    refuse($op // $sym, 'a statement here writes with `=`, `+=`, `-=`, `*=`, `/=` or `.=`')
        unless is_op($op) && $op->content =~ m{\A(?:=|\+=|-=|\*=|/=|\.=)\z} && @t;
    # `$x = c ? a : b` lowers to an if/else, each branch writing the same place.
    my ($q) = grep { is_op($t[$_], '?') } 0 .. $#t;
    if (defined $q) {
        refuse($op, 'a conditional expression stands on the right of a plain `=`') unless $op->content eq '=';
        my ($c) = grep { is_op($t[$_], ':') && $_ > $q } 0 .. $#t;
        refuse($t[$q], 'a `?` needs its `:`') unless defined $c;
        my $cond = cond_of([@t[0 .. $q - 1]], $env, $t[$q]);
        my $a = typed_rhs($name, $tty, [@t[$q + 1 .. $c - 1]], $env, $t[$q]);
        my $b = typed_rhs($name, $tty, [@t[$c + 1 .. $#t]], $env, $t[$c]);
        return ("if $cond {", "  $target = $a", '} else {', "  $target = $b", '}');
    }
    if ($op->content eq '=') {
        if (my $p = pipeline_rhs(\@t, $env, $sym)) {
            refuse($op, "`\$$name` holds a $tty, and this is a $p->{ty}")
                unless fits($tty, $p->{ty});
            return "$target = $p->{pix}";
        }
        return "$target = " . typed_rhs($name, $tty, \@t, $env, $op);
    }
    my $v = parse_expr(\@t, { %$env, at => $t[0] });
    my $o = substr $op->content, 0, 1;
    if ($o eq '.') {
        refuse($op, "`.=` joins strings: `\$$name` holds a $tty and this is a $v->{ty}")
            unless $tty eq 'String' && $v->{ty} eq 'String';
        return "$target = $target + " . group($v->{pix});
    }
    refuse($op, "`$o=` needs numbers: `\$$name` holds a $tty and this is a $v->{ty}") unless num_ty($tty) && num_ty($v->{ty});
    refuse($op, "`\$$name` holds an Int and `$o=` a Float would make it a Float") if $tty eq 'Int' && $v->{ty} eq 'Float';
    if ($o eq '/') {
        my $d = binop($op, '/', { ty => $tty, pix => $target }, $v, $env);
        refuse($op, "`\$$name` holds an Int, and Perl's `/` answers a Num") unless $tty eq 'Float';
        return "$target = $d->{pix}";
    }
    if ($o eq '%') {
        my $d = binop($op, '%', { ty => $tty, pix => $target }, $v, $env);
        return "$target = $d->{pix}";
    }
    return "$target = $target $o " . group($v->{pix});
}

# `$name =~ s/pat/repl/;` — the one that writes back. The compiled run
# has no place that a pattern quietly changes, so it is a write.
sub substitution {
    my ($toks, $env) = @_;
    my @t = @$toks;
    refuse($t[0], 'a substitution stands on its own line: `$name =~ s/old/new/;`') if @t > 3;
    my $name = substr $t[0]->content, 1;
    my $v = read_var($t[0], $name, $env);
    refuse($t[0], "`\$$name` holds a $v->{ty}, and a pattern changes a string") unless $v->{ty} eq 'String';
    refuse($t[0], "`\$$name` cannot be written here; it is what this is called with")
        if $env->{vars}{$name} && $env->{vars}{$name}{fixed};
    my ($pat, $flags) = regexp_of($t[2], $env);
    refuse($t[2], '`/r` answers a new string and changes nothing, so it belongs on the right of an `=`')
        if $flags =~ /r/;
    $uses_pl = 1;
    return "$v->{pix} = " . subst_call($t[2], $pat, $flags, $v->{pix}, $env);
}

sub typed_rhs {
    my ($name, $fty, $toks, $env, $at) = @_;
    my $v = parse_expr($toks, { %$env, at => $at });
    refuse($at, "`\$$name` holds a $fty, and this is a $v->{ty}" . maybe_hint($v, 'the value'))
        unless fits($fty, $v->{ty});
    return $v->{pix};
}

sub cond_of {
    my ($toks, $env, $at, $sink) = @_;
    my @t = @$toks;
    # `if ($c)` — the parentheses are the statement's, not the expression's
    @t = sig(($t[0]->schildren)[0]) if @t == 1 && $t[0]->isa('PPI::Structure::Condition');
    # `defined $x` on a value that may be nothing: the branch it guards
    # reads `$x` as the value. The compiled run spells that `if let`.
    if (my ($neg, @d) = narrowing_shape(\@t)) {
        my $v = parse_expr(\@d, { %$env, at => $at, allow_maybe => 1 });
        if ($v->{ty} =~ /\A(.+)\?\z/ && !$v->{maybe}) {
            refuse($at, '`defined` here is the whole condition of an `if` or `unless`, and its branch reads '
                      . 'the value; it does not go in a `while`, a `grep`, or beside `&&`')
                unless $sink;
            my $bind = '__v' . ++$tmp;
            $sink->{narrow} = { key => $v->{pix}, ty => $1, pix => $bind, neg => $neg };
            return "let some($bind) = $v->{pix}";
        }
    }
    my $v = parse_expr(\@t, { %$env, at => $at, ($sink ? (sink => $sink) : ()) });
    refuse($at, "a condition is a bool (got $v->{ty}); Perl's truthiness of a number or a string is not in the translator — compare it (`!= 0`, `ne \"\"`)")
        unless $v->{ty} eq 'Bool';
    return $v->{pix};
}

# `defined $x`, `defined($x)`, `!defined $x`, `not defined $x`: whether
# the run of tokens is one of those, which way round, and the tokens of
# `$x`. Anything else is not a narrowing.
sub narrowing_shape {
    my ($toks) = @_;
    my @t = @$toks;
    my $neg = 0;
    if (@t && (is_op($t[0], '!') || is_word($t[0], 'not'))) { $neg = 1; shift @t }
    return () unless @t >= 2 && is_word($t[0], 'defined');
    shift @t;
    @t = sig(($t[0]->schildren)[0]) if @t == 1 && $t[0]->isa('PPI::Structure::List') && $t[0]->schildren;
    return () unless @t && is_sym($t[0], '$');
    return ($neg, @t);
}

# The two branches of an `if` whose condition narrowed a value: the env
# each is read in, and whether the branches change places (a `!defined`
# reads the value in the else).
sub branch_envs {
    my ($env, $sink, $unless) = @_;
    my $n = $sink->{narrow} or return ($env, $env, 0);
    my $neg = $n->{neg} ^ ($unless ? 1 : 0);
    my $narrowed = { %$env, narrowed => { %{ $env->{narrowed} // {} }, $n->{key} => { ty => $n->{ty}, pix => $n->{pix} } } };
    return $neg ? ($env, $narrowed, 1) : ($narrowed, $env, 0);
}

# if / elsif / else, unless, while, for.
sub compound {
    my ($st, $env) = @_;
    my @t = sig($st);
    return loop($st, \@t, $env) if is_word($t[0], 'for') || is_word($t[0], 'foreach') || is_word($t[0], 'while');
    refuse($t[0], 'a statement here is `if`, `unless`, `while` or `for`')
        unless is_word($t[0], 'if') || is_word($t[0], 'unless');
    my @out;
    my $depth = 0;
    my $i = 0;
    while ($i < @t) {
        my $kw = $t[$i];
        if (is_word($kw, 'if') || is_word($kw, 'unless') || is_word($kw, 'elsif')) {
            my ($cnd, $blk) = @t[$i + 1, $i + 2];
            refuse($kw, "`" . $kw->content . "` takes its condition in parentheses and a block")
                unless $cnd && $cnd->isa('PPI::Structure::Condition') && $blk && $blk->isa('PPI::Structure::Block');
            my %caught;
            my $c = cond_of([$cnd], $env, $kw, \%caught);
            my ($tenv, $eenv, $swap) = branch_envs($env, \%caught, $kw->content eq 'unless');
            $c = "!($c)" if $kw->content eq 'unless' && !$caught{narrow};
            if ($swap) {
                # `if (!defined $x) { A } else { B }`: the compiled run
                # reads the value in its own then-branch, so B goes first.
                refuse($kw, 'an `elsif` cannot ask `!defined`; write the `if` the other way round')
                    if $kw->content eq 'elsif';
                my $nxt = $t[$i + 3];
                refuse($nxt, 'after `if (!defined $x)` comes `else` or nothing; for an `elsif`, write the `if` the other way round')
                    if is_word($nxt, 'elsif');
                my $else_blk = is_word($nxt, 'else') ? $t[$i + 4] : undef;
                refuse($nxt, '`else` takes a block') if is_word($nxt, 'else') && !($else_blk && $else_blk->isa('PPI::Structure::Block'));
                push @out, "if $c {";
                push @out, map { "  $_" } ($else_blk ? stmts($else_blk, scope($eenv)) : ());
                push @out, '} else {';
                push @out, map { "  $_" } stmts($blk, scope($tenv));
                push @out, '}';
                $i += $else_blk ? 5 : 3;
                refuse($t[$i], 'this does not belong in an if statement: `' . $t[$i]->content . '`') if $i < @t;
                return @out;
            }
            if ($kw->content eq 'elsif') {
                push @out, '} else {';
                push @out, "  if $c {";
                $depth++;
            } else {
                push @out, "if $c {";
            }
            my $branch = scope($tenv);
            $branch->{capture} = \%caught if $caught{pat} && $kw->content ne 'unless';
            push @out, map { ('  ' x ($depth + 1)) . $_ } stmts($blk, $branch);
            $i += 3;
        } elsif (is_word($kw, 'else')) {
            my $blk = $t[$i + 1];
            refuse($kw, '`else` takes a block') unless $blk && $blk->isa('PPI::Structure::Block');
            push @out, ('  ' x $depth) . '} else {';
            push @out, map { ('  ' x ($depth + 1)) . $_ } stmts($blk, scope($env));
            $i += 2;
        } else {
            refuse($kw, 'this does not belong in an if statement: `' . $kw->content . '`');
        }
    }
    push @out, ('  ' x $_) . '}' for reverse 0 .. $depth;
    return @out;
}

# A block's own names go out of scope with it.
sub scope { my ($env) = @_; return { %$env, vars => { %{ $env->{vars} } } } }

sub loop {
    my ($st, $toks, $env) = @_;
    my @t = @$toks;
    if (is_word($t[0], 'while')) {
        refuse($t[0], '`while` takes its condition in parentheses and a block')
            unless $t[1] && $t[1]->isa('PPI::Structure::Condition') && $t[2] && $t[2]->isa('PPI::Structure::Block');
        my $c = cond_of([$t[1]], { %$env, in_loop => 1 }, $t[0]);
        return ("while $c {", (map { "  $_" } stmts($t[2], { %{ scope($env) }, in_loop => 1 })), '}');
    }
    refuse($t[0], 'a loop is `for my $x (@items) { ... }` or `for my $i (0 .. $n) { ... }`')
        unless is_word($t[1], 'my') && is_sym($t[2], '$') && $t[3] && $t[3]->isa('PPI::Structure::List')
            && $t[4] && $t[4]->isa('PPI::Structure::Block');
    my $name = substr $t[2]->content, 1;
    my $over = list_source([$t[3]], $env, $t[0]);
    my $inner = scope($env);
    $inner->{vars}{$name} = { ty => $over->{ty}, pix => $name, fixed => 1 };
    $inner->{in_loop} = 1;
    $inner->{nonneg} = { %{ $env->{nonneg} // {} }, $name => 1 } if $over->{counted};
    return ("for $name in $over->{over} {",
            (map { "  $_" } stmts($t[4], $inner)), '}');
}

# `task(sub { ... }, on_done => sub ($v) { ... });` — work off the
# window's thread. The compiled run does it in an async fn; the
# interpreted one on a thread of perl's own. Both answer the same.
sub task_stmt {
    my ($toks, $env) = @_;
    my @t = @$toks;
    refuse($t[0], '`task` is started from a handler or a method of the app, whose fields the answer is kept in')
        if $env->{model};
    refuse($t[0], '`task` is called with parentheses') unless @t == 2 && $t[1]->isa('PPI::Structure::List');
    my @a = split_args($t[1]);
    refuse($t[0], '`task` takes the work and what to do with its answer: `task(sub { ... }, on_done => sub ($v) { ... })`')
        unless @a == 2 && !defined $a[0]{key} && ($a[1]{key} // '') eq 'on_done';
    my ($w, $wblock) = @{ $a[0]{toks} };
    refuse($a[0]{node}, '`task` runs a sub of its own: `task(sub { ... }, ...)`')
        unless is_word($w, 'sub') && $wblock && $wblock->isa('PPI::Structure::Block');
    my ($d, @drest) = @{ $a[1]{toks} };
    my ($dproto, $dblock) = @drest == 2 ? @drest : (undef, $drest[0]);
    refuse($a[1]{node}, '`on_done` takes the answer: `on_done => sub ($v) { ... }`')
        unless is_word($d, 'sub') && $dblock && $dblock->isa('PPI::Structure::Block');
    my @dp = sub_params($dproto);
    refuse($a[1]{node}, '`on_done` is called with the answer, and nothing else') unless @dp == 1;
    # The work: its statements, then the value its last one answers.
    my @wst = $wblock->schildren;
    refuse($wblock, 'the work answers a value with its last line') unless @wst;
    my $wenv = scope($env);
    my @lines;
    push @lines, stmt($wst[$_], $wenv) for 0 .. $#wst - 1;
    my $last = $wst[-1];
    my @lt = strip_semicolon(sig($last));
    shift @lt if is_word($lt[0], 'return');
    # The value the work answers: if it is one of the framework's own,
    # it waits on the engine's pool.
    my $awaiting = is_word($lt[0]) && $Rakugan::Manifest::NAMES{ $lt[0]->content };
    my $ans = parse_expr(\@lt, { %$wenv, at => $last, ($awaiting ? (awaiting => 1) : ()) });
    my $name = substr $dp[0], 1;
    my $denv = scope($env);
    $denv->{vars}{$name} = { ty => $ans->{ty}, pix => $name, fixed => 1 };
    push @lines, "var $name : $ans->{ty} = $ans->{pix}";
    push @lines, stmts($dblock, $denv);
    $env->{async} = 1;
    $env->{after_task} = 1;
    return @lines;
}

# --- the view, and the methods that answer part of it ---------------------
# A view's body is elements, the lists an app builds them into, `if`s
# and `for`s, and what it answers.
sub view {
    my $env = { ctx => 'view', vars => {}, elists => {}, nonneg => {}, at => $view_block };
    my @nodes = view_nodes($view_block, $env);
    refuse($view_block, '`view` answers one element; a screen that changes shape is an `if` inside it')
        unless @nodes == 1 && $nodes[0]{n} eq 'el';
    return $nodes[0];
}

sub view_nodes {
    my ($block, $env) = @_;
    my @st = grep { !$_->isa('PPI::Statement::Null') } $block->schildren;
    refuse($block, 'this answers no element') unless @st;
    return answer(\@st, 0, $env, $block);
}

# What a run of statements answers: the declarations and the appends on
# the way, and then one element — or one chosen with `return ... if`.
sub answer {
    my ($st, $i, $env, $at) = @_;
    refuse($at, 'this answers no element') if $i > $#$st;
    my $s = $st->[$i];
    if ($s->isa('PPI::Statement::Variable')) {
        view_declare($s, $env);
        return answer($st, $i + 1, $env, $at);
    }
    if ($s->isa('PPI::Statement::Compound') || is_word((strip_semicolon(sig($s)))[0], 'push')) {
        my ($target, @nodes) = append_of($s, $env);
        refuse($s, "`\@$target` is not a list of elements declared above this") unless $env->{elists}{$target};
        push @{ $env->{elists}{$target} }, @nodes;
        return answer($st, $i + 1, $env, $at);
    }
    my @t = strip_semicolon(sig($s));
    shift @t if is_word($t[0], 'return');
    my ($ix) = grep { is_word($t[$_], 'if') || is_word($t[$_], 'unless') } 1 .. $#t;
    if (defined $ix) {
        my %sink;
        my $cond = cond_of([@t[$ix + 1 .. $#t]], $env, $t[$ix], \%sink);
        my ($tenv, $eenv, $swap) = branch_envs($env, \%sink, $t[$ix]->content eq 'unless');
        $cond = "!($cond)" if $t[$ix]->content eq 'unless' && !$sink{narrow};
        my @then = child_nodes({ node => $t[0], toks => [@t[0 .. $ix - 1]] }, nonneg_in($tenv, $cond));
        my @else = answer($st, $i + 1, $eenv, $at);
        return $swap ? { n => 'if', cond => $cond, then => \@else, else => \@then }
                     : { n => 'if', cond => $cond, then => \@then, else => \@else };
    }
    refuse($st->[$i + 1], 'nothing follows the element this answers') if $i < $#$st;
    return child_nodes({ node => $t[0], toks => \@t }, $env);
}

# `my @cells = (text(...), ...);` — a list of elements an app builds up
# and hands to a container.
sub view_declare {
    my ($st, $env) = @_;
    my @t = strip_semicolon(sig($st));
    my $sym = $t[1];
    refuse($t[0], 'a view builds its parts into a list: `my @cells = (text("a"));`')
        unless is_word($t[0], 'my') && is_sym($sym, '@');
    my $name = substr $sym->content, 1;
    refuse($sym, "`\@$name` is declared twice") if $env->{elists}{$name};
    my @nodes;
    if (@t > 2) {
        refuse($sym, 'a list of elements starts as a list: `my @cells = (text("a"));`')
            unless is_op($t[2], '=') && @t == 4 && $t[3]->isa('PPI::Structure::List');
        push @nodes, child_nodes($_, $env) for split_args($t[3]);
    }
    $env->{elists}{$name} = \@nodes;
}

# `push @cells, E;`, and the `if`s and `for`s that do the same.
sub append_of {
    my ($st, $env) = @_;
    if ($st->isa('PPI::Statement::Compound')) {
        my @t = sig($st);
        if (is_word($t[0], 'for') || is_word($t[0], 'foreach')) {
            refuse($t[0], 'a loop in a view is `for my $x (@items) { push @cells, ... }`')
                unless is_word($t[1], 'my') && is_sym($t[2], '$') && $t[3] && $t[3]->isa('PPI::Structure::List')
                    && $t[4] && $t[4]->isa('PPI::Structure::Block');
            my $name = substr $t[2]->content, 1;
            my $over = list_source([$t[3]], $env, $t[0]);
            my $inner = { %$env, vars => { %{ $env->{vars} }, $name => { ty => $over->{ty}, pix => $name, fixed => 1 } } };
            $inner->{nonneg} = { %{ $env->{nonneg} // {} }, $name => 1 } if $over->{counted};
            if ($over->{indexed}) {
                my $inner = { %$env,
                    vars => { %{ $env->{vars} }, $name => { ty => 'Int', pix => $name, fixed => 1 } },
                    nonneg => { %{ $env->{nonneg} // {} }, $name => 1 },
                    repeat => { list => $over->{list}, it => 'it', ix => $name } };
                my ($target, @body) = block_appends($t[4], $inner);
                return ($target, { n => 'for', over => $over->{list}, it => 'it', ix => $name, body => \@body });
            }
            if ($over->{counted}) {
                refuse($t[0], 'a view repeats over a list, or over a run of numbers whose ends are written out')
                    unless defined $over->{from};
                my ($target, @nodes);
                for my $n ($over->{from} .. $over->{to}) {
                    my $one = { %$env, vars => { %{ $env->{vars} }, $name => { ty => 'Int', pix => $n, fixed => 1 } } };
                    my ($tg, @b) = block_appends($t[4], $one);
                    refuse($t[0], 'every branch here adds to one list of elements') if defined $target && $tg ne $target;
                    $target = $tg;
                    push @nodes, @b;
                }
                return ($target, @nodes);
            }
            my ($target, @body) = block_appends($t[4], $inner);
            return ($target, { n => 'for', over => $over->{over}, it => $name, body => \@body });
        }
        refuse($t[0], 'a view puts its parts under `if`, `unless` or `for`')
            unless is_word($t[0], 'if') || is_word($t[0], 'unless');
        return if_appends(\@t, 0, $env);
    }
    my @t = strip_semicolon(sig($st));
    refuse($t[0], 'a statement in a view adds to a list of elements: `push @cells, text("a");`')
        unless is_word($t[0], 'push') && is_sym($t[1], '@') && is_op($t[2], ',') && @t > 3;
    my $target = substr $t[1]->content, 1;
    my @rest = @t[3 .. $#t];
    my ($ix) = grep { is_word($rest[$_], 'if') || is_word($rest[$_], 'unless') } 1 .. $#rest;
    if (defined $ix) {
        my %sink;
        my $cond = cond_of([@rest[$ix + 1 .. $#rest]], $env, $rest[$ix], \%sink);
        my ($tenv, $eenv, $swap) = branch_envs($env, \%sink, $rest[$ix]->content eq 'unless');
        $cond = "!($cond)" if $rest[$ix]->content eq 'unless' && !$sink{narrow};
        my @then = child_nodes({ node => $rest[0], toks => [@rest[0 .. $ix - 1]] }, nonneg_in($tenv, $cond));
        return ($target, $swap ? { n => 'if', cond => $cond, then => [], else => \@then }
                               : { n => 'if', cond => $cond, then => \@then, else => [] });
    }
    return ($target, child_nodes({ node => $rest[0], toks => \@rest }, $env));
}

# An `if` chain in a view body, every branch adding to one list.
sub if_appends {
    my ($t, $i, $env) = @_;
    my $kw = $t->[$i];
    my ($cnd, $blk) = @{$t}[$i + 1, $i + 2];
    refuse($kw, '`' . $kw->content . '` takes its condition in parentheses and a block')
        unless $cnd && $cnd->isa('PPI::Structure::Condition') && $blk && $blk->isa('PPI::Structure::Block');
    my %sink;
    my $cond = cond_of([$cnd], $env, $kw, \%sink);
    my ($tenv, $eenv, $swap) = branch_envs($env, \%sink, $kw->content eq 'unless');
    $cond = "!($cond)" if $kw->content eq 'unless' && !$sink{narrow};
    my ($target, @then) = block_appends($blk, nonneg_in($tenv, $cond));
    my @else;
    if (my $next = $t->[$i + 3]) {
        if (is_word($next, 'elsif')) {
            refuse($next, 'after `if (!defined $x)` comes `else` or nothing; for an `elsif`, write the `if` the other way round')
                if $swap;
            my ($t2, @n) = if_appends($t, $i + 3, $eenv);
            refuse($next, 'every branch here adds to one list of elements') if $t2 ne $target;
            @else = @n;
        } elsif (is_word($next, 'else')) {
            refuse($next, '`else` takes a block') unless $t->[$i + 4] && $t->[$i + 4]->isa('PPI::Structure::Block');
            my ($t2, @n) = block_appends($t->[$i + 4], $eenv);
            refuse($next, 'every branch here adds to one list of elements') if $t2 ne $target;
            @else = @n;
        } else {
            refuse($next, 'this does not belong in an if statement: `' . $next->content . '`');
        }
    }
    return ($target, $swap ? { n => 'if', cond => $cond, then => \@else, else => \@then }
                           : { n => 'if', cond => $cond, then => \@then, else => \@else });
}

# A branch whose condition says a number is not negative lets the .pix
# read a list with it: `unless ($i < 0)` and `if ($i >= 0)` both do.
sub nonneg_in {
    my ($env, $cond) = @_;
    my ($name) = $cond =~ /\A!\((\S+) < 0\)\z/;
    ($name) = $cond =~ /\A(\S+) >= 0\z/ unless defined $name;
    return $env unless defined $name;
    return { %$env, nonneg => { %{ $env->{nonneg} // {} }, $name => 1 } };
}

sub block_appends {
    my ($block, $env) = @_;
    my ($target, @nodes);
    for my $st (grep { !$_->isa('PPI::Statement::Null') } $block->schildren) {
        my ($t, @n) = append_of($st, $env);
        refuse($st, 'every branch here adds to one list of elements') if defined $target && $t ne $target;
        $target = $t;
        push @nodes, @n;
    }
    refuse($block, 'this branch adds no element') unless defined $target;
    return ($target, @nodes);
}

# --- what a canvas is painted with ------------------------------------------
#
# A command is not an element: it has no handle, takes none of the
# keywords every element takes, and means nothing outside the canvas it
# is written in. What a paint sub holds is commands, `if`s and `for`s.
sub paint_nodes {
    my ($block, $env) = @_;
    my @out;
    for my $st (grep { !$_->isa('PPI::Statement::Null') } $block->schildren) {
        push @out, paint_stmt($st, $env);
    }
    return @out;
}

sub paint_stmt {
    my ($st, $env) = @_;
    if ($st->isa('PPI::Statement::Compound')) {
        my @t = sig($st);
        if (is_word($t[0], 'for') || is_word($t[0], 'foreach')) {
            refuse($t[0], 'a loop here is `for my $x (@items) { ... }`')
                unless is_word($t[1], 'my') && is_sym($t[2], '$') && $t[3] && $t[3]->isa('PPI::Structure::List')
                    && $t[4] && $t[4]->isa('PPI::Structure::Block');
            my $name = substr $t[2]->content, 1;
            my $over = list_source([$t[3]], $env, $t[0]);
            my $inner = { %$env, vars => { %{ $env->{vars} }, $name => { ty => $over->{ty}, pix => $name, fixed => 1 } } };
            return unrolled_paint($over, $name, $t[4], $env, $t[0]) if $over->{counted} && !$over->{indexed};
            if ($over->{indexed}) {
                $inner->{nonneg} = { %{ $env->{nonneg} // {} }, $name => 1 };
                $inner->{repeat} = { list => $over->{list}, it => 'it', ix => $name };
                return { n => 'for', over => $over->{list}, it => 'it', ix => $name,
                         body => [paint_nodes($t[4], $inner)] };
            }
            return { n => 'for', over => $over->{over}, it => $name, body => [paint_nodes($t[4], $inner)] };
        }
        refuse($t[0], 'a canvas paints under `if`, `unless` and `for`')
            unless is_word($t[0], 'if') || is_word($t[0], 'unless');
        return paint_if(\@t, 0, $env);
    }
    my @t = strip_semicolon(sig($st));
    my ($ix) = grep { is_word($t[$_], 'if') || is_word($t[$_], 'unless') } 1 .. $#t;
    if (defined $ix) {
        my $cond = cond_of([@t[$ix + 1 .. $#t]], $env, $t[$ix]);
        $cond = "!($cond)" if $t[$ix]->content eq 'unless';
        return { n => 'if', cond => $cond,
                 then => [paint_call([@t[0 .. $ix - 1]], $env)], else => [] };
    }
    return paint_call(\@t, $env);
}

sub unrolled_paint {
    my ($over, $name, $block, $env, $at) = @_;
    refuse($at, 'a canvas repeats over a list, or over a run of numbers whose ends are written out')
        unless defined $over->{from};
    my @out;
    for my $n ($over->{from} .. $over->{to}) {
        my $inner = { %$env, vars => { %{ $env->{vars} }, $name => { ty => 'Int', pix => $n, fixed => 1 } } };
        push @out, paint_nodes($block, $inner);
    }
    return @out;
}

sub paint_if {
    my ($t, $i, $env) = @_;
    my $kw = $t->[$i];
    my ($cnd, $blk) = @{$t}[$i + 1, $i + 2];
    refuse($kw, '`' . $kw->content . '` takes its condition in parentheses and a block')
        unless $cnd && $cnd->isa('PPI::Structure::Condition') && $blk && $blk->isa('PPI::Structure::Block');
    my $cond = cond_of([$cnd], $env, $kw);
    $cond = "!($cond)" if $kw->content eq 'unless';
    my @then = paint_nodes($blk, nonneg_in($env, $cond));
    my @else;
    if (my $next = $t->[$i + 3]) {
        if (is_word($next, 'elsif')) {
            @else = paint_if($t, $i + 3, $env);
        } elsif (is_word($next, 'else')) {
            refuse($next, '`else` takes a block') unless $t->[$i + 4] && $t->[$i + 4]->isa('PPI::Structure::Block');
            @else = paint_nodes($t->[$i + 4], $env);
        } else {
            refuse($next, 'this does not belong in an if statement: `' . $next->content . '`');
        }
    }
    return { n => 'if', cond => $cond, then => \@then, else => \@else };
}

sub paint_call {
    my ($toks, $env) = @_;
    my ($w, $list, @more) = @$toks;
    # A method that paints is written out where it is called: the
    # compiled run has commands inside a canvas and nothing to call.
    if (is_sym($w, '$') && ($w->content eq '$self' || (defined $app_var && $w->content eq "\$$app_var"))) {
        refuse($w, 'a method of the app is called `$self->name(...)`')
            unless is_op($toks->[1], '->') && is_word($toks->[2]);
        my $m = $method{ $toks->[2]->content }
            or refuse($toks->[2], '`' . $toks->[2]->content . "` is not a method of $class_name");
        refuse($toks->[2], '`' . $m->{name} . '` paints nothing; a canvas holds commands')
            unless $m->{paints};
        my @given = $toks->[3] ? split_args($toks->[3]) : ();
        refuse($toks->[2], "`$m->{name}` takes " . scalar(@{ $m->{params} }) . ' values')
            unless @given == @{ $m->{params} };
        my $inner = { ctx => $env->{ctx}, vars => {}, nonneg => { %{ $env->{nonneg} // {} } },
                      repeat => $env->{repeat}, at => $toks->[2] };
        for my $i (0 .. $#given) {
            my ($pname, $pty) = @{ $m->{params}[$i] };
            my $v = parse_expr($given[$i]{toks}, { %$env, at => $given[$i]{node} });
            refuse($given[$i]{node}, "`$m->{name}` takes a $pty here (got $v->{ty})")
                unless $v->{ty} eq $pty || ($pty eq 'Float' && $v->{ty} eq 'Int');
            $inner->{vars}{ substr $pname, 1 } = { ty => $pty, pix => $v->{pix}, fixed => 1 };
        }
        return paint_nodes($m->{block}, $inner);
    }
    refuse($toks->[0], 'a canvas is painted with its commands: ' . join(', ', sort keys %OP))
        unless is_word($w) && $OP{ $w->content };
    refuse($w, '`' . $w->content . '` is called with parentheses')
        unless $list && $list->isa('PPI::Structure::List') && !@more;
    my $op = $OP{ $w->content };
    my @a = split_args($list);
    my @need = grep { !exists $_->{default} } @{ $op->{params} };
    refuse($w, "`$op->{name}` takes " . scalar(@need) . ' values ('
             . join(', ', map { $_->{name} } @need) . ')')
        if @a < @need || (@a > @need && !defined $a[scalar @need]{key});
    my %want = (int => 'Int', str => 'String', bool => 'Bool');
    my $value = sub {
        my ($p, $arg) = @_;
        my $v = parse_expr($arg->{toks}, { %$env, at => $arg->{node} });
        refuse($arg->{node}, "`$op->{name}`'s $p->{name} is a $want{$p->{type}} (got $v->{ty})")
            unless $v->{ty} eq $want{ $p->{type} };
        return [$p->{pix}, $p->{type} eq 'str' ? str_value($v) : $v->{pix}];
    };
    my @props;
    push @props, $value->($need[$_], $a[$_]) for 0 .. $#need;
    # What is left over is named: a value with a default is given by
    # name or not at all.
    for my $arg (@a[scalar(@need) .. $#a]) {
        refuse($arg->{node}, "`$op->{name}` takes its first values in order and the rest by name")
            unless defined $arg->{key};
        my ($p) = grep { $_->{name} eq $arg->{key} && exists $_->{default} } @{ $op->{params} };
        refuse($arg->{node}, "`$op->{name}` has no `$arg->{key} =>`; it takes "
                           . join(', ', map { "`$_->{name}`" }
                                  grep { exists $_->{default} } @{ $op->{params} }))
            unless $p;
        push @props, $value->($p, $arg);
    }
    return { n => 'el', pix => $op->{pix}, props => \@props, kids => [], flat => 1 };
}

# --- components -----------------------------------------------------------
sub comp_name {
    my ($m) = @_;
    (my $n = ucfirst $m->{name}) =~ s/_(\w)/\u$1/g;
    refuse($m->{node}, "`$m->{name}` would be written `$n` in the compiled run, which is an element's own name; "
                     . 'pick another name')
        if $PIX_ELEMENT{$n};
    return $n;
}

# A method that answers an element is a component of its own, unless it
# CHOOSES between elements: a component's body is one element, so that
# one is written out where it is called.
sub component_of {
    my ($m, $nonneg) = @_;
    return $m if $m->{tried};
    $m->{tried} = 1;
    $m->{nonneg} = $nonneg // {};
    my $mark = scalar @handlers;
    my $env = { ctx => 'view', vars => {}, elists => {}, nonneg => { %{ $m->{nonneg} } }, at => $m->{node} };
    for my $p (@{ $m->{params} }) {
        next if $p->[0] =~ /\A\@/;
        $env->{vars}{ substr $p->[0], 1 } = { ty => $p->[1], pix => substr($p->[0], 1), fixed => 1 };
    }
    $env->{kids} = substr($m->{params}[-1][0], 1) if $m->{kids};
    my @nodes = view_nodes($m->{block}, $env);
    if (@nodes == 1 && $nodes[0]{n} eq 'el') {
        $m->{comp} = { name => comp_name($m), nodes => \@nodes };
    } else {
        # It chose; the handlers that translation made belong to each
        # place it is called instead.
        splice @handlers, $mark;
        $m->{inline} = 1;
    }
    return $m;
}

sub component_call {
    my ($word, $args, $extra, $env) = @_;
    my $m = $method{ $word->content };
    my @vparams = grep { $_->[0] =~ /\A\$/ } @{ $m->{params} };
    my @given = (($args ? split_args($args) : ()), @$extra);
    refuse($word, "`$m->{name}` takes " . scalar(@vparams) . ' value'
                . (@vparams == 1 ? '' : 's') . ($m->{kids} ? ' and the elements under it' : ''))
        if @given < @vparams || (@given > @vparams && !$m->{kids});
    my (@vals, @kidargs, %nonneg);
    for my $i (0 .. $#given) {
        if ($i < @vparams) {
            refuse($given[$i]{node}, "`$m->{name}` takes its values in order, without names") if defined $given[$i]{key};
            my $v = parse_expr($given[$i]{toks}, { %$env, at => $given[$i]{node} });
            my $pty = $vparams[$i][1];
            refuse($given[$i]{node}, "`$m->{name}` takes a $pty here (got $v->{ty})")
                unless $v->{ty} eq $pty || ($pty eq 'Float' && $v->{ty} eq 'Int');
            push @vals, $v->{pix};
            # A number a component reads a list with has to be one the
            # caller can prove is not negative, since a component's body
            # has nowhere to put the line that would make it good.
            $nonneg{ substr $vparams[$i][0], 1 } = 1
                if $v->{ty} eq 'Int' && ($v->{pix} =~ /\A\d+\z/ || ($env->{nonneg} // {})->{ $v->{pix} });
        } else {
            push @kidargs, $given[$i];
        }
    }
    component_of($m, \%nonneg);
    for my $p (keys %{ $m->{nonneg} }) {
        refuse($word, "`$m->{name}` reads a list with `\$$p`, so every place it is called has to hand it "
                    . 'a number that cannot be negative')
            unless $nonneg{$p};
    }
    my @kids = map { child_nodes($_, $env) } @kidargs;
    if ($m->{inline}) {
        my $ienv = { ctx => 'view', vars => {}, elists => {}, nonneg => { %{ $env->{nonneg} // {} } }, at => $word };
        $ienv->{vars}{ substr $vparams[$_][0], 1 } = { ty => $vparams[$_][1], pix => $vals[$_], fixed => 1 }
            for 0 .. $#vparams;
        $ienv->{elists}{ substr $m->{params}[-1][0], 1 } = \@kids if $m->{kids};
        return view_nodes($m->{block}, $ienv);
    }
    my @props = map { [substr($vparams[$_][0], 1), $vals[$_]] } 0 .. $#vparams;
    return { n => 'el', pix => $m->{comp}{name}, props => \@props, kids => \@kids };
}

# --- what each method is --------------------------------------------------
# One that answers an element is part of the screen; one that touches a
# field is the store's, and a view cannot call it; the rest are statics
# a view can.
sub classify {
    for my $m (@methods) {
        # The compiled run gives every field a reader of its own name,
        # so a method cannot share one.
        refuse($m->{node}, "`$m->{name}` is both a field and a method here, and the compiled run gives "
                         . 'a field a reader of its own name; rename one of them')
            if exists $field{ $m->{name} };
        $m->{calls} = [map { $_->content } grep {
            my $p = $_->sprevious_sibling;
            is_op($p, '->') && is_sym($p->sprevious_sibling, '$')
        } @{ $m->{block}->find('PPI::Token::Word') || [] }];
        # A field is touched under any sigil: `@items` is the list and
        # `$items[$i]` is one of its elements. A field read inside a
        # string counts too — the hole is a read like any other.
        $m->{own_fields} = grep { exists $field{ substr $_->content, 1 } }
            @{ $m->{block}->find('PPI::Token::Symbol') || [] };
        for my $q (@{ $m->{block}->find('PPI::Token::Quote::Double') || [] }) {
            my $text = $q->content;
            $m->{own_fields} ||= grep { exists $field{$_} } $text =~ /[\$\@%]\{?(\w+)/g;
        }
        $m->{answers} = [answers_of($m->{block})];
        # A method whose statements are drawing commands paints; it is
        # written out inside the canvas that calls it.
        $m->{paints} = grep {
            $OP{ $_->content } && !is_op($_->sprevious_sibling, '->')
        } @{ $m->{block}->find('PPI::Token::Word') || [] };
        # What could stop the method: a division, a root, a `die`, or a
        # call into the library with no `_or`. A `try` cannot reach into
        # a method, so a caller inside one is told.
        $m->{may_fail} = grep({ $_->content =~ m{\A[/%]=?\z} } @{ $m->{block}->find('PPI::Token::Operator') || [] })
            || grep({ my $n = $_->content;
                      $n eq 'die' || $n eq 'sqrt' || ($Rakugan::Manifest::NAMES{$n} && !cannot_fail($n)) }
                    @{ $m->{block}->find('PPI::Token::Word') || [] });
    }
    my $changed = 1;
    while ($changed) {
        $changed = 0;
        for my $m (@methods) {
            my $st = $m->{own_fields} || grep { $method{$_} && $method{$_}{stateful} } @{ $m->{calls} };
            if ($st && !$m->{stateful}) { $m->{stateful} = 1; $changed = 1 }
            my $mf = grep { $method{$_} && $method{$_}{may_fail} } @{ $m->{calls} };
            if ($mf && !$m->{may_fail}) { $m->{may_fail} = 1; $changed = 1 }
            my $el = grep {
                is_word($_) && ($ELEMENT{ $_->content } || ($method{ $_->content } && $method{ $_->content }{element}))
            } @{ $m->{answers} };
            if ($el && !$m->{element}) { $m->{element} = 1; $changed = 1 }
        }
    }
}

# The first token of everything a method answers: what it returns, and
# what its last statement is.
sub answers_of {
    my ($block) = @_;
    my @runs;
    for my $r (@{ $block->find('PPI::Statement::Break') || [] }) {
        my @t = sig($r);
        push @runs, [@t[1 .. $#t]] if is_word($t[0], 'return') && @t > 1;
    }
    my @st = grep { !$_->isa('PPI::Statement::Null') } $block->schildren;
    push @runs, [sig($st[-1])] if @st;
    my @out;
    for my $t (@runs) {
        next unless @$t;
        # `$self->name` answers whatever `name` does.
        push @out, (is_sym($t->[0], '$') && is_op($t->[1], '->') && is_word($t->[2])) ? $t->[2] : $t->[0];
    }
    return grep { defined } @out;
}

sub bodies {
    for my $name (@models) {
        for my $mm (@{ $model{$name}{order} }) {
            my $env = { ctx => 'fn', model => $name, vars => {}, nonneg => {}, ret => $mm->{ret}, at => $mm->{node} };
            $env->{vars}{ substr $_->[0], 1 } = { ty => $_->[1], pix => substr($_->[0], 1), fixed => 1 } for @{ $mm->{params} };
            $mm->{body} = [stmts($mm->{block}, $env)];
        }
    }
    for my $m (@methods) {
        next if $m->{element} || $m->{paints};
        my $env = { ctx => 'fn', vars => {}, nonneg => {}, ret => $m->{ret}, at => $m->{node} };
        $env->{vars}{ substr $_->[0], 1 } = { ty => $_->[1], pix => substr($_->[0], 1), fixed => 1 } for @{ $m->{params} };
        $m->{body} = [stmts($m->{block}, $env)];
        $m->{async} = $env->{async};
    }
    for my $s (@app_stmts) {
        refuse($s->{word}, '`task` is started from a handler, where something is happening') if $s->{what} eq 'task';
        if ($s->{what} ne 'every') { binding($s); next }
        my @a = split_args($s->{list});
        refuse($s->{word}, '`every` takes how many seconds, and what to do: `every(1.0, sub { ... })`')
            unless @a == 2 && !defined $a[0]{key} && !defined $a[1]{key};
        my $secs = parse_expr($a[0]{toks}, { ctx => 'fn', vars => {}, nonneg => {}, at => $a[0]{node} });
        refuse($a[0]{node}, "`every` takes a number of seconds (got $secs->{ty})") unless num_ty($secs->{ty});
        refuse($a[0]{node}, '`every` takes a number written out') unless $secs->{pix} =~ /\A[\d.]+\z/;
        my ($sub, $block) = @{ $a[1]{toks} };
        refuse($a[1]{node}, '`every` runs a sub: `every(1.0, sub { ... })`')
            unless is_word($sub, 'sub') && $block && $block->isa('PPI::Structure::Block');
        my $env = { ctx => 'fn', vars => {}, nonneg => {}, at => $a[1]{node} };
        push @timers, { ms => sprintf('%g', $secs->{pix} * 1000), body => [stmts($block, $env)] };
    }
}

# `shortcut("cmd+s", sub { ... })`, `menu_item("File", "Save", sub { ... })`,
# `on_key(sub ($chord) { ... })`, `on_file_drop(sub ($path) { ... })`.
# Each is declared before the app runs and outlives every build, which
# is why they are numbered apart from a build's own handlers.
sub binding {
    my ($s) = @_;
    my $what = $s->{what};
    my @a = split_args($s->{list});
    my %takes = (shortcut => 1, menu_item => 2, on_key => 0, on_file_drop => 0);
    my $want = $takes{$what} + 1;
    refuse($s->{word}, "`$what` takes " . ($want == 1 ? 'a sub' : ($want - 1) . ' names and a sub'))
        unless @a == $want;
    my @names;
    for my $i (0 .. $want - 2) {
        my $v = parse_expr($a[$i]{toks}, { ctx => 'fn', vars => {}, nonneg => {}, at => $a[$i]{node} });
        refuse($a[$i]{node}, "`$what` takes a name written out as a string") unless defined $v->{lit};
        push @names, $v->{pix};
    }
    my ($sub, @rest) = @{ $a[-1]{toks} };
    my ($proto, $block) = @rest == 2 ? @rest : (undef, $rest[0]);
    refuse($a[-1]{node}, "`$what` runs a sub: `$what(" . join('', map { '"…", ' } @names) . 'sub { ... })`')
        unless is_word($sub, 'sub') && $block && $block->isa('PPI::Structure::Block');
    my @params = sub_params($proto);
    my $text = $what eq 'on_key' || $what eq 'on_file_drop';
    refuse($proto // $sub, $text ? "`$what` hands over one string: `sub (\$s) { ... }`"
                                 : "`$what` runs a sub with nothing: `sub { ... }`")
        unless @params == ($text ? 1 : 0);
    my $env = { ctx => 'fn', vars => {}, nonneg => {}, at => $a[-1]{node} };
    $env->{vars}{ substr $params[0], 1 } = { ty => 'String', pix => substr($params[0], 1), fixed => 1 } if $text;
    push @binds, {
        what  => $what,
        names => \@names,
        param => $text ? substr($params[0], 1) : undef,
        body  => [stmts($block, $env)],
    };
}

# --- the run line ---------------------------------------------------------
sub run_line {
    my @args = split_args($run_list);
    my $first = @args ? join('', map { $_->content } @{ $args[0]{toks} }) : '';
    refuse($run_word, "`run` takes the app first: `run($class_name->new, ...)`")
        unless @args && !defined $args[0]{key}
            && ($first eq "$class_name->new" || (defined $app_var && $first eq "\$$app_var"));
    my %window;
    for my $a (@args[1 .. $#args]) {
        refuse($a->{node}, '`run` takes keyword arguments after the app') unless defined $a->{key};
        my $k = $a->{key};
        refuse($a->{node}, "`run` has no `$k =>`; it takes `title`, `width`, `height`, `padding`")
            unless $k =~ /\A(?:title|width|height|padding)\z/;
        my $v = parse_expr($a->{toks}, { ctx => 'view', vars => {}, nonneg => {}, at => $a->{node} });
        if ($k eq 'title') {
            refuse($a->{node}, '`title =>` takes a string literal') unless defined $v->{lit};
        } else {
            refuse($a->{node}, "`$k =>` takes a number") unless num_ty($v->{ty});
        }
        $window{$k} = $v->{pix};
    }
    refuse($run_word, '`width` and `height` go together') if defined $window{width} xor defined $window{height};
    return \%window;
}

# --- emit -----------------------------------------------------------------
my @out;

sub fn_head {
    my ($m, $name) = @_;
    my $p = @{ $m->{params} } ? '(' . join(', ', map { ($_->[0] =~ s/\A[\$\@%]//r) . ": $_->[1]" }
                                                @{ $m->{params} }) . ')' : '';
    my $r = defined $m->{ret} ? " $m->{ret}" : '';
    return ($m->{async} ? 'async ' : '') . 'fn ' . ($name // $m->{name}) . $p . $r;
}

sub emit {
    my ($tree) = @_;
    @out = ();
    for my $name (@values) {
        push @out, "struct $name {";
        push @out, "  var $_->{name} : $_->{ty} = $_->{init}" for @{ $value{$name} };
        push @out, '}', '';
    }
    for my $name (@models) {
        my $m = $model{$name};
        push @out, "class $name {";
        push @out, '  pub ' . ($_->{weak} ? 'weak ' : '') . "prop $_->{name} : $_->{ty}, default: $_->{init}" for @{ $m->{fields} };
        for my $mm (@{ $m->{order} }) {
            push @out, '', '  pub ' . fn_head($mm) . ' {', (map { "    $_" } @{ $mm->{body} }), '  }';
        }
        push @out, '}', '';
    }
    push @out, 'store App {';
    push @out, "  state $_ : $field{$_}{ty} = $field{$_}{init}" for @fields;
    for my $m (@methods) {
        next if $m->{element} || $m->{paints} || !$m->{stateful};
        push @out, '', '  ' . fn_head($m) . ' {', (map { "    $_" } @{ $m->{body} }), '  }';
    }
    for my $h (@handlers) {
        my $params = @{ $h->{params} } ? '(' . join(', ', map { "$_->[0]: $_->[1]" } @{ $h->{params} }) . ')' : '';
        push @out, '', '  ' . ($h->{async} ? 'async ' : '') . "fn $h->{id}$params {",
                   (map { "    $_" } @{ $h->{body} }), '  }';
    }
    for my $i (0 .. $#timers) {
        push @out, '', "  fn __tick$i \@every($timers[$i]{ms}) {", (map { "    $_" } @{ $timers[$i]{body} }), '  }';
    }
    my %n;
    for my $b (@binds) {
        my $i = $n{ $b->{what} }++;
        my $head = $b->{what} eq 'shortcut'  ? "fn __key$i \@key(" . join(', ', @{ $b->{names} }) . ')'
                 : $b->{what} eq 'menu_item' ? "fn __menu$i \@menu(" . join(', ', @{ $b->{names} }) . ')'
                 : $b->{what} eq 'on_key'    ? "fn __anykey$i($b->{param}: String) \@key"
                 :                             "fn __drop$i($b->{param}: String) \@drop";
        push @out, '', "  $head {", (map { "    $_" } @{ $b->{body} }), '  }';
    }
    push @out, '}';
    my @help = (grep({ !$_->{element} && !$_->{paints} && !$_->{stateful} } @methods), @lifted);
    if (@help) {
        push @out, '', 'class Helpers {';
        for my $m (@help) {
            push @out, '  pub static ' . fn_head($m) . ' {', (map { "    $_" } @{ $m->{body} }), '  }';
        }
        push @out, '}';
    }
    for my $m (@methods) {
        next unless $m->{comp};
        my @p = grep { $_->[0] =~ /\A\$/ } @{ $m->{params} };
        my $params = @p ? '(' . join(', ', map { substr($_->[0], 1) . ": $_->[1]" } @p) . ')' : '';
        push @out, '', "view $m->{comp}{name}$params {";
        emit_nodes($m->{comp}{nodes}, 1);
        push @out, '}';
    }
    push @out, '', 'view Main {';
    emit_nodes([$tree], 1);
    push @out, '}';
    return join("\n", @out) . "\n";
}

sub emit_nodes {
    my ($nodes, $lvl) = @_;
    my $pad = '  ' x $lvl;
    for my $n (@$nodes) {
        if ($n->{n} eq 'el') { emit_element($n, $lvl); next }
        if ($n->{n} eq 'slot') { push @out, "${pad}Slot { }"; next }
        if ($n->{n} eq 'if') {
            push @out, "${pad}if $n->{cond} {";
            emit_nodes($n->{then}, $lvl + 1);
            if (@{ $n->{else} }) {
                push @out, "$pad} else {";
                emit_nodes($n->{else}, $lvl + 1);
            }
            push @out, "$pad}";
            next;
        }
        my $names = join ', ', grep { defined } $n->{it}, $n->{ix};
        push @out, "${pad}for $names in $n->{over} {";
        emit_nodes($n->{body}, $lvl + 1);
        push @out, "$pad}";
    }
}

sub emit_element {
    my ($el, $lvl) = @_;
    my $pad = '  ' x $lvl;
    my @props = map { "$_->[0]: $_->[1]" } @{ $el->{props} };
    if (!@{ $el->{kids} } && (@props <= 3 || $el->{flat})) {
        push @out, @props ? "$pad$el->{pix} { " . join('; ', @props) . ' }' : "$pad$el->{pix} { }";
        return;
    }
    push @out, "$pad$el->{pix} {", (map { "$pad  $_" } @props);
    emit_nodes($el->{kids}, $lvl + 1);
    push @out, "$pad}";
}

1;
