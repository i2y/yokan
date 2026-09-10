use Rakugan;

class App {
    use Rakugan;
    field $n = 0;
    field $note = "";

    method go {
        state $calls = 0;
        $calls += 1;
        $n = $calls;
    }

    method view {
        return button("go", on_click => sub { $self->go });
    }
}

run(App->new, title => "perl");
