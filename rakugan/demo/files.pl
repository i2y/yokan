# Files, through the framework's own standard library. One
# implementation answers both runs: the compiled one links it through
# pixie's binding door, and the interpreted one reaches the same Rust
# through the engine's C face — so what the gate compares is one
# library answering twice, not two libraries agreeing.
use Rakugan;

class Files {
    use Rakugan;
    field $dir     = "demo/.gate/fs_demo";
    field $note    = "demo/.gate/fs_demo/note.txt";
    field $content = "(not loaded)";
    field $wrote   = 0;
    field @names   = empty(Str);
    field $ready   = false;
    field $size    = 0;
    field $in_dir  = false;
    field $tail    = "";

    method save {
        fs_make_dir($dir);
        $wrote = fs_write_text($note, "hello from one standard library");
    }

    method add_line {
        fs_append_text($note, " (and again)");
    }

    method listing {
        @names = fs_list_dir($dir);
    }

    method clean {
        fs_remove($note) if fs_exists($note);
        $self->listing;
    }

    # A place of the app's own, made on the way out. A demo has no
    # business in someone's home directory, so this one keeps to the
    # directory the gate already writes in.
    method data_dir {
        my $path = "demo/.gate/fs_demo_app";
        fs_make_dir($path);
        $ready = fs_exists($path);
    }

    # What the file is without reading it, and the rest of it after
    # the first write: the follower's read, from where it stopped
    # rather than from the top.
    method measure {
        $size   = fs_size($note);
        $in_dir = fs_is_dir($dir);
        $tail   = fs_read_text_from($note, $wrote);
    }

    method entry :Sig(Int) ($i) {
        return text($names[$i]);
    }

    method view {
        return column(
            text("content: $content"),
            text("wrote: $wrote bytes"),
            text("in $dir: @{[ scalar @names ]} file(s)"),
            list_view(scalar @names, sub ($i) { $self->entry($i) },
                      item_height => 20, height => 44),
            text("data dir ready: $ready"),
            text("size: $size bytes, dir: $in_dir, rest: '$tail'"),
            row(
                button("save",     on_click => sub { $self->save }),
                button("append",   on_click => sub { $self->add_line }),
                button("load",     on_click => sub { $content = fs_read_text($note) }),
                button("list",     on_click => sub { $self->listing }),
                button("data dir", on_click => sub { $self->data_dir }),
                button("remove",   on_click => sub { $self->clean }),
                button("measure",  on_click => sub { $self->measure }),
                spacing => 6,
            ),
            spacing => 8,
            padding => 12,
        );
    }
}

run(Files->new, title => "files");
