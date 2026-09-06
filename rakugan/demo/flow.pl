# Control flow in the handlers: a loop that skips, a loop that stops, a
# while, and a method that answers a value. Both runs do the same
# thing, which is what the gate compares.
use Rakugan;

class Flow {
    use Rakugan;
    field $count  = 0;
    field $total  = 0;
    field $status = "start";

    method double :Sig(Int => Int) ($v) {
        return $v * 2;
    }

    method step {
        $count += 1;
        if ($count > 3 && $count < 100) {
            $status = "big";
        } elsif ($count == 3) {
            $status = "three";
        } else {
            $status = "small";
        }
    }

    method tally {
        $total = 0;
        for my $i (1 .. 5) {
            next if $i == 3;
            $total += $self->double($i);
        }
    }

    method bump3 {
        $status = "working";
        while ($count < 3) {
            $count += 1;
        }
        $status = "done";
    }

    method find {
        for my $i (0 .. 9) {
            if ($i * $i > 10) {
                $count = $i;
                last;
            }
        }
    }

    method view {
        return column(
            text("count=$count total=$total status=$status"),
            row(
                button("step",  on_click => sub { $self->step }),
                button("tally", on_click => sub { $self->tally }),
                button("bump3", on_click => sub { $self->bump3 }),
                button("find",  on_click => sub { $self->find }),
                spacing => 6,
            ),
            spacing => 8,
            padding => 12,
        );
    }
}

run(Flow->new, title => "flow");
