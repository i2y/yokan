# Perl's own, under the gate.
#
# Nothing here is Rakugan's. `length`, `substr`, `uc`, `sort`, `grep`,
# `map`, `sprintf`, `List::Util` and `POSIX` are the language's, and
# what the gate says is that the two runs answer the same. Where the
# name is Perl's, perl's own output is the specification: every one of
# these is held to a table that perl printed.
use Rakugan;
use List::Util qw(sum max min first uniq);
use POSIX qw(floor ceil fmod strftime);

class Stdlib {
    use Rakugan;
    use List::Util qw(sum max min first uniq);
    use POSIX qw(floor ceil fmod strftime);

    field $hyp    = 0.0;
    field $spread = "-";
    field $sift   = "-";
    field $runs   = "-";
    field $stamp  = "-";
    field $words  = "-";
    field $unique = "-";
    field $picked = 0;
    field @scores = (3, 5, 8, 13, 21);
    field @votes  = ("ivy", "momo", "ivy", "ada", "momo", "ivy", "ada");

    method measure {
        $hyp = sqrt(3.0 * 3.0 + 4.0 * 4.0);
    }

    method stats {
        my @sorted = sort { $a <=> $b } @scores;
        my $mean = sum(@scores) / scalar @scores;
        my $median = $sorted[int(scalar(@sorted) / 2)];
        $spread = sprintf("mean %.1f median %d min %d max %d",
                          $mean, $median, min(@scores), max(@scores));
    }

    method sift_scores {
        my @big   = grep { $_ > 5 } @scores;
        my @small = grep { $_ <= 5 } @scores;
        my @big_text   = map { "$_" } @big;
        my @small_text = map { "$_" } @small;
        $sift = "big " . join(",", @big_text) . " small " . join(",", @small_text);
    }

    method combine {
        my @doubled = map { $_ * 2 } @scores;
        my @text = map { "$_" } @doubled;
        my $down = floor(2.7);
        my $up   = ceil(2.1);
        $runs = "doubled @{[ join(\",\", @text) ]} floor $down ceil $up";
    }

    method take_stamp {
        $stamp = strftime("%Y-%m-%d %H:%M:%S UTC", gmtime(1700000000));
    }

    method capitalize {
        my $line = "  the quick brown fox  ";
        my @parts = split(' ', $line);
        my @caps = map { ucfirst($_) } @parts;
        my $trimmed = substr($line, 2, length($line) - 4);
        my $n = length($trimmed);
        $words = join("-", @caps) . " ($n)";
    }

    method distinct {
        my @names = sort { $a cmp $b } uniq(@votes);
        $unique = join(",", @names);
    }

    method find {
        $picked = (first { $_ > 5 } @scores) // -1;
    }

    method view {
        return column(
            text("Perl's own, in both runs", size => 16, bold => true),
            text("hypotenuse: $hyp"),
            text("spread: $spread"),
            text("sift: $sift"),
            text("runs: $runs"),
            text("stamp: $stamp"),
            text("words: $words"),
            text("set: $unique"),
            text("first over five: $picked"),
            row(
                button("measure", on_click => sub { $self->measure }),
                button("stats",   on_click => sub { $self->stats }),
                button("sift",    on_click => sub { $self->sift_scores }),
                button("combine", on_click => sub { $self->combine }),
                spacing => 6,
            ),
            row(
                button("stamp",  on_click => sub { $self->take_stamp }),
                button("words",  on_click => sub { $self->capitalize }),
                button("set",    on_click => sub { $self->distinct }),
                button("first",  on_click => sub { $self->find }),
                spacing => 6,
            ),
            spacing => 6,
            padding => 14,
        );
    }
}

run(Stdlib->new, title => "stdlib");
