use Rakugan;

class Node {
    use Rakugan;
    field $n :param :reader = 0;
    method grow { $n += 1 }
}

class App {
    use Rakugan;
    field $node = Node->new(n => 3);

    method go { $node->grow }

    method view {
        return button("go", on_click => sub { $self->go });
    }
}

run(App->new, title => "maybe");
