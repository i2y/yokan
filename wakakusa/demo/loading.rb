# The bar that fills, in its three forms: with a caption above it, at a
# size the app chose, and sweeping for work with no known length.
require "wakakusa"

class Loading
  def initialize
    @ratio = 0.25
    @busy = false
  end

  def step
    @ratio = @ratio >= 1.0 ? 0.0 : @ratio + 0.25
  end

  def view
    column(
      text("ratio: #{@ratio}"),
      progress(@ratio, label: "Uploading"),
      progress(@ratio, width: 240.0, height: 6.0),
      progress(@ratio, indeterminate: @busy),
      row(
        button("step") { step },
        button("busy") { @busy = !@busy },
        spacing: 8.0
      ),
      spacing: 12.0,
      padding: 16.0
    )
  end
end

run(Loading.new, title: "loading")
