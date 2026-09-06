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
# handler's parameter type from the element it is written on, and
# everything else is checked against those.
#
# PPI does not know `class`, `field` or `method`. Their tokens still
# come through, in three shapes this file allows for: a `class` block
# and whatever follows it up to the next `;` arrive as one statement;
# a method whose name is a quote-like operator (`m`, `s`, `y`, `tr`,
# `q`, `qq`, `qw`, `qr`) is read as a regular expression, so those
# names are refused; and a run of field attributes is mis-tokenized,
# which does not matter yet because attributes are refused.
use strict;
use warnings;
use utf8;
use PPI;
use Rakugan::Vocab;

my %ROLES = map { $_ => 1 } qw(button label heading textInput image list listItem table dialog
                               progress slider group checkbox switch comboBox radioGroup tabList);
my %EASINGS = map { $_ => 1 } qw(linear in out inOut);
my %TYPE_WORD = (Int => 'Int', Str => 'String', Num => 'Float', Bool => 'Bool');
my %ELEMENT = %Rakugan::Vocab::ELEMENT;
my %RIDER = %Rakugan::Vocab::RIDER;
my @RIDERS = @Rakugan::Vocab::RIDERS;

# --- state for one translation ---------------------------------------------
my ($path, @lines);
my ($class_name, $class_block, $run_word, $run_list, $view_block);
my (@fields, %field);       # name => { ty, init }
my %bag;                    # a hash of keywords at the top of the file: name => [pieces]
my (@methods, %method);     # name => { block, body }
my @handlers;               # { id, params => [[name, ty]], body => [lines] }

# The one entry point: the .pix text and the window, or a refusal thrown
# as a string that already says everything.
sub translate {
    my ($file) = @_;
    ($path, $class_name, $class_block, $run_word, $run_list, $view_block) = ($file);
    (@fields, %field, %bag, @methods, %method, @handlers) = ();
    open my $fh, '<:encoding(UTF-8)', $path or die "$path: $!\n";
    my $src = do { local $/; <$fh> };
    close $fh;
    @lines = split /^/m, $src;
    my $doc = PPI::Document->new(\$src) or die PPI::Document->errstr . "\n";

    declarations($doc);
    class_body();
    $_->{body} = [stmts($_->{block}, { ctx => 'fn', params => {} })] for @methods;
    my $tree = view();
    my $window = run_line();
    return { pix => emit($tree), window => $window };
}

# The pixie.toml of the project a translation is built as.
sub project_toml {
    my ($stem, $window) = @_;
    my $toml = qq{[package]\nname = "$stem"\nversion = "0.1.0"\n};
    if (%$window) {
        $toml .= "\n[window]\n";
        $toml .= qq{title = $window->{title}\n} if defined $window->{title};
        $toml .= "width = $window->{width}\nheight = $window->{height}\n" if defined $window->{width};
        $toml .= "padding = $window->{padding}\n" if defined $window->{padding};
    }
    return $toml . "\n[crates]\n";
}

# --- refusals -------------------------------------------------------------
sub refuse {
    my ($node, $msg) = @_;
    my ($line, $col) = ($node->line_number, $node->column_number);
    my $src = $lines[$line - 1] // '';
    chomp $src;
    die sprintf "%s:%d:%d: Rakugan cannot take this — %s\n    %s\n    %s^\n",
        $path, $line, $col, $msg, $src, ' ' x ($col - 1);
}

sub sig { grep { $_->significant } $_[0]->children }
sub is_word { my ($t, $w) = @_; $t && $t->isa('PPI::Token::Word') && (!defined $w || $t->content eq $w) }
sub is_op   { my ($t, $o) = @_; $t && $t->isa('PPI::Token::Operator') && (!defined $o || $t->content eq $o) }
sub is_sym  { my ($t, $kind) = @_; $t && $t->isa('PPI::Token::Symbol') && (!defined $kind || $t->raw_type eq $kind) }
sub strip_semicolon { my @t = @_; pop @t if @t && $t[-1]->isa('PPI::Token::Structure') && $t[-1]->content eq ';'; @t }

