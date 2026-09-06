# Work that takes a while, done off the window's thread. `task` runs
# the sub on a thread of perl's own; when it answers, `on_done` is
# called on the window's thread with the answer.
#
# Nothing inside the work touches the app's state or the screen. That
# is the whole rule, and it is why the answer comes back as a value
# rather than the work writing it anywhere.
use Rakugan;

class Jobs {
    use Rakugan;
    field $status = "idle";
    field $answer = 0;
    field $done   = 0;

    method start {
        $status = "working";
        task(sub {
            # deliberately slow, and deliberately arithmetic: both runs
            # have to agree about what it answers.
            my $total = 0;
            my $i = 0;
            while ($i < 300000) {
                $total += $i % 7;
                $i += 1;
            }
            $total;
        }, on_done => sub ($v) {
            $answer = $v;
            $done += 1;
            $status = "done";
        });
    }

    method view {
        return column(
            text("background work", size => 18, bold => true),
            text("status: $status"),
            text("answer: $answer  ($done finished)"),
            button("start slow work", on_click => sub { $self->start }),
            spacing => 10,
            padding => 14,
        );
    }
}

run(Jobs->new, title => "tasks");
