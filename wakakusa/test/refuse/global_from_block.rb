require "wakakusa"

$name = ""

class App
  def view
    text_field($name) { |s| $name = s }
  end
end

run(App.new, title: "x")
