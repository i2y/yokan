"""Type stubs for yokan — pixie's engine driven from real CPython.

These stubs teach external type checkers the runtime's actual
shapes — most importantly that `@store` binds an INSTANCE to the
class name (methods are bound, `self` never appears at call sites)
and that `@model` / `@value` synthesize field-based constructors.
"""
from typing import (
    Any,
    Callable,
    Generic,
    Optional,
    ParamSpec,
    Sequence,
    TypedDict,
    TypeVar,
    Unpack,
    dataclass_transform,
    overload,
)

T = TypeVar("T")
P = ParamSpec("P")
R = TypeVar("R")
K = TypeVar("K")
V = TypeVar("V")

class Element:
    def __enter__(self) -> "Element": ...
    def __exit__(self, *exc: object) -> bool: ...

class State(Generic[T]):
    def __init__(self, value: T) -> None: ...
    def __call__(self) -> T: ...
    def value(self) -> T: ...
    def set(self, value: T) -> None: ...
    # `cell[k] = v` writes IN PLACE on the held dict or list.
    @overload
    def __setitem__(self: "State[dict[K, V]]", key: K, value: V) -> None: ...
    @overload
    def __setitem__(self: "State[list[V]]", key: int, value: V) -> None: ...
    def __iadd__(self, other: Any) -> "State[T]": ...
    def __isub__(self, other: Any) -> "State[T]": ...

# The shared properties: the keyword arguments EVERY element takes,
# under the same names and with the same meaning, whether it is a
# text, a button or a whole column.
class SharedPropsBase(TypedDict, total=False):
    """The shared properties EVERY element takes. The four families
    below add back the `width` / `height` / `a11y_label` an element
    does not already own under those names."""

    # A box around the element: `min_width` and `max_width` are
    # always here, `width` and `height` on every element that has
    # no prop of its own by that name.
    min_width: float
    max_width: float
    # Paint this element's subtree dimmed, swallow its clicks and mark
    # it disabled in the accessibility tree — a script step aimed at a
    # control inside is accepted and does nothing, as a person's click
    # would be. Takes True/False or a bool state/field read.
    disabled: bool
    # The palette this element's subtree resolves its color tokens in:
    # "light", "dark", or a str state/field read, so a view can offer a
    # theme switcher.
    theme: str
    # Tween this element's appearance over `animate` milliseconds;
    # `easing` is one of "linear", "in", "out", "inOut" (default
    # "out"), and `enter` / `exit` fade it in when it appears and out
    # when it goes. A script's `advance:<ms>` steps through the frames.
    animate: float
    easing: str
    enter: bool
    exit: bool
    # How many grid tracks this element covers when its parent is a
    # `grid`; inert in a column or a row.
    col_span: int
    row_span: int
    # `role` overrides the role the element derives (a screen reader's
    # "button" / "list" / …). A literal must be one of pixie's
    # vocabulary (button, label, heading, textInput, image, list,
    # listItem, table, dialog, progress, slider, group, checkbox,
    # switch, comboBox, radioGroup, tabList); a state/store-field read
    # is resolved at run time instead, and falls back to the derived
    # role when it does not name one of these. `a11y` in a `headless`
    # script prints the resulting tree.
    role: str
    # A line the window shows when the pointer rests on the element,
    # and a dumped property either way.
    tooltip: str

# The four narrower families, one per thing an element already owns.
# Which sides an element sizes for itself is the same table on both
# sides of the build, so a `width=` lands on the same prop either way.
class SharedPropsOwnLabel(SharedPropsBase, total=False):
    """checkbox / switch: their visible `label` is already their
    accessible name, so there is no `a11y_label` to give them."""

    width: float
    height: float

class SharedPropsOwnSize(SharedPropsBase, total=False):
    """button / image / svg / bar_chart / line_chart: `width` and
    `height` are their own props, not the shared ones."""

    a11y_label: str

