use Rakugan;

class App {
    use Rakugan;
    field @items = (1, "two");

    method view { return text("hi") }
}

run(App->new, title => "x");
