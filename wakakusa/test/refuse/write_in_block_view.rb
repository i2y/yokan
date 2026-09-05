require "wakakusa"

class App
  def initialize
    @seen = 0
  end

  def view
    column(spacing: 8.0) {
      @seen = @seen + 1
      text "seen #{@seen}"
    }
  end
end

run(App.new, title: "x")
