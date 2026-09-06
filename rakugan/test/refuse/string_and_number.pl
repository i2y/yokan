use Rakugan;

class App {
    use Rakugan;
    field $n = 0;

    method bump {
        $n = "10" + 5;
    }

    method view {
        return button("go", on_click => sub { $self->bump });
    }
}

run(App->new, title => "x");
