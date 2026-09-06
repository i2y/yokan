# The keywords every element takes, on elements that have nothing else
# in common: a theme scope on a spacer, a box around a column, a tween
# on a chooser, a tooltip on a rule, and the lock that makes a field
# and a button inert.
use Rakugan;

class Locks {
    use Rakugan;
    field $locked = false;
    field $saves  = 0;
    # The palette the spacer's subtree resolves its tokens in — a
    # keyword takes a value, not just a literal, so the lock switches it.
    field $mode   = "dark";
    field $tab    = 0;
    field $note   = "draft";

    method flip {
        $locked = !$locked;
        $mode = $locked ? "light" : "dark";
    }

    method view {
        return column(
            text("shared", size => 20, role => "heading"),
            row(
                text("mode: $mode  saves: $saves", size => 12),
                # A theme scope on a spacer: the keyword is the
                # element's, whichever element it is.
                spacer(grow => 1, theme => $mode),
                button("lock", tooltip => "flip the lock", on_click => sub { $self->flip }),
                spacing => 8,
            ),
            segmented(options => ["read", "write"], selected => $tab,
                      animate => 120, easing => "out",
                      on_change => sub ($i) { $tab = $i }),
            # A box around the section: 260 wide, never under 200.
            column(
                # The field takes two of the grid's three tracks, and
                # goes inert with the lock.
                grid(
                    text("note", size => 12),
                    text_field($note, col_span => 2, disabled => $locked,
                               on_change => sub ($t) { $note = $t }),
                    columns => 3, spacing => 8,
                ),
                button("save", disabled => $locked, tooltip => "count a save",
                       on_click => sub { $saves += 1 }),
                width => 260, min_width => 200, spacing => 8, padding => 8, background => "panel",
            ),
            link("Docs", "https://i2y.github.io/yokan/", role => "button"),
            divider(tooltip => "the end of the shared properties"),
            spacing => 10,
            padding => 14,
        );
    }
}

run(Locks->new, title => "shared");
