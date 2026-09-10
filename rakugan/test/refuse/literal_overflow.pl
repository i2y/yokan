use Rakugan;

class App {
    use Rakugan;
    field $n = 0;

    method go {
        $n = 4611686018427387904 * 4;
    }

    method view {
        return button("go", on_click => sub { $self->go });
    }
}

run(App->new, title => "overflow");
