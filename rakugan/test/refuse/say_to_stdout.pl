use Rakugan;

class App {
    use Rakugan;
    field $n = 0;

    method note {
        say "n is $n";
    }

    method view {
        return button("note", on_click => sub { $self->note });
    }
}

run(App->new, title => "x");