class SharedPropsOwnWidth(SharedPropsBase, total=False):
    """text: `width` is its own prop; the box still gives it a
    height."""

    height: float
    a11y_label: str

class SharedPropsOwnHeight(SharedPropsBase, total=False):
    """list_view / scroll_view / table: `height` is their own prop."""

    width: float
    a11y_label: str

class SharedProps(SharedPropsOwnLabel, total=False):
    """Every shared property an element takes."""

    # The name assistive technology reads instead of the one the
    # element would otherwise derive.
    a11y_label: str

# `bold` / `italic` / `mono` / `underline` are the typography flags;
# `mono` asks for a monospace family. `wrap` is "" (wrap at the
# parent's width), "nowrap" (one line, overflowing) or "ellipsis"
# (one line clipped with a trailing "…" — give it `width`, or a
# parent that sizes it, or there is nothing to clip against).
# `max_lines` clamps a wrapped paragraph to that many lines.
# `background`, `padding` and the `border_*` trio draw a box behind
# the text: padded and rounded with a background, inside a row, that
# is a pill.
def text(
    text: str,
    size: float = 0.0,
    color: str = "",
    align: str = "",
    grow: float = 0.0,
    bold: bool = False,
    italic: bool = False,
    mono: bool = False,
    underline: bool = False,
    wrap: str = "",
    max_lines: int = 0,
    width: float = 0.0,
    background: str = "",
    padding: float = 0.0,
    border_radius: float = 0.0,
    border_width: float = 0.0,
    border_color: str = "",
    **props: Unpack[SharedPropsOwnWidth],
) -> Element: ...
def button(
    label: str,
    on_click: Optional[Callable[[], Any]] = None,
    width: float = 0.0,
    height: float = 0.0,
    size: float = 0.0,
    background: str = "",
    grow: float = 0.0,
    color: str = "",
    hover_background: str = "",
    active_background: str = "",
    border_radius: float = 0.0,
    border_width: float = 0.0,
    border_color: str = "",
    basis: float = 0.0,
    **props: Unpack[SharedPropsOwnSize],
) -> Element: ...
def text_field(
    value: str,
    placeholder: str = "",
    on_change: Optional[Callable[[str], Any]] = None,
    on_submit: Optional[Callable[[str], Any]] = None,
    # A field that holds paragraphs: it wraps, `enter` writes a
    # newline instead of submitting, and the caret moves by visual
    # line. `rows` is how many lines are visible (default 4).
    multiline: bool = False,
    rows: float = 0.0,
    **props: Unpack[SharedProps],
) -> Element: ...
def column(
    *children: Element,
    spacing: float = -1.0,
    padding: float = 0.0,
    background: str = "",
    grow: float = 0.0,
    border_radius: float = 0.0,
    border_width: float = 0.0,
    border_color: str = "",
    **props: Unpack[SharedProps],
) -> Element: ...
def row(
    *children: Element,
    spacing: float = -1.0,
    padding: float = 0.0,
    background: str = "",
    grow: float = 0.0,
    border_radius: float = 0.0,
    border_width: float = 0.0,
    border_color: str = "",
    **props: Unpack[SharedProps],
) -> Element: ...
def grid(
    *children: Element,
    columns: int = 2,
    rows: int = 0,
    spacing: float = -1.0,
    padding: float = 0.0,
    background: str = "",
    grow: float = 0.0,
    border_radius: float = 0.0,
    border_width: float = 0.0,
    border_color: str = "",
    **props: Unpack[SharedProps],
) -> Element: ...
def grid_cell(child: Element, **props: Unpack[SharedProps]) -> Element:
    """The span written out: `grid_cell(child, col_span=2)` and
    `col_span=2` on the child itself are the same tree."""
