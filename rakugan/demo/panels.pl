# The elements that arrange or cover: tracks, layers, panes that
# scroll, and a panel over the rest of the window. A method that
# answers an element is a piece of the screen with a name.
use Rakugan;

class Panels {
    use Rakugan;
    field $open  = false;
    field $shown = 0;
    field @views = ("grid", "stack", "scrolls");

    method tracks {
        return grid(
            text("one"), text("two"),
            grid_cell(text("across both", align => "center", background => "#313244",
                           padding => 4, border_radius => 6), col_span => 2),
            text("three"), text("four"),
            columns => 2, spacing => 6,
        );
    }

    method layers {
        return stack(
            image("demo/assets/postcard.png", width => 180, height => 90),
            text("over the picture", size => 14, color => "#11111b",
                 background => "#f9e2af", padding => 4),
        );
    }

    method scrolls {
        return column(
            scroll_view(
                column((map { text("line $_") } 1 .. 12), spacing => 2),
                height => 90,
            ),
            h_scroll_view(
                row((map { text("col $_", width => 70) } 1 .. 10), spacing => 6),
            ),
            spacing => 8,
        );
    }

    method panel {
        return $self->tracks if $shown == 0;
        return $self->layers if $shown == 1;
        return $self->scrolls;
    }

    method view {
        return stack(
            column(
                row(
                    text("Panels", size => 18),
                    spacer(),
                    link("pixie", "https://example.invalid", size => 12),
                    spinner(size => 14),
                    spacing => 8,
                ),
                segmented(options => \@views, selected => $shown,
                          on_change => sub ($i) { $shown = $i }),
                $self->panel,
                button("about", on_click => sub { $open = true }),
                spacing => 10,
                padding => 14,
            ),
            modal(
                column(
                    text("A panel over the rest of it.", size => 14),
                    button("close", on_click => sub { $open = false }),
                    spacing => 8, padding => 12, background => "panel",
                ),
                open => $open,
            ),
        );
    }
}

run(Panels->new, title => "panels");
