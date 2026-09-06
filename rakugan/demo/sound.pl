# Sound. A WAV file is played and the call answers at once, so a handler
# that starts one carries on.
#
# A run under a script is silent: a gate must not need a machine with
# speakers, and both runs read that one flag through the same library,
# so neither is louder than the other. That is why this demo can be
# gated at all — the screen is what the two runs compare.
use Rakugan;

class Sound {
    use Rakugan;
    field $played = 0;
    field $last   = "-";
    field $volume = 0.6;

    method play :Sig(Str) ($name) {
        audio_play("demo/assets/sound/$name.wav", $volume);
        $played += 1;
        $last = $name;
    }

    method hush {
        audio_stop();
        $last = "stopped";
    }

    method view {
        return column(
            text("sound", size => 18, bold => true),
            text("played: $played   last: $last"),
            row(
                button("jump",   on_click => sub { $self->play("jump") }),
                button("pickup", on_click => sub { $self->play("pickup") }),
                button("blast",  on_click => sub { $self->play("blast") }),
                spacing => 6,
            ),
            row(
                button("shoot", on_click => sub { $self->play("shoot") }),
                button("over",  on_click => sub { $self->play("over") }),
                button("stop",  on_click => sub { $self->hush }),
                spacing => 6,
            ),
            slider(value => $volume, min => 0, max => 1, step => 0.1,
                   on_change => sub ($v) { $volume = $v }),
            text("volume @{[ sprintf('%.1f', $volume) ]}", size => 12, color => "#8a8f98"),
            spacing => 10,
            padding => 14,
        );
    }
}

run(Sound->new, title => "sound");
