use Rakugan;

class App {
    use Rakugan;

    method view {
        return text("hello", weight => 700);
    }
}

run(App->new, title => "x");
