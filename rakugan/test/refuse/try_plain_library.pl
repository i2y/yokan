use Rakugan;

class App {
    use Rakugan;
    field $note = "";

    method go {
        try { fs_write_text("/nonexistent/dir/x.txt", "a") } catch ($e) { $note = $e }
    }

    method view {
        return button("go", on_click => sub { $self->go });
    }
}

run(App->new, title => "try");
