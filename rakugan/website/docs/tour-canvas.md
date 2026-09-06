<!-- Written by website/tools/tour_pages.pl from the tour. Edit the tour. -->
# The canvas and the keyboard

A grid of virtual pixels, and keys read as a device rather than waited for.

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

