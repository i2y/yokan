# A panel over the rest of the window, opened and closed by the app.
use Rakugan;

class Dialog {
    use Rakugan;
    field $show   = false;
    field $status = "undecided";

    method decide :Sig(Str) ($answer) {
        $status = $answer;
        $show = false;
    }

    method view {
        my @cells = (
            text("status: $status", size => 16),
            button("open dialog", on_click => sub { $show = true }),
        );
        if ($show) {
            push @cells, modal(
                column(
                    text("accept the terms?", size => 18),
                    row(
                        button("accept",  on_click => sub { $self->decide("accepted") }),
                        button("decline", on_click => sub { $self->decide("declined") }),
                        spacing => 8,
                    ),
                    spacing => 8, padding => 12, background => "panel",
                ),
            );
        } else {
            push @cells, text("(dialog closed)", size => 12, color => "#8a8f98");
        }
        return column(@cells, spacing => 10, padding => 14);
    }
}

run(Dialog->new, title => "dialog");
