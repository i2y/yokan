# A calculator: one accumulator, one pending operation, and a display
# the app builds as a string. The look is a hash of keywords, handed to
# each key.
use Rakugan;

my %KEY     = (grow => 1, size => 20, background => "panel",
               hover_background => "#45475a", active_background => "#585b70");
my %FUN     = (%KEY, background => "#313244", color => "#a6adc8");
my %OP      = (%KEY, background => "#fab387", color => "#1e1e2e",
               hover_background => "#f8c49b", active_background => "#f5e0dc");
my %WIDE    = (%KEY, grow => 2, basis => 8);
my %READOUT = (size => 40, color => "text", align => "right", grow => 1.4);
my %KEYS    = (spacing => 8, grow => 1);

class Calc {
    use Rakugan;
    field $display = "0";
    field $acc     = 0.0;
    field $op      = "";
    field $fresh   = true;
    field $has_dot = false;

    method press :Sig(Str) ($d) {
        if ($fresh) {
            $display = $d;
            $fresh = false;
            $has_dot = false;
        } elsif ($display eq "0") {
            $display = $d;
        } else {
            $display = $display . $d;
        }
    }

    method dot {
        if ($fresh) {
            $display = "0.";
            $fresh = false;
            $has_dot = true;
        } elsif (!$has_dot) {
            $display = $display . ".";
            $has_dot = true;
        }
    }

    method negate {
        my $v = 0 + $display;
        return if $v == 0.0;
        $display = "@{[ 0.0 - $v ]}";
        $fresh = false;
    }

    method percent {
        $display = "@{[ (0 + $display) / 100.0 ]}";
        $fresh = true;
        $has_dot = false;
    }

    method apply :Sig(Str) ($nxt) {
        if ($fresh && $op ne "") {
            $op = $nxt;
            return;
        }
        my $cur = 0 + $display;
        $acc = $cur if $op eq "";
        $acc += $cur if $op eq "+";
        $acc -= $cur if $op eq "-";
        $acc *= $cur if $op eq "×";
        if ($op eq "÷") {
            if ($cur == 0.0) {
                $display = "Error";
                $acc = 0.0;
                $op = "";
                $fresh = true;
                return;
            }
            $acc /= $cur;
        }
        $display = "$acc";
        $op = $nxt;
        $fresh = true;
    }

    method clear {
        $display = "0";
        $acc = 0.0;
        $op = "";
        $fresh = true;
        $has_dot = false;
    }

    method digit :Sig(Str) ($d) {
        return button($d, %KEY, on_click => sub { $self->press($d) });
    }

    method op_key :Sig(Str) ($name) {
        return button($name, %OP, on_click => sub { $self->apply($name) });
    }

    method equals {
        return button("=", %OP, on_click => sub { $self->apply("") });
    }

    method view {
        return column(
            text($display, %READOUT),
            row(
                button("C", %FUN, on_click => sub { $self->clear }),
                button("±", %FUN, on_click => sub { $self->negate }),
                button("%", %FUN, on_click => sub { $self->percent }),
                $self->op_key("÷"),
                %KEYS,
            ),
            row($self->digit("7"), $self->digit("8"), $self->digit("9"), $self->op_key("×"), %KEYS),
            row($self->digit("4"), $self->digit("5"), $self->digit("6"), $self->op_key("-"), %KEYS),
            row($self->digit("1"), $self->digit("2"), $self->digit("3"), $self->op_key("+"), %KEYS),
            row(
                button("0", %WIDE, on_click => sub { $self->press("0") }),
                button(".", %KEY, on_click => sub { $self->dot }),
                $self->equals,
                %KEYS,
            ),
            spacing => 8, padding => 16, grow => 1,
        );
    }
}

run(Calc->new, title => "calc");
