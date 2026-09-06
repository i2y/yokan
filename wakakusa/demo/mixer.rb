# State an app keeps and a field that writes into it.
require "wakakusa"

class Mixer
  def initialize
    @volume = 5
    @title = "untitled"
    @muted = false
  end

  def view
    cells = [
      text("#{@title} — vol #{@volume}", size: 16.0),
      row(
        button("+1") { @volume += 1 },
        button("mute") { @muted = true },
        button("unmute") { @muted = false },
        spacing: 8.0
      ),
    ]
    cells.push(text("(muted)", size: 12.0, color: "#8a8f98")) if @muted
    cells.push(text_field(@title, placeholder: "title") { |t| @title = t })
    column(*cells, spacing: 10.0, padding: 14.0)
  end
end

run(Mixer.new, title: "mixer")
