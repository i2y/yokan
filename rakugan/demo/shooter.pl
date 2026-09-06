# Pyxel Shooter, ported to Rakugan.
#
# The original is `shooter.py` from Pyxel's examples (Takashi Kitao,
# MIT, https://github.com/kitao/pyxel), and `assets/shooter.png` is
# that example's own image bank written out with Pyxel's palette.
#
# What is different, and why. The effects are WAV files rather than the
# original's chiptune, since the engine plays files; a run under a
# script is silent, so the gate still compares two silent runs. And the
# numbers come from arithmetic written here rather than from a
# generator, because two runs do not share one and a game whose enemies
# arrive in different places is not one the gate can compare.
#
# Arrows move, space fires, enter starts and restarts, q closes.
use Rakugan;
use List::Util qw(max min);

my $WIDTH  = 120;
my $HEIGHT = 160;

my $SCENE_TITLE    = 0;
my $SCENE_PLAY     = 1;
my $SCENE_GAMEOVER = 2;

my $STAR_COLOR_HIGH = 12;
my $STAR_COLOR_LOW  = 5;

my $PLAYER_WIDTH  = 8;
my $PLAYER_HEIGHT = 8;
my $PLAYER_SPEED  = 2;

my $BULLET_WIDTH  = 2;
my $BULLET_HEIGHT = 8;
my $BULLET_COLOR  = 11;
my $BULLET_SPEED  = 4;

my $ENEMY_WIDTH  = 8;
my $ENEMY_HEIGHT = 8;
# Pyxel's 1.5 px a frame, in tenths.
my $ENEMY_SPEED = 15;

my $BLAST_START_RADIUS = 1;
my $BLAST_END_RADIUS   = 8;
my $BLAST_COLOR_IN     = 7;
my $BLAST_COLOR_OUT    = 10;

class Star {
    use Rakugan;
    # `y` is what the canvas draws; `y10` is where the star really is.
    field $x :param :reader = 0;
    field $y :param :reader = 0;
    field $y10 :param :reader = 0;
    field $speed10 :param :reader = 0;
    field $col :param :reader = 0;
}

class Bullet {
    use Rakugan;
    field $x :param :reader = 0;
    field $y :param :reader = 0;
}

class Enemy {
    use Rakugan;
    field $x :param :reader = 0;
    field $y :param :reader = 0;
    field $x10 :param :reader = 0;
    field $y10 :param :reader = 0;
    field $flip :param :reader = false;
    field $offset :param :reader = 0;
}

class Blast {
    use Rakugan;
    field $x :param :reader = 0;
    field $y :param :reader = 0;
    field $radius :param :reader = 0;
}

class Game {
    use Rakugan;
    use List::Util qw(max min);
    # Pyxel's own sixteen colors, which is what makes the numbers in
    # this file mean what they mean in the original.
    field @palette = ("#000000", "#2b335f", "#7e2072", "#19959c",
                      "#8b4852", "#395c98", "#a9c1ff", "#eeeeee",
                      "#d4186c", "#d38441", "#e9c35b", "#70c6a9",
                      "#7696de", "#a3a3a3", "#ff9798", "#edc7b0");
    field $sheet = "demo/assets/shooter.png";
    field $scene = 0;
    field $score = 0;
    field $frame = 0;
    field $title_col = 0;
    field $px = 56;
    field $py = 140;
    field @stars   = empty(Star);
    field @bullets = empty(Bullet);
    field @enemies = empty(Enemy);
    field @blasts  = empty(Blast);
    field @live_enemies = empty(Enemy);
    field @hit_bullets  = empty(Int);
    field $player_struck = false;
    field $enemy_struck  = false;
    # Whole numbers from arithmetic alone, so both runs play one game.
    field $seed = 7;

    ADJUST {
        for my $i (0 .. 99) {
            $self->boot_star;
        }
    }

    method roll :Sig(Int, Int => Int) ($lo, $hi) {
        $seed = ($seed * 1103515245 + 12345) % 2147483648;
        return $lo + int($seed / 256) % ($hi - $lo + 1);
    }

