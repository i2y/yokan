# Money kept in a database, with the values bound rather than spliced:
# an item called o'brien is an apostrophe and never a piece of SQL.
use Rakugan;

my %HEADING = (size => 20, color => "accent");
my %FAINT   = (size => 12, color => "#8a8f98");

class Ledger {
    use Rakugan;
    field $db      = "demo/.gate/ledger.db";
    field $name    = "";
    field $amount  = "";
    field $count   = 0;
    field $grand   = 0;
    field $food    = 0;
    field $transit = 0;
    field $fun     = 0;
    field @rows    = empty(Str);
    field @totals  = (0.0, 0.0, 0.0);

    ADJUST {
        $self->load;
    }

    method reset {
        sqlite_exec($db, "CREATE TABLE IF NOT EXISTS expenses(name TEXT, amount INTEGER, cat TEXT)");
        sqlite_exec($db, "DELETE FROM expenses");
        $self->load;
    }

    method add :Sig(Str) ($cat) {
        my $yen = int(0 + $amount);
        return if $yen <= 0;
        sqlite_exec($db, "INSERT INTO expenses VALUES (?, ?, ?)", [$name, "$yen", $cat]);
        $self->load;
    }

    method one_number :Sig(Str, Str => Int) ($sql, $cat) {
        return sqlite_query_int_or($db, $sql, 0, [$cat]);
    }

    method load {
        $count = sqlite_query_int_or($db, "SELECT COUNT(*) FROM expenses", 0);
        $grand = sqlite_query_int_or($db, "SELECT COALESCE(SUM(amount),0) FROM expenses", 0);
        my $by = "SELECT COALESCE(SUM(amount),0) FROM expenses WHERE cat=?";
        $food    = $self->one_number($by, "food");
        $transit = $self->one_number($by, "transit");
        $fun     = $self->one_number($by, "fun");
        @totals = ($food + 0.0, $transit + 0.0, $fun + 0.0);
        # whole rows, every column as text: the line is written here
        # rather than assembled in SQL
        my @found = sqlite_query_rows_or($db, "SELECT name, amount, cat FROM expenses ORDER BY rowid");
        @rows = map { "$_->[0]  ¥$_->[1]  ($_->[2])" } @found;
    }

    method entry_row :Sig(Int) ($i) {
        return text($rows[$i]);
    }

    method view {
        return column(
            text("ledger", %HEADING),
            row(
                text_field($name, placeholder => "item", on_change => sub ($t) { $name = $t }),
                text_field($amount, placeholder => "yen", on_change => sub ($t) { $amount = $t }),
                spacing => 6,
            ),
            row(
                button("food",    on_click => sub { $self->add("food") }),
                button("transit", on_click => sub { $self->add("transit") }),
                button("fun",     on_click => sub { $self->add("fun") }),
                button("reset",   on_click => sub { $self->reset }),
                spacing => 6,
            ),
            text("$count entries, ¥$grand in all", %FAINT),
            text("food ¥$food · transit ¥$transit · fun ¥$fun", %FAINT),
            bar_chart(\@totals, labels => ["food", "transit", "fun"], axis => true, height => 90),
            list_view(scalar @rows, sub ($i) { $self->entry_row($i) },
                      item_height => 22, height => 120),
            spacing => 10, padding => 14, background => "panel",
        );
    }
}

run(Ledger->new, title => "ledger");
