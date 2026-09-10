<!-- Written by website/tools/tour_pages.pl from the tour. Edit the tour. -->
# Views and control flow

How a screen is built, broken into pieces, and driven by what the app holds.

## Writing views

A container takes its children as arguments, and the keywords come
after them. A method that answers an element is a piece of a screen,
and calling it is how a view is broken up.

<!-- script: click:+1,dump -->
```perl
use Rakugan;

class Two {
    use Rakugan;
    field $count = 0;
    field $name  = "Ada";

    method field_line :Sig(Str, Str) ($label, $value) {
        return row(
            text($label, width => 90),
            text($value, bold => true),
            spacing => 6,
        );
    }

    method view {
        return column(
            text("count: $count", size => 34),
            $self->field_line("name", $name),
            $self->field_line("count", "$count"),
            row(
                button("+1",    on_click => sub { $count += 1 }),
                button("reset", on_click => sub { $count = 0 }),
                spacing => 8,
            ),
            spacing => 12,
            padding => 16,
        );
    }
}

run(Two->new, title => "two");
```

A method called from a view only reads. It may take the app's fields
and its own parameters, and it may not change anything: building a
screen twice has to build the same screen. A method that writes a field
is a handler's, and the refusal that names it says so.


## Control flow in a view

Inside a view, Perl is Perl. `if`, `unless`, a conditional expression,
a `for`, a `my`, a method call — all of it works. The parts go into a
list, and the list is what the container holds.

<!-- script: dump,click:hint,dump,click:pick 1,dump -->
```perl
use Rakugan;

class Control {
    use Rakugan;
    field @items     = ("milk", "eggs", "rice");
    field $picked    = -1;
    field $show_hint = true;

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
        push @cells, text("picked $items[$picked]")
            unless $picked < 0;
        push @cells, button("hint", on_click => sub { $show_hint = !$show_hint });
        return column(@cells, spacing => 10, padding => 14);
    }
}

run(Control->new, title => "control");
```

Two things about that loop are worth naming. An element's own handler
written inside it would close over `$i`, and `line` is a method for
that reason: it takes the number it needs. And a condition is a `Bool`
— Perl's truthiness of a number or a string is not in the dialect, so
`if ($picked)` is written `if ($picked >= 0)`.


## Form controls

```perl
    text_field($name, placeholder => "name", on_change => sub ($s) { $name = $s });
    int_field($qty, min => 0, max => 99, on_change => sub ($n) { $qty = $n });
    number_field($rate, min => 0, max => 1, step => 0.05,
                 on_change => sub ($v) { $rate = $v });
    checkbox("ready", checked => $ready, on_change => sub ($on) { $ready = $on });
    switch("dark", checked => $dark, on_change => sub ($on) { $dark = $on });
    slider(value => $vol, min => 0, max => 10, on_change => sub ($v) { $vol = $v });
    select(options => \@colors, selected => $pick, on_change => sub ($i) { $pick = $i });
    radio_group(options => \@sizes, selected => $size, on_change => sub ($i) { $size = $i });
    segmented(options => ["day", "week"], selected => $span, on_change => sub ($i) { $span = $i });
    tab_bar(labels => \@tabs, active => $tab, on_change => sub ($i) { $tab = $i });
```

Each of them takes the value it shows, and the sub receives what
changed. Nothing is bound behind your back: the field shows `$name`
because you wrote `$name`, and the sub is what puts it back. A list of
choices is passed as a reference (`\@colors`) or written out
(`["day", "week"]`).


## Handlers

A handler is an anonymous sub written on the keyword the element names
for it. It takes exactly what the event carries: nothing for a button,
one value for anything that changed.

```perl
    button("save", on_click => sub { $self->save });
    text_field($draft, on_change => sub ($s) { $draft = $s },
               on_submit  => sub ($s) { $self->add($s) });
```

`sub { ... }` and `sub ($v) { ... }` are the two shapes; a handler
given anything else, or given a parameter it is not called with, is
refused with the arity it should have had.


