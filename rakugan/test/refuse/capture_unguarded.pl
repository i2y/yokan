use Rakugan;

class App {
    use Rakugan;
    field $line = "a1";
    field $got  = "-";

    method look {
        my $ok = $line =~ /(\d+)/;
        $got = $1;
    }

    method view {
        return button("look", on_click => sub { $self->look });
    }
}

run(App->new, title => "x");
