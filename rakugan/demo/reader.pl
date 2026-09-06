# A document read and shown: nested JSON, reached by path.
#
# Wakakusa's copy of this demo serves the document to itself over a
# socket. A shipped Rakugan app has no perl in it, so it has no way to
# run a server written in Perl — the document is written to a file and
# read back instead, which is the same claim about the same JSON reader
# and one the gate can make without a network.
use Rakugan;

class Reader {
    use Rakugan;
    field $path   = "demo/.gate/feed.json";
    field $status = "idle";
    field @titles = empty(Str);
    field $top    = "-";

    method write_feed {
        fs_write_text($path,
            '{"items": ['
            . '{"title": "rakugan ships native perl apps", "points": 128},'
            . '{"title": "one engine, three doors", "points": 64},'
            . '{"title": "the gate arbitrates", "points": 256}'
            . ']}');
    }

    method read_feed {
        $self->write_feed;
        my $src = fs_read_text($path);
        my $n = jsondoc_length($src, "items");
        @titles = ();
        my $best = -1;
        my $name = "-";
        for my $i (0 .. $n - 1) {
            my $title = jsondoc_get_text($src, "items.$i.title");
            my $points = jsondoc_get_int($src, "items.$i.points");
            push @titles, $title;
            if ($points > $best) {
                $best = $points;
                $name = $title;
            }
        }
        $top = "$name ($best)";
        $status = "read $n items";
    }

    method line :Sig(Int) ($i) {
        return text($titles[$i], size => 13);
    }

    method view {
        return column(
            text("reader", size => 18, bold => true),
            text("status: $status", size => 12, color => "#8a8f98"),
            text("top: $top"),
            list_view(scalar @titles, sub ($i) { $self->line($i) },
                      item_height => 22, height => 80),
            button("fetch", on_click => sub { $self->read_feed }),
            spacing => 8,
            padding => 12,
        );
    }
}

run(Reader->new, title => "reader");
