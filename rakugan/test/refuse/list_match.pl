use Rakugan;

class App {
    use Rakugan;
    field $n = 0;
    field $note = "";

    method go {
        if (my ($p, $q) = "3-4" =~ /(\d+)-(\d+)/) { $n = 1 }
    }

    method view {
        return button("go", on_click => sub { $self->go });
    }
}

run(App->new, title => "perl");
