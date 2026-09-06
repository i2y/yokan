# A look kept in one place: a hash of keywords an app writes once and
# hands to an element. `theme =>` swaps the palette its subtree resolves
# colors in, so one keyword flips the whole panel.
use Rakugan;

my %CHIP    = (size => 18, color => "accent");
my %KEY     = (background => "#313244", hover_background => "#45475a");
my %KEY_HOT = (%KEY, background => "#fab387");

class Styled {
    use Rakugan;
    field $mode = "dark";
    field $n    = 0;

    method flip {
        $mode = $mode eq "dark" ? "light" : "dark";
    }

    method view {
        column(
            text("n=$n", %CHIP),
            row(
                button("+1", %KEY, on_click => sub { $n += 1 }),
                button("flip", %KEY_HOT, on_click => sub { $self->flip }),
                spacing => 6,
            ),
            spacing => 8, padding => 12, background => "panel", theme => $mode,
        );
    }
}

run(Styled->new, title => "styled");
