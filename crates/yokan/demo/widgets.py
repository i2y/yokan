# /// script
# requires-python = ">=3.14"
# ///
"""Every element in the catalog, in one app.

The chrome is two of them: a `list_view` of pages you pick from, and
the `scroll_view` beside it that holds the page. (The `split` is on
the Arranging page with the other arrangers, because a split's panes
are its arguments and a page needs a block to put an `if` in.)

A verification script walks the pages by name. `select:Input` picks
from the app's FIRST chooser, which is that list, and whatever the
page itself offers is numbered after it — `select@1:` and up. The
sweep's script visits all nine, which is how "every element still
dumps what it did" stays a line in the gate rather than a claim here.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from yokan import (  # noqa: E402
    bar_chart,
    button,
    canvas,
    checkbox,
    circle,
    column,
    component,
    context_menu,
    data_table,
    divider,
    grid,
    h_scroll_view,
    image,
    int_field,
    line,
    line_chart,
    link,
    list_view,
    menu_button,
    modal,
    number_field,
    pixel,
    pixel_text,
    progress,
    radio_group,
    rect,
    row,
    run,
    scroll_view,
    segmented,
    select,
    slider,
    spacer,
    spinner,
    split,
    stack,
    store,
    style,
    svg,
    switch,
    tab_bar,
    table,
    text,
    text_field,
    toast,
)

heading = style(size=18, color="accent")
faint = style(size=12, color="#8a8f98")
panel = style(padding=10, background="#1f2027", border_radius=8.0)
cell = style(size=12, background="#313244", padding=6, border_radius=6.0, align="center")


@store
class Gallery:
    pages: list[str] = [
        "Arranging",
        "Text",
        "Input",
        "Choosers",
        "Lists",
        "Reporting",
        "Over the app",
        "Canvas",
        "Riders",
    ]
    page: int = 0
    ratio: float = 0.28

    # What the input page binds. Every control shows a value the app
    # holds and hands the new one back — the same contract nine times.
    name: str = "Momo"
    note: str = "two lines\nof notes"
    price: float = 2.5
    qty: int = 3
    dark: bool = True
    wifi: bool = False
    volume: float = 6.0

    # The choosers: one list of options, four spellings, one index.
    fruits: list[str] = ["apple", "banana", "cherry"]
    fruit: int = 1
    tab: int = 0
    size: int = 2
    chosen: str = "nothing yet"

    # Lists and tables.
    rows: list[str] = ["ada", "bo", "cy", "dee", "eve", "fay"]
    kbs: list[int] = [12, 7, 31, 4, 19, 55]
    picked: int = -1
    sort_col: int = -1
    desc: bool = False

    # Reporting.
    pct: float = 0.62
    bars: list[float] = [3.0, 7.0, 5.0, 9.0, 4.0]
    trend: list[float] = [2.0, 4.0, 3.5, 6.0, 5.5, 8.0]

    # What is open over the app.
    asking: bool = False
    saved: bool = False

    # The canvas palette; a command's color is an index into it.
    palette: list[str] = ["#11111b", "#89b4fa", "#f38ba8", "#eeeeee"]

    def go(self, i: int) -> None:
        self.page = i

    def widen(self, r: float) -> None:
        self.ratio = min(0.5, max(0.15, r))

    def set_name(self, s: str) -> None:
        self.name = s

    def set_note(self, s: str) -> None:
        self.note = s

    def set_price(self, v: float) -> None:
        self.price = v

    def set_qty(self, n: int) -> None:
        self.qty = n

    def set_dark(self, on: bool) -> None:
        self.dark = on

    def set_wifi(self, on: bool) -> None:
        self.wifi = on

    def set_volume(self, v: float) -> None:
        self.volume = v

    def pick_fruit(self, i: int) -> None:
        self.fruit = i

    def pick_tab(self, i: int) -> None:
        self.tab = i

    def pick_size(self, i: int) -> None:
        self.size = i

    def act(self, i: int) -> None:
        self.chosen = self.fruits[i]

    def pick_row(self, i: int) -> None:
        self.picked = i

    def sort_by(self, i: int) -> None:
        self.sort_col = i
        self.desc = not self.desc

    def ask(self) -> None:
        self.asking = True

    def close(self) -> None:
        self.asking = False

    def save(self) -> None:
        self.saved = True

    def saved_shown(self) -> None:
        self.saved = False
        self.chosen = "the toast closed itself"


def page_row(i: int):
    """A row of the page list: the text a script picks it by."""
    with row(spacing=6, padding=6):
        text(Gallery.pages[i], size=14)


def file_row(i: int):
    # The padding keeps the right-hand number clear of the scrollbar
    # the viewport paints over its edge.
    with row(spacing=8, padding=6):
        text(Gallery.rows[i], size=14, grow=1.0)
        text(f"{Gallery.kbs[i]} kb", size=12, color="#7aa2f7")


def file_cells(i: int):
    return row(
        text(Gallery.rows[i]),
        text(f"{Gallery.kbs[i]}"),
    )


@component
def arranging():
    with column(spacing=10, **panel):
        text("column, row, grid, stack, spacer, divider, split, h_scroll_view", **faint)
        with row(spacing=8):
            text("a row")
            spacer()
            text("pushed to the edge", **faint)
        divider()
        # A grid lays equal tracks; `col_span=` is a rider, so it works
        # on any element rather than on a special cell.
        with grid(columns=3, spacing=6):
            text("one", **cell)
            text("two", **cell)
            text("three", **cell)
            text("spans two tracks", col_span=2, **cell)
            text("four", **cell)
        divider(color="accent", thickness=2.0)
        with stack():
            text("stacked under", size=28, color="#3b3f4a")
            text("and over", size=14)
        split(
            column(text("a pane"), padding=10, background="#313244", grow=1.0),
            column(text("and the other"), padding=10, background="#45475a", grow=1.0),
            ratio=Gallery.ratio,
            on_change=Gallery.widen,
            height=70.0,
        )
        with h_scroll_view():
            with row(spacing=8):
                text("one")
                text("two")
                text("three")
                text("four")
                text("five")
                text("and it scrolls sideways", **faint)


@component
def texts():
    with column(spacing=8, **panel):
        text("text, link, image, svg", **faint)
        text("bold", bold=True)
        text("italic", italic=True)
        text("mono 0x1f", mono=True)
        text("underlined", underline=True)
        text("a pill", size=12, background="#313244", padding=4, border_radius=6.0)
        text("one line, clipped with an ellipsis when it does not fit", wrap="ellipsis", width=180.0)
        link("yokan on GitHub", "https://github.com/i2y/yokan")
        with row(spacing=12):
            image("demo/assets/postcard.png", width=120.0, height=75.0)
            svg("demo/assets/yokan.svg", width=48.0, height=48.0)


@component
def inputs():
    with column(spacing=8, **panel):
        text("button, text_field, number_field, int_field, checkbox, switch, slider", **faint)
        button("a button", on_click=Gallery.save)
        text_field(Gallery.name, placeholder="name", on_change=Gallery.set_name)
        text_field(Gallery.note, multiline=True, rows=2, on_change=Gallery.set_note)
        number_field(Gallery.price, min=0.0, max=100.0, step=0.5, placeholder="price",
                     on_change=Gallery.set_price)
        int_field(Gallery.qty, min=1, max=99, placeholder="qty", on_change=Gallery.set_qty)
        checkbox("Dark mode", checked=Gallery.dark, on_change=Gallery.set_dark)
        switch("Wi-Fi", checked=Gallery.wifi, on_change=Gallery.set_wifi)
        slider(value=Gallery.volume, min=0.0, max=10.0, step=1.0, on_change=Gallery.set_volume)
        text(f"{Gallery.name}, {Gallery.qty} at {Gallery.price}, volume {Gallery.volume}", **faint)


@component
def choosers():
    with column(spacing=8, **panel):
        text("select, radio_group, tab_bar, segmented, menu_button", **faint)
        # A column stretches its children across it, so the two that
        # look wrong at full width sit in a row instead.
        with row(spacing=8):
            select(options=Gallery.fruits, selected=Gallery.fruit, on_change=Gallery.pick_fruit)
            menu_button("Actions", Gallery.fruits, on_select=Gallery.act)
        radio_group(options=Gallery.fruits, selected=Gallery.fruit, on_change=Gallery.pick_fruit)
        tab_bar(labels=Gallery.fruits, active=Gallery.tab, on_change=Gallery.pick_tab)
        with row(spacing=8):
            segmented(options=["S", "M", "L"], selected=Gallery.size, on_change=Gallery.pick_size)
        text(f"fruit {Gallery.fruit}, tab {Gallery.tab}, size {Gallery.size}", **faint)


@component
def lists():
    with column(spacing=8, **panel):
        text("list_view, table, data_table", **faint)
        list_view(
            len(Gallery.rows),
            file_row,
            item_height=26.0,
            height=78.0,
            selected=Gallery.picked,
            on_select=Gallery.pick_row,
            scroll_to=Gallery.picked,
        )
        table(
            ["name", "kb"],
            len(Gallery.rows),
            file_cells,
            widths=[2.0, 1.0],
            height=104.0,
            selected=Gallery.picked,
            on_select=Gallery.pick_row,
            sort=Gallery.sort_col,
            descending=Gallery.desc,
            on_sort=Gallery.sort_by,
        )
        with data_table():
            with row(spacing=8):
                text("kind", grow=1.0)
                text("what it is for", grow=2.0)
            with row(spacing=8):
                text("list_view", grow=1.0)
                text("rows of any shape, virtualized", grow=2.0)
            with row(spacing=8):
                text("table", grow=1.0)
                text("rows on column tracks, with a header", grow=2.0)


@component
def reporting():
    with column(spacing=8, **panel):
        text("progress, spinner, bar_chart, line_chart", **faint)
        progress(Gallery.pct, label="downloading", height=8.0)
        progress(0.0, indeterminate=True, height=8.0)
        with row(spacing=8):
            spinner()
            text("working", **faint)
        bar_chart(Gallery.bars, labels=["mon", "tue", "wed", "thu", "fri"], height=90.0, axis=True)
        line_chart(Gallery.trend, height=90.0, axis=True, color="#a6e3a1")


@component
def over_the_app():
    with column(spacing=8, **panel):
        text("modal, toast, context_menu", **faint)
        with row(spacing=8):
            button("show modal", on_click=Gallery.ask)
            button("show toast", on_click=Gallery.save)
        with context_menu(options=Gallery.fruits, on_select=Gallery.act):
            with column(padding=14, background="#313244", border_radius=8.0):
                text("right-click this card", size=14)
        text(f"chosen: {Gallery.chosen}", **faint)


@component
def riders():
    with column(spacing=8, **panel):
        text("the properties every element takes, whatever it is", **faint)
        text("a tooltip is in the dump either way", tooltip="so a script sees it")
        button("disabled", on_click=Gallery.save, disabled=True)
        text_field(Gallery.name, placeholder="also disabled", on_change=Gallery.set_name,
                   disabled=True)
        with column(spacing=6, theme="light", padding=8, background="surface",
                    border_radius=8.0):
            text("a scope with the other palette", size=13)
            button("in the light", on_click=Gallery.save)
        text("wider than it needs", width=220.0, background="#313244", padding=6)
        text("read as a heading", role="heading", size=15)
        image("demo/assets/yokan.svg", width=40.0, height=40.0,
              a11y_label="the Yokan mark")
        text("fades in when it appears", animate=200.0, enter=True, **faint)


@component
def painting():
    with column(spacing=8, **panel):
        text("canvas: virtual pixels, and every color an index", **faint)
        with canvas(64, 32, scale=4, background=0, palette=Gallery.palette):
            rect(2, 2, 14, 8, 1)
            circle(30, 10, 5, 2)
            line(2, 20, 61, 20, 3)
            pixel(40, 6, 3)
            pixel_text(2, 24, "PIXELS", 1)


def view() -> None:
    with column(grow=1.0):
        with row(spacing=8, padding=10):
            text("Yokan widgets", **heading)
            spacer()
            text(f"{len(Gallery.pages)} pages, every element", **faint)
        divider()
        with row(grow=1.0):
            list_view(
                len(Gallery.pages),
                page_row,
                item_height=30.0,
                height=430.0,
                width=150.0,
                selected=Gallery.page,
                on_select=Gallery.go,
            )
            divider()
            # A scroll_view takes a height rather than a share of its
            # row, so the column around it takes the width that is left
            # and carries the height. And a row centres what it holds,
            # so that height is what makes the page start at the top
            # rather than float in the middle.
            with column(grow=1.0, padding=10, height=450.0):
                with scroll_view(height=430.0):
                    if Gallery.page == 0:
                        arranging()
                    elif Gallery.page == 1:
                        texts()
                    elif Gallery.page == 2:
                        inputs()
                    elif Gallery.page == 3:
                        choosers()
                    elif Gallery.page == 4:
                        lists()
                    elif Gallery.page == 5:
                        reporting()
                    elif Gallery.page == 6:
                        over_the_app()
                    elif Gallery.page == 7:
                        painting()
                    else:
                        riders()
        if Gallery.asking:
            with modal():
                text("a modal takes the clicks behind it", size=14)
                button("close", on_click=Gallery.close)
        if Gallery.saved:
            toast("Saved", duration_ms=1500.0, on_close=Gallery.saved_shown)


if __name__ == "__main__":
    # A catalog wants room: the window asks for a size, and the
    # two panes take the height they were given.
    run(view, title="widgets", width=900.0, height=640.0)
