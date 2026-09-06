# data_table draws the table itself: the first row inside it is the
# header, every later row is a data row shaded in alternation, and the
# frame comes with the element. Columns line up because the cells of
# one column carry the same `grow` share.
use Rakugan;

class Fleet {
    use Rakugan;
    field %latency = (api => 42, db => 17, cache => 8, edge => 95);
    field $polls   = 0;

    method refresh {
        $polls += 1;
        $latency{"api"}   = (($latency{"api"}   // 0) * 3 + 29) % 140;
        $latency{"db"}    = (($latency{"db"}    // 0) * 5 + 11) % 140;
        $latency{"cache"} = (($latency{"cache"} // 0) * 7 + 3)  % 140;
        $latency{"edge"}  = (($latency{"edge"}  // 0) * 2 + 47) % 140;
    }

    method health :Sig(Int => Str) ($ms) {
        my $label = "ok";
        $label = "watch" if $ms > 60;
        $label = "slow" if $ms > 100;
        return $label;
    }

    method service_row :Sig(Str) ($name) {
        return row(
            text($name, grow => 2),
            text("@{[ $latency{$name} // 0 ]} ms", grow => 1, align => "right"),
            text($self->health($latency{$name} // 0), grow => 1, align => "center"),
            spacing => 8,
        );
    }

    method view {
        return column(
            text("fleet latency — $polls polls", size => 16),
            data_table(
                row(
                    text("service", grow => 2),
                    text("latency", grow => 1, align => "right"),
                    text("health",  grow => 1, align => "center"),
                    spacing => 8,
                ),
                $self->service_row("api"),
                $self->service_row("db"),
                $self->service_row("cache"),
                $self->service_row("edge"),
            ),
            button("refresh", on_click => sub { $self->refresh }),
            spacing => 10,
            padding => 14,
        );
    }
}

run(Fleet->new, title => "table");
