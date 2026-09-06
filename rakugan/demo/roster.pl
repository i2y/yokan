# The table that builds its rows on demand: the sub builds row i as a
# row of one cell per column, and the header and the rows sit on tracks
# whose shares are `widths`. Picking a row and sorting a column are the
# app's own methods.
use Rakugan;

class Roster {
    use Rakugan;
    field @teams   = ("red", "blue", "green", "gold");
    field @names   = empty(Str);
    field @team_of = empty(Str);
    field @scores  = empty(Int);
    # What each column is put in order by. A column of text is sorted
    # by a number beside it, because putting two strings in order is
    # not in the dialect yet.
    field @ids     = empty(Int);
    field @team_ix = empty(Int);
    field $sel     = -1;
    field $line    = "";
    field $sorted  = -1;
    field $desc    = false;

    ADJUST {
        for my $i (0 .. 23) {
            push @names, "member $i";
            push @ids, $i;
            push @team_ix, $i % 4;
            push @team_of, $teams[$i % 4];
            push @scores, ($i * 37 + 11) % 100;
        }
    }

    method pick :Sig(Int) ($i) {
        $sel = $i;
        $line = "$names[$i] ($team_of[$i], $scores[$i])";
    }

    # Whether row a belongs before row b, by the column being sorted.
    method before :Sig(Int, Int, Int => Bool) ($col, $a, $b) {
        my $up = true;
        if ($col == 2) {
            $up = $scores[$a] < $scores[$b];
        } elsif ($col == 1) {
            $up = $team_ix[$a] < $team_ix[$b];
        } else {
            $up = $ids[$a] < $ids[$b];
        }
        return $desc ? !$up : $up;
    }

    method swap :Sig(Int, Int) ($a, $b) {
        my $n = $names[$a];
        $names[$a] = $names[$b];
        $names[$b] = $n;
        my $t = $team_of[$a];
        $team_of[$a] = $team_of[$b];
        $team_of[$b] = $t;
        my $s = $scores[$a];
        $scores[$a] = $scores[$b];
        $scores[$b] = $s;
        my $d = $ids[$a];
        $ids[$a] = $ids[$b];
        $ids[$b] = $d;
        my $x = $team_ix[$a];
        $team_ix[$a] = $team_ix[$b];
        $team_ix[$b] = $x;
    }

    method sort_by :Sig(Int) ($col) {
        $desc = $col == $sorted ? !$desc : false;
        $sorted = $col;
        my $i = 1;
        while ($i < scalar @names) {
            my $k = $i;
            while ($k > 0) {
                if ($self->before($col, $k, $k - 1)) {
                    $self->swap($k, $k - 1);
                    $k -= 1;
                } else {
                    last;
                }
            }
            $i += 1;
        }
        $sel = -1;
        $line = "";
    }

    method member :Sig(Int) ($i) {
        return row(
            text($names[$i], grow => 2),
            text($team_of[$i], grow => 1),
            text("$scores[$i]", grow => 1, align => "right"),
        );
    }

    method view {
        return column(
            text("Roster — @{[ scalar @names ]} people", size => 16),
            table(["name", "team", "score"], scalar @names,
                  sub ($i) { $self->member($i) },
                  widths => [2, 1, 1], height => 220,
                  selected => $sel, sort => $sorted, descending => $desc,
                  on_select => sub ($i) { $self->pick($i) },
                  on_sort   => sub ($i) { $self->sort_by($i) }),
            text($line eq "" ? "nobody picked" : $line, size => 12),
            spacing => 10,
            padding => 14,
        );
    }
}

run(Roster->new, title => "roster");