def stack(*children: Element, **props: Unpack[SharedProps]) -> Element: ...
def list_view(
    count: int,
    row: Callable[[int], Any],
    item_height: float = 24.0,
    height: float = 0.0,
    virtualized: bool = True,
    grow: float = 0.0,
    **props: Unpack[SharedPropsOwnHeight],
) -> Element: ...
def scroll_view(
    *children: Element, height: float = 0.0, **props: Unpack[SharedPropsOwnHeight]
) -> Element: ...
def h_scroll_view(*children: Element, **props: Unpack[SharedProps]) -> Element: ...
def data_table(*children: Element, **props: Unpack[SharedProps]) -> Element:
    """The first `row` child is the header; later `row` children
    are data rows shaded in alternation. The frame comes with the
    element."""
def modal(
    *children: Element, open: bool = True, **props: Unpack[SharedProps]
) -> Element: ...
def image(
    source: str,
    width: float = 0.0,
    height: float = 0.0,
    **props: Unpack[SharedPropsOwnSize],
) -> Element: ...
def svg(
    source: str,
    width: float = 0.0,
    height: float = 0.0,
    **props: Unpack[SharedPropsOwnSize],
) -> Element: ...
# min/max pin the range (0/0 = from the data, which may be negative —
# the zero line is the baseline); `axis` draws tick labels and
# gridlines; `series` draws several lines/bar groups, `colors` one
# color per series.
def bar_chart(
    data: Sequence[float] = (),
    labels: Optional[Sequence[str]] = None,
    width: float = 0.0,
    height: float = 0.0,
    min: float = 0.0,
    max: float = 0.0,
    axis: bool = False,
    color: str = "",
    series: Sequence[Sequence[float]] = (),
    colors: Sequence[str] = (),
    **props: Unpack[SharedPropsOwnSize],
) -> Element: ...
def line_chart(
    data: Sequence[float] = (),
    labels: Optional[Sequence[str]] = None,
    width: float = 0.0,
    height: float = 0.0,
    min: float = 0.0,
    max: float = 0.0,
    axis: bool = False,
    color: str = "",
    series: Sequence[Sequence[float]] = (),
    colors: Sequence[str] = (),
    **props: Unpack[SharedPropsOwnSize],
) -> Element: ...
class Command:
    """One drawing command inside a `with canvas(...)` block. It is
    not an element: it takes none of the shared properties, and it
    paints on the canvas it was written in and nowhere else."""

def canvas(
    width: int,
    height: int,
    scale: int = 1,
    # The palette index the surface is cleared to.
    background: int = 0,
    # The colors this canvas paints in. A command's color is an INDEX
    # into this list: out of range paints the last one, and an empty
    # palette paints magenta.
    palette: Sequence[str] = (),
    **props: Unpack[SharedPropsOwnSize],
) -> Element:
    """A grid of virtual pixels, painted command by command.

    `width` and `height` count virtual pixels; `scale` is how many
    logical pixels each of them takes, so `canvas(160, 120, scale=4)`
    occupies 640x480. The commands go in the block:

        with canvas(160, 120, scale=4, palette=Game.palette):
            rect(x, y, 8, 8, 7)

    Coordinates are whole numbers — a pixel grid has no half pixels —
    and every color is an index into `palette`."""

def pixel(x: int, y: int, color: int) -> Command: ...
def line(x1: int, y1: int, x2: int, y2: int, color: int) -> Command: ...
def rect(x: int, y: int, w: int, h: int, color: int) -> Command: ...
def rect_outline(x: int, y: int, w: int, h: int, color: int) -> Command: ...
def circle(x: int, y: int, r: int, color: int) -> Command: ...
def circle_outline(x: int, y: int, r: int, color: int) -> Command: ...
def triangle(
    x1: int, y1: int, x2: int, y2: int, x3: int, y3: int, color: int
) -> Command: ...
def triangle_outline(
    x1: int, y1: int, x2: int, y2: int, x3: int, y3: int, color: int
) -> Command: ...
def sprite(
    x: int,
    y: int,
    source: str,
    u: int,
    v: int,
    w: int,
    h: int,
    # The palette index that is NOT copied (-1 = every pixel is), and
    # the two mirrorings.
    colkey: int = -1,
    flip_x: bool = False,
    flip_y: bool = False,
) -> Command:
    """A rectangle of `source`, copied onto the canvas. The sheet and
    the canvas share one palette, which is what `colkey` indexes."""

