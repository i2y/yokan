use Rakugan;

class App {
    use Rakugan;
    field $line = "a1";

    method fix {
        $line =~ s/(\d+)/$1 + 1/e;
    }

    method view {
        return button("fix", on_click => sub { $self->fix });
    }
}

run(App->new, title => "x");
