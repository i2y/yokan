use Rakugan;

class App {
    use Rakugan;
    field $sel = maybe(Int);
    field $note = "";

    method go { $note = "sel=$sel" }

    method view {
        return button("go", on_click => sub { $self->go });
    }
}

run(App->new, title => "maybe");
