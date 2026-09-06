<!-- Written by website/tools/tour_pages.pl from the tour. Edit the tour. -->
# The first app

An app is a class, its state is its fields, and types are written in only two places.

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

