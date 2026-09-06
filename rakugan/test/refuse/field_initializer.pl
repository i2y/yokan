use Rakugan;

class App {
    use Rakugan;
    field $n;

    method view { return text("hi") }
}

run(App->new, title => "x");
