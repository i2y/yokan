# One list of numbers, drawn twice. A chart takes its data as its first
# argument, so a view that computes the numbers reads in order.
use Rakugan;

class Trend {
    use Rakugan;
    field @values = (3.0, 5.0, 2.0);
    field $limit  = 4.5;

    method bump {
        push @values, 8.0;
    }

    method view {
        return column(
            text("points: @{[ scalar @values ]}", size => 14),
            line_chart(\@values, height => 120),
            bar_chart(\@values, height => 90),
            text("limit: @{[ sprintf('%.1f', $limit) ]}", size => 12, color => "#8a8f98"),
            row(
                button("add point",   on_click => sub { $self->bump }),
                button("raise limit", on_click => sub { $limit += 0.5 }),
                spacing => 8,
            ),
            spacing => 10,
            padding => 14,
        );
    }
}

run(Trend->new, title => "trend");
