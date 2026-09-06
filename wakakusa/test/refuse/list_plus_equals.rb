require "wakakusa"

class App
  def initialize
    @items = []
  end

  def add(t)
    @items += [t]
  end

  def view
    text("#{@items.length} items")
  end
end

run(App.new, title: "x")
