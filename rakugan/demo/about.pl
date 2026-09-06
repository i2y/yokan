# Links that open a page, and the system clipboard.
use Rakugan;

class About {
    use Rakugan;
    field $status = "";

    method view {
        return column(
            text("Rakugan", size => 28),
            text("version 0.1.0"),
            link("Website", "https://i2y.github.io/yokan/"),
            link("Source", "https://github.com/i2y/yokan"),
            link("Docs", "https://i2y.github.io/yokan/tour/"),
            button("copy link", on_click => sub {
                clipboard_set_text("https://github.com/i2y/yokan");
                $status = "copied";
            }),
            text("status: $status"),
            spacing => 8,
            padding => 14,
        );
    }
}

run(About->new, title => "about");
