# Charts that can say what they mean: a profit-and-loss bar chart whose
# losing months hang below the zero line, and a two-series line chart.
# `min`/`max` both zero take the range from the data; `axis` draws the
# tick labels and a faint gridline at each; `series` takes one list per
# line, `colors` one color each.
use Rakugan;

my %HEADING = (size => 18, color => "accent");
my %FAINT   = (size => 12, color => "#8a8f98");

class Book {
    use Rakugan;
    field @months   = ("Jan", "Feb", "Mar", "Apr", "May", "Jun");
    field @profit   = (12.0, -8.0, 4.0, -3.0, 15.0, -6.0);
    field @requests = (40.0, 55.0, 48.0, 62.0, 70.0, 58.0);
    field @errors   = (3.0, 9.0, 5.0, 12.0, 6.0, 4.0);
    field @traffic  = ([40.0, 55.0, 48.0, 62.0, 70.0, 58.0], [3.0, 9.0, 5.0, 12.0, 6.0, 4.0]);
    field $n        = 6;

    method next_month {
        $n += 1;
        # A month that both runs can agree about, worked out rather
        # than drawn from a generator neither shares.
        push @profit,   ($n * 7 % 41) - 18.0;
        push @months,   "M$n";
        push @requests, ($n * 13 % 50) + 30.0;
        push @errors,   ($n * 5 % 14) + 0.0;
        @traffic = (\@requests, \@errors);
    }

    method view {
        return column(
            text("Profit and loss", %HEADING),
            text("negative months hang below the zero line", %FAINT),
            bar_chart(\@profit, labels => \@months, axis => true, height => 150),
            text("Traffic", %HEADING),
            text("requests and errors, one color each", %FAINT),
            line_chart(series => \@traffic, labels => \@months,
                       colors => ["accent", "#f38ba8"], axis => true, max => 90, height => 150),
            row(button("next month", on_click => sub { $self->next_month }), spacing => 8),
            spacing => 12,
            padding => 16,
        );
    }
}

run(Book->new, title => "charts");
