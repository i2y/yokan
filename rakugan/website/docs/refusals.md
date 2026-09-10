<!-- Written by website/tools/refusals_page.pl from test/refuse/. Edit the fixtures. -->
# What Rakugan refuses

The dialect is a subset, and `rakugan check` is where you meet its edge.
It reads the app and names what it cannot take, with the file, the line
and the column, the line itself, and what to write instead. perl reads
the file first, so a shape perl rejects never reaches the translator.
`check` runs before every build and every gate, needs no compiler and no
window, and prints nothing at all when there is nothing to say.

Each of the 52 below has a file under `test/refuse/` that triggers it and
the message it must print, word for word. The sweep runs them, so a
refusal cannot quietly change its wording, and this page is quoted from
those same files.

## The class

The elements, the type names and `empty` are brought in by an import, and a Perl import is per package.

```console
test/refuse/class_pragma.pl:3:11: Rakugan cannot take this — `class App` needs `use Rakugan;` as its first line: a Perl import is per package, and the elements, `empty` and the type names have to be in this one
    class App {
              ^
```

A field's type is read from what it starts as, so a field with no initializer has no type to read.

```console
test/refuse/field_initializer.pl:5:11: Rakugan cannot take this — a field needs an initializer (`= 0`, `= ""`, `= empty(Str)`) — that is where its type comes from
        field $n;
              ^
```

The app's own fields are the app's alone: nothing hands them in and nothing reads them from outside.

```console
test/refuse/field_attribute.pl:5:14: Rakugan cannot take this — a field takes no attributes here (`:param`, `:reader`); its type is read from the initializer
        field $n :param = 0;
                 ^
```

The types of a method's parameters cannot be read off anything, so they are written down.

```console
test/refuse/method_without_sig.pl:7:12: Rakugan cannot take this — a method with parameters says what they are: `method add :Sig(Int) ($by) { ... }`
        method add ($by) {
               ^
```

perl reads the file first, and there a bare `s` begins a substitution.

```console
test/refuse/quote_method.pl:7:12: Rakugan cannot take this — a method named `s` reads as a regular expression to the parser; pick another name
        method s { $n += 1 }
               ^
```

The top of the file is where the app is made, not a second place to keep state.

```console
test/refuse/top_statement.pl:3:1: Rakugan cannot take this — a declaration at the top of the file is a hash of keywords (`my %PILL = (...)`), a name for a literal (`my $WIDTH = 120;`) or the app itself (`my $app = Counter->new;`)
    my @greetings = ("hello", "goodbye");
    ^
```

Inside a class with methods a field is reached by its name; calling another method of the same class from there is not carried yet.

```console
test/refuse/self_in_class.pl:6:28: Rakugan cannot take this — `$self` inside `Node`: a method reaches its own fields by name, and another method of the same class is not called from here yet
        method grow { $n += 1; $self->grow }
                               ^
```

A field starts at what `new` gives with no values, so the type is known before anything runs; values go in `ADJUST` or a handler.

```console
test/refuse/new_with_values_in_field.pl:11:19: Rakugan cannot take this — a field starts as `Node->new` with no values; give it values in `ADJUST`, or make it in a handler
        field $node = Node->new(n => 3);
                      ^
```

The compiled run gives every field a reader and a writer of its own name, and a method cannot take those names.

```console
test/refuse/method_named_like_field.pl:6:12: Rakugan cannot take this — `set_n` is the name the compiled run gives `n`'s own writer; put `:writer` on the field, or name the method for what it does
        method set_n :Sig(Int) ($v) { $n = $v }
               ^
```

## Types

A container that starts empty has nothing in it to read a type from.

```console
test/refuse/empty_list.pl:5:20: Rakugan cannot take this — a list that starts empty says what it will hold: `field @items = empty(Str);`
        field @items = ();
                       ^
```

A list is one type, because the compiled run holds it as one.

```console
test/refuse/mixed_list.pl:5:24: Rakugan cannot take this — a list holds one type: this one started with Int and this is String
        field @items = (1, "two");
                           ^
```

A keyword takes the type the table gives it, and the table is what both sides of the engine count with.

```console
test/refuse/wrong_type.pl:7:30: Rakugan cannot take this — `size =>` takes a number (got String)
            return text("hello", size => "large");
                                 ^
```

The message lists what that element does take, so the name is one lookup away.

