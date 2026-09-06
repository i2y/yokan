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
# What it takes today is the counter's shape: one class, scalar fields
# with literal initializers, a `view` method, handlers as anonymous
# subs, five elements. A field's type is read from its initializer and a
# handler's parameter type from the element table; nothing else is
# written down, and everything else is checked against those two.
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

# --- the vocabulary -------------------------------------------------------
# The five elements: the .pix element, the positional argument's
# keyword, the keywords each takes and the .pix property each becomes,
# and for a handler keyword what the handler is called with.
my %ELEMENTS = (
    text => {
        pix => 'Text', pos => 'text',
        keys => { text => 'text', size => 'fontSize' },
    },
    button => {
        pix => 'Button', pos => 'label',
        keys => { label => 'text', on_click => 'onClick' },
        handlers => { on_click => 'none' },
    },
    text_field => {
        pix => 'TextField', pos => 'value',
        keys => { value => 'text', placeholder => 'placeholder', on_change => 'onTextChanged' },
        handlers => { on_change => 'text' },
    },
    column => { pix => 'Column', children => 1, keys => { spacing => 'spacing', padding => 'padding' } },
    row    => { pix => 'Row',    children => 1, keys => { spacing => 'spacing', padding => 'padding' } },
);
# What a handler of each kind is called with: its parameter's type, and
# the name pixie gives the payload at the call site (`App.h3(text)`).
my %PAYLOAD_TYPE = (text => 'String');
my %PAYLOAD_NAME = (text => 'text');
my %NUMBER_KEYS = map { $_ => 1 } qw(size spacing padding);
my %QUOTELIKE = map { $_ => 1 } qw(m s y tr q qq qw qr);

# --- state for one translation ---------------------------------------------
my ($path, @lines);
my ($class_name, $class_block, $run_word, $run_list);
my (@fields, %field);
my @handlers;

# The one entry point: the .pix text and the window, or a refusal thrown
# as a string that already says everything.
sub translate {
    my ($file) = @_;
    ($path, $class_name, $class_block, $run_word, $run_list) = ($file);
    (@fields, %field, @handlers) = ();
    open my $fh, '<:encoding(UTF-8)', $path or die "$path: $!\n";
    my $src = do { local $/; <$fh> };
    close $fh;
    @lines = split /^/m, $src;
    my $doc = PPI::Document->new(\$src) or die PPI::Document->errstr . "\n";

    declarations($doc);
    class_body();
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

# --- the file's declarations ---------------------------------------------
# `use` lines, one class, and the `run(...)` line. PPI reads `class Name
# { ... }` and whatever follows it up to the next `;` as one statement,
# so the tokens are walked in runs, and a run ends where a declaration's
# shape ends.
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
        my @t = sig($st);
        pop @t if @t && $t[-1]->isa('PPI::Token::Structure') && $t[-1]->content eq ';';
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
                            . '(`use`, `class`, `run`) and never executes the file');
            }
        }
    }
    die "$path: no `class` found — an app is a class with a `view` method, handed to `run`\n" unless defined $class_name;
    die "$path: no `run(...)` found — the last line hands the app to `run`\n" unless defined $run_word;
    die "$path: the file starts with `use Rakugan;` — it turns on what the dialect assumes and brings `run`\n" unless $pragma;
}

