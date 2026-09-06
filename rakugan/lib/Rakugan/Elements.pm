package Rakugan::Elements;
# The elements an app writes its screen with. Five of them for now,
# written by hand from wakakusa/elements.toml; the generator that
# writes this file from that table comes with the rest of the
# vocabulary.
#
# An element opens with `el(kind)`, writes the properties a person
# actually wrote (a value equal to the table's default is not sent, as
# the generated Ruby does not send it), hands over its children, and
# closes with `end`, which answers the handle its parent consumes.
use v5.40;
use Carp qw(croak);
use Rakugan::Door;
use Rakugan::Runtime;

our @ELEMENTS = qw(text button text_field column row);

# crates/pixie-capi/src/vocab.rs — the numbers both sides count with.
use constant {
    KIND_TEXT => 1, KIND_BUTTON => 2, KIND_TEXT_FIELD => 3, KIND_COLUMN => 4, KIND_ROW => 5,
    K_WIDTH => 1, K_HEIGHT => 2,
    K_TEXT => 16, K_SIZE => 17, K_COLOR => 18, K_ALIGN => 19, K_GROW => 20,
    K_BOLD => 21, K_ITALIC => 22, K_MONO => 23, K_UNDERLINE => 24, K_WRAP => 25,
    K_MAX_LINES => 26, K_BACKGROUND => 27, K_PADDING => 28, K_BORDER_RADIUS => 29,
    K_BORDER_WIDTH => 30, K_BORDER_COLOR => 31, K_LABEL => 32, K_ON_CLICK => 33,
    K_HOVER_BACKGROUND => 34, K_ACTIVE_BACKGROUND => 35, K_BASIS => 36, K_VALUE => 37,
    K_PLACEHOLDER => 38, K_ON_CHANGE => 39, K_ON_SUBMIT => 40, K_MULTILINE => 41,
    K_ROWS => 42, K_SPACING => 43,
};

# Each element's own keywords, in the order elements.rb writes them:
# [name, writer, key, default].
my @BOX = (
    [spacing => num => K_SPACING, -1], [padding => num => K_PADDING, 0],
    [background => str => K_BACKGROUND, ''], [grow => num => K_GROW, 0],
    [border_radius => num => K_BORDER_RADIUS, 0], [border_width => num => K_BORDER_WIDTH, 0],
    [border_color => str => K_BORDER_COLOR, ''],
);
my %SPEC = (
    text => [
        [size => num => K_SIZE, 0], [color => str => K_COLOR, ''], [align => str => K_ALIGN, ''],
        [grow => num => K_GROW, 0], [bold => bool => K_BOLD, 0], [italic => bool => K_ITALIC, 0],
        [mono => bool => K_MONO, 0], [underline => bool => K_UNDERLINE, 0],
        [wrap => str => K_WRAP, ''], [max_lines => int => K_MAX_LINES, 0],
        [width => num => K_WIDTH, 0], [background => str => K_BACKGROUND, ''],
        [padding => num => K_PADDING, 0], [border_radius => num => K_BORDER_RADIUS, 0],
        [border_width => num => K_BORDER_WIDTH, 0], [border_color => str => K_BORDER_COLOR, ''],
    ],
    button => [
        [on_click => on_none => K_ON_CLICK], [width => num => K_WIDTH, 0],
        [height => num => K_HEIGHT, 0], [size => num => K_SIZE, 0],
        [background => str => K_BACKGROUND, ''], [grow => num => K_GROW, 0],
        [color => str => K_COLOR, ''], [hover_background => str => K_HOVER_BACKGROUND, ''],
        [active_background => str => K_ACTIVE_BACKGROUND, ''],
        [border_radius => num => K_BORDER_RADIUS, 0], [border_width => num => K_BORDER_WIDTH, 0],
        [border_color => str => K_BORDER_COLOR, ''], [basis => num => K_BASIS, 0],
    ],
    text_field => [
        [placeholder => str => K_PLACEHOLDER, ''], [on_change => on_text => K_ON_CHANGE],
        [on_submit => on_text => K_ON_SUBMIT], [multiline => bool => K_MULTILINE, 0],
        [rows => num => K_ROWS, 0],
    ],
    column => \@BOX,
    row    => \@BOX,
);

my %WRITE = (
    num     => sub ($el, $key, $v) { Rakugan::Door::num($el, $key, $v) },
    str     => sub ($el, $key, $v) { Rakugan::Door::str($el, $key, $v) },
    bool    => sub ($el, $key, $v) { Rakugan::Door::bool($el, $key, $v ? 1 : 0) },
    int     => sub ($el, $key, $v) { Rakugan::Door::int($el, $key, $v) },
    on_none => \&Rakugan::Runtime::register,
    on_text => \&Rakugan::Runtime::register,
);

sub _differs ($writer, $v, $default) {
    return defined $v         if $writer =~ /^on_/;
    return $v ne $default     if $writer eq 'str';
    return !!$v != !!$default if $writer eq 'bool';
    return $v != $default;
}

# Children and keywords arrive mixed in one list, as in
# `column(text(...), row(...), spacing => 8)`. A string naming one of
# the element's keywords takes the next item as its value; anything
# else is a child handle.
sub _args ($spec, @list) {
    my %known = map { $_->[0] => 1 } @$spec;
    my (@kids, %kw);
    while (@list) {
        my $item = shift @list;
        if (defined $item && !ref $item && $known{$item}) { $kw{$item} = shift @list }
        else                                               { push @kids, $item }
    }
    return (\@kids, \%kw);
}

sub _element ($kind, $name, $head, $kids, $kw) {
    my $spec = $SPEC{$name};
    my $el = Rakugan::Door::el($kind);
    Rakugan::Door::str($el, @$head) if $head;
    for my $row (@$spec) {
        my ($key_name, $writer, $key, $default) = @$row;
        next unless exists $kw->{$key_name};
        my $v = delete $kw->{$key_name};
        $WRITE{$writer}->($el, $key, $v) if _differs($writer, $v, $default);
    }
    croak "$name has no `" . join('`, `', sort keys %$kw) . "`" if %$kw;
    for my $kid (@$kids) {
        croak "$name was given something that is not an element" unless defined $kid && $kid =~ /^\d+$/;
    }
    Rakugan::Door::children($el, $kids) if @$kids;
    return Rakugan::Door::end($el);
}

sub _leaf ($kind, $name, $head, @rest) {
    my ($kids, $kw) = _args($SPEC{$name}, @rest);
    croak "$name takes no children" if @$kids;
    return _element($kind, $name, $head, [], $kw);
}

# --- the vocabulary -----------------------------------------------------

# A run of text.
sub text ($s, @rest) { _leaf(KIND_TEXT, text => [K_TEXT, $s], @rest) }
# A button; `on_click` runs when it is pressed.
sub button ($label, @rest) { _leaf(KIND_BUTTON, button => [K_LABEL, $label], @rest) }
# A line a person types into; `on_change` is called with the text per keystroke.
sub text_field ($value, @rest) { _leaf(KIND_TEXT_FIELD, text_field => [K_VALUE, $value], @rest) }
# Its children down the page.
sub column (@list) { _element(KIND_COLUMN, column => undef, _args($SPEC{column}, @list)) }
# Its children across the page.
sub row (@list) { _element(KIND_ROW, row => undef, _args($SPEC{row}, @list)) }

1;