# --- the file's declarations ---------------------------------------------
# `use` lines, hashes of keywords, one class, and the `run(...)` line.
# PPI reads `class Name { ... }` and whatever follows it up to the next
# `;` as one statement, so the tokens are walked in runs, and a run ends
# where a declaration's shape ends.
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
    my $pragma;
    for my $st ($doc->schildren) {
        next if $st->isa('PPI::Statement::End') || $st->isa('PPI::Statement::Null');
        if ($st->isa('PPI::Statement::Include')) {
            $pragma = $st if ($st->module // '') eq 'Rakugan';
            next;
        }
        if ($st->isa('PPI::Statement::Variable')) {
            bag($st);
            next;
        }
        my @t = strip_semicolon(sig($st));
        while (@t) {
            if (is_word($t[0], 'class')) {
                refuse($t[0], 'one class per app in this translator') if defined $class_name;
                refuse($t[0], 'a class is written `class Name { ... }`')
                    unless @t >= 3 && is_word($t[1]) && $t[2]->isa('PPI::Structure::Block');
                ($class_name, $class_block) = ($t[1]->content, $t[2]);
                splice @t, 0, 3;
            } elsif (is_word($t[0], 'run')) {
                refuse($t[0], '`run` is called with parentheses: `run(Counter->new, title => "...")`')
                    unless @t >= 2 && $t[1]->isa('PPI::Structure::List');
                ($run_word, $run_list) = @t[0, 1];
                splice @t, 0, 2;
            } else {
                refuse($t[0], 'a statement at the top of the file — the compiled app reads the declarations '
                            . '(`use`, a hash of keywords, `class`, `run`) and never executes the file');
            }
        }
    }
    die "$path: no `class` found — an app is a class with a `view` method, handed to `run`\n" unless defined $class_name;
    die "$path: no `run(...)` found — the last line hands the app to `run`\n" unless defined $run_word;
    die "$path: the file starts with `use Rakugan;` — it turns on what the dialect assumes and brings `run`\n" unless $pragma;
}

# `my %PILL = (size => 11, color => "#11111b");` — a bag of keywords an
# app writes once and hands to elements with `%PILL`. Another bag inside
# the list is merged in place, so `(%PILL, background => "#2fa84f")` is
# the pill with a background.
sub bag {
    my ($st) = @_;
    my @t = strip_semicolon(sig($st));
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
    $bag{$name} = \@pieces;
}

