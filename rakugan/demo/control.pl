# Ordinary Perl inside a view: `if`, `unless`, a conditional
# expression, a loop, and a method that answers part of the screen.
# The parts go into a list, and the list is what the container holds.
use Rakugan;

class Control {
    use Rakugan;
    field @items     = ("milk", "eggs", "rice");
    field $picked    = -1;
    field $show_hint = true;
    field $tab       = 0;

    method hint {
        return text("pick one", size => 12, color => "#8a8f98");
    }

    method line :Sig(Str, Int) ($name, $i) {
        return row(
            text($i == $picked ? "▸ $name" : "  $name"),
            button("pick $i", on_click => sub { $picked = $i }),
            spacing => 8,
        );
    }

    method tab_button :Sig(Int) ($n) {
        return button("tab $n", on_click => sub { $tab = $n });
    }

    method view {
        my @cells = (text("control flow", size => 18, bold => true));
        if ($show_hint) {
            push @cells, $self->hint;
        } else {
            push @cells, text("hidden", size => 12);
        }
        for my $i (0 .. $#items) {
            push @cells, $self->line($items[$i], $i);
        }
        push @cells, text("picked $items[$picked]", color => $picked % 2 == 0 ? "accent" : "#f38ba8")
            unless $picked < 0;
        push @cells, row(
            button("hint", on_click => sub { $show_hint = !$show_hint }),
            (map { $self->tab_button($_) } 0 .. 2),
            spacing => 6,
        );
        push @cells, text("tab $tab");
        return column(@cells, spacing => 10, padding => 14);
    }
}

run(Control->new, title => "control");
