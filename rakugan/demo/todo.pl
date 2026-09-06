# A list whose rows are built on demand: the builder is called for the
# rows in view, not for all of them. The row number is the sub's own
# argument, so the line, the marker and that row's button all read it.
use Rakugan;

class Todo {
    use Rakugan;
    field @items = ("milk");
    field $draft = "";
    field $done  = -1;

    method add :Sig(Str) ($t) {
        push @items, $t;
        $draft = "";
    }

    method line :Sig(Int) ($i) {
        my @cells = (text("@{[ $i + 1 ]}. $items[$i]"));
        push @cells, text("done", color => "accent") if $i == $done;
        push @cells, button("done", on_click => sub { $done = $i });
        return row(@cells, spacing => 8);
    }

    method view {
        return column(
            text("todo — @{[ scalar @items ]} items", size => 16),
            text_field($draft, placeholder => "add and press enter",
                       on_change => sub ($t) { $draft = $t },
                       on_submit => sub ($t) { $self->add($t) }),
            list_view(scalar @items, sub ($i) { $self->line($i) },
                      item_height => 26, height => 280),
            button("clear", on_click => sub { @items = () }),
            spacing => 10,
            padding => 14,
        );
    }
}

run(Todo->new, title => "todo");
