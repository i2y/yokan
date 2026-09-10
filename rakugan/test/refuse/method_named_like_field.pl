use Rakugan;

class Node {
    use Rakugan;
    field $n :reader = 0;
    method set_n :Sig(Int) ($v) { $n = $v }
    method grow { $n += 1 }
}

class App {
    use Rakugan;
    field $node = Node->new;

    method go { $node->grow }

    method view {
        return button("go", on_click => sub { $self->go });
    }
}

run(App->new, title => "maybe");
