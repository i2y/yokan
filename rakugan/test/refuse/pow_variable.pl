use Rakugan;

class App {
    use Rakugan;
    field $n = 0;
    field $note = "";

    method go {
        $n = 2 ** $n;
    }

    method view {
        return button("go", on_click => sub { $self->go });
    }
}

run(App->new, title => "perl");
