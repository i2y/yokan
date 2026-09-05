require "wakakusa"

class App
  def initialize
    @items = ["a", "b"]
    @picked = -1
  end

  def view
    column(spacing: 8.0) {
      @items.each_with_index do |name, i|
        button(name) { @picked = i }
      end
    }
  end
end

run(App.new, title: "x")
