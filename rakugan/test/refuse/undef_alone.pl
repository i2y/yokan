use Rakugan;

class App {
    use Rakugan;
    field $sel = undef;

    method go { $sel = 3 }

    method view {
        return button("go", on_click => sub { $self->go });
    }
}

run(App->new, title => "maybe");
