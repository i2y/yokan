# Links that open a page, and the system clipboard.
require "wakakusa"

class About
  def initialize
    @status = ""
  end

  def view
    column(spacing: 8.0, padding: 14.0) {
      text "Wakakusa", size: 28.0
      text "version 0.1.0"
      link("Website", "https://i2y.github.io/yokan/")
      link("Source", "https://github.com/i2y/yokan")
      link("Docs", "https://i2y.github.io/yokan/tour/")
      button("copy link") do
        clipboard_set("https://github.com/i2y/yokan")
        @status = "copied"
      end
      text "status: #{@status}"
    }
  end
end

run(About.new, title: "about")
