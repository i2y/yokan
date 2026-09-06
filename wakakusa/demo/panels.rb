# The elements that arrange or cover: tracks, layers, panes that
# scroll, and a panel over the rest of the window.
require "wakakusa"

class Panels
  def initialize
    @open = false
    @view = 0
    @views = ["grid", "stack", "scrolls"]
  end

  def tracks
    grid(
      text("one"), text("two"),
      grid_cell(text("across both", align: "center", background: "#313244",
                     padding: 4.0, border_radius: 6.0), col_span: 2),
      text("three"), text("four"),
      columns: 2, spacing: 6.0
    )
  end

  def layers
    stack(
      image("demo/assets/postcard.png", width: 180.0, height: 90.0),
      text("over the picture", size: 14.0, color: "#11111b",
           background: "#f9e2af", padding: 4.0)
    )
  end

  def scrolls
    column(
      scroll_view(
        column(*(1..12).map { |n| text("line #{n}") }, spacing: 2.0),
        height: 90.0
      ),
      h_scroll_view(
        row(*(1..10).map { |n| text("col #{n}", width: 70.0) }, spacing: 6.0)
      ),
      spacing: 8.0
    )
  end

  def panel
    return tracks if @view == 0
    return layers if @view == 1

    scrolls
  end

  def view
    stack(
      column(
        row(
          text("Panels", size: 18.0),
          spacer,
          link("pixie", "https://example.invalid", size: 12.0),
          spinner(size: 14.0),
          spacing: 8.0
        ),
        segmented(options: @views, selected: @view) { |i| @view = i },
        panel,
        button("about") { @open = true },
        spacing: 10.0,
        padding: 14.0
      ),
      modal(
        column(
          text("A panel over the rest of it.", size: 14.0),
          button("close") { @open = false },
          spacing: 8.0, padding: 12.0, background: "panel"
        ),
        open: @open
      )
    )
  end
end

run(Panels.new, title: "panels")
