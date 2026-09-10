use Rakugan;

class App {
    use Rakugan;
    field $n = 0;
    field $half = 0.0;
    field $note = "";

    method halve { $half = 1 / $n }

    method go {
        try { $self->halve } catch ($e) { $note = $e }
    }

    method view {
        return button("go", on_click => sub { $self->go });
    }
}

run(App->new, title => "try");