# --- fields and methods --------------------------------------------------
sub class_body {
    my $pragma;
    for my $st ($class_block->schildren) {
        next if $st->isa('PPI::Statement::Null');
        if ($st->isa('PPI::Statement::Include')) {
            $pragma = 1 if ($st->module // '') eq 'Rakugan';
            next;
        }
        my @t = strip_semicolon(sig($st));
        next unless @t;
        # A `method` block and whatever follows it up to the next `;`
        # arrive as one statement, like a `class` block does.
        while (@t) {
            if (is_word($t[0], 'field')) {
                field(@t);
                @t = ();
            } elsif (is_word($t[0], 'method')) {
                my $name = $t[1];
                refuse($t[0], 'a method is written `method name { ... }`') unless is_word($name);
                my $block = $t[2];
                refuse($name, 'a method with parameters comes with `:Sig`; not in the translator yet')
                    if $block && $block->isa('PPI::Structure::List');
                refuse($name, 'a method needs a block') unless $block && $block->isa('PPI::Structure::Block');
                if ($name->content eq 'view') {
                    refuse($name, '`view` is declared twice') if $view_block;
                    $view_block = $block;
                } else {
                    refuse($name, '`' . $name->content . '` is declared twice') if exists $method{ $name->content };
                    push @methods, $method{ $name->content } = { name => $name->content, block => $block, node => $name };
                }
                splice @t, 0, 3;
            } else {
                refuse($t[0], 'inside the class: `use Rakugan;`, `field` and `method` only');
            }
        }
    }
    refuse($class_block, "$class_name has no `method view`") unless $view_block;
    refuse($class_block, "`class $class_name` needs `use Rakugan;` as its first line: a Perl import is per "
                       . 'package, and the elements, `empty` and the type names have to be in this one')
        unless $pragma;
}

sub field {
    my @t = @_;
    my $sym = $t[1];
    refuse($t[0], 'a field is written `field $name = <literal>;`') unless $sym && $sym->isa('PPI::Token::Symbol');
    my $name = substr $sym->content, 1;
    refuse($sym, "`$name` is declared twice") if exists $field{$name};
    refuse($sym, 'a hash field waits on the map shape; not in the translator yet') if $sym->raw_type eq '%';
    my $eq = $t[2];
    refuse($eq // $sym, 'a field takes no attributes here (`:param`, `:reader`); its type is read from the initializer')
        if $eq && !is_op($eq, '=');
    refuse($sym, 'a field needs an initializer (`= 0`, `= ""`, `= empty(Str)`) — that is where its type comes from')
        unless $eq && @t >= 4;
    my @init = @t[3 .. $#t];
    my ($ty, $pix);
    if ($sym->raw_type eq '@') {
        ($ty, $pix) = list_init(\@init, $sym);
    } else {
        refuse($init[0], 'a scalar field starts as one literal') if @init > 2 || (@init == 2 && !is_op($init[0], '-'));
        ($ty, $pix) = literal_run(\@init);
    }
    push @fields, $name;
    $field{$name} = { ty => $ty, init => $pix };
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
            my ($t, $p) = literal_run($a->{toks});
            refuse($a->{node}, "a list holds one type: this one started with $ty and this is $t") if defined $ty && $t ne $ty;
            $ty = $t;
            push @pix, $p;
        }
        return ("List<$ty>", '[' . join(', ', @pix) . ']');
    }
    refuse($sym, 'a list field starts as a literal list or as `empty(Str)`');
}

# `Str`, `Int`, `Num`, `Bool`, `ArrayRef[Int]` — a type written where perl
# reads it as a value.
sub type_of_words {
    my ($toks, $at) = @_;
    my ($w, $param) = @$toks;
    refuse($at, 'a type here is `Int`, `Str`, `Num`, `Bool` or `ArrayRef[...]`') unless is_word($w);
    if ($w->content eq 'ArrayRef') {
        refuse($w, '`ArrayRef` takes its element type in brackets: `ArrayRef[Int]`')
            unless $param && $param->isa('PPI::Structure::Constructor') && @$toks == 2;
        my @st = $param->schildren;
        refuse($param, '`ArrayRef` takes one type') unless @st == 1;
        return 'List<' . type_of_words([sig($st[0])], $param) . '>';
    }
    refuse($w, '`HashRef` waits on the map shape; not in the translator yet') if $w->content eq 'HashRef';
    refuse($w, 'a type here is `Int`, `Str`, `Num`, `Bool` or `ArrayRef[...]`; `' . $w->content . '` is not one')
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
        refuse($tok, 'a number literal here is a plain decimal') unless $s =~ /\A\d+(\.\d+)?\z/;
        return $s =~ /\./ ? ('Float', $s) : ('Int', $s);
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
# read: in the view (a field reads as `App.name`) or in a handler (bare),
# and which parameters are in scope. Answers { ty, pix, lit? } — `lit`
# is the text of a string literal, kept so two of them can be joined.
my %BP = ('or' => 1, '||' => 1, 'and' => 2, '&&' => 2,
          '==' => 5, '!=' => 5, '<' => 5, '>' => 5, '<=' => 5, '>=' => 5, 'eq' => 5, 'ne' => 5,
          '+' => 10, '-' => 10, '.' => 10, '*' => 20, '/' => 20);

sub parse_expr {
    my ($toks, $env) = @_;
    refuse($env->{at}, 'an expression is missing here') unless @$toks;
    my $i = 0;
    my $r = expr_bp(\$i, $toks, $env, 0);
    refuse($toks->[$i], 'this does not continue the expression: `' . $toks->[$i]->content . '`') if $i < @$toks;
    return $r;
}

sub op_of {
    my ($t) = @_;
    return $t->content if is_op($t) && exists $BP{ $t->content };
    return $t->content if is_word($t) && $t->content =~ /\A(?:or|and|eq|ne)\z/;
    return undef;
}

sub expr_bp {
    my ($ip, $toks, $env, $min) = @_;
    my $lhs = primary($ip, $toks, $env);
    while ($$ip < @$toks) {
        my $op = $toks->[$$ip];
        my $o = op_of($op);
        last unless defined $o;
        my $bp = $BP{$o};
        last if $bp < $min;
        $$ip++;
        my $rhs = expr_bp($ip, $toks, $env, $bp + 1);
        $lhs = binop($op, $o, $lhs, $rhs);
        # a chain of comparisons (`a < b < c`) is Perl's own mistake; refuse it
        if ($bp == 5 && $$ip < @$toks) {
            my $next = op_of($toks->[$$ip]);
            refuse($toks->[$$ip], 'a comparison does not chain; write `a < b && b < c`') if defined $next && $BP{$next} == 5;
        }
    }
    return $lhs;
}

sub num_ty { my ($t) = @_; $t eq 'Int' || $t eq 'Float' }

sub binop {
    my ($node, $o, $l, $r) = @_;
    if ($o eq '.') {
        refuse($node, 'joining strings with `.` is in the translator only for two literals so far; a run of literals across lines is fine')
            unless defined $l->{lit} && defined $r->{lit};
        my $lit = $l->{lit} . $r->{lit};
        return { ty => 'String', pix => '"' . pix_text($node, $lit) . '"', lit => $lit };
    }
    if ($o =~ /\A(?:\+|-|\*|\/)\z/) {
        refuse($node, "`$o` needs a number on both sides (got $l->{ty} and $r->{ty})") unless num_ty($l->{ty}) && num_ty($r->{ty});
        refuse($node, "Perl's `/` answers a Num even for whole numbers; not in this translator yet") if $o eq '/';
        my $ty = ($l->{ty} eq 'Float' || $r->{ty} eq 'Float') ? 'Float' : 'Int';
        return { ty => $ty, pix => group($l->{pix}) . " $o " . group($r->{pix}) };
    }
    if ($o eq 'eq' || $o eq 'ne') {
        refuse($node, "`$o` compares strings (got $l->{ty} and $r->{ty})") unless $l->{ty} eq 'String' && $r->{ty} eq 'String';
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

sub group { my ($s) = @_; return $s =~ / / && $s !~ /\A\(.*\)\z/ && $s !~ /\A"/ ? "($s)" : $s }

sub primary {
    my ($ip, $toks, $env) = @_;
    my $t = $toks->[$$ip];
    refuse($toks->[-1], 'the expression ends early') unless $t;
    if (is_op($t, '-')) {
        $$ip++;
        my $n = $toks->[$$ip];
        refuse($t, 'a minus here needs a number after it') unless $n && $n->isa('PPI::Token::Number');
        $$ip++;
        my ($ty, $pix) = literal($n);
        return { ty => $ty, pix => "-$pix" };
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
        return { ty => $ty, pix => $pix, ($ty eq 'String' ? (lit => $t->literal) : ()) };
    }
    if ($t->isa('PPI::Token::Quote::Double')) { $$ip++; return interpolate($t, $env) }
    if (is_word($t, 'true') || is_word($t, 'false')) { $$ip++; return { ty => 'Bool', pix => $t->content } }
    if ($t->isa('PPI::Token::Symbol')) {
        $$ip++;
        refuse($t, 'only a scalar is read here') unless $t->raw_type eq '$';
        my $name = substr $t->content, 1;
        refuse($t, '`$self` is read for its methods (`$self->name`) and nothing else') if $name eq 'self';
        return read_var($t, $name, $env);
    }
    if ($t->isa('PPI::Structure::List')) {
        $$ip++;
        my @st = $t->schildren;
        refuse($t, 'one expression inside these parentheses') unless @st == 1;
        my $v = parse_expr([sig($st[0])], { %$env, at => $t });
        return { %$v, pix => "($v->{pix})" };
    }
    refuse($t, 'this is not an expression the translator takes: `' . $t->content . '`');
}

sub read_var {
    my ($node, $name, $env) = @_;
    return { ty => $env->{params}{$name}, pix => $name } if exists $env->{params}{$name};
    if (exists $field{$name}) {
        return { ty => $field{$name}{ty}, pix => $env->{ctx} eq 'view' ? "App.$name" : $name };
    }
    refuse($node, "`\$$name` is not a field of $class_name" . ($env->{ctx} eq 'fn' ? " nor this handler's parameter" : ''));
}

# "count: $count" — literal text and holes, each hole a field or a parameter.
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
        if ($s =~ s/\A\$\{(\w+)\}// || $s =~ s/\A\$(\w+)//) {
            my $name = $1;
            refuse($tok, "`\$$name\[` or `\$$name\{` in a string is an element read; not in the translator") if $s =~ /\A[\[{]/;
            my $v = $name eq 'self' ? refuse($tok, '`$self` has no text') : read_var($tok, $name, $env);
            refuse($tok, "`\$$name` is a Float, and Perl prints one with %.15g, which the compiled run does not do yet") if $v->{ty} eq 'Float';
            refuse($tok, "`\$$name` is a Bool, and Perl prints one as `1` or nothing, which the compiled run does not do yet") if $v->{ty} eq 'Bool';
            refuse($tok, "`\$$name` is a list; a list has no text") if $v->{ty} =~ /^List/;
            $out .= pix_text($tok, $buf) . '#{' . $v->{pix} . '}';
            $lit .= $buf;
            $buf = '';
            $plain = 0;
            next;
        }
        refuse($tok, 'an array does not interpolate here; write `\@` for a literal at sign') if $s =~ /\A\@\w/;
        $s =~ s/\A(.)//s;
        $buf .= $1;
    }
    $out .= pix_text($tok, $buf);
    $lit .= $buf;
    return { ty => 'String', pix => qq{"$out"}, ($plain ? (lit => $lit) : ()) };
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
sub parse_element {
    my ($toks, $at) = @_;
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
        if (!defined $a->{key} && @{ $a->{toks} } == 1 && is_sym($a->{toks}[0], '%')) {
            my $bname = substr $a->{toks}[0]->content, 1;
            refuse($a->{node}, "`%$bname` is not a hash of keywords declared at the top of the file") unless exists $bag{$bname};
            push @args, @{ $bag{$bname} };
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
            } elsif ($RIDER{$k}) {
                refuse($a->{node}, "`$name`'s own `label` is already the name a screen reader reads; there is no second one to give")
                    if $spec->{owns_label} && $k eq 'a11y_label';
                $given{$k} = $a;
            } else {
                my @takes = sort +(map { $_->{name} } grep { !$_->{pos} } @{ $spec->{props} }), (map { $_->{name} } @RIDERS);
                refuse($a->{node}, "`$name` has no `$k =>`; it takes " . join(', ', map { "`$_`" } @takes));
            }
            next;
        }
        if (@positional) {
            my $p = shift @positional;
            $given{ $p->{name} } = $a;
            next;
        }
        refuse($a->{node}, "`$name` takes no children") unless $spec->{children};
        push @children, parse_element($a->{toks}, $a->{node});
    }
    for my $p (@positional) {
        refuse($w, "`$name` needs its $p->{name}") unless exists $p->{default};
    }

    my @props;
    for my $r (@RIDERS) {
        next if $own{ $r->{name} } || !exists $given{ $r->{name} };
        push @props, [$r->{pix}, rider_value($r, $given{ $r->{name} })];
    }
    for my $p (@{ $spec->{props} }) {
        next unless exists $given{ $p->{name} };
        my $a = $given{ $p->{name} };
        if (defined $p->{handler}) {
            push @props, [$p->{pix}, parse_handler($a->{toks}, $p, $a->{node})];
        } else {
            push @props, [$p->{pix}, prop_value($name, $p, $a)];
        }
    }
    return { pix => $spec->{pix}, props => \@props, children => \@children };
}

# The value of an element's own keyword, checked against its type.
sub prop_value {
    my ($name, $p, $a) = @_;
    my $env = { ctx => 'view', params => {}, at => $a->{node} };
    my $t = $p->{type};
    if ($t eq 'rows') {
        refuse($a->{node}, "a list that builds its rows on demand comes with the todo demo; not in the translator yet");
    }
    if ($t eq 'strs' || $t eq 'nums' || $t eq 'nums2') {
        my ($bs, $sym, @more) = @{ $a->{toks} };
        refuse($a->{node}, "`$p->{name} =>` takes a list field the view can re-read: `\\\@items`, with `field \@items = ...` on the class")
            unless $bs && $bs->isa('PPI::Token::Cast') && $bs->content eq '\\' && is_sym($sym, '@') && !@more;
        my $fname = substr $sym->content, 1;
        refuse($sym, "`\@$fname` is not a field of $class_name") unless exists $field{$fname};
        my $want = $t eq 'strs' ? 'List<String>' : $t eq 'nums' ? 'List<Float>' : 'List<List<Float>>';
        my $have = $field{$fname}{ty};
        refuse($sym, "`$p->{name} =>` takes a $want; `\@$fname` holds a $have")
            unless $have eq $want || ($t eq 'nums' && $have eq 'List<Int>') || ($t eq 'nums2' && $have eq 'List<List<Int>>');
        return "App.$fname";
    }
    my $v = parse_expr($a->{toks}, $env);
    if ($t eq 'str')  { refuse($a->{node}, "`$p->{name} =>` takes a string (got $v->{ty})") unless $v->{ty} eq 'String'; return $v->{pix} }
    if ($t eq 'num')  { refuse($a->{node}, "`$p->{name} =>` takes a number (got $v->{ty})") unless num_ty($v->{ty}); return $v->{pix} }
    if ($t eq 'int')  { refuse($a->{node}, "`$p->{name} =>` takes a whole number (got $v->{ty})") unless $v->{ty} eq 'Int'; return $v->{pix} }
    if ($t eq 'bool') { return bool_value($p->{name}, $v, $a->{node}) }
    refuse($a->{node}, "`$p->{name} =>` has a type the translator does not know: $t");
}

sub bool_value {
    my ($what, $v, $node) = @_;
    return 'true'  if $v->{ty} eq 'Int' && $v->{pix} eq '1';
    return 'false' if $v->{ty} eq 'Int' && $v->{pix} eq '0';
    refuse($node, "`$what =>` takes `true` or `false`, or a bool field (got $v->{ty})") unless $v->{ty} eq 'Bool';
    return $v->{pix};
}

# The value of a keyword every element takes.
sub rider_value {
    my ($r, $a) = @_;
    my $env = { ctx => 'view', params => {}, at => $a->{node} };
    my $v = parse_expr($a->{toks}, $env);
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
    if (defined $v->{lit}) {
        refuse($a->{node}, "`easing =>` is one of " . join(', ', map { "\"$_\"" } sort keys %EASINGS)) if $k eq 'easing' && !$EASINGS{ $v->{lit} };
        refuse($a->{node}, "unknown role `$v->{lit}`; one of " . join(', ', sort keys %ROLES)) if $k eq 'role' && !$ROLES{ $v->{lit} };
        refuse($a->{node}, '`theme =>` is "light", "dark" or a str field') if $k eq 'theme' && $v->{lit} !~ /\A(?:light|dark)\z/;
    }
    return $v->{pix};
}

# --- handlers and statements -----------------------------------------------

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
    my ($toks, $p, $at) = @_;
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
    my %penv = map { substr($_, 1) => $pty } @params;
    my @st = $block->schildren;
    refuse($block, 'an empty handler') unless @st;
    # `sub { $self->flip }`: the method itself is the handler.
    if (@st == 1 && $kind eq 'none') {
        my @t = strip_semicolon(sig($st[0]));
        if (@t == 3 && is_sym($t[0], '$') && $t[0]->content eq '$self' && is_op($t[1], '->') && is_word($t[2])) {
            my $m = $t[2]->content;
            refuse($t[2], "`$m` is not a method of $class_name") unless exists $method{$m};
            return "App.$m()";
        }
    }
    my $id = 'h' . scalar @handlers;
    my @body = stmts($block, { ctx => 'fn', params => \%penv });
    push @handlers, { id => $id, params => [map { [substr($_, 1), $pty] } @params], body => \@body };
    return "App.$id($payload)";
}

# The statements of a handler or a method, as .pix lines.
sub stmts {
    my ($block, $env) = @_;
    my @out;
    for my $st ($block->schildren) {
        next if $st->isa('PPI::Statement::Null');
        push @out, stmt($st, $env);
    }
    return @out;
}

sub stmt {
    my ($st, $env) = @_;
    if ($st->isa('PPI::Statement::Compound')) { return compound($st, $env) }
    refuse($st, 'a `my` inside a handler or a method comes later; not in the translator yet') if $st->isa('PPI::Statement::Variable');
    my @t = strip_semicolon(sig($st));
    refuse($st, 'an empty statement') unless @t;
    # a trailing `if`/`unless`: `$x = 1 if $c;`
    for my $i (1 .. $#t) {
        if (is_word($t[$i], 'if') || is_word($t[$i], 'unless')) {
            my $cond = cond_of([@t[$i + 1 .. $#t]], $env, $t[$i]);
            $cond = "!($cond)" if $t[$i]->content eq 'unless';
            my @inner = simple_stmt([@t[0 .. $i - 1]], $env, $st);
            return ("if $cond {", (map { "  $_" } @inner), '}');
        }
    }
    return simple_stmt(\@t, $env, $st);
}

# `$field = expr`, `$field += expr`, `$self->method`.
sub simple_stmt {
    my ($toks, $env, $st) = @_;
    my @t = @$toks;
    my ($lhs, $op, @rhs) = @t;
    if (is_sym($lhs, '$') && $lhs->content eq '$self') {
        refuse($lhs, 'a method of the app is called as `$self->name`') unless is_op($op, '->') && is_word($rhs[0]);
        my $m = $rhs[0]->content;
        refuse($rhs[0], "`$m` is not a method of $class_name") unless exists $method{$m};
        refuse($rhs[1], 'a method with arguments comes with `:Sig`; not in the translator yet')
            if @rhs > 1 && !($rhs[1]->isa('PPI::Structure::List') && !$rhs[1]->schildren && @rhs == 2);
        return "App.$m()";
    }
    refuse($t[0] // $st, 'a statement here writes a field (`$count += 1`, `$name = $s`) or calls a method (`$self->flip`)')
        unless is_sym($lhs, '$') && is_op($op) && $op->content =~ /\A(?:=|\+=|-=|\*=)\z/ && @rhs;
    my $name = substr $lhs->content, 1;
    refuse($lhs, "`\$$name` is not a field; a handler writes the app's fields") unless $field{$name};
    my $fty = $field{$name}{ty};
    # `$x = c ? a : b` lowers to an if/else, each branch writing the field.
    my ($q) = grep { is_op($rhs[$_], '?') } 0 .. $#rhs;
    if (defined $q) {
        refuse($op, 'a conditional expression stands on the right of a plain `=`') unless $op->content eq '=';
        my ($c) = grep { is_op($rhs[$_], ':') && $_ > $q } 0 .. $#rhs;
        refuse($rhs[$q], 'a `?` needs its `:`') unless defined $c;
        my $cond = cond_of([@rhs[0 .. $q - 1]], $env, $rhs[$q]);
        my $a = typed_rhs($name, $fty, [@rhs[$q + 1 .. $c - 1]], $env, $rhs[$q]);
        my $b = typed_rhs($name, $fty, [@rhs[$c + 1 .. $#rhs]], $env, $rhs[$c]);
        return ("if $cond {", "  $name = $a", '} else {', "  $name = $b", '}');
    }
    if ($op->content eq '=') {
        return "$name = " . typed_rhs($name, $fty, \@rhs, $env, $op);
    }
    my $v = parse_expr(\@rhs, { %$env, at => $rhs[0] });
    my $o = substr $op->content, 0, 1;
    refuse($op, "`$o=` needs numbers: `\$$name` holds a $fty and this is a $v->{ty}") unless num_ty($fty) && num_ty($v->{ty});
    refuse($op, "`\$$name` holds an Int and `$o=` a Float would make it a Float") if $fty eq 'Int' && $v->{ty} eq 'Float';
    return "$name = $name $o " . group($v->{pix});
}

sub typed_rhs {
    my ($name, $fty, $toks, $env, $at) = @_;
    my $v = parse_expr($toks, { %$env, at => $at });
    refuse($at, "`\$$name` holds a $fty, and this is a $v->{ty}") unless $v->{ty} eq $fty || ($fty eq 'Float' && $v->{ty} eq 'Int');
    return $v->{pix};
}

sub cond_of {
    my ($toks, $env, $at) = @_;
    my @t = @$toks;
    # `if ($c)` — the parentheses are the statement's, not the expression's
    @t = sig(($t[0]->schildren)[0]) if @t == 1 && $t[0]->isa('PPI::Structure::Condition');
    my $v = parse_expr(\@t, { %$env, at => $at });
    refuse($at, "a condition is a bool (got $v->{ty}); Perl's truthiness of a number or a string is not in the translator — compare it (`!= 0`, `ne \"\"`)")
        unless $v->{ty} eq 'Bool';
    return $v->{pix};
}

# if / elsif / else, and unless.
sub compound {
    my ($st, $env) = @_;
    my @t = sig($st);
    refuse($t[0], 'a loop inside a handler or a method comes later; not in the translator yet')
        unless is_word($t[0], 'if') || is_word($t[0], 'unless');
    my @out;
    my $depth = 0;
    my $i = 0;
    while ($i < @t) {
        my $kw = $t[$i];
        if (is_word($kw, 'if') || is_word($kw, 'unless') || is_word($kw, 'elsif')) {
            my ($cnd, $blk) = @t[$i + 1, $i + 2];
            refuse($kw, "`" . $kw->content . "` takes its condition in parentheses and a block") unless $cnd && $cnd->isa('PPI::Structure::Condition') && $blk && $blk->isa('PPI::Structure::Block');
            my $c = cond_of([$cnd], $env, $kw);
            $c = "!($c)" if $kw->content eq 'unless';
            if ($kw->content eq 'elsif') {
                push @out, '} else {';
                push @out, "  if $c {";
                $depth++;
            } else {
                push @out, "if $c {";
            }
            push @out, map { ('  ' x ($depth + 1)) . $_ } stmts($blk, $env);
            $i += 3;
        } elsif (is_word($kw, 'else')) {
            my $blk = $t[$i + 1];
            refuse($kw, '`else` takes a block') unless $blk && $blk->isa('PPI::Structure::Block');
            push @out, ('  ' x $depth) . '} else {';
            push @out, map { ('  ' x ($depth + 1)) . $_ } stmts($blk, $env);
            $i += 2;
        } else {
            refuse($kw, 'this does not belong in an if statement: `' . $kw->content . '`');
        }
    }
    push @out, ('  ' x $_) . '}' for reverse 0 .. $depth;
    return @out;
}

# --- the view and the run line -------------------------------------------
sub view {
    my @st = $view_block->schildren;
    refuse($view_block, '`view` answers one element: one statement in its body') unless @st == 1;
    my @t = strip_semicolon(sig($st[0]));
    shift @t if @t && is_word($t[0], 'return');
    return parse_element(\@t, $st[0]);
}

sub run_line {
    my @args = split_args($run_list);
    refuse($run_word, "`run` takes the app first: `run($class_name->new, ...)`")
        unless @args && !defined $args[0]{key}
            && join('', map { $_->content } @{ $args[0]{toks} }) eq "$class_name->new";
    my %window;
    for my $a (@args[1 .. $#args]) {
        refuse($a->{node}, '`run` takes keyword arguments after the app') unless defined $a->{key};
        my $k = $a->{key};
        refuse($a->{node}, "`run` has no `$k =>`; it takes `title`, `width`, `height`, `padding`")
            unless $k =~ /\A(?:title|width|height|padding)\z/;
        my $v = parse_expr($a->{toks}, { ctx => 'view', params => {}, at => $a->{node} });
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

sub emit {
    my ($tree) = @_;
    @out = ('store App {');
    push @out, "  state $_ : $field{$_}{ty} = $field{$_}{init}" for @fields;
    for my $m (@methods) {
        push @out, '', "  fn $m->{name} {", (map { "    $_" } @{ $m->{body} }), '  }';
    }
    for my $h (@handlers) {
        my $params = @{ $h->{params} } ? '(' . join(', ', map { "$_->[0]: $_->[1]" } @{ $h->{params} }) . ')' : '';
        push @out, '', "  fn $h->{id}$params {", (map { "    $_" } @{ $h->{body} }), '  }';
    }
    push @out, '}', '', 'view Main {';
    emit_element($tree, 1);
    push @out, '}';
    return join("\n", @out) . "\n";
}

sub emit_element {
    my ($el, $lvl) = @_;
    my $pad = '  ' x $lvl;
    my @props = map { "$_->[0]: $_->[1]" } @{ $el->{props} };
    if (!@{ $el->{children} } && @props <= 3) {
        push @out, @props ? "$pad$el->{pix} { " . join('; ', @props) . ' }' : "$pad$el->{pix} { }";
        return;
    }
    push @out, "$pad$el->{pix} {", (map { "$pad  $_" } @props);
    emit_element($_, $lvl + 1) for @{ $el->{children} };
    push @out, "$pad}";
}

1;
