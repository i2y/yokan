use Rakugan;

class App {
    use Rakugan;
    field $n = 0;

    method s { $n += 1 }

    method view { return text("hi") }
}

run(App->new, title => "x");