def pixel_text(x: int, y: int, text: str, color: int) -> Command:
    """A line of text in the canvas's own 4x6 font, on the pixel grid."""

def progress(
    value: float,
    width: float = 0.0,
    height: float = 0.0,
    # The caption above the track, which is also this element's
    # accessible name — so `a11y_label` is deliberately absent here.
    label: str = "",
    indeterminate: bool = False,
    **props: Unpack[SharedPropsBase],
) -> Element:
    """A track filled to `value` (0..1); `indeterminate=True` sweeps
    instead, for work with no known length."""
def checkbox(
    label: str,
    checked: bool = False,
    on_change: Optional[Callable[[bool], Any]] = None,
    # `label` is ALREADY this toggle's accessible name (pixie derives
    # it from the same visible text) — the dialect has no way to give
    # it a different one, so a11y_label is deliberately absent here.
    **props: Unpack[SharedPropsOwnLabel],
) -> Element: ...
def switch(
    label: str,
    checked: bool = False,
    on_change: Optional[Callable[[bool], Any]] = None,
    **props: Unpack[SharedPropsOwnLabel],
) -> Element: ...
def slider(
    value: float = 0.0,
    min: float = 0.0,
    max: float = 1.0,
    step: float = 0.0,
    on_change: Optional[Callable[[float], Any]] = None,
    **props: Unpack[SharedProps],
) -> Element: ...
def select(
    options: Sequence[str] = (),
    selected: int = 0,
    on_change: Optional[Callable[[int], Any]] = None,
    **props: Unpack[SharedProps],
) -> Element: ...
def radio_group(
    options: Sequence[str] = (),
    selected: int = 0,
    on_change: Optional[Callable[[int], Any]] = None,
    **props: Unpack[SharedProps],
) -> Element: ...
def tab_bar(
    labels: Sequence[str] = (),
    active: int = 0,
    on_change: Optional[Callable[[int], Any]] = None,
    **props: Unpack[SharedProps],
) -> Element: ...
def spinner(size: float = 0.0, **props: Unpack[SharedProps]) -> Element: ...
def link(label: str, url: str, size: float = 0.0, **props: Unpack[SharedProps]) -> Element:
    """Text that opens `url` in the browser when clicked; a headless
    run records the click and opens nothing."""
def table(
    columns: Sequence[str],
    count: int,
    row: Callable[[int], Any],
    widths: Sequence[float] = (),
    item_height: float = 24.0,
    height: float = 0.0,
    grow: float = 0.0,
    selected: int = -1,
    on_select: Optional[Callable[[int], Any]] = None,
    sort: int = -1,
    descending: bool = False,
    on_sort: Optional[Callable[[int], Any]] = None,
    **props: Unpack[SharedPropsOwnHeight],
) -> Element:
    """A virtualized table: `row(i)` builds row i as a `row` of one
    cell per column, laid on tracks whose shares are `widths`.
    `on_select` receives the clicked row's index, `on_sort` the
    clicked header's; the app re-sorts its own lists. In a headless
    script `select:<first cell>` picks a row and `click:<column>`
    sorts. The row builder returns its `row(...)` (the compiled run
    takes that form, or one `with row():` block); `selected` and
    `sort` are state or store-field reads, since a literal could
    never reflect the selection."""

