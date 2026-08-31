# /// script
# requires-python = ">=3.14"
# ///
"""http from the standard library. get_text blocks until the
response arrives — the interpreted and the compiled app both block
on that same statement. The gate needs no network: an @py escape
starts an in-process fixture server in both runs, because escapes
run the same CPython either way.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

import yokan as ui  # noqa: E402
from yokan import py, State  # noqa: E402
from yokan import http  # noqa: E402


@py
def serve() -> int:
    import http.server
    import threading

    class H(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            body = b"hello from fixture"
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


def start():
    port.set(serve())


def fetch():
    content.set(http.get_text(f"http://127.0.0.1:{port()}/"))


def view():
    with ui.column(spacing=8, padding=12):
        ui.text(f"got: {content()}")
        with ui.row(spacing=6):
            ui.button("start", on_click=start)
            ui.button("fetch", on_click=fetch)


if __name__ == "__main__":
    ui.run(view, title="webfetch")
