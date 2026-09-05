require "wakakusa"

class App
  def initialize
    @n = 0
  end

  def view
    button("+1", on_click: proc { @n += 1 })
  end
end

run(App.new, title: "x")
