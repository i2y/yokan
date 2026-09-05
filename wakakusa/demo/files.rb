# Files, with Ruby's own File and Dir. Nothing here is Wakakusa's: both
# runs call the same methods of the same standard library, and the gate
# is what says they answer the same.
require "wakakusa"

DIR = "demo/.gate/fs_demo"
NOTE = "demo/.gate/fs_demo/note.txt"

class Files
  def initialize
    @content = "(not loaded)"
    @wrote = 0
    @names = []
    @ready = false
  end

  def save
    Dir.mkdir(DIR) unless Dir.exist?(DIR)
    @wrote = File.write(NOTE, "hello from one standard library")
  end

  def add_line
    File.write(NOTE, " (and again)", mode: "a")
  end

  def listing
    @names = Dir.children(DIR).sort
  end

  def clean
    File.delete(NOTE) if File.exist?(NOTE)
    listing
  end

  # A place of the app's own, made on the way out. A demo has no
  # business in someone's home directory, so this one keeps to the
  # directory the gate already writes in — and beside the one it lists,
  # not inside it, or the listing would depend on the order.
  def data_dir
    path = "demo/.gate/fs_demo_app"
    Dir.mkdir(path) unless Dir.exist?(path)
    @ready = Dir.exist?(path)
  end

  def entry(i)
    text @names[i]
  end

  def view
    column(spacing: 8.0, padding: 12.0) {
      text "content: #{@content}"
      text "wrote: #{@wrote} bytes"
      text "in #{DIR}: #{@names.length} file(s)"
      list_view(@names.length, item_height: 20.0, height: 44.0) { |i| entry(i) }
      text "data dir ready: #{@ready}"
      row(spacing: 6.0) {
        button("save") { save }
        button("append") { add_line }
        button("load") { @content = File.read(NOTE) }
        button("list") { listing }
        button("data dir") { data_dir }
        button("remove") { clean }
      }
    }
  end
end

run(Files.new, title: "files")
