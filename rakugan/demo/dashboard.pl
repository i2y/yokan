# A timer: declared before the app runs, told every second. Both runs
# tick off the same clock — a frame in a window, an `advance:` in a
# script — so the same number of ticks lands in both.
#
# The step is worked out rather than drawn from a generator: the two
# runs draw on different ones, and a dashboard that cannot be compared
# is not worth gating.
use Rakugan;

class Dashboard {
    use Rakugan;
    field @hist = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    field $at    = 0;
    field $ticks = 0;
    field $cur   = 0.25;

    method tick {
        $ticks += 1;
        my $step = (($ticks * 37 % 41) / 100.0) - 0.2;
        my $v = $cur + $step;
        $v = 0.0 if $v < 0.0;
        $v = 1.0 if $v > 1.0;
        $cur = $v;
        $hist[$at] = $v;
        $at = ($at + 1) % 12;
    }

    method view {
        return column(
            row(
                text("load, sampled every second", size => 13, color => "#8a8f98", grow => 1),
                spinner(size => 16),
                spacing => 8,
            ),
            text("@{[ sprintf('%.2f', $cur) ]}", size => 40),
            progress($cur),
            line_chart(\@hist, height => 120),
            text("$ticks ticks · 12 slots", size => 12, color => "#8a8f98"),
            spacing => 12,
            padding => 16,
        );
    }
}

my $app = Dashboard->new;
every(1.0, sub { $app->tick });
run($app, title => "dashboard");