```console
test/refuse/unknown_keyword.pl:7:30: Rakugan cannot take this — `text` has no `weight =>`; it takes `a11y_label`, `align`, `animate`, `background`, `bold`, `border_color`, `border_radius`, `border_width`, `col_span`, `color`, `disabled`, `easing`, `enter`, `exit`, `grow`, `height`, `italic`, `max_lines`, `max_width`, `min_width`, `mono`, `padding`, `role`, `row_span`, `size`, `theme`, `tooltip`, `underline`, `width`, `wrap`
            return text("hello", weight => 700);
                                 ^
```

Perl's truthiness of a number or a string is not in the dialect: a condition is a `Bool`.

```console
test/refuse/truthiness.pl:9:35: Rakugan cannot take this — a condition is a bool (got Int); Perl's truthiness of a number or a string is not in the translator — compare it (`!= 0`, `ne ""`)
            push @cells, text("some") if $n;
                                      ^
```

Reading a string as a number is written out, the way perl would do it silently.

```console
test/refuse/string_and_number.pl:8:19: Rakugan cannot take this — `+` needs a number on both sides (got String and Int); `0 + $s` reads a string as a number, the way perl does
            $n = "10" + 5;
                      ^
```

Perl counts letters there, and the compiled run has no such counting in it.

```console
test/refuse/string_increment.pl:8:13: Rakugan cannot take this — `$tag` holds a String, and Perl's `++` on a string counts letters (`"az"++` is `"ba"`), which the compiled run does not do; join or replace the string instead
            $tag++;
                ^
```

A field's type is read from its initializer, and `undef` alone names none; `maybe(Int)` says what it may hold.

```console
test/refuse/undef_alone.pl:5:18: Rakugan cannot take this — `undef` alone says nothing about the type; say what this may hold: `maybe(Int)`, `maybe(Str)`
        field $sel = undef;
                     ^
```

Nothing has no text; the app says what to print then, or reads the value inside `defined`.

```console
test/refuse/maybe_in_text.pl:8:25: Rakugan cannot take this — this may be nothing, and nothing has no text; say what to print then: `$x // "-"`, or read it inside `if (defined $x)`
        method go { $note = "sel=$sel" }
                            ^
```

A member of nothing is where perl dies at run time; inside `if (defined $x)` the object is there in both runs.

```console
test/refuse/maybe_member.pl:14:25: Rakugan cannot take this — `$root` may be nothing; read it inside `if (defined $root)`, where it is the object
        method go { $note = $root->label }
                            ^
```

The branch of that `if` is where the value is read as a value; a `while` or an `&&` has no such branch.

```console
test/refuse/defined_in_while.pl:9:9: Rakugan cannot take this — `defined` here is the whole condition of an `if` or `unless`, and its branch reads the value; it does not go in a `while`, a `grep`, or beside `&&`
            while (defined $sel) { $n += 1; $sel = undef }
            ^
```

Inside the branch the name is a copy of the value; a write there would change the copy in one run and the field in the other.

```console
test/refuse/write_inside_defined.pl:11:13: Rakugan cannot take this — `$sel` is read as the value it holds inside `if (defined $sel)`; write it outside that block
                $sel = undef;
                ^
```

perl keeps a constant per package, and two classes may share the name — as long as they mean one thing by it.

```console
test/refuse/constant_twice.pl:6:18: Rakugan cannot take this — `LIMIT` is declared twice, and not the same way
        use constant LIMIT => 10;
                     ^
```

perl would grow the number into one with a fraction; 64 bits is the edge the dialect holds, and two written numbers are worked out before anything is built.

```console
test/refuse/literal_overflow.pl:8:34: Rakugan cannot take this — this comes to 18446744073709551616, and a whole number here holds 64 bits — perl would grow it into a number with a fraction, which the compiled run cannot follow; write it with a `.0` to mean that number
            $n = 4611686018427387904 * 4;
                                     ^
```

## Views and handlers

Building a screen twice has to build the same screen, so building it only reads.

```console
test/refuse/view_calls_method.pl:13:21: Rakugan cannot take this — `bumped` touches the app's state, and building a view only reads; give it what it needs as parameters, or read a field
            return text("n=@{[ $self->bumped ]}");
                        ^
```

A row reads its own number; anything else is worked out where it can be checked.

```console
test/refuse/negative_index.pl:9:21: Rakugan cannot take this — an index a view cannot prove is not negative; a row reads its own index, and anything else is worked out in a handler
            return text("at: $items[$at]");
                        ^
