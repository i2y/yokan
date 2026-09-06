use Rakugan;

class App {
    use Rakugan;
    field $n = 0;

    method bumped :Sig(=> Int) {
        $n += 1;
        return $n;
    }

    method view {
        return text("n=@{[ $self->bumped ]}");
    }
}

run(App->new, title => "x");
