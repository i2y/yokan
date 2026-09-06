# A hundred thousand rows, filtered as you type. The list is built on
# demand: only the rows in the window are ever made, so the filter is
# the only thing that touches all of them.
#
# The numbers are worked out rather than drawn from a generator: the
# two runs do not share one, and a viewer whose rows cannot be compared
# would not be worth gating.
use Rakugan;

class Viewer {
    use Rakugan;
    field @stems  = ("kuro", "shiro", "aka", "ao", "momo", "yuki", "hana", "sora");
    field @tails  = ("maru", "suke", "chan", "gou", "ta", "emon");
    field @cats   = ("alpha", "beta", "gamma", "delta", "epsilon");
    field @names  = empty(Str);
    field @kind   = empty(Str);
    field @values = empty(Num);
    field @shown  = empty(Int);
    field $q      = "";

    ADJUST {
        for my $i (0 .. 99999) {
            my $stem = $stems[$i % 8];
            my $tail = $tails[int($i / 8) % 6];
            push @names, sprintf("%s%s-%06d", $stem, $tail, $i);
            push @kind, $cats[$i % 5];
            push @values, (($i * 37 % 4000) / 100.0) + 30.0;
            push @shown, $i;
        }
    }

    method filter :Sig(Str) ($text) {
        $q = $text;
        if ($q eq "") {
            @shown = (0 .. 99999);
        } else {
            my $low = lc($q);
            @shown = grep { index(lc($names[$_]), $low) >= 0 || index($kind[$_], $low) >= 0 } 0 .. 99999;
        }
    }

    method line :Sig(Int) ($i) {
        return row(
            text(sprintf("%06d", $i), size => 12, color => "#8a8f98"),
            text($names[$i], grow => 1),
            text($kind[$i], size => 12, color => "#7aa2f7"),
            text(sprintf("%.2f", $values[$i]), align => "right"),
            spacing => 12,
        );
    }

    method view {
        return column(
            text("csv viewer — 100000 rows, built on demand", size => 13, color => "#8a8f98"),
            text_field($q, placeholder => "filter…", on_change => sub ($t) { $self->filter($t) }),
            text("@{[ scalar @shown ]} / 100000 rows match", size => 12),
            list_view(scalar @shown, sub ($k) { $self->line($shown[$k]) },
                      item_height => 26, height => 430),
            spacing => 10,
            padding => 14,
        );
    }
}

run(Viewer->new, title => "csv_viewer");
