use Rakugan;

class App {
    use Rakugan;
    field $status = "idle";
    field $answer = 0;

    method start {
        task(sub { 41 + 1 }, on_done => sub ($v) { $answer = $v });
        $status = "working";
    }

    method view {
        return button("start", on_click => sub { $self->start });
    }
}

run(App->new, title => "task");
