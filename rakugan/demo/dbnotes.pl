# A database, reached through the framework's own library so that both
# runs call one implementation. Write `?` in the statement and put the
# values beside it, and text a person typed can never become part of
# the statement.
use Rakugan;

class Notes {
    use Rakugan;
    field $db      = "demo/.gate/notes.db";
    field $changed = 0;
    field @rows    = empty(Str);

    method setup {
        sqlite_exec($db, "CREATE TABLE IF NOT EXISTS notes(t TEXT)");
        sqlite_exec($db, "DELETE FROM notes");
        $changed = sqlite_exec($db, "INSERT INTO notes VALUES ('alpha'),('beta'),('gamma')");
    }

    method load {
        @rows = sqlite_query_text($db, "SELECT t FROM notes ORDER BY t");
    }

    method note_row :Sig(Int) ($i) {
        return text($rows[$i]);
    }

    method view {
        return column(
            text("inserted=$changed rows=@{[ scalar @rows ]}"),
            row(
                button("setup", on_click => sub { $self->setup }),
                button("load",  on_click => sub { $self->load }),
                spacing => 6,
            ),
            list_view(scalar @rows, sub ($i) { $self->note_row($i) },
                      item_height => 22, height => 120),
            spacing => 8,
            padding => 12,
        );
    }
}

run(Notes->new, title => "dbnotes");
