require "wakakusa"

class App
  def initialize
    @n = 0
  end

  def view
    button("+1", on_click: :bump)
  end
end

run(App.new, title: "x")
