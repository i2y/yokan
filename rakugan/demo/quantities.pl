# The two fields that hold a number rather than text: enter or leaving
# them commits, text that is not a number is dropped and the shown
# value returns to what the app holds.
use Rakugan;

class Order {
    use Rakugan;
    field $qty   = 1;
    field $price = 0.0;

    method reset {
        $qty = 1;
        $price = 0.0;
    }

    method view {
        return column(
            text("Order line", size => 18),
            row(
                text("quantity"),
                int_field($qty, min => 1, max => 99, placeholder => "qty",
                          on_change => sub ($n) { $qty = $n }),
                spacing => 8,
            ),
            row(
                text("unit price"),
                number_field($price, min => 0, max => 1000, step => 0.5,
                             placeholder => "price",
                             on_change => sub ($p) { $price = $p }),
                spacing => 8,
            ),
            text("total  @{[ $qty * $price ]}"),
            button("reset", on_click => sub { $self->reset }),
            spacing => 10,
            padding => 14,
        );
    }
}

run(Order->new, title => "quantities");
