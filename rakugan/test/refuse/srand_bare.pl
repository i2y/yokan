use Rakugan;

class App {
    use Rakugan;
    field $n = 0;

    method go {
        srand();
        $n = int(rand(10));
    }

    method view {
        return button("go", on_click => sub { $self->go });
    }
}

run(App->new, title => "random");
