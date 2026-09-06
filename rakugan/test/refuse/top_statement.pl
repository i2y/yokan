use Rakugan;

my @greetings = ("hello", "goodbye");

class App {
    use Rakugan;
    method view { return text("hi") }
}

run(App->new, title => "x");
