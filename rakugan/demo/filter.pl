# A chooser that changes what a list shows. The rows are built on
# demand, so the list is asked only for the ones in view.
use Rakugan;

class Alerts {
    use Rakugan;
    field @levels = ("all", "crit", "warn");
    field $level  = 0;
    field @crit = (
        "crit  09:02  payments p95 breach — circuit breaker armed",
        "crit  09:11  db failover triggered",
        "crit  09:20  worker pool exhausted",
    );
    field @warn = (
        "warn  09:05  error budget burn 2x on web",
        "warn  09:14  cache hit rate below 80%",
        "warn  09:24  edge latency above SLO",
    );
    field @visible = empty(Str);

    ADJUST {
        push @visible, $_ for @crit;
        push @visible, $_ for @warn;
    }

    method pick :Sig(Int) ($i) {
        $level = $i;
        @visible = ();
        if ($i == 0) {
            push @visible, $_ for @crit;
            push @visible, $_ for @warn;
        } elsif ($i == 1) {
            push @visible, $_ for @crit;
        } else {
            push @visible, $_ for @warn;
        }
    }

    method alert_row :Sig(Int) ($i) {
        return text($visible[$i], size => 12);
    }

    method view {
        return column(
            text("alert filter", size => 16),
            segmented(options => \@levels, selected => $level,
                      on_change => sub ($i) { $self->pick($i) }),
            text("@{[ scalar @visible ]} shown", size => 12, color => "textDim"),
            list_view(scalar @visible, sub ($i) { $self->alert_row($i) },
                      item_height => 22, height => 150),
            spacing => 10,
            padding => 14,
        );
    }
}

run(Alerts->new, title => "filter");
