<!-- Written by website/tools/tour_pages.pl from the tour. Edit the tour. -->
# Look, and the window

The keywords every element takes, the palette, and what the window itself brings.

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
which is what [`task`](tour-lib.md#timers-and-work-off-the-windows-thread) is for:

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

