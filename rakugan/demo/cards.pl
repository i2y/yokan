# A piece of screen with a name is a method that answers an element,
# and one that wraps other elements takes them after its own values.
use Rakugan;

class Cards {
    use Rakugan;
    field $a = 0;
    field $b = 0;

    method card :Sig(Str) ($title, @kids) {
        return column(
            text($title, size => 18),
            @kids,
            spacing => 4, padding => 8,
            border_width => 1, border_color => "accent", border_radius => 8,
        );
    }

    method view {
        return column(
            $self->card("counters",
                row(text("a: $a"),  button("+1",  on_click => sub { $a += 1 }),  spacing => 6),
                row(text("b: $b"),  button("+10", on_click => sub { $b += 10 }), spacing => 6),
            ),
            text("outside the card", size => 12),
            spacing => 10,
            padding => 16,
        );
    }
}

run(Cards->new, title => "cards");
