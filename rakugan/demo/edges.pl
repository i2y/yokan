# The edges: an index past the end of a list, and a number far past
# what a machine word holds that keeps growing. Both runs have to
# answer the same, and this is the demo that says so.
#
# The number is the largest whole one the dialect holds. Past that,
# perl grows a whole number into one with a fraction and the compiled
# run has 64 bits and nowhere to grow, so a literal past the edge is
# refused by name rather than quietly wrapping.
use Rakugan;

class Edges {
    use Rakugan;
    field @xs     = (7);
    field $picked = 0;
    field $big    = 9223372036854775807;
    field $steps  = 0;

    method partial {
        $steps += 1;
        $picked = $xs[9] // -1;
    }

    method view {
        return column(
            text("picked=$picked steps=$steps"),
            text("big=$big"),
            button("oob",     on_click => sub { $picked = $xs[5] // -1 }),
            button("shrink",  on_click => sub { $big = $big - 1 }),
            button("partial", on_click => sub { $self->partial }),
            spacing => 8,
            padding => 12,
        );
    }
}

run(Edges->new, title => "edges");
