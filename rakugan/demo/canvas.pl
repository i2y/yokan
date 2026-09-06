# A canvas: a grid of virtual pixels, painted command by command.
#
# `canvas(width, height, scale => …, background => …, palette => …,
# paint => sub { … })` opens the grid, and the sub paints it — pixel,
# line, rect, rect_outline, circle, circle_outline, triangle,
# triangle_outline, sprite and pixel_text. `scale` says how many
# logical pixels one virtual pixel takes, so a 64x40 canvas at six is
# 384x240 on screen.
#
# Every color is a NUMBER: the index of a color in `palette`. That is
# how tools for pixel art work, so drawing written for one moves here
# with its numbers unchanged.
#
# The commands are not elements. Nothing here can be clicked, themed,
# sized or animated, and a loop inside the canvas is the ordinary loop:
# what its body paints joins the frame where it stands.
use Rakugan;

my %HEADING = (size => 18, color => "accent");
my %FAINT   = (size => 12, color => "#8a8f98");

class Blip {
    use Rakugan;
    field $x :param :reader = 0;
    field $y :param :reader = 0;
    field $c :param :reader = 0;
}

class Sky {
    use Rakugan;
    # Five colors are enough to show that the index IS the color.
    field @palette = ("#11111b", "#89b4fa", "#f38ba8", "#eeeeee", "#a6e3a1");
    field $frame  = 0;
    field $ball_x = 30;
    field $ball_y = 18;
    field $dx     = 1;
    field $dy     = 1;
    field @blips  = (Blip->new(x => 6, y => 4, c => 1), Blip->new(x => 20, y => 9, c => 2),
                     Blip->new(x => 50, y => 6, c => 3), Blip->new(x => 58, y => 30, c => 4));

    method seed {
        @blips = (Blip->new(x => 6, y => 4, c => 1), Blip->new(x => 20, y => 9, c => 2),
                  Blip->new(x => 50, y => 6, c => 3), Blip->new(x => 58, y => 30, c => 4));
    }

    method tick {
        $frame += 1;
        # The keyboard is read here, in the tick, never in a view.
        # `keys_down` is "held right now", so holding an arrow steers.
        $dx = -1 if keys_down("left");
        $dx = 1 if keys_down("right");
        $dy = -$dy if keys_pressed("space");
        my $x = $ball_x + $dx;
        my $y = $ball_y + $dy;
        if ($x < 4) {
            $x = 4;
            $dx = 1;
        }
        if ($x > 59) {
            $x = 59;
            $dx = -1;
        }
        if ($y < 4) {
            $y = 4;
            $dy = 1;
        }
        if ($y > 35) {
            $y = 35;
            $dy = -1;
        }
        $ball_x = $x;
        $ball_y = $y;
    }

    method view {
        return column(
            text("Canvas", %HEADING),
            text("a grid of virtual pixels; every color is an index", %FAINT),
            canvas(64, 40, scale => 6, background => 0, palette => \@palette, paint => sub {
                rect(2, 2, 12, 6, 1);
                rect_outline(16, 2, 12, 6, 2);
                circle_outline(34, 5, 4, 3);
                line(2, 11, 61, 11, 2);
                triangle(3, 37, 8, 28, 13, 37, 4);
                for my $b (@blips) {
                    pixel($b->x, $b->y, $b->c);
                }
                circle($ball_x, $ball_y, 3, 3);
                pixel_text(2, 14, "FRAME $frame", 3);
            }),
            row(button("seed", on_click => sub { $self->seed }), spacing => 8),
            spacing => 12,
            padding => 16,
        );
    }
}

my $app = Sky->new;
every(0.05, sub { $app->tick });
run($app, title => "canvas");
