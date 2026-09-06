use Rakugan;

my $greeting = "hello";

class App {
    use Rakugan;
    method view { return text("hi") }
}

run(App->new, title => "x");
