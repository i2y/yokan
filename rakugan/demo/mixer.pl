# State an app keeps and a field that writes into it. The parts of the
# screen go into a list, and one of them is only there some of the time.
use Rakugan;

class Mixer {
    use Rakugan;
    field $volume = 5;
    field $title  = "untitled";
    field $muted  = false;

    method view {
        my @cells = (
            text("$title — vol $volume", size => 16),
            row(
                button("+1",     on_click => sub { $volume += 1 }),
                button("mute",   on_click => sub { $muted = true }),
                button("unmute", on_click => sub { $muted = false }),
                spacing => 8,
            ),
        );
        push @cells, text("(muted)", size => 12, color => "#8a8f98") if $muted;
        push @cells, text_field($title, placeholder => "title", on_change => sub ($t) { $title = $t });
        return column(@cells, spacing => 10, padding => 14);
    }
}

run(Mixer->new, title => "mixer");
