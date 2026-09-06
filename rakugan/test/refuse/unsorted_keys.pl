use Rakugan;

class App {
    use Rakugan;
    field %prices = (apple => 120);
    field $total  = 0;

    method sum {
        $total = 0;
        for my $k (keys %prices) {
            $total += $prices{$k} // 0;
        }
    }

    method view {
        return button("sum", on_click => sub { $self->sum });
    }
}

run(App->new, title => "x");
