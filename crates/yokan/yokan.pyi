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
    TypeVar,
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
    animate: float = 0.0,
    easing: str = "",
    enter: bool = False,
    exit: bool = False,
    tooltip: str = "",
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
    animate: float = 0.0,
    easing: str = "",
    enter: bool = False,
    exit: bool = False,
    col_span: int = 1,
    row_span: int = 1,
    tooltip: str = "",
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
    tooltip: str = "",
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
    theme: str = "",
    animate: float = 0.0,
    easing: str = "",
    enter: bool = False,
    exit: bool = False,
    tooltip: str = "",
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
    tooltip: str = "",
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
    tooltip: str = "",
) -> Element: ...
def grid_cell(child: Element, col_span: int = 1, row_span: int = 1, tooltip: str = "") -> Element: ...
def stack(*children: Element, tooltip: str = "") -> Element: ...
def list_view(
    count: int,
    row: Callable[[int], Any],
    item_height: float = 24.0,
    height: float = 0.0,
    virtualized: bool = True,
    grow: float = 0.0,
    tooltip: str = "",
) -> Element: ...
def scroll_view(*children: Element, height: float = 0.0, tooltip: str = "") -> Element: ...
def h_scroll_view(*children: Element, tooltip: str = "") -> Element: ...
def data_table(*children: Element, tooltip: str = "") -> Element:
    """The first `row` child is the header; later `row` children
    are data rows shaded in alternation. The frame comes with the
    element."""
def modal(*children: Element, open: bool = True, tooltip: str = "") -> Element: ...
def image(source: str, width: float = 0.0, height: float = 0.0, tooltip: str = "") -> Element: ...
def svg(source: str, width: float = 0.0, height: float = 0.0, tooltip: str = "") -> Element: ...

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
    tooltip: str = "",
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
    tooltip: str = "",
) -> Element: ...
def progress(value: float) -> Element: ...
def checkbox(
    label: str,
    checked: bool = False,
    on_change: Optional[Callable[[bool], Any]] = None,
    tooltip: str = "",
) -> Element: ...
def switch(
    label: str,
    checked: bool = False,
    on_change: Optional[Callable[[bool], Any]] = None,
    tooltip: str = "",
) -> Element: ...
def slider(
    value: float = 0.0,
    min: float = 0.0,
    max: float = 1.0,
    step: float = 0.0,
    on_change: Optional[Callable[[float], Any]] = None,
    tooltip: str = "",
) -> Element: ...
def select(
    options: Sequence[str] = (),
    selected: int = 0,
    on_change: Optional[Callable[[int], Any]] = None,
    tooltip: str = "",
) -> Element: ...
def radio_group(
    options: Sequence[str] = (),
    selected: int = 0,
    on_change: Optional[Callable[[int], Any]] = None,
    tooltip: str = "",
) -> Element: ...
def tab_bar(
    labels: Sequence[str] = (),
    active: int = 0,
    on_change: Optional[Callable[[int], Any]] = None,
    tooltip: str = "",
) -> Element: ...
def link(label: str, url: str, size: float = 0.0, tooltip: str = "") -> Element:
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
    tooltip: str = "",
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
    tooltip: str = "",
) -> Element: ...
def int_field(
    value: int,
    min: int = 0,
    max: int = 0,
    step: int = 1,
    placeholder: str = "",
    on_change: Optional[Callable[[int], Any]] = None,
    tooltip: str = "",
) -> Element: ...
def segmented(
    options: Sequence[str] = (),
    selected: int = 0,
    on_change: Optional[Callable[[int], Any]] = None,
    tooltip: str = "",
) -> Element:
    """A row of joined toggle buttons; the handler receives the
    chosen index."""

def spinner(size: float = 0.0, tooltip: str = "") -> Element: ...
def spacer(grow: float = 0.0, tooltip: str = "") -> Element:
    """Takes the parent's remaining space along its main axis; 0 = one share."""

def divider(color: str = "", thickness: float = 0.0, tooltip: str = "") -> Element:
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
def run(
    view: Callable[..., Any],
    state: Any = None,
    title: str = "pixie",
    watch: bool = True,
    theme: Optional[str] = None,
    on_start: Optional[Callable[[], Any]] = None,
    width: float = 0.0,
    height: float = 0.0,
) -> None:
    """width/height request the window size in logical pixels (as a
    pair; 0 = the engine default). Compiled builds bake them via the
    project's pixie.toml [window]; title crosses the same way."""

def _headless(
    view: Callable[..., Any],
    state: Any = None,
    script: str = "",
    on_start: Optional[Callable[[], Any]] = None,
) -> str: ...

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

class math:
    @staticmethod
    def sqrt(v: float) -> float: ...
    @staticmethod
    def sin(v: float) -> float: ...
    @staticmethod
    def cos(v: float) -> float: ...
    @staticmethod
    def pow(a: float, b: float) -> float: ...
    @staticmethod
    def fabs(v: float) -> float: ...
    @staticmethod
    def floor(v: float) -> int: ...
    @staticmethod
    def ceil(v: float) -> int: ...
    @staticmethod
    def pi() -> float: ...

class json:
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
    # Writes a str, int, float, bool, a list of one of those, or a
    # dict with str keys. A dict is written in key order.
    @staticmethod
    def dumps(value: Any) -> str: ...

class time:
    @staticmethod
    def now_ms() -> int: ...
    # UTC.
    @staticmethod
    def format_ms(ms: int, fmt: str) -> str: ...
    # The machine's own zone — both runs read the same zone database.
    @staticmethod
    def format_local_ms(ms: int, fmt: str) -> str: ...
    @staticmethod
    def local_offset_minutes(ms: int) -> int: ...
    # Blocks the caller. Inside `task(...)` the compiled run awaits it,
    # so the window keeps drawing while it waits.
    @staticmethod
    def sleep_ms(ms: int) -> int: ...

class random:
    @staticmethod
    def seed(n: int) -> None: ...
    @staticmethod
    def int(lo: int, hi: int) -> int: ...
    @staticmethod
    def float() -> float: ...