# --- fields and methods --------------------------------------------------
sub class_body {
    my ($view_block, $pragma);
    for my $st ($class_block->schildren) {
        next if $st->isa('PPI::Statement::Null');
        if ($st->isa('PPI::Statement::Include')) {
            $pragma = 1 if ($st->module // '') eq 'Rakugan';
            next;
        }
        my @t = sig($st);
        next unless @t;
        pop @t if $t[-1]->isa('PPI::Token::Structure') && $t[-1]->content eq ';';
        if (is_word($t[0], 'field')) {
            field(@t);
        } elsif (is_word($t[0], 'method')) {
            my $name = $t[1];
            if ($name && $name->isa('PPI::Token::Regexp') && $name->content =~ /\A(m|s|y|tr|q|qq|qw|qr)\b/) {
                # `method m { ... }`: PPI read the name as a quote-like operator.
                refuse($name, "a method named `$1` reads as a regular expression to the parser; pick another name");
            }
            refuse($t[0], 'a method is written `method name { ... }`') unless is_word($name);
            refuse($name, 'only `view` yet; an app method comes with the store shape') unless $name->content eq 'view';
            refuse($name, '`view` takes no parameters') if @t != 3;
            refuse($name, '`view` needs a block') unless $t[2]->isa('PPI::Structure::Block');
            $view_block = $t[2];
        } else {
            refuse($t[0], 'inside the class: `use Rakugan;`, `field` and `method view` only');
        }
    }
    refuse($class_block, "$class_name has no `method view`") unless $view_block;
    refuse($class_block, "`class $class_name` needs `use Rakugan;` as its first line: a Perl import is per "
                       . 'package, and the elements, `empty` and the type names have to be in this one')
        unless $pragma;
    $class_block->{__view} = $view_block;
}

sub field {
    my @t = @_;
    my $sym = $t[1];
    refuse($t[0], 'a field is written `field $name = <literal>;`') unless $sym && $sym->isa('PPI::Token::Symbol');
    my $name = substr $sym->content, 1;
    refuse($sym, 'only scalar fields yet; a list or a hash field is written `field @items = empty(Str);` '
               . 'and comes with the todo demo')
        unless $sym->raw_type eq '$';
    refuse($sym, "`\$$name` is declared twice") if exists $field{$name};
    my $eq = $t[2];
    refuse($eq // $sym, 'a field takes no attributes here (`:param`, `:reader`); its type is read from the initializer')
        if $eq && !is_op($eq, '=');
    refuse($sym, 'a field needs a literal initializer (`= 0`, `= ""`) — that is where its type comes from')
        unless $eq && @t == 4;
    my ($ty, $pix) = literal($t[3]);
    push @fields, $name;
    $field{$name} = { ty => $ty, init => $pix };
}

# A literal: a number or a string, with its type and its .pix spelling.
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
        refuse($tok, 'a field initializer is a plain string; there is nothing to interpolate yet') if $s =~ /(?<!\\)[\$\@]/;
        return ('String', '"' . pix_text($tok, unescape($tok, $s)) . '"');
    }
    refuse($tok, 'a number or a string literal was expected here');
}

# The escapes a double-quoted string may carry, resolved to characters.
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
# and which parameters are in scope. Answers [type, pix].
my %BP = ('+' => 10, '-' => 10, '*' => 20, '/' => 20);

sub parse_expr {
    my ($toks, $env) = @_;
    refuse($env->{at}, 'an expression is missing here') unless @$toks;
    my $i = 0;
    my $r = expr_bp(\$i, $toks, $env, 0);
    refuse($toks->[$i], 'this does not continue the expression: `' . $toks->[$i]->content . '`') if $i < @$toks;
    return $r;
}

sub expr_bp {
    my ($ip, $toks, $env, $min) = @_;
    my $lhs = primary($ip, $toks, $env);
    while ($$ip < @$toks) {
        my $op = $toks->[$$ip];
        last unless is_op($op) && exists $BP{$op->content};
        my $bp = $BP{$op->content};
        last if $bp < $min;
        $$ip++;
        my $rhs = expr_bp($ip, $toks, $env, $bp + 1);
        $lhs = binop($op, $lhs, $rhs);
    }
    return $lhs;
}

sub binop {
    my ($op, $l, $r) = @_;
    my $o = $op->content;
    refuse($op, "`$o` needs a number on both sides (got $l->[0] and $r->[0])")
        unless $l->[0] =~ /^(Int|Float)$/ && $r->[0] =~ /^(Int|Float)$/;
    refuse($op, "Perl's `/` answers a Num even for whole numbers; not in this translator yet") if $o eq '/';
    my $ty = ($l->[0] eq 'Float' || $r->[0] eq 'Float') ? 'Float' : 'Int';
    return [$ty, group($l->[1]) . " $o " . group($r->[1])];
}

sub group { my ($s) = @_; return $s =~ / / ? "($s)" : $s }

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
        return [$ty, "-$pix"];
    }
    if ($t->isa('PPI::Token::Number') || $t->isa('PPI::Token::Quote::Single')) {
        $$ip++;
        my ($ty, $pix) = literal($t);
        return [$ty, $pix];
    }
    if ($t->isa('PPI::Token::Quote::Double')) { $$ip++; return interpolate($t, $env) }
    if ($t->isa('PPI::Token::Symbol')) {
        $$ip++;
        refuse($t, 'only scalars are read here') unless $t->raw_type eq '$';
        return read_var($t, substr($t->content, 1), $env);
    }
    if ($t->isa('PPI::Structure::List')) {
        $$ip++;
        my @st = $t->schildren;
        refuse($t, 'one expression inside these parentheses') unless @st == 1;
        return parse_expr([sig($st[0])], { %$env, at => $t });
    }
    refuse($t, 'this is not an expression the translator takes: `' . $t->content . '`');
}

