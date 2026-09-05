# What a screen reader is told, and what the pointer shows. `role:`
# takes a value, so the summary line is a heading until there is a
# result under it and then it is not.
require "wakakusa"

class Labels
  def initialize
    @title = "Reports"
    @query = ""
    @summary_role = "heading"
  end

  def view
    column(
      text(@title, size: 22.0, role: "heading"),
      row(
        svg("demo/assets/yokan.svg", width: 20.0, height: 20.0, a11y_label: "Yokan"),
        svg("demo/assets/search.svg", width: 20.0, height: 20.0, a11y_label: "Search"),
        # The one element carrying a tooltip, a role, a name and a tween
        # at once, which is what pins the order they wrap in.
        button("save", animate: 150.0, easing: "out", role: "button",
               a11y_label: "Save the report", tooltip: "Save this report") do
          @summary_role = "label"
        end,
        spacing: 6.0, role: "group", a11y_label: "toolbar"
      ),
      text_field(@query, placeholder: "search", a11y_label: "search") { |q| @query = q },
      text("1 of 4 saved", role: @summary_role),
      progress(0.4),
      spacing: 8.0,
      padding: 12.0
    )
  end
end

run(Labels.new, title: "labels", width: 420.0, height: 320.0)
