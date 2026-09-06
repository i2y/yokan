# The reference: everything in this file is what Rakugan takes. The app
# is a class, its state is its fields, `view` is a method, and a handler
# is an anonymous sub that closes over the fields.
#
#   rakugan run  demo/counter.pl
#   rakugan gate demo/counter.pl --script "click:+1,input:Momo"
use Rakugan;

class Counter {
    use Rakugan;
    field $count = 0;
    field $name  = "";

    method view {
        column(
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
