use Rakugan;
use lib 'test/refuse/lib';
use My::Counter;

class App {
    use Rakugan;
    field $n = 0;
    field $note = "";

    method go {
        $n = My::Counter::next();
    }

    method view {
        return button("go", on_click => sub { $self->go });
    }
}

run(App->new, title => "perl");
