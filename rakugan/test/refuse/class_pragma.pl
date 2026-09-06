use Rakugan;

class App {
    field $n = 0;
    method view { return text("hi") }
}

run(App->new, title => "x");
