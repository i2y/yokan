use Rakugan;

class App {
    use Rakugan;
    field $n = 0;

    method view {
        return button("go", on_click => sub ($x) { $n += 1 });
    }
}

run(App->new, title => "x");
