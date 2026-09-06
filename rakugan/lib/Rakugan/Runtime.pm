package Rakugan::Runtime;
# What the door holds between builds, and how an element is written to
# the engine from its row of the table. The shape is
# wakakusa/lib/wakakusa/runtime.rb's: the engine holds no Perl value at
# all — it has numbers, and asks for the rest.
use v5.40;
use Carp qw(croak);
use Scalar::Util qw(looks_like_number);
use Rakugan::Door;
use Rakugan::Vocab;

# This build's handlers and row builders, by the numbers the engine
# gets. A build starts both lists over, so a number means something
# only inside the build that handed it out.
my @handlers;
my @rows;
# The object whose `view` answers the tree.
my $app;

sub register ($el, $key, $cb) {
    push @handlers, $cb;
    Rakugan::Door::on($el, $key, $#handlers);
    return;
}

sub register_rows ($el, $key, $cb) {
    push @rows, $cb;
    Rakugan::Door::rows($el, $key, $#rows);
    return;
}

# The engine asks for the tree.
sub _build {
    @handlers = ();
    @rows = ();
    return $app->view;
}

# The engine hands back the number an element carried and says what
# the event carries; that decides what the handler is called with.
sub _on_event ($id, $kind) {
    my $cb = $handlers[$id] or return;
    my %pay = %Rakugan::Vocab::PAY;
    if    ($kind == $pay{none})  { $cb->() }
    elsif ($kind == $pay{text})  { $cb->(Rakugan::Door::event_text()) }
    elsif ($kind == $pay{bool})  { $cb->(Rakugan::Door::event_int() != 0 ? true : false) }
    elsif ($kind == $pay{int})   { $cb->(Rakugan::Door::event_int()) }
    elsif ($kind == $pay{float}) { $cb->(Rakugan::Door::event_num()) }
    else  { die "an event of kind $kind reached a door that does not know it\n" }
    return;
}

# The engine asks for one row of a list that builds its rows on demand.
sub _row_build ($handler, $index) {
    my $cb = $rows[$handler] or return 0;
    return $cb->($index) // 0;
}

# --- writing an element from the table --------------------------------------

my %WRITE = (
    str   => sub ($el, $key, $v) { Rakugan::Door::str($el, $key, $v) },
    num   => sub ($el, $key, $v) { Rakugan::Door::num($el, $key, $v) },
    int   => sub ($el, $key, $v) { Rakugan::Door::int($el, $key, $v) },
    bool  => sub ($el, $key, $v) { Rakugan::Door::bool($el, $key, $v ? 1 : 0) },
    strs  => sub ($el, $key, $v) { Rakugan::Door::push_str($el, $key, $_) for @$v },
    nums  => sub ($el, $key, $v) { Rakugan::Door::push_num($el, $key, $_) for @$v },
    nums2 => sub ($el, $key, $v) {
        for my $inner (@$v) {
            Rakugan::Door::list_break($el, $key);
            Rakugan::Door::push_num($el, $key, $_) for @$inner;
        }
    },
);

# A keyword crosses only when it differs from what the engine would
# have used anyway; a list crosses when it has something in it.
sub _differs ($type, $v, $default) {
    return 1 unless defined $default;
    return scalar(@$v) > 0  if $type =~ /\A(?:strs|nums|nums2)\z/;
    return $v ne $default   if $type eq 'str';
    return !!$v != !!$default if $type eq 'bool';
    return $v != $default;
}

sub _is_keyword ($item, $own) {
    return defined $item && !ref $item && !looks_like_number($item)
        && ($own->{$item} || $Rakugan::Vocab::RIDER{$item});
}

# One element, from its row of the table and what the app wrote.
# Positional arguments come first — a required one is taken as it
# stands, an optional one only when the next argument is not a keyword
# — then keywords and children in any order: a string naming one of
# the element's keywords (or one every element takes) takes the next
# item as its value, and anything else is a child's handle.
sub element ($spec, @args) {
    my $name = $spec->{name};
    my %own = map { $_->{name} => $_ } @{ $spec->{props} };
    my (%given, @kids);
    for my $p (grep { $_->{pos} } @{ $spec->{props} }) {
        last unless @args;
        last if exists $p->{default} && _is_keyword($args[0], \%own);
        $given{ $p->{name} } = shift @args;
    }
    while (@args) {
        my $item = shift @args;
        if (_is_keyword($item, \%own)) {
            croak "`$name` was given `$item` twice" if exists $given{$item};
            croak "`$name` was given `$item` with nothing after it" unless @args;
            $given{$item} = shift @args;
            next;
        }
        croak "`$name` takes no children" unless $spec->{children};
        croak "`$name` was given something that is not an element" unless defined $item && $item =~ /\A\d+\z/;
        push @kids, $item;
    }
    for my $p (grep { $_->{pos} && !exists $_->{default} } @{ $spec->{props} }) {
        croak "`$name` needs its $p->{name}" unless exists $given{ $p->{name} };
    }

    my $el = Rakugan::Door::el($spec->{kind});
    for my $p (@{ $spec->{props} }) {
        next unless exists $given{ $p->{name} };
        my $v = $given{ $p->{name} };
        if (defined $p->{handler}) {
            croak "`$name`'s `$p->{name}` takes a sub" unless ref $v eq 'CODE';
            register($el, $p->{key}, $v);
            next;
        }
        if ($p->{type} eq 'rows') {
            croak "`$name`'s `$p->{name}` takes a sub that answers the row" unless ref $v eq 'CODE';
            register_rows($el, $p->{key}, $v);
            next;
        }
        if ($p->{type} =~ /\A(?:strs|nums|nums2)\z/) {
            croak "`$name`'s `$p->{name}` takes a list reference" unless ref $v eq 'ARRAY';
        }
        next unless $p->{pos} || _differs($p->{type}, $v, $p->{default});
        $WRITE{ $p->{type} }->($el, $p->{key}, $v);
    }
    # The keywords every element takes: whatever was written crosses. An
    # element that owns one of these names under its own meaning took it
    # above, so it never gets here.
    for my $r (@Rakugan::Vocab::RIDERS) {
        next if $own{ $r->{name} } || !exists $given{ $r->{name} };
        croak "this element's own `label` is already the name a screen reader reads; there is no second one to give"
            if $spec->{owns_label} && $r->{name} eq 'a11y_label';
        $WRITE{ $r->{type} }->($el, $r->{key}, $given{ $r->{name} });
    }
    Rakugan::Door::children($el, \@kids) if @kids;
    return Rakugan::Door::end($el);
}

# Open the window and hand it the app: an object with a `view` method
# that answers one element. Under PIXIE_SCRIPT there is no window: the
# engine builds the tree, prints it, replays the script and returns.
sub run ($the_app, %opt) {
    for my $k (sort keys %opt) {
        die "run has no `$k =>`; it takes title, width, height, padding\n"
            unless $k =~ /\A(?:title|width|height|padding)\z/;
    }
    die "run takes an object with a `view` method\n" unless ref $the_app && $the_app->can('view');
    $app = $the_app;
    return Rakugan::Door::run($opt{title} // 'rakugan', $opt{width} // 0, $opt{height} // 0,
                              $opt{padding} // -1, \&_build, \&_on_event, \&_row_build);
}

1;
