require "wakakusa"

class App
  def initialize
    @seen = 0
  end

  def view
    @seen = @seen + 1
    text("seen #{@seen}")
  end
end

run(App.new, title: "x")