```

A handler is called with what the event carries, and nothing else.

```console
test/refuse/handler_arity.pl:8:45: Rakugan cannot take this — this handler is called with nothing; drop the parameter
            return button("go", on_click => sub ($x) { $n += 1 });
                                                ^
```

The two runs would have to agree about a key that is not there, so the app says what to answer.

```console
test/refuse/bare_hash_read.pl:9:26: Rakugan cannot take this — a hash may not have that key, so say what to answer when it does not: `$prices{$k} // 0`
            $picked = $prices{"apple"};
                             ^
```

perl's order for a hash changes every time perl starts, and a screen cannot depend on that.

```console
test/refuse/unsorted_keys.pl:10:20: Rakugan cannot take this — a hash hands `keys` back in the order perl happens to hold it, which is a different order every time perl starts; write `sort keys %h`
            for my $k (keys %prices) {
                       ^
```

Building a screen twice has to build the same screen, and a number drawn while building it would not be.

```console
test/refuse/rand_in_view.pl:10:28: Rakugan cannot take this — a view calls what cannot change; draw the number in a handler and keep it in a field
            return column(text("roll: @{[ int(rand(6)) ]}"), button("go", on_click => sub { $self->go }));
                               ^
```

## Perl the compiled run has no perl for

A compiled app writes its screen, which is where the gate reads the tree from.

```console
test/refuse/say_to_stdout.pl:8:9: Rakugan cannot take this — a compiled app writes its screen, not its standard output; `warn` goes to standard error
            say "n is $n";
            ^
```

A shipped app carries no compiler.

```console
test/refuse/string_eval.pl:8:14: Rakugan cannot take this — a string `eval` compiles Perl while the app runs, and a shipped app carries no compiler; catch a failure with `try` / `catch`
            $n = eval "1 + 1";
                 ^
```

The pattern is compiled when the app is translated, so it has to be there to compile.

```console
test/refuse/pattern_built.pl:9:27: Rakugan cannot take this — a pattern here is written out; one built while the app runs would have to be compiled by something the shipped app does not carry
            $found = "abc" =~ /$needle/;
                              ^
```

A pattern that runs code needs perl, and the compiled run has none.

```console
test/refuse/pattern_code.pl:8:27: Rakugan cannot take this — a pattern that runs code (`(?{ … })`) is perl's own; the compiled run has no perl in it
            $found = "abc" =~ /a(?{ print "hi" })b/;
                              ^
```

The same reason: the replacement would be perl, run while the app runs.

```console
test/refuse/substitute_eval.pl:8:18: Rakugan cannot take this — a replacement here is text, with `$1` … `$9` for what the pattern caught; `/e` runs perl and the compiled run has none
            $line =~ s/(\d+)/$1 + 1/e;
                     ^
```

Outside that `if` it would be whatever the last successful match anywhere had left.

```console
test/refuse/capture_unguarded.pl:10:16: Rakugan cannot take this — what a pattern caught is read where the match is known to have happened: inside the `if` that made it
            $got = $1;
                   ^
```

The compiled run stops the handler at the failure, so nothing of it runs after that on either path.

```console
test/refuse/try_finally.pl:9:50: Rakugan cannot take this — `finally` runs after either path, and the compiled run has no unwinding to hang it on; write the line after the `try`
            try { $n = 1 } catch ($e) { $note = $e } finally { $n = 2 }
                                                     ^
```

Inside a loop the catch would run once per failure and the loop would go on; a `try` inside the loop says what happens then.

```console
test/refuse/try_loop.pl:12:44: Rakugan cannot take this — a `try` does not reach into a loop yet; put the `try` inside the loop, around the line that can fail
                for my $x (@xs) { $total += $x / $n }
                                               ^
```

A method is compiled once, and a `try` outside it cannot reach in; inside it, the line that can fail is right there.

```console
test/refuse/try_method.pl:12:22: Rakugan cannot take this — `halve` can fail — it divides, takes a root, writes `die` or calls the library — and a `try` here does not reach into it yet; put the `try` inside `halve`, around the line that can fail
            try { $self->halve } catch ($e) { $note = $e }
                         ^
```

A failure the compiled run cannot hand to the catch would be caught in one run and not in the other.

```console
test/refuse/try_plain_library.pl:8:15: Rakugan cannot take this — `fs_write_text` can fail, and the library has no form of it a `try` can take yet; call it before the `try`, or write its `_or` twin
            try { fs_write_text("/nonexistent/dir/x.txt", "a") } catch ($e) { $note = $e }
                  ^
```

perl never runs those lines, so the compiled run must not either; the refusal spares reading code that does nothing.

```console
test/refuse/after_die.pl:9:9: Rakugan cannot take this — nothing after `die` runs; drop these lines, or put the `die` under an `if`
            $n = 1;
            ^
```

perl runs those lines at once and the compiled run when the work is done; before the `task`, both run them at once.

```console
test/refuse/after_task.pl:10:9: Rakugan cannot take this — `task` is the last thing a handler does: the compiled run reaches these lines when the work is done, and perl reaches them at once; write them before the `task`
            $status = "working";
            ^
```

perl seeds itself from the clock and the process when nothing else does, so an unseeded app draws a different sequence every start, in either run.

```console
test/refuse/rand_unseeded.pl:7:26: Rakugan cannot take this — `rand` in an app that never calls `srand` draws a different sequence every time it starts, in both runs; seed it once (`srand(42);`) and the two runs draw the same numbers
        method go { $n = int(rand(10)) }
                             ^
```

The same reason: `srand()` with nothing picks a seed of its own.

```console
test/refuse/srand_bare.pl:8:9: Rakugan cannot take this — `srand` with nothing picks a seed of its own, a different one in each run; write the seed: `srand(42)`
            srand();
            ^
```

perl answers a fraction for a negative power and a whole number otherwise, and a type is one or the other.

```console
test/refuse/pow_variable.pl:9:16: Rakugan cannot take this — a whole number to a power that is not written out may come out a fraction, and the compiled run has to know; write `2.0 ** $n` for a fraction, or `int(2.0 ** $n)`
            $n = 2 ** $n;
                   ^
```

A Perl function the translator has no twin for yet is said to be Perl's own, with what to write instead where there is something.

```console
test/refuse/sleep_builtin.pl:9:9: Rakugan cannot take this — `sleep` is Perl's own, and not in the translator yet; a handler that waits freezes the window; `task(sub { ... }, on_done => ...)` does the waiting elsewhere
            sleep(1);
            ^
```

A shipped app is one file; a module of your own would have to be translated with it.

```console
test/refuse/module_call.pl:11:14: Rakugan cannot take this — `My::Counter::next` comes from a module of your own, and a module does not reach the compiled run yet: an app is one file, and the modules the translator knows are List::Util's and POSIX's
            $n = My::Counter::next();
                 ^
```

`eval { … }` is the older spelling of `try` / `catch`, which the dialect takes.

```console
test/refuse/eval_block.pl:9:9: Rakugan cannot take this — `eval { ... }` catches a failure; write it as perl 5.40 does: `try { ... } catch ($e) { ... }`
            eval { $n = 1 };
            ^
```

A sub held in a variable is a closure, and the compiled run has no shape for one; a method of the app is what it calls.

```console
test/refuse/sub_value.pl:9:17: Rakugan cannot take this — a sub written here has no shape in the compiled run; write a method of the app and call it
            my $f = sub { 1 };
                    ^
```

A jump to a labeled loop has no shape in the compiled run; a flag the inner loop sets does the same.

```console
test/refuse/label_loop.pl:9:9: Rakugan cannot take this — a loop with a label, and `next LABEL` / `last LABEL`, are not carried yet; a flag the inner loop sets and the outer loop reads does the same
            OUTER: for my $i (1 .. 3) {
            ^
```

A `state` variable is a field the method keeps for itself, and the app has fields for that.

```console
test/refuse/state_var.pl:9:9: Rakugan cannot take this — `state` is Perl's own, and not in the translator yet; a field of the app holds what a `state` variable would
            state $calls = 0;
            ^
```

A match walked one step at a time keeps its place in perl's own bookkeeping; taking every match first is the same list.

```console
test/refuse/match_in_loop.pl:10:9: Rakugan cannot take this — a match in a condition is asked once; to walk every match, take them all first: `my @found = ($s =~ /(\d)/g);` and loop over `@found`
            while ($t =~ /(\d)/g) { $n += 1 }
            ^
```

The names come from `$1` and `$2` once the match is known to have happened, which is inside the `if`.

```console
test/refuse/list_match.pl:9:9: Rakugan cannot take this — a match that names what it caught is written as the match, then the names: `if ($s =~ /(\d+)-(\d+)/) { my $a = $1; my $b = $2; ... }`
            if (my ($p, $q) = "3-4" =~ /(\d+)-(\d+)/) { $n = 1 }
            ^
```
