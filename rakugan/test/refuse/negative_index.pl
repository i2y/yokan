use Rakugan;

class App {
    use Rakugan;
    field @items = ("milk", "eggs");
    field $at    = 0;

    method view {
        return text("at: $items[$at]");
    }
}

run(App->new, title => "x");