    method boot_star {
        my $x = $self->roll(0, $WIDTH - 1);
        my $y = $self->roll(0, $HEIGHT - 1);
        my $speed10 = $self->roll(10, 25);
        my $col = $STAR_COLOR_LOW;
        $col = $STAR_COLOR_HIGH if $speed10 > 18;
        push @stars, Star->new(x => $x, y => $y, y10 => $y * 10, speed10 => $speed10, col => $col);
    }

    method tick {
        quit() if keys_pressed("q");
        $frame += 1;
        $title_col = $frame % 16;
        $self->move_stars;
        if ($scene == $SCENE_TITLE) {
            $scene = $SCENE_PLAY if keys_pressed("enter");
        } elsif ($scene == $SCENE_PLAY) {
            $self->play;
        } else {
            $self->over;
        }
    }

    method move_stars {
        my @out = empty(Star);
        for my $s (@stars) {
            push @out, $self->next_star($s);
        }
        @stars = @out;
    }

    method next_star :Sig(Star => Star) ($s) {
        my $y10 = $s->y10 + $s->speed10;
        $y10 -= $HEIGHT * 10 if $y10 >= $HEIGHT * 10;
        return Star->new(x => $s->x, y => int($y10 / 10), y10 => $y10,
                         speed10 => $s->speed10, col => $s->col);
    }

    method play {
        if ($frame % 6 == 0) {
            my $x = $self->roll(0, $WIDTH - $ENEMY_WIDTH);
            push @enemies, Enemy->new(x => $x, y => 0, x10 => $x * 10, y10 => 0,
                                      flip => false, offset => $self->roll(0, 59));
        }
        $self->collide;
        $self->move_player;
        $self->move_bullets;
        $self->move_enemies;
        $self->move_blasts;
    }

    method over {
        $self->move_bullets;
        $self->move_enemies;
        $self->move_blasts;
        return unless keys_pressed("enter");
        $scene = $SCENE_PLAY;
        $px = 56;
        $py = 140;
        $score = 0;
        @enemies = ();
        @bullets = ();
        @blasts = ();
    }

    method move_player {
        my $x = $px;
        my $y = $py;
        $x -= $PLAYER_SPEED if keys_down("left");
        $x += $PLAYER_SPEED if keys_down("right");
        $y -= $PLAYER_SPEED if keys_down("up");
        $y += $PLAYER_SPEED if keys_down("down");
        $px = min(max($x, 0), $WIDTH - $PLAYER_WIDTH);
        $py = min(max($y, 0), $HEIGHT - $PLAYER_HEIGHT);
        return unless keys_pressed("space");
        push @bullets, Bullet->new(x => $px + 3, y => $py - 4);
        audio_play("demo/assets/sound/shoot.wav", 0.35);
    }

    method move_bullets {
        my @out = empty(Bullet);
        for my $b (@bullets) {
            my $y = $b->y - $BULLET_SPEED;
            push @out, Bullet->new(x => $b->x, y => $y) if $y + $BULLET_HEIGHT - 1 >= 0;
        }
        @bullets = @out;
    }

    method move_enemies {
        my @out = empty(Enemy);
        for my $e (@enemies) {
            my $x10 = $e->x10;
            my $flip = true;
            if (($frame + $e->offset) % 60 < 30) {
                $x10 += $ENEMY_SPEED;
                $flip = false;
            } else {
                $x10 -= $ENEMY_SPEED;
            }
            my $y10 = $e->y10 + $ENEMY_SPEED;
            push @out, Enemy->new(x => int($x10 / 10), y => int($y10 / 10), x10 => $x10, y10 => $y10,
                                  flip => $flip, offset => $e->offset)
                if $y10 <= ($HEIGHT - 1) * 10;
        }
        @enemies = @out;
    }

    method move_blasts {
        my @out = empty(Blast);
        for my $b (@blasts) {
            my $r = $b->radius + 1;
            push @out, Blast->new(x => $b->x, y => $b->y, radius => $r) if $r <= $BLAST_END_RADIUS;
        }
        @blasts = @out;
    }

