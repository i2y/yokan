# Pyxel Jump, ported to Rakugan.
#
# The original is `02_jump_game.py` from Pyxel's examples (Takashi
# Kitao, MIT, https://github.com/kitao/pyxel), and `assets/jump.png` is
# that example's own image bank written out with Pyxel's palette. The
# port follows it line by line: `pyxel.blt` becomes `sprite`,
# `pyxel.btn` becomes `keys_down`, `pyxel.cls(12)` becomes the canvas
# background, and 12 still means the same color, because inside a
# canvas a color is an index into the palette this file declares.
#
# What is different, and why. The three effects are WAV files rather
# than the original's chiptune, since the engine plays files; a run
# under a script is silent, so the gate still compares two silent runs.
# And the numbers come from arithmetic written here rather than from a
# generator: two runs do not share one, and a game whose floors land in
# different places is not one the gate can compare. Everything else is
# the game.
#
# Left and right move; the rest is gravity.
use Rakugan;
use List::Util qw(max min);

class Cloud {
    use Rakugan;
    field $x :param :reader = 0;
    field $y :param :reader = 0;
}

class Floor {
    use Rakugan;
    field $x :param :reader = 0;
    field $y :param :reader = 0;
    field $alive :param :reader = true;
}

class Fruit {
    use Rakugan;
    field $x :param :reader = 0;
    field $y :param :reader = 0;
    field $kind :param :reader = 0;
    field $alive :param :reader = true;
}

class Game {
    use Rakugan;
    use List::Util qw(max min);
    field @palette = ("#000000", "#2b335f", "#7e2072", "#19959c",
                      "#8b4852", "#395c98", "#a9c1ff", "#eeeeee",
                      "#d4186c", "#d38441", "#e9c35b", "#70c6a9",
                      "#7696de", "#a3a3a3", "#ff9798", "#edc7b0");
    field $sheet = "demo/assets/jump.png";
    field $score = 0;
    field $px    = 72;
    field $py    = -16;
    field $dy    = 0;
    field $alive = true;
    field $frame = 0;
    # What the view needs whole: the parallax offsets and which of the
    # two player sprites to cut out.
    field $tree_off = 0;
    field $far_off  = 0;
    field $near_off = 0;
    field $player_u = 0;
    # Whole numbers from arithmetic alone, so both runs draw one game.
    field $seed = 11;
    field @far   = (Cloud->new(x => -10, y => 75), Cloud->new(x => 40, y => 65), Cloud->new(x => 90, y => 60));
    field @near  = (Cloud->new(x => 10, y => 25), Cloud->new(x => 70, y => 35), Cloud->new(x => 120, y => 15));
    field @floors = empty(Floor);
    field @fruits = empty(Fruit);

    ADJUST {
        for my $i (0 .. 3) {
            $self->boot_one($i);
        }
    }

    method roll :Sig(Int, Int => Int) ($lo, $hi) {
        $seed = ($seed * 1103515245 + 12345) % 2147483648;
        return $lo + int($seed / 256) % ($hi - $lo + 1);
    }

    method boot_one :Sig(Int) ($i) {
        push @floors, Floor->new(x => $i * 60, y => $self->roll(8, 104), alive => true);
        push @fruits, Fruit->new(x => $i * 60, y => $self->roll(0, 104), kind => $self->roll(0, 2), alive => true);
    }

    method tick {
        $frame += 1;
        $tree_off = $frame % 160;
        $far_off = int($frame / 16) % 160;
        $near_off = int($frame / 8) % 160;
        $self->update_player;
        $self->update_floors;
        $self->update_fruits;
    }

    method update_player {
        $px = max($px - 2, 0) if keys_down("left");
        $px = min($px + 2, 144) if keys_down("right");
        $py += $dy;
        $dy = min($dy + 1, 8);
        $player_u = 0;
        $player_u = 16 if $dy > 0;
        return if $py <= 120;
        audio_play("demo/assets/sound/over.wav", 0.5) if $alive;
        $alive = false;
        return if $py <= 600;
        $score = 0;
        $px = 72;
        $py = -16;
        $dy = 0;
        $alive = true;
    }

