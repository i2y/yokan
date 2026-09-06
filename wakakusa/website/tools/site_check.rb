#!/usr/bin/env ruby
# frozen_string_literal: true
#
# What the site claims, checked against the tree it claims it about.
#
# The pages quote things that live elsewhere: the exact wording of a
# refusal, the vocabulary table, the list of demos. Each of those has
# one source of truth, and this program fails when a page has drifted
# from it. It reads only; nothing here rewrites a page.
#
#   tools/site_check.rb

SITE = File.expand_path("..", __dir__)
ROOT = File.expand_path("..", SITE)

fails = []
def ok(label) = puts("OK   #{label}")

# --- the two generated pages ------------------------------------------------

%w[elements_page demos_page].each do |gen|
  if system(RbConfig.ruby, File.join(__dir__, "#{gen}.rb"), "--check", out: File::NULL)
    ok gen
  else
    fails << "#{gen} is behind its source (run tools/#{gen}.rb)"
  end
end

# --- the refusals page quotes what `check` really prints ---------------------

pages = %w[docs/refusals.md docs-ja/refusals.md].map { |p| [p, File.read(File.join(SITE, p))] }
Dir[File.join(ROOT, "test", "refuse", "*.expected")].sort.each do |fixture|
  # `path:line:col: <message>` — the message is what a page quotes; the
  # path in front of it is the fixture's own.
  first = File.readlines(fixture).first.to_s
  message = first.sub(/\A\S+?:\d+:\d+:\s*/, "").strip
  name = File.basename(fixture, ".expected")
  missing = pages.reject { |_, text| text.include?(message) }.map(&:first)
  if missing.empty?
    ok "refusal #{name}"
  else
    fails << "the message from test/refuse/#{name} is not on #{missing.join(", ")}"
  end
end

# --- both languages carry the same pages ------------------------------------

en = Dir[File.join(SITE, "docs", "*.md")].map { |f| File.basename(f) }.sort
ja = Dir[File.join(SITE, "docs-ja", "*.md")].map { |f| File.basename(f) }.sort
if en == ja
  ok "both languages: #{en.length} pages"
else
  fails << "the two languages differ: only EN #{(en - ja).join(", ")}; only JA #{(ja - en).join(", ")}"
end

# --- every page in the nav exists, and every page is in the nav -------------

{ "zensical.toml" => "docs", "zensical.ja.toml" => "docs-ja" }.each do |config, dir|
  text = File.read(File.join(SITE, config))
  nav = text[/^nav = \[.*?^\]/m].to_s.scan(/"([\w-]+\.md)"/).flatten
  files = Dir[File.join(SITE, dir, "*.md")].map { |f| File.basename(f) }
  gone = nav - files
  unlisted = files - nav
  fails << "#{config} lists a page that is not there: #{gone.join(", ")}" unless gone.empty?
  fails << "#{dir} has a page the nav never shows: #{unlisted.join(", ")}" unless unlisted.empty?
  ok "#{config}: #{nav.length} pages in the nav" if gone.empty? && unlisted.empty?
end

# --- images: every page's images exist --------------------------------------

%w[docs docs-ja].each do |dir|
  missing = []
  Dir[File.join(SITE, dir, "*.md")].each do |page|
    File.read(page).scan(/(?:!\[[^\]]*\]\(|<img src=")(images\/[^)"#]+)/).flatten.uniq.each do |rel|
      missing << "#{File.basename(page)} → #{rel}" unless File.file?(File.join(SITE, dir, rel))
    end
  end
  if missing.empty?
    ok "#{dir}: every image is there"
  else
    fails << "#{dir} points at images that are not there: #{missing.join(", ")}"
  end
end

if fails.empty?
  puts "SITE OK"
else
  fails.each { |f| warn "FAIL #{f}" }
  exit 1
end
