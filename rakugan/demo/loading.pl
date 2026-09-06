# The bar that fills, in its three forms: with a caption above it, at a
# size the app chose, and sweeping for work with no known length.
use Rakugan;

class Loading {
    use Rakugan;
    field $ratio = 0.25;
    field $busy  = false;

    method step {
        $ratio = $ratio >= 1.0 ? 0.0 : $ratio + 0.25;
    }

    method view {
        return column(
            text("ratio: $ratio"),
            progress($ratio, label => "Uploading"),
            progress($ratio, width => 240, height => 6),
            progress($ratio, indeterminate => $busy),
            row(
                button("step", on_click => sub { $self->step }),
                button("busy", on_click => sub { $busy = !$busy }),
                spacing => 8,
            ),
            spacing => 12,
            padding => 16,
        );
    }
}

run(Loading->new, title => "loading");
