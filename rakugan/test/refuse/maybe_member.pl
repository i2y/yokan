use Rakugan;

class Node {
    use Rakugan;
    field $label :param :reader = "n";
    method rename :Sig(Str) ($l) { $label = $l }
}

class App {
    use Rakugan;
    field $root = maybe(Node);
    field $note = "";

    method go { $note = $root->label }

    method view {
        return button("go", on_click => sub { $self->go });
    }
}

run(App->new, title => "maybe");
