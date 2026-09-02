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

def text(
    text: str,
    size: float = 0.0,
    color: str = "",
    align: str = "",
    grow: float = 0.0,
    animate: float = 0.0,
    easing: str = "",
    enter: bool = False,
    exit: bool = False,
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
) -> Element: ...
def text_field(
    value: str,
    placeholder: str = "",
    on_change: Optional[Callable[[str], Any]] = None,
    on_submit: Optional[Callable[[str], Any]] = None,
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
) -> Element: ...
def grid_cell(child: Element, col_span: int = 1, row_span: int = 1) -> Element: ...
def stack(*children: Element) -> Element: ...
def list_view(
    count: int,
    row: Callable[[int], Any],
    item_height: float = 24.0,
    height: float = 0.0,
    virtualized: bool = True,
    grow: float = 0.0,
) -> Element: ...
def scroll_view(*children: Element, height: float = 0.0) -> Element: ...
def h_scroll_view(*children: Element) -> Element: ...
def data_table(*children: Element) -> Element:
    """The first `row` child is the header; later `row` children
    are data rows shaded in alternation. The frame comes with the
    element."""
def modal(*children: Element, open: bool = True) -> Element: ...
def image(source: str, width: float = 0.0, height: float = 0.0) -> Element: ...
def svg(source: str, width: float = 0.0, height: float = 0.0) -> Element: ...
def bar_chart(
    data: Sequence[float],
    labels: Optional[Sequence[str]] = None,
    width: float = 0.0,
    height: float = 0.0,
) -> Element: ...
def line_chart(
    data: Sequence[float],
    labels: Optional[Sequence[str]] = None,
    width: float = 0.0,
    height: float = 0.0,
) -> Element: ...
def progress(value: float) -> Element: ...
def checkbox(
    label: str,
    checked: bool = False,
    on_change: Optional[Callable[[bool], Any]] = None,
) -> Element: ...
def switch(
    label: str,
    checked: bool = False,
    on_change: Optional[Callable[[bool], Any]] = None,
) -> Element: ...
def slider(
    value: float = 0.0,
    min: float = 0.0,
    max: float = 1.0,
    step: float = 0.0,
    on_change: Optional[Callable[[float], Any]] = None,
) -> Element: ...
def select(
    options: Sequence[str] = (),
    selected: int = 0,
    on_change: Optional[Callable[[int], Any]] = None,
) -> Element: ...
def radio_group(
    options: Sequence[str] = (),
    selected: int = 0,
    on_change: Optional[Callable[[int], Any]] = None,
) -> Element: ...
def tab_bar(
    labels: Sequence[str] = (),
    active: int = 0,
    on_change: Optional[Callable[[int], Any]] = None,
) -> Element: ...
def spinner(size: float = 0.0) -> Element: ...
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

class sqlite:
    """Bundled sqlite; interpreted and compiled apps run the same
    implementation. query_text answers column 0 of each row as text
    — shape rows with SQL, order with ORDER BY."""
    @staticmethod
    def exec(path: str, sql: str) -> int: ...
    @staticmethod
    def query_text(path: str, sql: str) -> list[str]: ...
    @staticmethod
    def query_int(path: str, sql: str) -> int: ...
    @staticmethod
    def query_int_or(path: str, sql: str, default: int) -> int: ...
    @staticmethod
    def query_text_or(path: str, sql: str) -> list[str]: ...

class strings:
    @staticmethod
    def to_int(s: str, default: int) -> int: ...
    @staticmethod
    def to_float(s: str, default: float) -> float: ...

class notify:
    @staticmethod
    def send(title: str, body: str) -> None: ...

class http:
    """HTTP GET. The call blocks until the response arrives — the
    interpreted and the compiled app block on the same statement. A
    failure raises; get_text_or answers the fallback instead."""
    @staticmethod
    def get_text(url: str) -> str: ...
    @staticmethod
    def get_text_or(url: str, default: str) -> str: ...

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

class time:
    @staticmethod
    def now_ms() -> int: ...
    @staticmethod
    def format_ms(ms: int, fmt: str) -> str: ...
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