    # A floor the player lands on drops away and bounces them. The
    # original edits the tuple in the list; these are built fresh
    # instead, and the bounce a floor writes is what the floors after
    # it see.
    method update_floors {
        my @out = empty(Floor);
        for my $f (@floors) {
            push @out, $self->next_floor($f);
        }
        @floors = @out;
    }

    method next_floor :Sig(Floor => Floor) ($f) {
        my $x = $f->x;
        my $y = $f->y;
        my $alive_now = $f->alive;
        if ($alive_now) {
            if ($px + 16 >= $x && $px <= $x + 40 && $py + 16 >= $y && $py <= $y + 8 && $dy > 0) {
                $alive_now = false;
                $score += 10;
                $dy = -12;
                audio_play("demo/assets/sound/jump.wav", 0.5);
            }
        } else {
            $y += 6;
        }
        $x -= 4;
        if ($x < -40) {
            $x += 240;
            $y = $self->roll(8, 104);
            $alive_now = true;
        }
        return Floor->new(x => $x, y => $y, alive => $alive_now);
    }

    method update_fruits {
        my @out = empty(Fruit);
        for my $f (@fruits) {
            push @out, $self->next_fruit($f);
        }
        @fruits = @out;
    }

    method next_fruit :Sig(Fruit => Fruit) ($f) {
        my $x = $f->x;
        my $y = $f->y;
        my $kind = $f->kind;
        my $alive_now = $f->alive;
        if ($alive_now && abs($x - $px) < 12 && abs($y - $py) < 12) {
            $alive_now = false;
            $score += ($kind + 1) * 100;
            $dy = min($dy, -8);
            audio_play("demo/assets/sound/pickup.wav", 0.5);
        }
        $x -= 2;
        if ($x < -40) {
            $x += 240;
            $y = $self->roll(0, 104);
            $kind = $self->roll(0, 2);
            $alive_now = true;
        }
        return Fruit->new(x => $x, y => $y, kind => $kind, alive => $alive_now);
    }

    method tree :Sig(Int) ($i) {
        sprite($i * 160 - $tree_off, 104, $sheet, 0, 48, 160, 16, colkey => 12);
    }

    method far_strip :Sig(Int) ($i) {
        for my $c (@far) {
            sprite($c->x + $i * 160 - $far_off, $c->y, $sheet, 64, 32, 32, 8, colkey => 12);
        }
    }

    method near_strip :Sig(Int) ($i) {
        for my $c (@near) {
            sprite($c->x + $i * 160 - $near_off, $c->y, $sheet, 0, 32, 56, 8, colkey => 12);
        }
    }

    method view {
        return column(
            canvas(160, 120, scale => 4, background => 12, palette => \@palette, paint => sub {
                # sky, mountain, and the trees that scroll fastest
                sprite(0, 88, $sheet, 0, 88, 160, 32);
                sprite(0, 88, $sheet, 0, 64, 160, 24, colkey => 12);
                for my $i (0 .. 1) {
                    $self->tree($i);
                }
                # two layers of cloud, each strip drawn twice so it wraps
                for my $i (0 .. 1) {
                    $self->far_strip($i);
                }
                for my $i (0 .. 1) {
                    $self->near_strip($i);
                }
                for my $f (@floors) {
                    sprite($f->x, $f->y, $sheet, 0, 16, 40, 8, colkey => 12);
                }
                for my $f (@fruits) {
                    if ($f->alive) {
                        sprite($f->x, $f->y, $sheet, 32 + $f->kind * 16, 0, 16, 16, colkey => 12);
                    }
                }
                sprite($px, $py, $sheet, $player_u, 0, 16, 16, colkey => 12);
                pixel_text(5, 4, sprintf("SCORE %4d", $score), 1);
                pixel_text(4, 4, sprintf("SCORE %4d", $score), 7);
            }),
            spacing => 0,
            padding => 0,
        );
    }
}

my $app = Game->new;
every(0.033, sub { $app->tick });
run($app, title => "Pyxel Jump", width => 640, height => 480, padding => 0);
