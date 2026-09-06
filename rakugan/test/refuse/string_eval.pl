use Rakugan;

class App {
    use Rakugan;
    field $n = 0;

    method risky {
        $n = eval "1 + 1";
    }

    method view {
        return button("go", on_click => sub { $self->risky });
    }
}

run(App->new, title => "x");
