use Rakugan;

class App {
    use Rakugan;
    field %prices = (apple => 120);
    field $picked = 0;

    method pick {
        $picked = $prices{"apple"};
    }

    method view {
        return button("pick", on_click => sub { $self->pick });
    }
}

run(App->new, title => "x");
