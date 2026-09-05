require "wakakusa"

class App
  def view
    text("hello", weight: 700.0)
  end
end

run(App.new, title: "x")
