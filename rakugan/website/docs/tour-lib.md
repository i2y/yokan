<!-- Written by website/tools/tour_pages.pl from the tour. Edit the tour. -->
# Perl, data, and work

Perl's own library, the framework's, what to do when a call fails, and work that must not freeze the window.

## Perl's own standard library

Where the name is Perl's, perl is the specification. `length`, `substr`,
`index`, `rindex`, `uc`, `lc`, `ucfirst`, `lcfirst`, `reverse`, `join`,
`split`, `sprintf`, `abs`, `int`, `sqrt`, `sort`, `grep`, `map`,
`scalar`, `exists`, `defined`, `keys`, `values`, `rand`, `srand`,
`List::Util`'s `sum`, `max`, `min`, `first`, `uniq` and `shuffle`, and
`POSIX`'s `floor`, `ceil`, `fmod` and `strftime` are the language's
own, not Rakugan's.

Random numbers are perl's own too. Since 5.20 perl carries one
generator on every platform, so after `srand(42)` both runs draw the
same `rand` and deal the same `shuffle`, and the gate compares them like
anything else. An app that never seeds is refused: each of its runs
would draw a sequence of its own.

<!-- script: click:run,dump -->
```perl
use Rakugan;
use List::Util qw(sum max min shuffle);
use POSIX qw(floor);

class Stats {
    use Rakugan;
    use List::Util qw(sum max min shuffle);
    use POSIX qw(floor);
    field @scores = (3, 5, 8, 13, 21);
    field $line   = "-";

    method summarize {
        my @sorted = sort { $a <=> $b } @scores;
        my $mean = sum(@scores) / scalar @scores;
        my @big  = grep { $_ > 5 } @scores;
        my @text = map { "$_" } @big;
        srand(3);
        my @dealt = shuffle(@scores);
        my @hand  = map { "$_" } @dealt;
        $line = sprintf("mean %.1f median %d min %d max %d floor %d big %s dealt %s",
                        $mean, $sorted[int(scalar(@sorted) / 2)],
                        min(@scores), max(@scores), floor(2.7),
                        join(",", @text), join(",", @hand));
    }

    method view {
        return column(
            text($line),
            button("run", on_click => sub { $self->summarize }),
            spacing => 8,
            padding => 12,
        );
    }
}

run(Stats->new, title => "stats");
```

The compiled run does not carry perl, so each of those is written once
in Rust and linked; the interpreted run calls perl's. That the two agree
is not a hope: every one of them is held to a table that perl printed
(`crates/rakugan-stdlib/tests/expected/`, a thousand rows), and
`cargo test -p rakugan-stdlib` compares them. A function whose answer
depends on the machine's locale or clock reads UTC in both runs.


## The framework's standard library

Files, a database, the network and a few other things the operating
system owns come from the framework rather than from Perl, and there
one implementation answers both runs: the compiled one links it, the
interpreted one reaches the same Rust through the engine's C face. Each
name says which layer it is from.

```perl
    fs_write_text($path, $body);       fs_read_text($path);
    fs_read_text_or($path, "(none)");  fs_exists($path);
    fs_list_dir($dir);                 fs_make_dir($dir);
    fs_append_text($path, $more);      fs_remove($path);
    fs_app_dir("myapp");

    http_get_text($url);               http_get_text_or($url, "");
    http_post_text($url, $body);       http_status($url);

    jsondoc_get_text($doc, "user.name");   jsondoc_get_int($doc, "items.0.qty");
    jsondoc_length($doc, "items");         jsondoc_has($doc, "user.email");

    strings_to_int($s);                strings_to_float($s);
    clock_format_ms($ms, "%Y-%m-%d");  clock_local_offset_minutes();
    notify_send("done", "the file is written");
```

A call that can fail comes in two forms: the plain one, which stops the
handler when it fails, and the `_or` twin, which takes what to answer
instead. Which one to write is the question of whether a missing file
is an error or a default.

A database is the same arrangement, because it is no use unless both
runs read the one file the same way:

```perl
    sqlite_exec($db, "CREATE TABLE IF NOT EXISTS notes(t TEXT)");
    sqlite_exec($db, "INSERT INTO notes VALUES (?)", [$draft]);
    my @rows  = sqlite_query_text($db, "SELECT t FROM notes ORDER BY t");
    my $count = sqlite_query_int_or($db, "SELECT COUNT(*) FROM notes", 0);
```

