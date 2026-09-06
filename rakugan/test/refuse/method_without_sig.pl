use Rakugan;

class App {
    use Rakugan;
    field $n = 0;

    method add ($by) {
        $n += $by;
    }

    method view {
        return button("go", on_click => sub { $self->add(2) });
    }
}

run(App->new, title => "x");
