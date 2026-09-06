# Rakugan language tour

<!-- The identity paragraph is the owner's to write. What follows is a
     draft standing in for it, in the shape the other two tours use. -->

Rakugan turns a Perl desktop app into one native binary. `rakugan
build` reads your Perl, translates it to [pixie](../docs/PIXIE.md) and
links it with the drawing engine (**gpui**, the engine behind the Zed
editor); while you are working, the same file runs under perl instead,
reaching that engine through a small XS door. Whether the two are the
same program is something you check rather than hope: `rakugan gate`
drives both with one interaction script and compares the screens they
drew, byte for byte. The subs that build the screen — `text`, `button`,
`column` and thirty more — come with Rakugan; the app itself is a plain
Perl class. How much of Perl you can write is the dialect this page
describes, and what falls outside it is named at
[What Rakugan refuses](#what-rakugan-refuses).

This page is the language, in the order you meet it. Everything in it
runs: `tools/tour_check.pl` pulls every complete example out of this
file and puts it through the same command a demo goes through, so a
rename in the vocabulary breaks this page before a reader meets it.
日本語版は [TOUR.ja.md](TOUR.ja.md).

## Table of contents

- [The smallest app](#the-smallest-app)
- [Holding state](#holding-state)
- [Types, and where they come from](#types-and-where-they-come-from)
- [Writing views](#writing-views)
- [Control flow in a view](#control-flow-in-a-view)
- [Form controls](#form-controls)
- [Handlers](#handlers)
- [Lists, charts, and rows built on demand](#lists-charts-and-rows-built-on-demand)
- [Hashes](#hashes)
- [Value classes](#value-classes)
- [Regular expressions](#regular-expressions)
- [The canvas](#the-canvas)
- [The keyboard](#the-keyboard)
- [The keywords every element takes](#the-keywords-every-element-takes)
- [Themes and animation](#themes-and-animation)
- [The window](#the-window)
- [Perl's own standard library](#perls-own-standard-library)
- [The framework's standard library](#the-frameworks-standard-library)
- [Timers and work off the window's thread](#timers-and-work-off-the-windows-thread)
- [While you are writing it](#while-you-are-writing-it)
- [Headless runs and the gate](#headless-runs-and-the-gate)
- [What Rakugan refuses](#what-rakugan-refuses)
- [Shipping](#shipping)
- [What does not work yet](#what-does-not-work-yet)

## The smallest app

An app is a class with a `view` method that answers one element, and
`run` opens a window on it. `use Rakugan;` goes at the top of the file
and again as the first line inside the class: a Perl import is per
package, and `class App { ... }` is a package of its own.

<!-- script: dump -->
```perl
use Rakugan;

class Hello {
    use Rakugan;

    method view {
        return text("hello", size => 28);
    }
}

run(Hello->new, title => "hello");
```

```console
$ rakugan run  demo/hello.pl      # a window, under perl
$ rakugan gate demo/hello.pl --script "dump"
GATE OK — 1 dump line identical in both runs
```

Perl 5.40 or newer runs the app, because that is where `class` is a
feature you can rely on. The command itself runs under whatever perl is
first on your path.

## Holding state

The state is the class's fields. A handler is an anonymous sub, and it
closes over the fields, so it writes them the way any method would.
After a handler the whole view is built again from what the fields now
say.

<!-- script: click:+1,click:+1,dump,input:Momo,dump -->
```perl
use Rakugan;

class Counter {
    use Rakugan;
    field $count = 0;
    field $name  = "";

    method view {
        return column(
            text("count: $count", size => 34),
            row(
                button("+1",    on_click => sub { $count += 1 }),
                button("+10",   on_click => sub { $count += 10 }),
                button("reset", on_click => sub { $count = 0 }),
                spacing => 8,
            ),
            text_field($name, placeholder => "your name",
                       on_change => sub ($s) { $name = $s }),
            text("hello, $name"),
            spacing => 12,
            padding => 16,
        );
    }
}

run(Counter->new, title => "counter");
```

There is no separate store, nothing to observe, and no `new` of your
own to write. A field's initializer is where its state starts.

## Types, and where they come from

Rakugan is typed, and almost none of the types are written down. A
field's type is read from its initializer: `= 0` is an `Int`, `= 0.0` a
`Num`, `= ""` a `Str`, `= false` a `Bool`. A container that starts
empty has nothing to read, so it says what it will hold:

```perl
    field @names  = empty(Str);
    field %counts = empty(Int);
```

A method with parameters says what they are, in an attribute:

```perl
    method add :Sig(Int) ($by) { $count += $by }
    method label :Sig(Int, Str => Str) ($n, $unit) { return "$n $unit" }
```

`:Sig(A, B => R)` is the parameters, then what the method answers; a
method that answers nothing leaves the arrow off, and a method with no
parameters needs no attribute at all. The type names are the ones
Types::Standard uses: `Int`, `Num`, `Str`, `Bool`, `ArrayRef[Int]`,
`HashRef[Str]`, and a class of your own by its name.

Everything else is worked out. `my` locals take the type of what they
are given, an expression's type follows from its parts, and a mismatch
is named with the line it is on rather than found at run time.

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
match with one engine. Two things follow. A pattern built from a
variable is refused, because a shipped app carries nothing to compile it
with; and `/e`, which runs perl on the replacement, is refused for the
same reason. `$1` outside the `if` is refused too: it would be whatever
the last successful match anywhere had left, which is not a thing two
runs can be held to.

## The canvas

`canvas` is a grid of virtual pixels, painted by the commands in the
sub written on its `paint` keyword. Inside it a color is a NUMBER: the
index of a color in the palette the app declares. That is how tools for
pixel art work, so drawing written for a pixel machine ports line for
line with its numbers unchanged.

<!-- script: advance:50,dump -->
```perl
use Rakugan;

class Sky {
    use Rakugan;
    field @palette = ("#11111b", "#89b4fa", "#f38ba8", "#eeeeee", "#a6e3a1");
    field $frame = 0;

    method tick {
        $frame += 1;
    }

    method view {
        return canvas(64, 40, scale => 6, background => 0, palette => \@palette,
                      paint => sub {
            rect(2, 2, 12, 6, 1);
            rect_outline(16, 2, 12, 6, 2);
            circle_outline(34, 5, 4, 3);
            line(2, 11, 61, 11, 2);
            triangle(3, 37, 8, 28, 13, 37, 4);
            circle(30, 18, 3, 3);
            pixel_text(2, 14, "FRAME $frame", 3);
        });
    }
}

my $app = Sky->new;
every(0.05, sub { $app->tick });
run($app, title => "sky");
```

The commands are `pixel`, `line`, `rect`, `rect_outline`, `circle`,
`circle_outline`, `triangle`, `triangle_outline`, `sprite` and
`pixel_text`. They are not elements: nothing here can be clicked,
themed, sized or animated, and a loop inside the canvas is the ordinary
loop — what its body paints joins the frame where it stands.

`sprite` copies a rectangle out of an image that shares the canvas's
palette, and `colkey` names the index that is not copied:

```perl
    sprite($px, $py, "demo/assets/sheet.png", 0, 0, 16, 16, colkey => 0);
```

## The keyboard

A game asks what the hands are doing rather than waiting to be told.

```perl
    method tick {
        $x -= 2 if keys_down("left");
        $x += 2 if keys_down("right");
        $self->fire if keys_pressed("space");
        quit() if keys_pressed("q");
    }
```

`keys_down` is "held right now", `keys_pressed` is "went down since the
previous tick" (a held key answers once), and `keys_released` is the
other edge. Read them in a timer, never in a view: a view that read the
keyboard would draw one thing in a window and another under a script.

`demo/jump.pl` and `demo/shooter.pl` are two of Pyxel's own examples
(Takashi Kitao, MIT), ported to this vocabulary and gated.

A canvas can be looked at without a window. `PIXIE_FRAMES=<dir>` writes
a PNG of the first canvas after every script step, drawn by the same
rasterizer the window uses, and `PIXIE_FRAME_SCALE` draws the grid
bigger than the app asks. The dump says what a frame IS; this says what
it looks like, over ssh, in CI, or while the screen is locked.

```console
$ PIXIE_FRAMES=frames PIXIE_SCRIPT="advance:34,advance:34" ./demo/.gate/jump
$ ls frames
0000.png  0001.png
```

## The keywords every element takes

Fifteen properties ride on every element under one name and one
meaning: `width`, `height`, `min_width`, `max_width`, `disabled`,
`theme`, `animate`, `easing`, `enter`, `exit`, `col_span`, `row_span`,
`role`, `a11y_label`, `tooltip`.

```perl
    button("save", disabled => $busy, tooltip => "write the file", width => 120);
    text("total", role => "heading", a11y_label => "the running total");
```

An element that owns one of those names under its own meaning keeps it:
a `text`'s `width` is the text's, and the box leaves it alone.

## Themes and animation

The palette and any movement are keywords too.

```perl
    column(@cells, theme => "dark");
    text("saved", animate => 150, easing => "out", enter => true);
```

`theme` swaps the palette under one part of the screen. `animate` is
how many milliseconds a change takes and `easing` is its shape
(`linear`, `in`, `out`, `inOut`), with `enter` and `exit` for what
appears and disappears. A color is a hex string or one of the palette's
own names (`accent`, `muted`, `danger`), so a screen written once
follows both palettes.

## The window

Four things come from the window itself, declared after the app is made
and before `run`:

<!-- script: click:+1,key:cmd+s,dump,menu:Clear,dump -->
```perl
use Rakugan;

class Keys {
    use Rakugan;
    field $count = 0;
    field $saved = 0;
    field $last  = "-";

    method save  { $saved = $count }
    method clear { $count = 0; $saved = 0 }

    method typed :Sig(Str) ($key) {
        $last = $key;
    }

    method view {
        return column(
            text("count: $count  saved: $saved"),
            text("last key: $last"),
            button("+1", on_click => sub { $count += 1 }),
            spacing => 8,
            padding => 12,
        );
    }
}

my $app = Keys->new;

menu_item("Count", "Clear", sub { $app->clear });
shortcut("cmd+s", sub { $app->save });
on_key(sub ($chord) { $app->typed($chord) });

run($app, title => "keys");
```

`on_file_drop(sub ($path) { ... })` is the fourth. These live for as
long as the app does.

The clipboard is a pair of calls:

```perl
    clipboard_set_text($text);
    $text = clipboard_get_text();
```

A file dialog waits for a person, so it belongs off the window's thread,
which is what [`task`](#timers-and-work-off-the-windows-thread) is for:

```perl
    task(sub { fs_open_dialog("Choose a file") },
         on_done => sub ($path) { $self->took($path) });
    task(sub { fs_save_dialog("notes.txt") },
         on_done => sub ($path) { $self->write_to($path) });
```

A headless script answers a dialog with a `file:<path>` step, which is
what makes a dialog a checked interaction like any other.

Sound is a file played and then forgotten:

```perl
    audio_play("demo/assets/sound/blip.wav");        # as it was recorded
    audio_play("demo/assets/sound/blast.wav", 0.4);  # at a level, 0.0 to 1.0
    audio_stop();
```

The call answers at once; nothing waits for the end of the sound. A run
under a script is silent — a gate must not need a machine with speakers
— and a machine with no audio device, or a file that cannot be read,
plays nothing rather than failing the app. WAV is what the engine
decodes.

## Perl's own standard library

Where the name is Perl's, perl is the specification. `length`, `substr`,
`index`, `rindex`, `uc`, `lc`, `ucfirst`, `lcfirst`, `reverse`, `join`,
`split`, `sprintf`, `abs`, `int`, `sqrt`, `sort`, `grep`, `map`,
`scalar`, `exists`, `defined`, `keys`, `values`, `List::Util`'s `sum`,
`max`, `min`, `first` and `uniq`, and `POSIX`'s `floor`, `ceil`, `fmod`
and `strftime` are the language's own, not Rakugan's.

<!-- script: click:run,dump -->
```perl
use Rakugan;
use List::Util qw(sum max min);
use POSIX qw(floor);

class Stats {
    use Rakugan;
    use List::Util qw(sum max min);
    use POSIX qw(floor);
    field @scores = (3, 5, 8, 13, 21);
    field $line   = "-";

    method summarize {
        my @sorted = sort { $a <=> $b } @scores;
        my $mean = sum(@scores) / scalar @scores;
        my @big  = grep { $_ > 5 } @scores;
        my @text = map { "$_" } @big;
        $line = sprintf("mean %.1f median %d min %d max %d floor %d big %s",
                        $mean, $sorted[int(scalar(@sorted) / 2)],
                        min(@scores), max(@scores), floor(2.7),
                        join(",", @text));
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

## Headless runs and the gate

`PIXIE_SCRIPT` replaces the person. The engine builds the tree, drives
it with the steps, and prints the dumps:

```
click:<label>      press a button by the label it shows
input:<text>       type into a field   submit    press enter in it
slide / select     move a slider, pick an option
key:<chord>        a keystroke bound to a shortcut
keydown:<key> / keyup:<key>    hold a key down, let it up
menu:<item>        pick a menu item    file:<path>   answer a dialog
drop:<path>        a file dragged onto the window
advance:<ms>       move the clock      theme:dark|light
dump               print the tree      a11y   print what a reader reads
```

`rakugan gate` runs the app twice with one script — perl through the XS
door, and the binary the translator's `.pix` was built into — and
compares the two transcripts byte for byte.

```console
$ rakugan gate demo/counter.pl --script "click:+1,dump,input:Momo"
GATE OK — 3 dump lines identical in both runs
```

The gate is the promise. Everything else in this page is a way of
writing something the gate can keep.

## What Rakugan refuses

`rakugan check` reads the app and names what it cannot take, with the
line and the rewrite. perl reads the file first — it is the parser of
record, so a shape perl rejects never reaches the translator — and then
the translator names the first thing outside the dialect. It runs before
every build and every gate, and prints nothing when there is nothing to
say.

```console
$ rakugan check demo/broken.pl
demo/broken.pl:9:26: Rakugan cannot take this — a hash may not have that
key, so say what to answer when it does not: `$prices{$k} // 0`
        $picked = $prices{"apple"};
                  ^
```

What it refuses, and what to write instead:

- A field with no initializer, or with `:param` on the app's own class.
  The initializer is where the type comes from.
- A list or hash that starts empty without saying what it holds. Write
  `empty(Str)`.
- A list holding two types.
- A method with parameters and no `:Sig`.
- A condition that is not a `Bool`. Compare it: `!= 0`, `ne ""`.
- `+` with a string on one side. `0 + $s` reads a string as a number.
- `++` on a string. Perl counts letters there (`"az"++` is `"ba"`), and
  the compiled run does not.
- A hash read with no `//`, and `keys` without `sort`.
- An index a view cannot prove is inside the list.
- A method that writes a field, called while a view is being built.
- `$1` outside the `if` that matched, a pattern built at run time, and
  `/e` on a substitution.
- `print`, `printf`, `say`: a compiled app writes its screen, not its
  standard output. `warn` goes to standard error and is taken.
- A string `eval`, `goto`, `local`, `wantarray`, `each`, `tie`, `bless`,
  `ref`, `AUTOLOAD`.
- A handler that is not a sub, or takes a parameter it is not called
  with.
- An unknown keyword on an element, or one given the wrong type — the
  message lists what that element takes.

Each of those has a fixture under `test/refuse/` holding the message it
prints, so a refusal cannot quietly change its wording.

## Shipping

```console
$ rakugan build demo/todo.pl --release --app
built: demo/.gate/todo (11.6 MB)
bundle: demo/dist/todo.app (11.8 MB)
```

`--release` drops the symbol table; `--app` wraps the binary in a macOS
application bundle, ad-hoc signed, with `<stem>.png` or `<stem>.icns`
beside the app as its icon. The binary carries the engine and the
translated app and links nothing but the system's own libraries, so the
bundle is the whole program: it opens on a machine with neither perl
5.40 nor the toolchain installed.

## What does not work yet

- The dialect is a subset, and the list under
  [What Rakugan refuses](#what-rakugan-refuses) is what it leaves out.
  Every entry there is a shape the translator cannot yet carry to the
  compiled run, not a judgement about Perl.
- No references except the ones named here: a list or a hash passed to
  an element, and a class of your own. No code references beyond
  handlers, no references to references, no `ref`.
- No modules of your own. An app is one file, and the only functions
  from a module the translator knows are `List::Util`'s and `POSIX`'s;
  a `use` of anything else is read and ignored, and a call into it is
  refused by name.
- No `sprintf` beyond `%s %d %i %f %F %e %E %g %G %x %X %o %b %%`, with
  a width, a precision, and the `-`, `+`, ` `, `0` and `#` flags.
- Random numbers are not the same generator in the two runs, so a
  program that wants one sequence in both writes the generator itself.
  The two games do, in a few lines of arithmetic.
- `check` names what is listed above, but it does not yet see everything
  the translator gets wrong; the gate is still what catches the rest.
- macOS only. The binary carries the engine it draws with, so even a
  small app weighs about 12 MB.
