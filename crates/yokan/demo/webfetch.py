# /// script
# requires-python = ">=3.14"
# ///
"""http from the standard library: GET with a deadline, GET with
headers, POST, and the status code on its own. Every one of them
blocks until the answer arrives — the interpreted and the compiled
app both block on that same statement (put one in a `task` to keep
the window live). The gate needs no network: an @py escape starts
an in-process fixture server in both runs, because escapes run the
same CPython either way.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import button, column, py, row, run, State, text  # noqa: E402
from yokan import http  # noqa: E402


@py
def serve() -> int:
    import http.server
    import threading

    class H(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            who = self.headers.get("X-Who", "nobody")
            body = f"hello from fixture (for {who})".encode()
            self.send_response(200)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_POST(self):
            n = int(self.headers.get("Content-Length", "0"))
            body = b"echo: " + self.rfile.read(n)
            self.send_response(200)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, format: str, *args: object) -> None:
            pass

    srv = http.server.HTTPServer(("127.0.0.1", 0), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv.server_address[1]


port: State[int] = State(0)
content: State[str] = State("(none)")
code: State[int] = State(0)


def start():
    port.set(serve())


def fetch():
    # a second argument is the deadline in milliseconds
    content.set(http.get_text(f"http://127.0.0.1:{port()}/", 2000))


def introduce():
    content.set(http.get_text_with(f"http://127.0.0.1:{port()}/", {"X-Who": "yokan"}))


def send():
    content.set(http.post_text(f"http://127.0.0.1:{port()}/", "ping"))


def check():
    code.set(http.status(f"http://127.0.0.1:{port()}/"))


def view():
    with column(spacing=8, padding=12):
        text(f"got: {content()}")
        text(f"status: {code()}")
        with row(spacing=6):
            button("start", on_click=start)
            button("fetch", on_click=fetch)
            button("headers", on_click=introduce)
            button("post", on_click=send)
            button("status", on_click=check)


if __name__ == "__main__":
    run(view, title="webfetch")