## Lists, charts, and rows built on demand

A list with a hundred thousand rows is not a hundred thousand elements.
`list_view` and `table` take a count and a sub that builds row `$i`,
and only the rows on screen are ever built.

<!-- script: dump,click:add,dump -->
```perl
use Rakugan;

class Ledger {
    use Rakugan;
    field @names  = ("rent", "coffee", "books");
    field @totals = (1200, 4, 36);

    method add {
        push @names, "misc";
        push @totals, 12;
    }

    method entry :Sig(Int) ($i) {
        return row(
            text($names[$i], grow => 2),
            text("$totals[$i]", grow => 1, align => "right"),
            spacing => 8,
        );
    }

    method view {
        return column(
            text("spending", size => 18, bold => true),
            list_view(scalar @names, sub ($i) { $self->entry($i) },
                      item_height => 24, height => 120),
            bar_chart(\@totals, labels => \@names, axis => true, height => 90),
            button("add", on_click => sub { $self->add }),
            spacing => 10,
            padding => 14,
        );
    }
}

run(Ledger->new, title => "ledger");
```

`table` is the same shape with a header and sortable columns, and
`data_table` draws a table you build row by row. The row sub is called
while the screen is being drawn, so it reads state and never writes it,
and the only index it may use is its own: an index a view cannot prove
is inside the list is refused, and worked out in a handler instead.


## Hashes

A hash on the app is read with a fallback, always:

```perl
    field %prices = (apple => 120, banana => 80);

    method pick {
        $picked = $prices{"apple"} // -1;
        $label  = exists $prices{"cherry"} ? "cherry known" : "no cherry";
        $prices{"cherry"} = 200;
    }
```

`$prices{$k}` alone is refused, because the key may not be there and
the two runs would have to agree about what happens then. `// 0` says
what to answer, `exists` asks, and `keys` is written `sort keys %prices`
— perl hands the keys back in the order it happens to hold them, which
is a different order every time perl starts, and a screen cannot depend
on that.


## Values that may be nothing

Perl has `undef`, and an app has uses for it: a selection nobody has
made yet, a parent that is not there. A field that starts as nothing
says what it may hold, `field $sel = maybe(Int);`, and that is where
its type comes from, as with `empty`. `undef` is what a handler writes
to put it back to nothing, and a method that may answer nothing spells
that `:Sig(Int => Maybe[Int])`, the way Types::Standard spells it.

Reading it is where the two runs have to agree, so the read is inside
the question: `if (defined $sel) { … }` is the branch where `$sel` is
the value, and outside that branch it is not read as one. `//` is the
other way, in one expression: `$sel // 0` is the value, or the one after
it. `$sel` alone in text, in arithmetic or as an argument is refused,
because nothing has no text and no sum.

<!-- script: dump,click:pick,dump,click:clear,dump -->
```perl
use Rakugan;

class Choice {
    use Rakugan;
    field $sel  = maybe(Int);
    field $note = "-";

    method pick :Sig(Int => Maybe[Int]) ($v) {
        return undef if $v < 0;
        return $v;
    }

    method choose :Sig(Int) ($v) {
        $sel = $self->pick($v);
        if (defined $sel) {
            $note = "chose $sel";
        } else {
            $note = "nothing to choose";
        }
    }

    method view {
        my @cells = (text("note: $note"), text("or zero: @{[ $sel // 0 ]}"));
        if (defined $sel) {
            push @cells, text("selection: $sel");
        } else {
            push @cells, text("(no selection)");
        }
        return column(
            @cells,
            row(
                button("pick",  on_click => sub { $self->choose(7) }),
                button("clear", on_click => sub { $self->choose(-1) }),
                spacing => 6,
            ),
            spacing => 8,
            padding => 12,
        );
    }
}

run(Choice->new, title => "choice");
```

`unless (defined $sel)` and `if (!defined $sel)` are the same branch
the other way round. `defined` is the whole condition there, not one
side of an `&&`, and inside its branch `$sel` is read and not written.


