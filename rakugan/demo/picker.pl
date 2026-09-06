# The platform's own panels, and a file dragged onto the window. A
# dialog waits for a person, so it is asked for off the window's
# thread; a script answers one with `file:<path>` and drops one with
# `drop:<path>`.
use Rakugan;

class Picker {
    use Rakugan;
    field $chosen = "(nothing yet)";
    field $body   = "";
    field $saved  = "(not saved)";

    method took :Sig(Str) ($path) {
        $chosen = $path;
        $body = fs_read_text_or($path, "(unreadable)") if $path ne "";
    }

    method open_one {
        task(sub { fs_open_dialog("Choose a file") },
             on_done => sub ($path) { $self->took($path) });
    }

    method save_as {
        task(sub { fs_save_dialog("notes.txt") },
             on_done => sub ($path) {
                 if ($path ne "") {
                     fs_write_text($path, $body);
                     $saved = $path;
                 }
             });
    }

    method view {
        return column(
            text("chosen: $chosen"),
            text("first line: @{[ substr($body, 0, 40) ]}"),
            text("saved to: $saved"),
            row(
                button("open…", tooltip => "the platform's own panel",
                       on_click => sub { $self->open_one }),
                button("save as…", on_click => sub { $self->save_as }),
                spacing => 6,
            ),
            spacing => 8,
            padding => 12,
        );
    }
}

my $app = Picker->new;
on_file_drop(sub ($path) { $app->took($path) });
run($app, title => "picker");
