use Rakugan;

class App {
    use Rakugan;
    field $n = 0;

    method view {
        my @cells = (text("n=$n"));
        push @cells, text("some") if $n;
        return column(@cells);
    }
}

run(App->new, title => "x");
