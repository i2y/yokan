# /// script
# requires-python = ">=3.14"
# ///
"""An image, a vector icon, and an OS notification. `image` and
`svg` take a path (resolved from the run directory in development
and from beside the executable after shipping) with `width=` /
`height=`; an svg renders as a monochrome icon, tinted with the
theme's text color. `notify.send(title, body)` queues an OS notification:
delivered through Notification Center when the app runs as an
`.app` bundle; a bare dev run and headless runs drop it quietly."""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import State, button, column, image, notify, row, run, svg, text  # noqa: E402

sent: State[int] = State(0)


def send():
    notify.send("Yokan", "a postcard from the demo")
    sent.set(sent() + 1)


def view():
    with column(spacing=10, padding=14):
        text("postcard", size=22)
        with row(spacing=12):
            image("demo/assets/postcard.png", width=160.0, height=100.0)
            svg("demo/assets/yokan.svg", width=56.0, height=56.0)
        text(f"sent: {sent()}")
        button("send", on_click=send)


if __name__ == "__main__":
    run(view, title="postcard")