sub read_var {
    my ($node, $name, $env) = @_;
    return [$env->{params}{$name}, $name] if exists $env->{params}{$name};
    if (exists $field{$name}) {
        return [$field{$name}{ty}, $env->{ctx} eq 'view' ? "App.$name" : $name];
    }
    refuse($node, "`\$$name` is not a field of $class_name" . ($env->{ctx} eq 'fn' ? " nor this handler's parameter" : ''));
}

# "count: $count" — literal text and holes, each hole a field or a parameter.
sub interpolate {
    my ($tok, $env) = @_;
    my $s = $tok->string;
    my ($out, $buf) = ('', '');
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
            my $v = read_var($tok, $name, $env);
            refuse($tok, "`\$$name` is a Float, and Perl prints one with %.15g, which the compiled run does not do yet") if $v->[0] eq 'Float';
            $out .= pix_text($tok, $buf) . '#{' . $v->[1] . '}';
            $buf = '';
            next;
        }
        refuse($tok, 'an array does not interpolate here; write `\@` for a literal at sign') if $s =~ /\A\@\w/;
        $s =~ s/\A(.)//s;
        $buf .= $1;
    }
    $out .= pix_text($tok, $buf);
    return ['String', qq{"$out"}];
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

# --- elements and handlers ----------------------------------------------
sub parse_element {
    my ($toks, $at) = @_;
    my ($w, $list, @more) = @$toks;
    refuse($at, 'a view is made of elements: ' . join(', ', map { "$_(...)" } sort keys %ELEMENTS))
        unless is_word($w) && $ELEMENTS{$w->content};
    refuse($w, '`' . $w->content . '` is called with parentheses') unless $list && $list->isa('PPI::Structure::List') && !@more;
    my $name = $w->content;
    my $el = $ELEMENTS{$name};
    my (@props, @children, $positional);
    for my $a (split_args($list)) {
        my $env = { ctx => 'view', params => {}, at => $a->{node} };
        if (defined $a->{key}) {
            my $k = $a->{key};
            my @takes = sort grep { !defined $el->{pos} || $_ ne $el->{pos} } keys %{ $el->{keys} };
            refuse($a->{node}, "`$name` has no `$k =>`; it takes " . join(', ', map { "`$_`" } @takes))
                unless $el->{keys}{$k} && !(defined $el->{pos} && $k eq $el->{pos});
            my $prop = $el->{keys}{$k};
            if ($el->{handlers} && $el->{handlers}{$k}) {
                push @props, [$prop, parse_handler($a->{toks}, $el->{handlers}{$k}, $a->{node})];
            } elsif ($NUMBER_KEYS{$k}) {
                my $v = parse_expr($a->{toks}, $env);
                refuse($a->{node}, "`$k =>` takes a number") unless $v->[0] =~ /^(Int|Float)$/;
                push @props, [$prop, $v->[1]];
            } else {
                my $v = parse_expr($a->{toks}, $env);
                refuse($a->{node}, "`$k =>` takes a string") unless $v->[0] eq 'String';
                push @props, [$prop, $v->[1]];
            }
        } elsif ($el->{children}) {
            push @children, parse_element($a->{toks}, $a->{node});
        } else {
            refuse($a->{node}, "`$name` takes one positional argument, its $el->{pos}") if $positional++;
            my $v = parse_expr($a->{toks}, $env);
            refuse($a->{node}, "`$name`'s $el->{pos} is a string") unless $v->[0] eq 'String';
            unshift @props, [$el->{keys}{$el->{pos}}, $v->[1]];
        }
    }
    refuse($w, "`$name` needs its $el->{pos}") if defined $el->{pos} && !$positional;
    return { pix => $el->{pix}, props => \@props, children => \@children };
}

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
        refuse($node, 'a parameter here is a plain scalar (`$s`)')
            if grep { !($_->isa('PPI::Token::Symbol') && $_->raw_type eq '$') && !is_op($_, ',') } @t;
        return map { $_->content } grep { $_->isa('PPI::Token::Symbol') } @t;
    }
    refuse($node, 'a handler is `sub { ... }` or `sub ($x) { ... }`');
}

