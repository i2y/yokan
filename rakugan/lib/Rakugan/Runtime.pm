package Rakugan::Runtime;
# What the door holds between builds, and the calls the element subs
# make. The shape is wakakusa/lib/wakakusa/runtime.rb's: the engine
# holds no Perl value at all — it has numbers, and asks for the rest.
use v5.40;
use Rakugan::Door;

# What a handler is called with; the engine names the kind when it
# hands an event over (crates/pixie-capi/src/vocab.rs, PAY_*).
use constant { PAY_NONE => 0, PAY_TEXT => 1, PAY_BOOL => 2, PAY_INT => 3, PAY_FLOAT => 4 };

# This build's handlers, by the number the engine gets. A build starts
# the list over, so a handler number means something only inside the
# build that handed it out.
my @handlers;
# The object whose `view` answers the tree.
my $app;

sub register ($el, $key, $cb) {
    push @handlers, $cb;
    Rakugan::Door::on($el, $key, $#handlers);
    return;
}

# The engine asks for the tree.
sub _build {
    @handlers = ();
    return $app->view;
}

# The engine hands back the number an element carried and says what
# the event carries; that decides what the handler is called with.
sub _on_event ($id, $kind) {
    my $cb = $handlers[$id] or return;
    if    ($kind == PAY_TEXT) { $cb->(Rakugan::Door::event_text()) }
    elsif ($kind == PAY_NONE) { $cb->() }
    else  { die "an event of kind $kind reached a door that does not take it yet\n" }
    return;
}

# Open the window and hand it the app: an object with a `view` method
# that answers one element. Under PIXIE_SCRIPT there is no window: the
# engine builds the tree, prints it, replays the script and returns.
sub run ($the_app, %opt) {
    for my $k (sort keys %opt) {
        die "run has no `$k =>`; it takes title, width, height, padding\n"
            unless $k =~ /^(title|width|height|padding)$/;
    }
    die "run takes an object with a `view` method\n" unless ref $the_app && $the_app->can('view');
    $app = $the_app;
    return Rakugan::Door::run($opt{title} // 'rakugan', $opt{width} // 0, $opt{height} // 0,
                              $opt{padding} // -1, \&_build, \&_on_event);
}

1;