# A typed number: `enter` or leaving the field commits, text that is
# not a number is dropped and the shown value returns to `value`;
# min/max clamp, step snaps (0 = free). In a headless script
# `input:<text>` commits.
def number_field(
    value: float,
    min: float = 0.0,
    max: float = 0.0,
    step: float = 0.0,
    placeholder: str = "",
    on_change: Optional[Callable[[float], Any]] = None,
    **props: Unpack[SharedProps],
) -> Element: ...
def int_field(
    value: int,
    min: int = 0,
    max: int = 0,
    step: int = 1,
    placeholder: str = "",
    on_change: Optional[Callable[[int], Any]] = None,
    **props: Unpack[SharedProps],
) -> Element: ...
def segmented(
    options: Sequence[str] = (),
    selected: int = 0,
    on_change: Optional[Callable[[int], Any]] = None,
    **props: Unpack[SharedProps],
) -> Element:
    """A row of joined toggle buttons; the handler receives the
    chosen index."""

def spacer(grow: float = 0.0, **props: Unpack[SharedProps]) -> Element:
    """Takes the parent's remaining space along its main axis; 0 = one share."""

def divider(color: str = "", thickness: float = 0.0, **props: Unpack[SharedProps]) -> Element:
    """A rule across the parent: horizontal in a column, vertical in a row."""

# The work runs off the UI thread in both runs — a Python thread
# during development, the engine's pool for the standard-library
# calls inside it once compiled. It is the last statement of its
# handler; `on_error=` is development-run only (catch a failing call
# with try/except around it).
def task(
    work: Callable[[], Any],
    on_done: Optional[Callable[[Any], Any]] = None,
    on_error: Optional[Callable[[BaseException], Any]] = None,
) -> None: ...
def py(f: Callable[P, R]) -> Callable[P, R]: ...
# A line on stderr, from either run. `print` writes to stdout, which
# is where a headless run's screen dump goes, so the dialect asks for
# this instead.
def log(msg: str) -> int: ...

# Declared Rust crates (the app's `# [tool.yokan.crates]` block):
# `crates.<name>.<fn>(...)` calls the crate in both runs.
crates: Any

def store(cls: type[T]) -> T:
    """The decorated NAME is the singleton instance, not the class:
    `Cart.add(...)` is a bound call and `Cart.total` an attribute
    read. Typed so checkers bind `self` the way the runtime does."""

@dataclass_transform()
def model(cls: type[T]) -> type[T]:
    """An observed, instantiable class with a field-based
    constructor synthesized from the annotations (defaults
    honored; mutable defaults become per-instance copies)."""

@dataclass_transform(frozen_default=True)
def value(cls: type[T]) -> type[T]:
    """Shorthand for @dataclass(frozen=True): a native value class."""

# A NOT-owning model reference: `parent: Weak[Node] = None`. Reads
# answer the target or None once every owner dropped it; use it for
# back pointers, so ownership cycles cannot form. To a checker the
# field simply IS `Node | None` — which is exactly how it reads.
type Weak[X] = X | None

@overload
def component(f: Callable[P, Any]) -> Callable[P, Element]: ...
@overload
def component(
    *, slots: bool = False
) -> Callable[[Callable[P, Any]], Callable[P, Element]]: ...
def slot() -> None: ...
def local(init: T) -> State[T]: ...
# A timer is DECLARED at module level (or under the __main__ guard)
# and starts with the app. Both runs fire it off the same clock: a
# frame in a window, an `advance:<ms>` in a headless script.
def every(seconds: float, on_tick: Callable[[], Any]) -> None: ...
# A shortcut is declared the same way, with the chord spelled the way
# the platform spells it ("cmd+s", "shift-tab", "ctrl+alt+k"); a
# headless script presses one with `key:cmd+s`.
def shortcut(chord: str, on_press: Callable[[], Any]) -> None: ...
# One handler for every key, which receives the chord it was.
def on_key(handler: Callable[[str], Any]) -> None: ...
# One item in the application's menu bar: the menu it sits in, the
# name it shows, and the handler it runs. Declaration order is menu
# order; a headless script picks one with `menu:Save`.
def menu_item(menu: str, item: str, on_pick: Callable[[], Any]) -> None: ...
# What happens to a file dragged onto the window: the handler receives
# its path. A headless script drops one with `drop:<path>`.
def on_file_drop(handler: Callable[[str], Any]) -> None: ...
def quit() -> None:
    """Ask the window to close. A handler asks and the window answers
    on its next frame; a headless run has no window, so a script runs
    its remaining steps and both runs print the same dumps."""

