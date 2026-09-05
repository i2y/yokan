# A page fetched and read, with nothing outside the machine involved:
# the app serves the document to itself on a port the operating system
# picks, so both runs read the same bytes and the gate can compare them.
#
# The fetch happens off the window's thread, which is what `task` is
# for; the server answers on a thread of its own.
require "wakakusa"
require "net/http"
require "uri"
require "socket"
require "json"

BODY = '{"items": [' \
       '{"title": "wakakusa ships native ruby apps", "points": 128},' \
       '{"title": "one engine, two doors", "points": 64},' \
       '{"title": "the gate arbitrates", "points": 256}' \
       ']}'

class Reader
  def initialize
    @status = "idle"
    @titles = []
    @top = "-"
    @port = 0
  end

  # A server that answers exactly one request and then closes. Serving
  # in a loop would leave a thread sitting in `accept`, and a compiled
  # run waits for its threads before it exits.
  def serve_one
    server = TCPServer.new("127.0.0.1", 0)
    @port = server.addr[1]
    Thread.new do
      socket = server.accept
      # past the request head: a blank line is where it ends
      loop do
        line = socket.gets
        break if line.nil? || line == "\r\n"
      end
      socket.print("HTTP/1.1 200 OK\r\nContent-Length: #{BODY.bytesize}\r\n" \
                   "Connection: close\r\n\r\n#{BODY}")
      socket.close
      server.close
    end
  end

  def fetch
    @status = "fetching"
    serve_one
    job = task { Net::HTTP.get(URI("http://127.0.0.1:#{@port}/")) }
    on_done(job) { read(task_answer) }
  end

  def read(body)
    doc = JSON.parse(body)
    items = doc["items"]
    @titles = items.map { |it| it["title"] }
    best = items.max_by { |it| it["points"] }
    @top = "#{best["title"]} (#{best["points"]})"
    @status = "read #{items.length} items"
  end

  def line(i)
    text @titles[i], size: 13.0
  end

  def view
    column(spacing: 8.0, padding: 12.0) {
      text "reader", size: 18.0, bold: true
      text "status: #{@status}", size: 12.0, color: "#8a8f98"
      text "top: #{@top}"
      list_view(@titles.length, item_height: 22.0, height: 80.0) { |i| line(i) }
      button("fetch") { fetch }
    }
  end
end

run(Reader.new, title: "reader")
