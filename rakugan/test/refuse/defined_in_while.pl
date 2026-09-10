use Rakugan;

class App {
    use Rakugan;
    field $sel = maybe(Int);
    field $n = 0;

    method go {
        while (defined $sel) { $n += 1; $sel = undef }
    }

    method view {
        return button("go", on_click => sub { $self->go });
    }
}

run(App->new, title => "maybe");
