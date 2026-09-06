use Rakugan;

class App {
    use Rakugan;
    field $found = false;

    method look {
        $found = "abc" =~ /a(?{ print "hi" })b/;
    }

    method view {
        return button("look", on_click => sub { $self->look });
    }
}

run(App->new, title => "x");
