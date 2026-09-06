#!/usr/bin/env ruby
# frozen_string_literal: true
#
# The demos' sounds, written from arithmetic.
#
# Wakakusa plays WAV files, and a repository is a poor place for sound
# nobody can regenerate — so the files under `demo/assets/sound/` are
# made here, out of square waves and an envelope, in the spirit of the
# machines the two ported games came from. Run it after changing a
# recipe; the files are committed so a checkout needs no Ruby to play
# them.
#
#   tools/gen_sounds.rb            write the WAVs
#   tools/gen_sounds.rb --check    fail if what is on disk differs

require "fileutils"

RATE = 22_050
OUT = File.expand_path("../demo/assets/sound", __dir__)

# A note is a frequency in Hz and a length in seconds; 0 Hz is noise,
# which is what an explosion is made of.
def tone(freq, secs, shape: :square, from: 1.0, to: 0.0, seed: 1)
  n = (RATE * secs).round
  rnd = seed
  (0...n).map do |i|
    t = i.to_f / RATE
    env = from + (to - from) * (i.to_f / n)
    v =
      if freq.zero?
        # a small deterministic generator, so two runs of this script
        # write the same bytes
        rnd = (rnd * 1_103_515_245 + 12_345) & 0x7fffffff
        ((rnd >> 16) & 1).zero? ? -1.0 : 1.0
      else
        phase = (t * freq) % 1.0
        case shape
        when :square then phase < 0.5 ? 1.0 : -1.0
        when :triangle then (phase < 0.5 ? phase * 4 - 1 : 3 - phase * 4)
        else Math.sin(phase * 2 * Math::PI)
        end
      end
    (v * env * 0.35 * 32_767).round.clamp(-32_768, 32_767)
  end
end

def silence(secs)
  Array.new((RATE * secs).round, 0)
end

def wav(samples)
  data = samples.pack("s<*")
  [
    "RIFF", [36 + data.bytesize].pack("V"), "WAVE",
    "fmt ", [16, 1, 1, RATE, RATE * 2, 2, 16].pack("Vv2V2v2"),
    "data", [data.bytesize].pack("V"), data
  ].join
end

# name => the notes it is made of.
SOUNDS = {
  # the player leaves the floor: up, quickly
  "jump" => tone(440, 0.04) + tone(660, 0.05) + tone(880, 0.05, to: 0.2),
  # something is picked up: two notes, the second higher
  "pickup" => tone(880, 0.05) + tone(1320, 0.09, to: 0.1),
  # a shot leaves the ship
  "shoot" => tone(1200, 0.03, from: 0.8) + tone(700, 0.05, from: 0.5, to: 0.0),
  # something is hit: noise, falling away
  "blast" => tone(0, 0.22, from: 0.9, to: 0.0),
  # the run is over: three notes down
  "over" => tone(392, 0.11) + tone(311, 0.11) + tone(233, 0.22, to: 0.0),
  # a small confirmation, for the demo
  "blip" => tone(660, 0.05, shape: :triangle, to: 0.2),
}.freeze

check = ARGV.include?("--check")
FileUtils.mkdir_p(OUT) unless check
stale = []
SOUNDS.each do |name, samples|
  path = File.join(OUT, "#{name}.wav")
  bytes = wav(samples)
  if check
    stale << name unless File.exist?(path) && File.binread(path) == bytes
  else
    File.binwrite(path, bytes)
    puts format("%-8s %6d bytes", name, bytes.bytesize)
  end
end

if check
  unless stale.empty?
    warn "gen_sounds: behind the recipes: #{stale.join(", ")}"
    exit 1
  end
  puts "sounds: up to date"
end
