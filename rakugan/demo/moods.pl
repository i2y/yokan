# Values that are one of a few named things, and a value that may be
# nothing at all. Perl writes the first as constants and the second as
# `undef`: a field that starts as nothing says what it may hold, and
# `defined` is the `if` inside which it is read as the value. The
# tracker is a second class with methods of its own, which the app
# holds and calls.
use Rakugan;

class Tracker {
    use Rakugan;
    use constant { HAPPY => "happy", SAD => "sad" };
    field $last  :reader = maybe(Int);
    field $trend :reader = HAPPY;

    method note :Sig(Int) ($v) {
        $last = $v;
        $trend = $trend eq HAPPY ? SAD : HAPPY;
    }

    method wipe {
        $last = undef;
    }
}

class Moods {
    use Rakugan;
    use constant { HAPPY => "happy", SAD => "sad" };
    field $mood    = HAPPY;
    field $sel     = maybe(Int);
    field $note    = "-";
    field $tracker = Tracker->new;

    method flip {
        $mood = $mood eq HAPPY ? SAD : HAPPY;
    }

    method describe {
        if (defined $sel) {
            $note = "chose $sel";
        } else {
            $note = "nothing chosen";
        }
    }

    method mood_line {
        return text("mood: up", size => 18, color => "accent", animate => 120, easing => "out") if $mood eq HAPPY;
        return text("mood: down", size => 18, color => "#f38ba8", animate => 120, easing => "out");
    }

    method view {
        my @cells = ($self->mood_line);
        if (defined $sel) {
            push @cells, text("selection: $sel");
        } else {
            push @cells, text("(no selection)");
        }
        push @cells, text("note: $note");
        if (defined $tracker->last) {
            push @cells, text("tracked: @{[ $tracker->last ]}", size => 12);
        } else {
            push @cells, text("(nothing tracked)", size => 12);
        }
        return column(
            @cells,
            row(
                button("flip",     on_click => sub { $self->flip }),
                button("pick",     on_click => sub { $sel = 7 }),
                button("clear",    on_click => sub { $sel = undef }),
                button("describe", on_click => sub { $self->describe }),
                button("track",    animate => 100, easing => "inOut", on_click => sub { $tracker->note(9) }),
                button("wipe",     on_click => sub { $tracker->wipe }),
                spacing => 6,
            ),
            spacing => 8,
            padding => 12,
        );
    }
}

run(Moods->new, title => "moods");
