#!/usr/bin/env ruby
# frozen_string_literal: true
#
# Every complete app in the tour, run.
#
# A fenced `ruby` block that requires wakakusa and ends in `run(` is an
# app, not an illustration, so it is written out and put through the
# same command a demo goes through. A `<!-- script: … -->` line just
# above the fence says what to drive it with; without one the block is
# only checked, which is what an example with no state to change needs.
#
# They are written into `demo/.gate/`, beside every other generated
# thing. The point is that the tour cannot drift. A rename in the vocabulary
# or a new refusal breaks the page that teaches it, here, before a
# reader meets it.

require "fileutils"

ROOT = File.expand_path("..", __dir__)
OUT = File.join(ROOT, "demo", ".gate")

def apps(path)
  found = []
  script = nil
  fence = nil
  File.readlines(path).each do |line|
    if fence
      if line.start_with?("```")
        found.push([script, fence.join]) if fence.join.include?("run(")
        fence = nil
        script = nil
      else
        fence.push(line)
      end
      next
    end
    if (m = line.match(/<!-- script:\s*(.*?)\s*-->/))
      script = m[1]
    elsif line.start_with?("```ruby")
      fence = []
    elsif !line.strip.empty?
      script = nil
    end
  end
  found.select { |(_, body)| body.include?('require "wakakusa"') }
end

def main
  pages = ARGV.empty? ? [File.join(ROOT, "TOUR.md")] : ARGV
  FileUtils.mkdir_p(OUT)
  fail_count = 0
  pages.each do |page|
    list = apps(page)
    warn "#{File.basename(page)}: no complete app in it" if list.empty?
    list.each_with_index do |(script, body), i|
      # The whole path names the file, so a page under website/ cannot
      # overwrite the tour page of the same name.
      stem = page.sub(%r{\A\./}, "").sub(/\.md\z/, "").downcase.tr("/.-", "___")
      file = File.join(OUT, format("%s_%02d.rb", stem, i))
      File.write(file, body)
      cmd = [File.join(ROOT, "bin", "wakakusa")]
      cmd += script ? ["gate", file, "--script", script] : ["check", file]
      ok = system(*cmd, out: File::NULL)
      puts format("%-4s %s%s", ok ? "OK" : "FAIL", File.basename(file), script ? "  (#{script})" : "")
      fail_count += 1 unless ok
    end
  end
  puts "TOUR: #{fail_count.zero? ? "every example runs" : "#{fail_count} failed"}"
  exit(fail_count.zero? ? 0 : 1)
end

main
