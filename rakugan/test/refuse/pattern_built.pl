use Rakugan;

class App {
    use Rakugan;
    field $needle = "a";
    field $found  = false;

    method look {
        $found = "abc" =~ /$needle/;
    }

    method view {
        return button("look", on_click => sub { $self->look });
    }
}

run(App->new, title => "x");
