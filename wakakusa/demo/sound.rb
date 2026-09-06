# Sound. A WAV file is played and the call answers at once, so a handler
# that starts one carries on.
#
# A run under a script is silent: a gate must not need a machine with
# speakers, and both runs read that one flag through the same library,
# so neither is louder than the other. That is why this demo can be
# gated at all — the screen is what the two runs compare.
require "wakakusa"

DIR = File.join(__dir__, "assets", "sound")

class Sound
  def initialize
    @played = 0
    @last = "-"
    @volume = 0.6
  end

  def play(name)
    audio_play(File.join(DIR, "#{name}.wav"), @volume)
    @played += 1
    @last = name
  end

  def hush
    audio_stop
    @last = "stopped"
  end

  def view
    column(spacing: 10.0, padding: 14.0) {
      text "sound", size: 18.0, bold: true
      text "played: #{@played}   last: #{@last}"
      row(spacing: 6.0) {
        button("jump") { play("jump") }
        button("pickup") { play("pickup") }
        button("blast") { play("blast") }
      }
      row(spacing: 6.0) {
        button("shoot") { play("shoot") }
        button("over") { play("over") }
        button("stop") { hush }
      }
      slider(value: @volume, min: 0.0, max: 1.0, step: 0.1) { |v| @volume = v }
      text format("volume %.1f", @volume), size: 12.0, color: "#8a8f98"
    }
  end
end

run(Sound.new, title: "sound")
