use Rakugan;

class App {
    use Rakugan;
    field $n = 0;
    field $total = 0.0;
    field $note = "";
    field @xs = (1, 2, 3);

    method go {
        try {
            for my $x (@xs) { $total += $x / $n }
        } catch ($e) {
            $note = $e;
        }
    }

    method view {
        return button("go", on_click => sub { $self->go });
    }
}

run(App->new, title => "try");
