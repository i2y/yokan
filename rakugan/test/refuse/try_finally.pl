use Rakugan;

class App {
    use Rakugan;
    field $n = 0;
    field $note = "";

    method go {
        try { $n = 1 } catch ($e) { $note = $e } finally { $n = 2 }
    }

    method view {
        return button("go", on_click => sub { $self->go });
    }
}

run(App->new, title => "try");
