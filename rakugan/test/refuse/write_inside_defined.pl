use Rakugan;

class App {
    use Rakugan;
    field $sel = maybe(Int);
    field $n = 0;

    method go {
        if (defined $sel) {
            $n = $sel;
            $sel = undef;
        }
    }

    method view {
        return button("go", on_click => sub { $self->go });
    }
}

run(App->new, title => "maybe");
