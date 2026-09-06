# Text as a pill: a background with padding and a radius. And the rest
# of what a run of text can be — monospace, underlined, italic, clipped
# with an ellipsis, or wrapped and then clamped. A bag of keywords an
# app writes once is a hash at the top of the file.
use Rakugan;

my %PILL      = (size => 11, color => "#11111b", padding => 4, border_radius => 10);
my %PILL_OK   = (%PILL, background => "#2fa84f");
my %PILL_WARN = (%PILL, background => "#fab387");
my %PILL_CRIT = (%PILL, background => "#f38ba8");

class Badges {
    use Rakugan;
    field $tint = "#45475a";
    field $hot  = false;

    method flip {
        $hot = !$hot;
        $tint = $hot ? "#f38ba8" : "#45475a";
    }

    method view {
        column(
            text("Badges", size => 20, bold => true),
            row(
                text("● OK", %PILL_OK),
                text("● WARN", %PILL_WARN),
                text("● CRIT", %PILL_CRIT),
                text("● BUILD", size => 11, color => "#cdd6f4", background => $tint,
                     padding => 4, border_radius => 10, border_width => 1,
                     border_color => "#585b70"),
                spacing => 6,
            ),
            button("flip", on_click => sub { $self->flip }),
            text("commit 9f2c1ab8e04d", mono => true, size => 12),
            text("an underlined note", underline => true),
            text("in italics, for contrast", italic => true),
            # An ellipsis needs a bounded box to clip against.
            text("a single line far too long for the box it was given, so it ends in an ellipsis",
                 wrap => "ellipsis", width => 260),
            # The clamp is the other half: this one wraps, then stops.
            text("a paragraph that wraps at the window's width and then stops after two lines, "
                 . "because a clamped label is what a card summary wants",
                 max_lines => 2, width => 260),
            spacing => 8,
            padding => 12,
        );
    }
}

run(Badges->new, title => "badges");
