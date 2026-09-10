use Rakugan;

class App {
    use Rakugan;
    field $n = 0;
    field $note = "";

    method go {
        OUTER: for my $i (1 .. 3) {
            for my $j (1 .. 3) {
                next OUTER if $j == 2;
                $n += 1;
            }
        }
    }

    method view {
        return button("go", on_click => sub { $self->go });
    }
}

run(App->new, title => "perl");