# `sub { ... }` or `sub ($x) { ... }` on a handler keyword: becomes a
# store fn, and the property reads `App.hN(...)`.
sub parse_handler {
    my ($toks, $kind, $at) = @_;
    my ($sub, @rest) = @$toks;
    refuse($at, 'a handler is an anonymous sub written here: `on_click => sub { ... }`')
        unless is_word($sub, 'sub');
    my ($proto, $block) = @rest == 2 ? @rest : (undef, $rest[0]);
    refuse($sub, 'a handler is `sub { ... }` or `sub ($x) { ... }`')
        unless $block && $block->isa('PPI::Structure::Block');
    my @params = sub_params($proto);
    my $pty = $PAYLOAD_TYPE{$kind};
    if ($kind eq 'none') {
        refuse($proto, 'this handler is called with nothing; drop the parameter') if @params;
    } else {
        refuse($proto // $sub, "this handler is called with one value, a $pty: write `sub (\$x) { ... }`") unless @params == 1;
    }
    my %penv = map { substr($_, 1) => $pty } @params;
    my $id = 'h' . scalar @handlers;
    my @body = map { handler_stmt($_, { ctx => 'fn', params => \%penv, at => $_ }) } $block->schildren;
    refuse($block, 'an empty handler') unless @body;
    push @handlers, { id => $id, params => [map { [substr($_, 1), $pty] } @params], body => \@body };
    return "App.$id(" . ($kind eq 'none' ? '' : $PAYLOAD_NAME{$kind}) . ')';
}

# One statement of a handler: a write to a field.
sub handler_stmt {
    my ($st, $env) = @_;
    my @t = sig($st);
    pop @t if @t && $t[-1]->isa('PPI::Token::Structure') && $t[-1]->content eq ';';
    my ($lhs, $op, @rhs) = @t;
    refuse($t[0] // $st, 'a handler statement here writes a field: `$count += 1`, `$name = $s`')
        unless $lhs && $lhs->isa('PPI::Token::Symbol') && is_op($op) && $op->content =~ /\A(=|\+=|-=|\*=)\z/ && @rhs;
    my $name = substr $lhs->content, 1;
    refuse($lhs, "`\$$name` is not a field; a handler writes the app's fields") unless $field{$name};
    my $fty = $field{$name}{ty};
    my $v = parse_expr(\@rhs, { %$env, at => $rhs[0] });
    if ($op->content eq '=') {
        refuse($op, "`\$$name` holds a $fty, and this is a $v->[0]") unless $v->[0] eq $fty;
        return "$name = $v->[1]";
    }
    my $o = substr $op->content, 0, 1;
    refuse($op, "`$o=` needs numbers: `\$$name` holds a $fty and this is a $v->[0]")
        unless $fty =~ /^(Int|Float)$/ && $v->[0] =~ /^(Int|Float)$/;
    refuse($op, "`\$$name` holds an Int and `$o=` a Float would make it a Float") if $fty eq 'Int' && $v->[0] eq 'Float';
    return "$name = $name $o " . group($v->[1]);
}

# --- the view and the run line -------------------------------------------
sub view {
    my $block = $class_block->{__view};
    my @st = $block->schildren;
    refuse($block, '`view` answers one element: one statement in its body') unless @st == 1;
    my @t = sig($st[0]);
    pop @t if @t && $t[-1]->isa('PPI::Token::Structure') && $t[-1]->content eq ';';
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
            unless $k =~ /^(title|width|height|padding)$/;
        my $v = parse_expr($a->{toks}, { ctx => 'view', params => {}, at => $a->{node} });
        if ($k eq 'title') {
            refuse($a->{node}, '`title =>` takes a string literal') unless $v->[0] eq 'String' && $v->[1] !~ /#\{/;
        } else {
            refuse($a->{node}, "`$k =>` takes a number") unless $v->[0] =~ /^(Int|Float)$/;
        }
        $window{$k} = $v->[1];
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
    if (!@{ $el->{children} } && @props <= 2) {
        push @out, "$pad$el->{pix} { " . join('; ', @props) . ' }';
        return;
    }
    push @out, "$pad$el->{pix} {", (map { "$pad  $_" } @props);
    emit_element($_, $lvl + 1) for @{ $el->{children} };
    push @out, "$pad}";
}

1;