    # The two rectangle tests, resolved into new lists. Where the
    # original sets `is_alive = False` and filters afterwards, this
    # keeps the ones that live.
    method collide {
        @live_enemies = ();
        @hit_bullets = ();
        $player_struck = false;
        for my $e (@enemies) {
            $self->resolve($e);
        }
        my @live = empty(Bullet);
        for my $i (0 .. $#bullets) {
            push @live, $bullets[$i] unless $self->was_hit($i);
        }
        @enemies = @live_enemies;
        @bullets = @live;
        $scene = $SCENE_GAMEOVER if $player_struck;
    }

    method was_hit :Sig(Int => Bool) ($i) {
        my $hit = false;
        for my $h (@hit_bullets) {
            $hit = true if $h == $i;
        }
        return $hit;
    }

    method resolve :Sig(Enemy) ($e) {
        $enemy_struck = false;
        for my $i (0 .. $#bullets) {
            if ($self->shot($e, $bullets[$i])) {
                $enemy_struck = true;
                push @hit_bullets, $i;
            }
        }
        if ($enemy_struck) {
            push @blasts, Blast->new(x => $e->x + 4, y => $e->y + 4, radius => $BLAST_START_RADIUS);
            $score += 10;
            audio_play("demo/assets/sound/blast.wav", 0.5);
        } elsif ($self->rammed($e)) {
            push @blasts, Blast->new(x => $px + 4, y => $py + 4, radius => $BLAST_START_RADIUS);
            $player_struck = true;
            audio_play("demo/assets/sound/over.wav", 0.6);
        } else {
            push @live_enemies, $e;
        }
    }

    method shot :Sig(Enemy, Bullet => Bool) ($e, $b) {
        return $e->x + $ENEMY_WIDTH > $b->x && $b->x + $BULLET_WIDTH > $e->x
            && $e->y + $ENEMY_HEIGHT > $b->y && $b->y + $BULLET_HEIGHT > $e->y;
    }

    method rammed :Sig(Enemy => Bool) ($e) {
        return $px + $PLAYER_WIDTH > $e->x && $e->x + $ENEMY_WIDTH > $px
            && $py + $PLAYER_HEIGHT > $e->y && $e->y + $ENEMY_HEIGHT > $py;
    }

    method blast_of :Sig(Blast) ($b) {
        circle($b->x, $b->y, $b->radius, $BLAST_COLOR_IN);
        circle_outline($b->x, $b->y, $b->radius, $BLAST_COLOR_OUT);
    }

    method view {
        return column(
            canvas($WIDTH, $HEIGHT, scale => 4, background => 0, palette => \@palette, paint => sub {
                for my $s (@stars) {
                    pixel($s->x, $s->y, $s->col);
                }
                if ($scene == $SCENE_TITLE) {
                    pixel_text(35, 66, "Pyxel Shooter", $title_col);
                    pixel_text(31, 126, "- PRESS ENTER -", 13);
                } elsif ($scene == $SCENE_PLAY) {
                    sprite($px, $py, $sheet, 0, 0, $PLAYER_WIDTH, $PLAYER_HEIGHT, colkey => 0);
                } else {
                    pixel_text(43, 66, "GAME OVER", 8);
                    pixel_text(31, 126, "- PRESS ENTER -", 13);
                }
                for my $b (@bullets) {
                    rect($b->x, $b->y, $BULLET_WIDTH, $BULLET_HEIGHT, $BULLET_COLOR);
                }
                for my $e (@enemies) {
                    sprite($e->x, $e->y, $sheet, 8, 0, $ENEMY_WIDTH, $ENEMY_HEIGHT,
                           colkey => 0, flip_x => $e->flip);
                }
                for my $b (@blasts) {
                    $self->blast_of($b);
                }
                pixel_text(39, 4, sprintf("SCORE %5d", $score), 7);
            }),
            spacing => 0,
            padding => 0,
        );
    }
}

my $app = Game->new;
every(0.033, sub { $app->tick });
# `padding => 0`: the canvas IS the app, so it paints to the window's
# edge rather than sitting inside the engine's ring.
run($app, title => "Pyxel Shooter", width => 480, height => 640, padding => 0);
