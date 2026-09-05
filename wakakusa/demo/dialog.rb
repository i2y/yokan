# A panel over the rest of the window, opened and closed by the app.
require "wakakusa"

class Dialog
  def initialize
    @show = false
    @status = "undecided"
  end

  def decide(answer)
    @status = answer
    @show = false
  end

  def view
    column(spacing: 10.0, padding: 14.0) {
      text "status: #{@status}", size: 16.0
      button("open dialog") { @show = true }
      if @show
        modal {
          text "accept the terms?", size: 18.0
          row(spacing: 8.0) {
            button("accept") { decide("accepted") }
            button("decline") { decide("declined") }
          }
        }
      else
        text "(dialog closed)", size: 12.0, color: "#8a8f98"
      end
    }
  end
end

run(Dialog.new, title: "dialog")
