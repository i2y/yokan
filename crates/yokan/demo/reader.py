# /// script
# requires-python = ">=3.14"
# ///
"""A feed reader: http + json over a realistic nested payload. The
fixture is an @py escape serving JSON in BOTH tiers, the parse
loop builds rows with dynamic paths (f"items.{i}.title"), and the
list renders through the virtualized list_view.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import (
    button,
    column,
    list_view,
    py,
    row,
    run,
    State,
    store,
    text,
)
from yokan import http, jsondoc  # noqa: E402


@py
def serve() -> int:
    import http.server
    import threading

    body = (
        '{"items": ['
        '{"title": "yokan ships native python apps", "points": 128},'
        '{"title": "one rust crate, two doors", "points": 64},'
        '{"title": "the gate arbitrates", "points": 256}'
        "]}"
    ).encode()

    class H(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
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


@store
class Feed:
    rows: list[str] = []
    total_points: int = 0

    def refresh(self, src: str) -> None:
        self.rows = []
        self.total_points = 0
        for i in range(jsondoc.length(src, "items")):
            self.rows = self.rows + [jsondoc.get_text(src, f"items.{i}.title")]
            self.total_points += jsondoc.get_int(src, f"items.{i}.points")


def start():
    port.set(serve())


def fetch():
    Feed.refresh(http.get_text(f"http://127.0.0.1:{port()}/feed"))


def item_row(i):
    return text(Feed.rows[i])


def view():
    with column(spacing=8, padding=12):
        text(f"stories={len(Feed.rows)} points={Feed.total_points}", size=16)
        list_view(len(Feed.rows), item_row, item_height=22.0, height=90.0)
        with row(spacing=6):
            button("start", on_click=start)
            button("fetch", on_click=fetch)


if __name__ == "__main__":
    run(view, title="reader")
