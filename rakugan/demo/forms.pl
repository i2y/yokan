# The controls a person changes: a box, a switch, a track, and the four
# choosers. Each hands its new value to the sub written on it.
use Rakugan;

class Settings {
    use Rakugan;
    field $dark   = false;
    field $wifi   = true;
    field $volume = 5.0;
    field @fruits = ("apple", "banana", "cherry");
    field $fruit  = 0;
    field @sizes  = ("small", "medium", "large");
    field $size   = 1;
    field @tabs   = ("General", "Details", "About");
    field $tab    = 0;
    field $note   = "";

    method panel {
        return text("general panel", size => 12) if $tab == 0;
        return text("details panel", size => 12) if $tab == 1;
        return text("about panel", size => 12);
    }

    method view {
        return column(
            checkbox("Dark mode", checked => $dark,
                     tooltip => "the whole window follows this",
                     on_change => sub ($on) { $dark = $on }),
            switch("Wi-Fi", checked => $wifi, on_change => sub ($on) { $wifi = $on }),
            slider(value => $volume, min => 0, max => 10, step => 1,
                   tooltip => "0 to 10, in whole steps",
                   on_change => sub ($v) { $volume = $v }),
            select(options => \@fruits, selected => $fruit,
                   on_change => sub ($i) { $fruit = $i }),
            radio_group(options => \@sizes, selected => $size,
                        on_change => sub ($i) { $size = $i }),
            tab_bar(labels => \@tabs, active => $tab,
                    on_change => sub ($i) { $tab = $i }),
            $self->panel,
            text_field($note, placeholder => "notes (enter writes a newline)",
                       multiline => true, rows => 3,
                       on_change => sub ($t) { $note = $t }),
            text("dark=$dark  wifi=$wifi  vol=@{[ sprintf('%.1f', $volume) ]}"),
            text("fruit#$fruit  size#$size  tab#$tab"),
            spacing => 10,
            padding => 14,
        );
    }
}

run(Settings->new, title => "forms", width => 460, height => 420);