def run(
    view: Callable[..., Any],
    state: Any = None,
    title: str = "pixie",
    watch: bool = True,
    theme: Optional[str] = None,
    on_start: Optional[Callable[[], Any]] = None,
    width: float = 0.0,
    height: float = 0.0,
    padding: Optional[float] = None,
) -> None:
    """width/height request the window size in logical pixels (as a
    pair; 0 = the engine default). `padding` is the inset between the
    window and the app's tree — 16 px unless you say otherwise, and
    `0.0` lets the app paint to the window's edge, which is what a
    canvas that IS the app wants. Compiled builds bake all of them via
    the project's pixie.toml [window]; title crosses the same way."""

def headless(
    view: Callable[..., Any],
    state: Any = None,
    script: str = "",
    on_start: Optional[Callable[[], Any]] = None,
) -> str:
    """Run the app against a script with no window and answer the
    screen as text: the dump before the steps, then the dump after.
    This is what a unit test asserts on."""

# The name it had before 0.3. Still resolves, so a test written
# against it keeps running.
_headless = headless

class fs:
    """Files, from the bundled standard library. Interpreted and
    compiled apps run the same implementation. A failing call raises
    (catchable with try/except); the *_or variants answer the
    fallback instead."""
    @staticmethod
    def read_text(path: str) -> str: ...
    @staticmethod
    def write_text(path: str, text: str) -> int: ...
    @staticmethod
    def exists(path: str) -> bool: ...
    @staticmethod
    def read_text_or(path: str, default: str) -> str: ...
    @staticmethod
    def append_text(path: str, text: str) -> int: ...
    # The names in a directory, sorted.
    @staticmethod
    def list_dir(path: str) -> list[str]: ...
    @staticmethod
    def make_dir(path: str) -> int: ...
    @staticmethod
    def remove(path: str) -> int: ...
    # The directory this app may keep its own files in, created if it
    # is not there yet.
    @staticmethod
    def app_dir(name: str) -> str: ...
    # The platform's own panels. A dialog waits for a person, so it
    # runs inside `task(...)`; the answer is a path, or "" when the
    # person cancelled. A headless script answers with `file:<path>`.
    @staticmethod
    def open_dialog(title: str = "") -> str: ...
    @staticmethod
    def save_dialog(name: str = "") -> str: ...

class sqlite:
    """Bundled sqlite; interpreted and compiled apps run the same
    implementation. query_text answers column 0 of each row as text,
    query_rows answers every column; order with ORDER BY.

    Every call takes an optional `params` list: write `?` in the
    statement and the values beside it, and text a user typed can
    never become SQL. Values bind as text and the column's affinity
    converts, so an INTEGER column stores a number."""
    @staticmethod
    def exec(path: str, sql: str, params: list[str] = ...) -> int: ...
    @staticmethod
    def query_text(path: str, sql: str, params: list[str] = ...) -> list[str]: ...
    @staticmethod
    def query_int(path: str, sql: str, params: list[str] = ...) -> int: ...
    @staticmethod
    def query_rows(path: str, sql: str, params: list[str] = ...) -> list[list[str]]: ...
    @staticmethod
    def query_int_or(path: str, sql: str, default: int, params: list[str] = ...) -> int: ...
    @staticmethod
    def query_text_or(path: str, sql: str, params: list[str] = ...) -> list[str]: ...
    @staticmethod
    def query_rows_or(path: str, sql: str, params: list[str] = ...) -> list[list[str]]: ...

class strings:
    @staticmethod
    def to_int(s: str, default: int) -> int: ...
    @staticmethod
    def to_float(s: str, default: float) -> float: ...

