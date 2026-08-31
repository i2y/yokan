# /// script
# requires-python = ">=3.14"
# ///
"""try/except over the standard library: a failing http.get_text
raises, a Python `try` around it catches, and `f"{e}"` renders the
same message whether the app runs interpreted or compiled. An
uncaught failure aborts just the handler that raised — the app
keeps running.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

import yokan as ui  # noqa: E402
from yokan import py, State  # noqa: E402
from yokan import fs, http  # noqa: E402


@py
def serve() -> int:
    import http.server
    import threading

    body = b"hello from fixture"

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


@py
def parse_num(s: str) -> int:
    return int(s)


@py
def risky(mode: str) -> int:
    if mode == "value":
        raise ValueError("bad value here")
    if mode == "key":
        raise KeyError("missing-key")
    return 7


port: State[int] = State(0)
num: State[int] = State(0)
body: State[str] = State("(none)")
status: State[str] = State("-")
note: State[str] = State("-")


def start():
    port.set(serve())


def fetch_dead():
    try:
        body.set(http.get_text("http://127.0.0.1:9/nothing"))
    except Exception as e:
        status.set(f"offline: {e}")


def fetch_ok():
    try:
        body.set(http.get_text(f"http://127.0.0.1:{port()}/"))
    except Exception:
        status.set("unreachable")


def peek():
    try:
        note.set(fs.read_text("demo/.gate/absent.txt"))
    except Exception as e:
        note.set(f"no file: {e}")


def parse():
    try:
        num.set(parse_num("41x"))
    except Exception as e:
        note.set(f"bad: {e}")


def parse_ok():
    try:
        num.set(parse_num("41"))
    except Exception:
        note.set("unexpected")


def multi_v():
    try:
        num.set(risky("value"))
    except ValueError as e:
        note.set(f"VE: {e}")
    except KeyError as e:
        note.set(f"KE: {e}")
    except Exception:
        note.set("other")


def multi_k():
    try:
        num.set(risky("key"))
    except ValueError as e:
        note.set(f"VE: {e}")
    except KeyError as e:
        note.set(f"KE: {e}")
    except Exception:
        note.set("other")


def full():
    try:
        a = risky("fine")
        note.set(f"got {a}")
        b = risky("value")
        num.set(a + b)
    except (ValueError, KeyError) as e:
        status.set(f"caught: {e}")
    except Exception:
        status.set("other")
    else:
        status.set("clean run")
    finally:
        body.set("finally ran")


def full_ok():
    try:
        a = risky("fine")
        b = risky("fine")
        num.set(a + b)
    except Exception as e:
        status.set(f"caught: {e}")
    else:
        status.set("clean run")
    finally:
        body.set("finally ran")


def mixed():
    try:
        note.set(fs.read_text("demo/.gate/absent.txt"))
    except (KeyError, RuntimeError) as e:
        status.set(f"io: {e}")
    except Exception:
        status.set("other")


def multi_ok():
    try:
        num.set(risky("fine"))
    except ValueError:
        note.set("VE")
    except Exception:
        note.set("other")


def view():
    with ui.column(spacing=8, padding=12):
        ui.text(f"body: {body()}")
        ui.text(f"status: {status()}", size=12)
        ui.text(f"note: {note()}", size=12)
        ui.text(f"num: {num()}", size=12)
        with ui.row(spacing=6):
            ui.button("start", on_click=start)
            ui.button("dead", on_click=fetch_dead)
            ui.button("ok", on_click=fetch_ok)
            ui.button("peek", on_click=peek)
            ui.button("parse", on_click=parse)
            ui.button("parse_ok", on_click=parse_ok)
            ui.button("mv", on_click=multi_v)
            ui.button("mk", on_click=multi_k)
            ui.button("mo", on_click=multi_ok)
            ui.button("full", on_click=full)
            ui.button("full_ok", on_click=full_ok)
            ui.button("mixed", on_click=mixed)


if __name__ == "__main__":
    ui.run(view, title="tryfetch")