Write `?` in the statement and put the values beside it: text a person
typed can never become part of the statement that way.


## When something fails

A call that can fail has two forms, and the `_or` twin is the one to
reach for first: `fs_read_text_or($path, "")` folds the failure into a
default and asks nothing more. When the reason matters, catch it.
`try` / `catch` is perl's own, written as perl 5.40 writes it, and `$e`
holds what perl would hand it: the message, then the file and the line
of the statement that failed.

<!-- script: click:read,dump,click:halve,dump -->
```perl
use Rakugan;

class Notes {
    use Rakugan;
    field $body  = "(none)";
    field $share = 0.0;
    field $count = 0;
    field $note  = "-";

    method read {
        try {
            $body = fs_read_text("demo/.gate/absent.txt");
            $note = "read";
        } catch ($e) {
            $note = "no file: $e";
        }
    }

    method halve {
        try {
            $share = 100 / $count;
        } catch ($e) {
            $note = $e;
        }
    }

    method view {
        return column(
            text("body: $body"),
            text("share: $share"),
            text("note: $note"),
            row(
                button("read",  on_click => sub { $self->read }),
                button("halve", on_click => sub { $self->halve }),
                spacing => 6,
            ),
            spacing => 8,
            padding => 12,
        );
    }
}

run(Notes->new, title => "failing");
```

Uncaught, a failure stops the handler. The lines before it have taken
effect, the lines after it do not run, the app stays up, and the
message goes to standard error, in both runs. `die "…"` fails on
purpose; `warn "…"` writes to standard error and carries on. Each adds
` at FILE line N.` unless the text ends its own line, as perl does, and
the compiled run names the same file and line.

Perl's own failures are in the same arrangement: a division by zero, a
`%` by zero and the root of a negative number die with perl's words in
both runs, and a `try` around them catches them. Of the framework's
calls, `fs_read_text`, `http_get_text`, `http_post_text` and
`sqlite_query_int` can be caught; the other calls that can fail are
refused inside a `try` by name, and their `_or` twins are the way to
write them there. Three shapes a `try` does not reach yet: a loop (put
the `try` inside the loop, around the line that can fail), a method
that can fail (put the `try` inside the method), and `finally`, because
the compiled run stops the handler at the failure and has no place that
runs after it either way.


## Timers and work off the window's thread

Work you want repeated goes to `every`, declared before `run`:

```perl
my $app = Clock->new;
every(1.0, sub { $app->tick });
run($app, title => "clock");
```

Both runs tick off one clock: a frame in a window, an `advance:` step
in a script.

A handler that blocks freezes the window. Hand the slow work to `task`,
and say what to do with the answer in `on_done`:

<!-- script: click:start,advance:2000,dump -->
```perl
use Rakugan;

class Jobs {
    use Rakugan;
    field $status = "idle";
    field $answer = 0;

    method start {
        $status = "working";
        task(sub {
            my $total = 0;
            my $i = 0;
            while ($i < 300000) {
                $total += $i % 7;
                $i += 1;
            }
            $total;
        }, on_done => sub ($v) {
            $answer = $v;
            $status = "done";
        });
    }

    method view {
        return column(
            text("status: $status  answer: $answer"),
            button("start", on_click => sub { $self->start }),
            spacing => 10,
            padding => 14,
        );
    }
}

run(Jobs->new, title => "tasks");
```

Neither call waits. `task` starts the work; `on_done` runs on the
window's thread once the work has answered, and the handler ends and the
window carries on. Nothing inside the work may touch the app's state or
the screen — the `on_done` sub is where that belongs, and it is why the
answer comes back as a value rather than the work writing it anywhere.


## While you are writing it

`rakugan run` watches the app's file. Save, and the window picks the
edit up: the file is read again, the class with it, and the object the
window is holding answers with the new `view`. A file that does not
compile leaves the window on what it had and says so in the terminal,
and the next save that does compile takes.

Fields keep their values across a save, because the object does. Adding
a field, or changing what one starts as, takes effect the next time you
start the app: the object in the window was made before the change.

