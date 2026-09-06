use Rakugan;

class App {
    use Rakugan;
    field $n :param = 0;

    method view { return text("n=$n") }
}

run(App->new, title => "x");
