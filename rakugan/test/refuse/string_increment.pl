use Rakugan;

class App {
    use Rakugan;
    field $tag = "az";

    method bump {
        $tag++;
    }

    method view {
        return button("bump", on_click => sub { $self->bump });
    }
}

run(App->new, title => "x");
