use Rakugan;

class App {
    use Rakugan;
    field $n = 0;

    method go { srand(42) }

    method view {
        return column(text("roll: @{[ int(rand(6)) ]}"), button("go", on_click => sub { $self->go }));
    }
}

run(App->new, title => "random");
