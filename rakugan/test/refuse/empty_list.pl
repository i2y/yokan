use Rakugan;

class App {
    use Rakugan;
    field @items = ();

    method view { return text("hi") }
}

run(App->new, title => "x");
