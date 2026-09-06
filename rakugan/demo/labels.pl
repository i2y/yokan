# What a screen reader is told, and what the pointer shows. `role` takes
# a value, so the summary line is a heading until there is a result
# under it and then it is not.
use Rakugan;

class Labels {
    use Rakugan;
    field $title        = "Reports";
    field $query        = "";
    field $summary_role = "heading";

    method view {
        column(
            text($title, size => 22, role => "heading"),
            row(
                svg("demo/assets/yokan.svg", width => 20, height => 20, a11y_label => "Yokan"),
                svg("demo/assets/search.svg", width => 20, height => 20, a11y_label => "Search"),
                # The one element carrying a tooltip, a role, a name and a tween
                # at once, which is what pins the order they wrap in.
                button("save", animate => 150, easing => "out", role => "button",
                       a11y_label => "Save the report", tooltip => "Save this report",
                       on_click => sub { $summary_role = "label" }),
                spacing => 6, role => "group", a11y_label => "toolbar",
            ),
            text_field($query, placeholder => "search", a11y_label => "search",
                       on_change => sub ($q) { $query = $q }),
            text("1 of 4 saved", role => $summary_role),
            progress(0.4),
            spacing => 8,
            padding => 12,
        );
    }
}

run(Labels->new, title => "labels", width => 420, height => 320);