## Value classes

A second class with no `view` is a value: `:param` says what `new` is
given, `:reader` what can be read back, and the compiled run holds it
as a value rather than as something two names can share.

<!-- script: dump,click:right,dump,click:measure,dump -->
```perl
use Rakugan;

class Point {
    use Rakugan;
    field $x :param :reader = 0;
    field $y :param :reader = 0;
}

class Points {
    use Rakugan;
    field $sel  = Point->new(x => 3, y => 4);
    field $dist = 0;

    method view {
        return column(
            text("p=(@{[ $sel->x ]}, @{[ $sel->y ]}) d2=$dist"),
            row(
                button("right",   on_click => sub { $sel = Point->new(x => $sel->x + 5, y => $sel->y) }),
                button("measure", on_click => sub { $dist = $sel->x * $sel->x + $sel->y * $sel->y }),
                spacing => 6,
            ),
            spacing => 8,
            padding => 12,
        );
    }
}

run(Points->new, title => "points");
```

A value is replaced rather than edited, which is why the buttons above
build a new `Point`. A list of them is `field @seen = empty(Point);`,
and it is what the games carry their stars and their bullets in.


## Classes with methods

A second class with methods is an object rather than a value: two names
can hold the same one, and a change made through either is seen through
both, which is how perl's own objects behave and how the compiled run
holds them. `:param` says what `new` is given, `:reader` what can be
read from outside, and a method reaches the fields by their names.
perl 5.42 adds `:writer`, which gives a field `set_name`, and the
dialect takes it.

<!-- script: click:bump,click:bump,dump -->
```perl
use Rakugan;

class Tally {
    use Rakugan;
    field $count :reader = 0;
    field $label :param :reader = "clicks";

    method bump :Sig(Int) ($by) {
        $count += $by;
    }

    method rename :Sig(Str) ($to) {
        $label = $to;
    }
}

class Board {
    use Rakugan;
    field $tally = Tally->new;
    field $note  = "-";

    method bump {
        $tally->bump(2);
        $tally->rename("clicks so far") if $tally->count > 2;
        $note = "@{[ $tally->label ]}: @{[ $tally->count ]}";
    }

    method view {
        return column(
            text("note: $note"),
            text("count: @{[ $tally->count ]}"),
            button("bump", on_click => sub { $self->bump }),
            spacing => 8,
            padding => 12,
        );
    }
}

run(Board->new, title => "tally");
```

Objects point at one another through fields that may be nothing,
`field $kid = maybe(Node);`, set from a method and read inside
`defined`. A pointer back is weakened, as perl asks, with `weaken($parent)`
after the assignment in the method that makes it, so a parent and a
child do not keep each other alive; the compiled run declares that
field weak and frees the chain at the same statement perl does.
`demo/links.pl` is that shape, and `demo/moods.pl` is a class the app
holds and asks.

A few things the compiled run gives such a class of its own, so the
dialect refuses them: a method named like a field or `set_<field>`
(those are the field's own reader and writer there), `$self` inside
one of its methods, a list or a hash as one of its fields, `ADJUST`,
and `Name->new` with values as a field's initializer; start it with
`Name->new` and fill it in `ADJUST` or a handler.


## Regular expressions

A pattern is written out in the file, and what it caught is read inside
the `if` that made the match:

```perl
    if ($line =~ /^(\w+)\s*=\s*(\d+)$/) {
        $key = $1;
        $val = 0 + $2;
    }
    $count = () = $text =~ /\bfoo\b/g;
    $clean = $line =~ s/\s+/ /gr;
    my @parts = split /,\s*/, $line;
```

The pattern itself is compiled when the app is translated, so both runs
match with one engine. Two things follow from that. A pattern built from
a variable is refused, because a shipped app carries nothing to compile
it with; and `/e`, which runs perl on the replacement, is refused for the
same reason. A third refusal has a reason of its own: `$1` outside the
`if` would be whatever the last successful match anywhere had left, which
is not a thing two runs can be held to.

