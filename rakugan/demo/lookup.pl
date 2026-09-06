# A hash on the app: reading with a fallback, asking whether a key is
# there, and adding one while the window is open.
use Rakugan;

class Lookup {
    use Rakugan;
    field %prices = (apple => 120, banana => 80);
    field $picked = 0;
    field $label  = "none";

    method pick_apple {
        $picked = $prices{"apple"} // -1;
        $label = exists $prices{"cherry"} ? "cherry known" : "no cherry";
    }

    method add_cherry {
        $prices{"cherry"} = 200;
        $picked = $prices{"cherry"} // -1;
        $label = "cherry known" if exists $prices{"cherry"};
    }

    method view {
        return column(
            text("picked=$picked n=@{[ scalar keys %prices ]} $label"),
            text("apple costs @{[ $prices{'apple'} // -1 ]} right now", size => 12),
            row(
                button("apple",  on_click => sub { $self->pick_apple }),
                button("cherry", on_click => sub { $self->add_cherry }),
                button("miss",   on_click => sub { $picked = $prices{"durian"} // -7 }),
                spacing => 6,
            ),
            spacing => 8,
            padding => 12,
        );
    }
}

run(Lookup->new, title => "lookup");
