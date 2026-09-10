use Rakugan;
use constant LIMIT => 12;

class App {
    use Rakugan;
    use constant LIMIT => 10;
    field $n = 0;

    method go { $n = LIMIT }

    method view {
        return button("go", on_click => sub { $self->go });
    }
}

run(App->new, title => "maybe");