class notify:
    @staticmethod
    def send(title: str, body: str) -> None: ...

class clipboard:
    """The system clipboard. A window exchanges it with the platform,
    a headless run keeps it to itself — so a copy and a paste are a
    checked interaction like any other."""
    @staticmethod
    def set_text(text: str) -> int: ...
    @staticmethod
    def get_text() -> str: ...

class audio:
    """Sound. `play(path)` starts a WAV and returns at once; several
    play together, and `stop()` ends them all.

    A scripted run is silent — a gate must not need a machine with
    speakers, and a dump has no sound in it — so this is one of the few
    things only a window shows. A machine with no audio device, or a
    file that cannot be read, plays nothing rather than failing the
    app."""
    @staticmethod
    def play(path: str, volume: float = 1.0) -> int:
        """`volume` is a level between 0 and 1, where 1 is the file as
        it was recorded. Loud is the one mistake a sound cannot take
        back, so something that plays often should ask for less."""
    @staticmethod
    def stop() -> int: ...

class keys:
    """The keyboard as a device: not which chord was pressed, but
    which keys are down. Read them from a timer's tick — a view is
    rebuilt on the framework's schedule, so what it read there would
    be a moment the app never chose.

    A key's name is bare (`left`, `space`, `z`); the modifiers answer
    under their own names (`shift`, `cmd`, `ctrl`, `alt`), so
    `down("left")` is true whether or not shift is held with it.
    `pressed` and `released` are spent by the tick that saw them, so
    holding a key fires once however many frames it stays down."""
    @staticmethod
    def down(key: str) -> bool: ...
    @staticmethod
    def pressed(key: str) -> bool: ...
    @staticmethod
    def released(key: str) -> bool: ...

class http:
    """HTTP. The call blocks until the response arrives — the
    interpreted and the compiled app block on the same statement, and
    inside `task(...)` the compiled run awaits it. A failure raises;
    the *_or variants answer the fallback instead."""
    # A second argument is the deadline in milliseconds.
    @staticmethod
    def get_text(url: str, timeout_ms: int = ...) -> str: ...
    @staticmethod
    def get_text_or(url: str, default: str) -> str: ...
    @staticmethod
    def get_text_with(url: str, headers: dict[str, str]) -> str: ...
    # POST a body; the content type defaults to text/plain.
    @staticmethod
    def post_text(url: str, body: str, content_type: str = ...) -> str: ...
    @staticmethod
    def post_text_or(url: str, body: str, default: str) -> str: ...
    # The status code, or 0 when the request never reached a server.
    @staticmethod
    def status(url: str) -> int: ...

def style(**kwargs: Any) -> dict[str, Any]: ...

# `math`, `random` and `statistics` are Python's own — write
# `import math`, and a type checker reads typeshed's stub for it.

# Reads into a JSON document by dotted path ("items.0.title").
# Python's `json` has none of these, and `json.dumps` is Python's own
# — write `import json` for it.
class jsondoc:
    @staticmethod
    def get_text(src: str, path: str) -> str: ...
    @staticmethod
    def get_int(src: str, path: str) -> int: ...
    @staticmethod
    def get_float(src: str, path: str) -> float: ...
    @staticmethod
    def get_bool(src: str, path: str) -> bool: ...
    @staticmethod
    def length(src: str, path: str) -> int: ...
    @staticmethod
    def has(src: str, path: str) -> bool: ...

# The machine's own zone, which Python's `time` reaches only through
# `localtime` and a struct. Reading the clock itself is Python's
# `time`, and calendar work is Python's `datetime`.
class clock:
    # UTC.
    @staticmethod
    def format_ms(ms: int, fmt: str) -> str: ...
    # The machine's own zone — both runs read the same zone database.
    @staticmethod
    def format_local_ms(ms: int, fmt: str) -> str: ...
    @staticmethod
    def local_offset_minutes(ms: int) -> int: ...

