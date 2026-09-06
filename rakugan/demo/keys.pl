# The keyboard as a set of chords, and the same handlers in the
# application's menu bar. A script presses one with `key:cmd+s` and
# picks one with `menu:Save`.
use Rakugan;

class Keys {
    use Rakugan;
    field $count  = 0;
    field $saved  = 0;
    field $last   = "-";
    field $pasted = "(nothing)";

    method save {
        $saved = $count;
    }

    method clear {
        $count = 0;
        $saved = 0;
    }

    method copy_count {
        clipboard_set_text("count=$count");
    }

    method paste {
        $pasted = clipboard_get_text();
    }

    method typed :Sig(Str) ($key) {
        $last = $key;
    }

    method view {
        return column(
            text("count: $count  saved: $saved"),
            text("last key: $last"),
            text("pasted: $pasted"),
            row(
                button("+1",    on_click => sub { $count += 1 }),
                button("save",  on_click => sub { $self->save }),
                button("copy",  on_click => sub { $self->copy_count }),
                button("paste", on_click => sub { $self->paste }),
                spacing => 6,
            ),
            spacing => 8,
            padding => 12,
        );
    }
}

my $app = Keys->new;

menu_item("Count", "Save",  sub { $app->save });
menu_item("Count", "Clear", sub { $app->clear });

shortcut("cmd+s",        sub { $app->save });
shortcut("cmd+shift+r",  sub { $app->clear });
shortcut("cmd+shift+c",  sub { $app->copy_count });
shortcut("cmd+shift+v",  sub { $app->paste });
on_key(sub ($chord) { $app->typed($chord) });

run($app, title => "keys");
