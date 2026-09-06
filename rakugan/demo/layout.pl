# spacer and divider: a filler and a rule. The header row's spacer
# pushes "ping" to the far edge; the footer's does the same for the
# count. divider draws the rules, the second one heavier and colored.
use Rakugan;

class Layout {
    use Rakugan;
    field $pings = 0;

    method view {
        column(
            row(
                text("Layout", size => 18),
                spacer(),
                button("ping", on_click => sub { $pings += 1 }),
            ),
            divider(),
            column(
                text("Section one", size => 14),
                text("spacer() takes the slack a row leaves behind."),
                divider(thickness => 2, color => "accent"),
                text("Section two", size => 14),
                text("divider() draws a rule across its parent."),
                spacing => 6,
            ),
            row(
                spacer(),
                text("pings: $pings"),
            ),
            spacing => 12,
            padding => 16,
        );
    }
}

run(Layout->new, title => "layout");
