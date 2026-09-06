use Rakugan;

class App {
    use Rakugan;

    method view {
        return text("hello", size => "large");
    }
}

run(App->new, title => "x");
