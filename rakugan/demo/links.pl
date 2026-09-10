# Objects that point at one another. A perl object is a reference, and
# two names can hold the same one; the compiled run keeps that. The
# pointer back is weakened, as perl itself asks, so a parent and a
# child do not keep each other alive: cut the owning chain and the
# survivor's pointer back answers nothing, in both runs.
use Rakugan;

class Node {
    use Rakugan;
    field $label  :param :reader = "n";
    field $kid    :reader :writer = maybe(Node);
    field $parent :reader = maybe(Node);

    method hang_under :Sig(Node) ($p) {
        $parent = $p;
        weaken($parent);
    }
}

class Tree {
    use Rakugan;
    field $root = maybe(Node);
    field $keep = maybe(Node);
    field $note = "-";

    method build {
        my $a = Node->new(label => "alpha");
        my $b = Node->new(label => "beta");
        $a->set_kid($b);
        $b->hang_under($a);
        $root = $a;
        $keep = $b;
    }

    method peek {
        if (defined $root) {
            if (defined $root->kid) {
                if (defined $root->kid->parent) {
                    $note = "kid=@{[ $root->kid->label ]} parent=@{[ $root->kid->parent->label ]}";
                } else {
                    $note = "kid=@{[ $root->kid->label ]} parent=gone";
                }
            } else {
                $note = "no kid";
            }
        } elsif (defined $keep) {
            if (defined $keep->parent) {
                $note = "kept @{[ $keep->label ]}, parent=@{[ $keep->parent->label ]}";
            } else {
                $note = "kept @{[ $keep->label ]}, parent=gone";
            }
        } else {
            $note = "no root";
        }
    }

    method view {
        my @cells = (text("note: $note"));
        if (defined $root) {
            push @cells, text("root: @{[ $root->label ]}");
        } else {
            push @cells, text("root: (none)");
        }
        return column(
            @cells,
            row(
                button("build", on_click => sub { $self->build }),
                button("peek",  on_click => sub { $self->peek }),
                button("drop",  on_click => sub { $root = undef }),
                spacing => 6,
            ),
            spacing => 8,
            padding => 12,
        );
    }
}

run(Tree->new, title => "links");
