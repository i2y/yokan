//! pixie's kernel + gpui engine behind a C ABI.
//!
//! This is the substrate's C face, not any one language's: a door
//! written in another language opens the cdylib while developing and
//! links the staticlib when it ships, and both runs then drive exactly
//! the same engine — which is what makes comparing them worth doing.
//! Wakakusa (`wakakusa/`) is the first caller.
//!
//! The shape is the one `crates/yokan/src/lib.rs` gives CPython, with
//! the language-specific parts taken out. Elements are not one function
//! each: the caller opens one with `pixie_el(kind)`, writes properties
//! into it by number, and closes it with `pixie_end`, which builds the
//! `Element` and wraps it in the riders. Adding an element to the
//! vocabulary therefore adds an arm to `materialize` and nothing to the
//! ABI. The numbers come from `wakakusa/elements.toml` through
//! `vocab.rs`, so neither side can invent one.
//!
//! Handles are 1-based indices into a per-build arena; 0 is "no
//! element". A container consumes its children's handles and the build
//! callback returns the root's, so a handle is only meaningful inside
//! the build that made it. Handlers are integers the caller assigns and
//! the engine hands back. Strings cross as NUL-terminated UTF-8 and are
//! copied on entry: the caller's buffer is only promised for the
//! duration of the call (spinel's rule for `:str`, and the right one for
//! CRuby too). The engine holds nothing from the other side, which is
//! why two garbage collectors in one process never meet.

use std::cell::{Cell, RefCell};
use std::ffi::{CStr, c_char};
use std::rc::Rc;

use pixie_engine_gpui::{ReloadWatch, run_app};
use pixie_kernel::{
    BoolListener, Component, Element, ErasedHandle, FloatListener, IntListener, LazyRows, List,
    Listener, Runtime, Str, TextListener, World, mount,
};

mod vocab;
use vocab::*;

/// The app's view: answers the root element's handle for this build.
pub type PixieBuildFn = extern "C" fn() -> i64;
/// The app's event dispatcher: the handler id and the kind of thing
/// this event carries (`PAY_*`). What it carries is then fetched with
/// `pixie_event_int` / `pixie_event_num` / `pixie_event_text`.
///
/// Two reasons for the shape, both learned from a caller compiled by
/// spinel. It cannot receive a `double` or a `const char *` as a
/// callback argument — it reads both as integers, silently, so 2.5
/// arrives as 2 — and it types a list of blocks by what they are
/// called with, so one registry holding handlers of every kind cannot
/// be typed at all. Naming the kind lets the caller keep one registry
/// per kind and fetch the payload through ordinary calls, which
/// convert correctly.
pub type PixieEventFn = extern "C" fn(i64, i64);
/// The app's row builder: a handler id and a row number, answered with
/// that row's element handle.
pub type PixieRowFn = extern "C" fn(i64, i64) -> i64;
/// The app's timer: the id of the tick that just came due.
pub type PixieTimerFn = extern "C" fn(i64);
/// The app's reloader: read the app's file again, answering non-zero
/// when it took. Only a windowed run ever calls it.
pub type PixieReloadFn = extern "C" fn() -> i32;

/// A slot in the build arena: an element being written, one finished
/// and waiting to be consumed, or one already taken.
enum Slot {
    Open(Bag),
    Done(Element),
    Gone,
}

thread_local! {
    static ARENA: RefCell<Vec<Slot>> = const { RefCell::new(Vec::new()) };
    static EVENT_FN: Cell<Option<PixieEventFn>> = const { Cell::new(None) };
    static ROW_FN: Cell<Option<PixieRowFn>> = const { Cell::new(None) };
    static TIMER_FN: Cell<Option<PixieTimerFn>> = const { Cell::new(None) };
    /// Timers are declared before the app runs, so they wait here for
    /// the World to exist.
    static PENDING_TIMERS: RefCell<Vec<(f64, i64)>> = const { RefCell::new(Vec::new()) };
    /// The file to watch and what to call when it changes.
    static WATCH: RefCell<Option<(String, PixieReloadFn)>> = const { RefCell::new(None) };
    /// What the event now being delivered carried. Written just before
    /// the callback and read back by it.
    static EVENT_INT: Cell<i64> = const { Cell::new(0) };
    static EVENT_NUM: Cell<f64> = const { Cell::new(0.0) };
    static EVENT_TEXT: RefCell<Vec<char>> = const { RefCell::new(Vec::new()) };
    static CURRENT_VIEW: Cell<Option<ErasedHandle>> = const { Cell::new(None) };
}

// ---------------------------------------------------------------------------
// The bag: what the caller wrote into one element, by key.

/// Properties written into an element that is still open. Small enough
/// that a scan beats a map, and the read set is what the table test
/// checks an arm against.
#[derive(Default)]
struct Bag {
    kind: i32,
    strs: Vec<(i32, String)>,
    nums: Vec<(i32, f64)>,
    ints: Vec<(i32, i64)>,
    bools: Vec<(i32, bool)>,
    str_lists: Vec<(i32, Vec<Vec<String>>)>,
    num_lists: Vec<(i32, Vec<Vec<f64>>)>,
    handlers: Vec<(i32, i64)>,
    rows: Vec<(i32, i64)>,
    children: Vec<Element>,
    read: RefCell<Vec<i32>>,
}

fn find<T: Copy>(v: &[(i32, T)], key: i32) -> Option<T> {
    v.iter().find(|(k, _)| *k == key).map(|(_, x)| *x)
}

/// The value a property reads as when the caller left it out: the
/// element's own default, else the one every element shares.
fn def_num(kind: i32, key: i32) -> f64 {
    find_def(DEF_NUM, kind, key).unwrap_or(0.0)
}
fn def_int(kind: i32, key: i32) -> i64 {
    find_def(DEF_INT, kind, key).unwrap_or(0)
}
fn def_bool(kind: i32, key: i32) -> bool {
    find_def(DEF_BOOL, kind, key).unwrap_or(false)
}
fn def_str(kind: i32, key: i32) -> &'static str {
    find_def(DEF_STR, kind, key).unwrap_or("")
}

fn find_def<T: Copy>(table: &[(i32, i32, T)], kind: i32, key: i32) -> Option<T> {
    table
        .iter()
        .find(|(k, y, _)| *k == kind && *y == key)
        .or_else(|| table.iter().find(|(k, y, _)| *k == 0 && *y == key))
        .map(|(_, _, v)| *v)
}

impl Bag {
    fn mark(&self, key: i32) {
        let mut r = self.read.borrow_mut();
        if !r.contains(&key) {
            r.push(key);
        }
    }

    fn s(&self, key: i32) -> Str {
        self.mark(key);
        match self.strs.iter().find(|(k, _)| *k == key) {
            Some((_, v)) => Str::from(v.as_str()),
            None => Str::from(def_str(self.kind, key)),
        }
    }

    fn n(&self, key: i32) -> f64 {
        self.mark(key);
        find(&self.nums, key).unwrap_or_else(|| def_num(self.kind, key))
    }

    fn i(&self, key: i32) -> i64 {
        self.mark(key);
        find(&self.ints, key).unwrap_or_else(|| def_int(self.kind, key))
    }

    fn b(&self, key: i32) -> bool {
        self.mark(key);
        find(&self.bools, key).unwrap_or_else(|| def_bool(self.kind, key))
    }

    /// The sizing riders read this: a box is built because somebody
    /// WROTE the property, so a width that reads 0.0 is not the same
    /// as a width nobody wrote.
    fn opt_n(&self, key: i32) -> Option<f64> {
        self.mark(key);
        find(&self.nums, key)
    }

    fn strs(&self, key: i32) -> List<Str> {
        self.mark(key);
        match self.str_lists.iter().find(|(k, _)| *k == key) {
            Some((_, v)) => v.iter().flatten().map(|s| Str::from(s.as_str())).collect(),
            None => List::new(),
        }
    }

    fn nums(&self, key: i32) -> List<f64> {
        self.mark(key);
        match self.num_lists.iter().find(|(k, _)| *k == key) {
            Some((_, v)) => v.iter().flatten().copied().collect(),
            None => List::new(),
        }
    }

    fn nums2(&self, key: i32) -> List<List<f64>> {
        self.mark(key);
        match self.num_lists.iter().find(|(k, _)| *k == key) {
            Some((_, v)) => v.iter().map(|inner| inner.iter().copied().collect()).collect(),
            None => List::new(),
        }
    }

    fn on(&self, key: i32) -> Listener {
        self.mark(key);
        match find(&self.handlers, key) {
            Some(h) => Rc::new(move |w: &mut World| fire(w, h, PAY_NONE, 0, 0.0, "")),
            None => Rc::new(|_| {}),
        }
    }

    fn on_text(&self, key: i32) -> Option<TextListener> {
        self.mark(key);
        find(&self.handlers, key).map(|h| -> TextListener {
            Rc::new(move |w: &mut World, s: Str| fire(w, h, PAY_TEXT, 0, 0.0, s.as_str()))
        })
    }

    fn on_bool(&self, key: i32) -> Option<BoolListener> {
        self.mark(key);
        find(&self.handlers, key).map(|h| -> BoolListener {
            Rc::new(move |w: &mut World, v: bool| fire(w, h, PAY_BOOL, i64::from(v), 0.0, ""))
        })
    }

    fn on_int(&self, key: i32) -> Option<IntListener> {
        self.mark(key);
        find(&self.handlers, key)
            .map(|h| -> IntListener { Rc::new(move |w: &mut World, v: i64| fire(w, h, PAY_INT, v, 0.0, "")) })
    }

    fn on_float(&self, key: i32) -> Option<FloatListener> {
        self.mark(key);
        find(&self.handlers, key).map(|h| -> FloatListener {
            Rc::new(move |w: &mut World, v: f64| fire(w, h, PAY_FLOAT, 0, v, ""))
        })
    }

    /// The lazy half of a list or a table: the engine asks for the rows
    /// it is about to paint and no others.
    fn lazy(&self, key: i32, len: usize) -> Option<LazyRows> {
        self.mark(key);
        find(&self.rows, key).map(|h| LazyRows {
            len,
            build: Rc::new(move |_w: &World, range: std::ops::Range<usize>| {
                range
                    .map(|i| match ROW_FN.with(|c| c.get()) {
                        Some(f) => take(f(h, i as i64))
                            .unwrap_or_else(|| err_text("the row builder answered no element")),
                        None => err_text("no row builder was registered"),
                    })
                    .collect()
            }),
        })
    }
}

/// Hand an event to the app, then mark the view for rebuilding — every
/// event rebuilds, which is the same trade the Python side makes.
fn fire(w: &mut World, handler: i64, kind: i64, i: i64, d: f64, s: &str) {
    EVENT_INT.with(|c| c.set(i));
    EVENT_NUM.with(|c| c.set(d));
    EVENT_TEXT.with(|c| *c.borrow_mut() = s.chars().collect());
    if let Some(f) = EVENT_FN.with(|c| c.get()) {
        f(handler, kind);
    }
    if let Some(hv) = CURRENT_VIEW.with(|c| c.get()) {
        w.mark_view_dirty(hv);
    }
}

/// What the event being delivered carried. Valid for the length of the
/// handler call; the next event overwrites it.
#[unsafe(no_mangle)]
pub extern "C" fn pixie_event_int() -> i64 {
    EVENT_INT.with(|c| c.get())
}

#[unsafe(no_mangle)]
pub extern "C" fn pixie_event_num() -> f64 {
    EVENT_NUM.with(|c| c.get())
}

/// The text an event carried, as characters: how many, and the n-th
/// one as its code point.
///
/// It does not cross as a C string, and that is not squeamishness: a
/// caller compiled by spinel types a `const char *` coming back through
/// its FFI as a C string for good, and a value of that type cannot then
/// be handed to a block whose identity the compiler does not know —
/// which is every handler an app writes. Numbers have no such trouble,
/// and a string built from them is an ordinary string in both runs.
#[unsafe(no_mangle)]
pub extern "C" fn pixie_event_text_length() -> i64 {
    EVENT_TEXT.with(|c| c.borrow().len() as i64)
}

#[unsafe(no_mangle)]
pub extern "C" fn pixie_event_text_char(i: i64) -> i64 {
    if i < 0 {
        return 0;
    }
    EVENT_TEXT.with(|c| c.borrow().get(i as usize).map_or(0, |ch| *ch as i64))
}

// ---------------------------------------------------------------------------
// The arena.

fn with_open<R>(h: i64, f: impl FnOnce(&mut Bag) -> R) -> Option<R> {
    if h <= 0 {
        return None;
    }
    ARENA.with(|a| match a.borrow_mut().get_mut((h - 1) as usize) {
        Some(Slot::Open(bag)) => Some(f(bag)),
        _ => None,
    })
}

fn take(h: i64) -> Option<Element> {
    if h <= 0 {
        return None;
    }
    ARENA.with(|a| {
        let mut a = a.borrow_mut();
        match a.get_mut((h - 1) as usize) {
            Some(slot @ Slot::Done(_)) => match std::mem::replace(slot, Slot::Gone) {
                Slot::Done(el) => Some(el),
                _ => None,
            },
            _ => None,
        }
    })
}

/// # Safety
/// `p` is NULL or a NUL-terminated string that outlives the call.
unsafe fn text_arg(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    // SAFETY: the caller promises a NUL-terminated string.
    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
}

fn err_text(msg: &str) -> Element {
    let mut el = Element::text(msg);
    if let Element::Text {
        font_size, color, ..
    } = &mut el
    {
        *font_size = 14.0;
        *color = Str::from("#ff6666");
    }
    el
}

// ---------------------------------------------------------------------------
// The builder protocol.

/// Open an element of `kind`. Write its properties, then close it with
/// `pixie_end`.
#[unsafe(no_mangle)]
pub extern "C" fn pixie_el(kind: i32) -> i64 {
    ARENA.with(|a| {
        let mut a = a.borrow_mut();
        a.push(Slot::Open(Bag {
            kind,
            ..Default::default()
        }));
        a.len() as i64
    })
}

/// # Safety
/// `v` is NULL or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pixie_str(el: i64, key: i32, v: *const c_char) {
    let v = unsafe { text_arg(v) };
    with_open(el, |b| b.strs.push((key, v)));
}

#[unsafe(no_mangle)]
pub extern "C" fn pixie_num(el: i64, key: i32, v: f64) {
    with_open(el, |b| b.nums.push((key, v)));
}

#[unsafe(no_mangle)]
pub extern "C" fn pixie_int(el: i64, key: i32, v: i64) {
    with_open(el, |b| b.ints.push((key, v)));
}

#[unsafe(no_mangle)]
pub extern "C" fn pixie_bool(el: i64, key: i32, v: i32) {
    with_open(el, |b| b.bools.push((key, v != 0)));
}

/// Append to the list at `key`, starting one if this is the first.
///
/// # Safety
/// `v` is NULL or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pixie_push_str(el: i64, key: i32, v: *const c_char) {
    let v = unsafe { text_arg(v) };
    with_open(el, |b| match b.str_lists.iter_mut().find(|(k, _)| *k == key) {
        Some((_, lists)) => match lists.last_mut() {
            Some(inner) => inner.push(v),
            None => lists.push(vec![v]),
        },
        None => b.str_lists.push((key, vec![vec![v]])),
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn pixie_push_num(el: i64, key: i32, v: f64) {
    with_open(el, |b| match b.num_lists.iter_mut().find(|(k, _)| *k == key) {
        Some((_, lists)) => match lists.last_mut() {
            Some(inner) => inner.push(v),
            None => lists.push(vec![v]),
        },
        None => b.num_lists.push((key, vec![vec![v]])),
    });
}

/// Start a new inner list at `key` — one series of a chart, one row of
/// a table. A list nobody breaks is a single flat one.
#[unsafe(no_mangle)]
pub extern "C" fn pixie_list_break(el: i64, key: i32) {
    with_open(el, |b| match b.num_lists.iter_mut().find(|(k, _)| *k == key) {
        Some((_, lists)) => lists.push(Vec::new()),
        None => b.num_lists.push((key, vec![Vec::new()])),
    });
}

/// The handler this property calls: an integer the caller assigned.
#[unsafe(no_mangle)]
pub extern "C" fn pixie_on(el: i64, key: i32, handler: i64) {
    with_open(el, |b| b.handlers.push((key, handler)));
}

/// The row builder this property calls, for a list that builds its
/// rows on demand.
#[unsafe(no_mangle)]
pub extern "C" fn pixie_rows(el: i64, key: i32, handler: i64) {
    with_open(el, |b| b.rows.push((key, handler)));
}

/// Move `n` finished elements into this one. They are consumed: a
/// handle put into a container is spent.
///
/// # Safety
/// `ids` points at `n` readable handles, or `n` is 0.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pixie_children(el: i64, ids: *const i64, n: usize) {
    if ids.is_null() || n == 0 {
        return;
    }
    // SAFETY: the caller promises `n` readable handles at `ids`.
    let handles = unsafe { std::slice::from_raw_parts(ids, n) };
    let kids: Vec<Element> = handles
        .iter()
        .map(|&h| take(h).unwrap_or_else(|| err_text("<element already used>")))
        .collect();
    with_open(el, |b| b.children = kids);
}

/// Build the element and wrap it in the properties every element takes.
/// Answers the same handle, now holding something a container can eat.
#[unsafe(no_mangle)]
pub extern "C" fn pixie_end(el: i64) -> i64 {
    if el <= 0 {
        return 0;
    }
    let bag = ARENA.with(|a| {
        let mut a = a.borrow_mut();
        match a.get_mut((el - 1) as usize) {
            Some(slot @ Slot::Open(_)) => match std::mem::replace(slot, Slot::Gone) {
                Slot::Open(bag) => Some(bag),
                _ => None,
            },
            _ => None,
        }
    });
    let Some(mut bag) = bag else { return 0 };
    let kids = std::mem::take(&mut bag.children);
    let built = apply_riders(bag.kind, materialize(&bag, kids), &bag);
    ARENA.with(|a| {
        if let Some(slot) = a.borrow_mut().get_mut((el - 1) as usize) {
            *slot = Slot::Done(built);
        }
    });
    el
}

// ---------------------------------------------------------------------------
// One arm per element. Transcribed from `crates/yokan/src/lib.rs`, so
// the same call builds the same tree whichever language wrote it.

fn materialize(b: &Bag, children: Vec<Element>) -> Element {
    match b.kind {
        KIND_TEXT => Element::Text {
            text: b.s(K_TEXT),
            font_size: b.n(K_SIZE),
            color: b.s(K_COLOR),
            align: b.s(K_ALIGN),
            grow: b.n(K_GROW),
            bold: b.b(K_BOLD),
            italic: b.b(K_ITALIC),
            mono: b.b(K_MONO),
            underline: b.b(K_UNDERLINE),
            wrap: b.s(K_WRAP),
            max_lines: b.i(K_MAX_LINES),
            width: b.n(K_WIDTH),
            background: b.s(K_BACKGROUND),
            padding: b.n(K_PADDING),
            border_radius: b.n(K_BORDER_RADIUS),
            border_width: b.n(K_BORDER_WIDTH),
            border_color: b.s(K_BORDER_COLOR),
        },
        KIND_BUTTON => Element::Button {
            label: b.s(K_LABEL),
            background: b.s(K_BACKGROUND),
            hover_background: b.s(K_HOVER_BACKGROUND),
            active_background: b.s(K_ACTIVE_BACKGROUND),
            width: b.n(K_WIDTH),
            height: b.n(K_HEIGHT),
            font_size: b.n(K_SIZE),
            color: b.s(K_COLOR),
            grow: b.n(K_GROW),
            basis: b.n(K_BASIS),
            border_radius: b.n(K_BORDER_RADIUS),
            border_width: b.n(K_BORDER_WIDTH),
            border_color: b.s(K_BORDER_COLOR),
            on_click: b.on(K_ON_CLICK),
        },
        KIND_TEXT_FIELD => Element::TextField {
            value: b.s(K_VALUE),
            placeholder: b.s(K_PLACEHOLDER),
            on_change: b.on_text(K_ON_CHANGE),
            on_submit: b.on_text(K_ON_SUBMIT),
            multiline: b.b(K_MULTILINE),
            rows: b.n(K_ROWS),
        },
        KIND_COLUMN => Element::Column {
            spacing: b.n(K_SPACING),
            padding: b.n(K_PADDING),
            background: b.s(K_BACKGROUND),
            grow: b.n(K_GROW),
            border_radius: b.n(K_BORDER_RADIUS),
            border_width: b.n(K_BORDER_WIDTH),
            border_color: b.s(K_BORDER_COLOR),
            children,
        },
        KIND_ROW => Element::Row {
            spacing: b.n(K_SPACING),
            padding: b.n(K_PADDING),
            background: b.s(K_BACKGROUND),
            grow: b.n(K_GROW),
            border_radius: b.n(K_BORDER_RADIUS),
            border_width: b.n(K_BORDER_WIDTH),
            border_color: b.s(K_BORDER_COLOR),
            children,
        },
        KIND_GRID => Element::Grid {
            columns: b.i(K_COLUMNS),
            rows: b.i(K_ROWS),
            spacing: b.n(K_SPACING),
            padding: b.n(K_PADDING),
            background: b.s(K_BACKGROUND),
            grow: b.n(K_GROW),
            border_radius: b.n(K_BORDER_RADIUS),
            border_width: b.n(K_BORDER_WIDTH),
            border_color: b.s(K_BORDER_COLOR),
            children,
        },
        // The span written out: the one child IS the element, and the
        // `col_span:` rider around it makes the cell.
        KIND_GRID_CELL => children
            .into_iter()
            .next()
            .unwrap_or_else(|| err_text("grid_cell takes one element")),
        KIND_STACK => Element::Stack(children),
        KIND_SCROLL_VIEW => Element::ScrollView {
            height: b.n(K_HEIGHT),
            children,
        },
        KIND_H_SCROLL_VIEW => Element::HScrollView(children),
        KIND_DATA_TABLE => Element::DataTable(children),
        KIND_MODAL => Element::Modal {
            open: b.b(K_OPEN),
            children,
        },
        KIND_LIST_VIEW => {
            let count = b.i(K_COUNT).max(0) as usize;
            Element::ListView {
                virtualized: b.b(K_VIRTUALIZED),
                item_height: b.n(K_ITEM_HEIGHT),
                height: b.n(K_HEIGHT),
                grow: b.n(K_GROW),
                children: Vec::new(),
                lazy: b.lazy(K_ROW, count),
            }
        }
        KIND_TABLE => {
            let count = b.i(K_COUNT).max(0) as usize;
            Element::Table {
                columns: b.strs(K_COLUMNS),
                widths: b.nums(K_WIDTHS),
                item_height: b.n(K_ITEM_HEIGHT),
                height: b.n(K_HEIGHT),
                grow: b.n(K_GROW),
                selected: b.i(K_SELECTED),
                sort: b.i(K_SORT),
                descending: b.b(K_DESCENDING),
                on_select: b.on_int(K_ON_SELECT),
                on_sort: b.on_int(K_ON_SORT),
                children: Vec::new(),
                lazy: b.lazy(K_ROW, count),
            }
        }
        KIND_IMAGE => Element::Image {
            source: b.s(K_SOURCE),
            width: b.n(K_WIDTH),
            height: b.n(K_HEIGHT),
        },
        KIND_SVG => Element::Svg {
            source: b.s(K_SOURCE),
            width: b.n(K_WIDTH),
            height: b.n(K_HEIGHT),
        },
        KIND_BAR_CHART => Element::BarChart {
            data: b.nums(K_DATA),
            labels: b.strs(K_LABELS),
            width: b.n(K_WIDTH),
            height: b.n(K_HEIGHT),
            min: b.n(K_MIN),
            max: b.n(K_MAX),
            axis: b.b(K_AXIS),
            color: b.s(K_COLOR),
            series: b.nums2(K_SERIES),
            colors: b.strs(K_COLORS),
        },
        KIND_LINE_CHART => Element::LineChart {
            data: b.nums(K_DATA),
            labels: b.strs(K_LABELS),
            width: b.n(K_WIDTH),
            height: b.n(K_HEIGHT),
            min: b.n(K_MIN),
            max: b.n(K_MAX),
            axis: b.b(K_AXIS),
            color: b.s(K_COLOR),
            series: b.nums2(K_SERIES),
            colors: b.strs(K_COLORS),
        },
        KIND_PROGRESS => Element::ProgressBar {
            value: b.n(K_VALUE),
            width: b.n(K_WIDTH),
            height: b.n(K_HEIGHT),
            label: b.s(K_LABEL),
            indeterminate: b.b(K_INDETERMINATE),
        },
        KIND_CHECKBOX => Element::Checkbox {
            label: b.s(K_LABEL),
            checked: b.b(K_CHECKED),
            on_toggle: b.on_bool(K_ON_CHANGE),
        },
        KIND_SWITCH => Element::Switch {
            label: b.s(K_LABEL),
            checked: b.b(K_CHECKED),
            on_toggle: b.on_bool(K_ON_CHANGE),
        },
        KIND_SLIDER => Element::Slider {
            value: b.n(K_VALUE),
            min: b.n(K_MIN),
            max: b.n(K_MAX),
            step: b.n(K_STEP),
            on_change: b.on_float(K_ON_CHANGE),
        },
        KIND_SELECT => Element::Select {
            options: b.strs(K_OPTIONS),
            selected: b.i(K_SELECTED),
            on_select: b.on_int(K_ON_CHANGE),
        },
        KIND_RADIO_GROUP => Element::RadioGroup {
            options: b.strs(K_OPTIONS),
            selected: b.i(K_SELECTED),
            on_select: b.on_int(K_ON_CHANGE),
        },
        KIND_SEGMENTED => Element::Segmented {
            options: b.strs(K_OPTIONS),
            selected: b.i(K_SELECTED),
            on_select: b.on_int(K_ON_CHANGE),
        },
        KIND_TAB_BAR => Element::TabBar {
            labels: b.strs(K_LABELS),
            active: b.i(K_ACTIVE),
            on_select: b.on_int(K_ON_CHANGE),
        },
        KIND_NUMBER_FIELD => Element::NumberField {
            value: b.n(K_VALUE),
            min: b.n(K_MIN),
            max: b.n(K_MAX),
            step: b.n(K_STEP),
            placeholder: b.s(K_PLACEHOLDER),
            on_change: b.on_float(K_ON_CHANGE),
        },
        KIND_INT_FIELD => Element::IntField {
            value: b.i(K_VALUE),
            min: b.i(K_MIN),
            max: b.i(K_MAX),
            step: b.i(K_STEP),
            placeholder: b.s(K_PLACEHOLDER),
            on_change: b.on_int(K_ON_CHANGE),
        },
        KIND_LINK => Element::Link {
            label: b.s(K_LABEL),
            url: b.s(K_URL),
            font_size: b.n(K_SIZE),
        },
        KIND_SPINNER => Element::Spinner { size: b.n(K_SIZE) },
        KIND_SPACER => Element::Spacer { grow: b.n(K_GROW) },
        KIND_DIVIDER => Element::Divider {
            color: b.s(K_COLOR),
            thickness: b.n(K_THICKNESS),
        },
        other => err_text(&format!("no element numbered {other}")),
    }
}

// ---------------------------------------------------------------------------
// The riders: the properties every element takes, wrapped around it in
// the order both of pixie's lowerers wrap them — element, Semantics,
// Tooltip, Disabled, Sized, Themed, Anim, GridCell. The order is not
// cosmetic: a rider nested one layer off dumps differently.

fn apply_riders(kind: i32, el: Element, b: &Bag) -> Element {
    let el = wrap_sem(el, b.s(K_ROLE), b.s(K_A11Y_LABEL));
    let el = wrap_tip(el, b.s(K_TOOLTIP));
    let el = wrap_disabled(el, b.b(K_DISABLED));
    // A side the element sizes for itself is its own property and was
    // read by the arm above; the box takes only the rest.
    let owns_width = NATIVE_SIZE.contains(&kind) || NATIVE_WIDTH.contains(&kind);
    let owns_height = NATIVE_SIZE.contains(&kind) || NATIVE_HEIGHT.contains(&kind);
    let el = wrap_sized(
        el,
        if owns_width { None } else { b.opt_n(K_WIDTH) },
        if owns_height { None } else { b.opt_n(K_HEIGHT) },
        b.opt_n(K_MIN_WIDTH),
        b.opt_n(K_MAX_WIDTH),
    );
    let el = wrap_theme(el, b.s(K_THEME));
    let el = wrap_anim(el, b.n(K_ANIMATE), b.s(K_EASING), b.b(K_ENTER), b.b(K_EXIT));
    wrap_span(el, b.i(K_COL_SPAN), b.i(K_ROW_SPAN))
}

fn wrap_sem(el: Element, role: Str, label: Str) -> Element {
    if role.is_empty() && label.is_empty() {
        return el;
    }
    Element::Semantics {
        role,
        label,
        children: vec![el],
    }
}

fn wrap_tip(el: Element, tooltip: Str) -> Element {
    if tooltip.is_empty() {
        return el;
    }
    Element::Tooltip {
        text: tooltip,
        children: vec![el],
    }
}

fn wrap_disabled(el: Element, disabled: bool) -> Element {
    if !disabled {
        return el;
    }
    Element::Disabled { children: vec![el] }
}

fn wrap_sized(
    el: Element,
    width: Option<f64>,
    height: Option<f64>,
    min_width: Option<f64>,
    max_width: Option<f64>,
) -> Element {
    if width.is_none() && height.is_none() && min_width.is_none() && max_width.is_none() {
        return el;
    }
    Element::Sized {
        width: width.unwrap_or(0.0),
        height: height.unwrap_or(0.0),
        min_width: min_width.unwrap_or(0.0),
        max_width: max_width.unwrap_or(0.0),
        children: vec![el],
    }
}

fn wrap_theme(el: Element, theme: Str) -> Element {
    if theme.is_empty() {
        return el;
    }
    Element::Themed {
        theme,
        children: vec![el],
    }
}

fn wrap_anim(el: Element, animate: f64, easing: Str, enter: bool, exit: bool) -> Element {
    if animate == 0.0 && easing.is_empty() && !enter && !exit {
        return el;
    }
    let e = pixie_kernel::anim::Easing::parse(easing.as_str())
        .unwrap_or(pixie_kernel::anim::Easing::Out);
    Element::Anim {
        duration: animate,
        easing: e,
        enter,
        exit,
        opacity: 1.0,
        children: vec![el],
    }
}

fn wrap_span(el: Element, col_span: i64, row_span: i64) -> Element {
    if col_span <= 1 && row_span <= 1 {
        return el;
    }
    Element::GridCell {
        col_span: col_span.max(1),
        row_span: row_span.max(1),
        children: vec![el],
    }
}

// ---------------------------------------------------------------------------
// The app's callbacks, and the run.

/// Registers the function an event reaches. Call before `pixie_run`.
#[unsafe(no_mangle)]
pub extern "C" fn pixie_set_event_handler(f: PixieEventFn) {
    EVENT_FN.with(|c| c.set(Some(f)));
}

/// Registers the function that builds row `i` of a lazy list.
#[unsafe(no_mangle)]
pub extern "C" fn pixie_set_row_builder(f: PixieRowFn) {
    ROW_FN.with(|c| c.set(Some(f)));
}

/// Registers the function a timer's tick reaches.
#[unsafe(no_mangle)]
pub extern "C" fn pixie_set_timer_handler(f: PixieTimerFn) {
    TIMER_FN.with(|c| c.set(Some(f)));
}

/// Watch `path` and call `f` when it changes, so a window can pick up
/// an edit to the app without being restarted. Declared before
/// `pixie_run`, and only a windowed run acts on it: a headless one is
/// over before an edit could arrive.
///
/// # Safety
/// `path` is NULL or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pixie_watch(path: *const c_char, f: PixieReloadFn) {
    let path = unsafe { text_arg(path) };
    if path.is_empty() {
        return;
    }
    WATCH.with(|w| *w.borrow_mut() = Some((path, f)));
}

/// Ask for `handler` to be told every `seconds`. Declared before
/// `pixie_run`; the engine fires it off the same clock a frame and a
/// script's `advance:` move, so both runs tick the same number of
/// times.
#[unsafe(no_mangle)]
pub extern "C" fn pixie_every(seconds: f64, handler: i64) {
    PENDING_TIMERS.with(|t| t.borrow_mut().push((seconds, handler)));
}

/// Hand the queued timers to the engine, now that there is a World.
fn install_timers(rt: &Runtime) {
    for (seconds, handler) in PENDING_TIMERS.with(|t| t.take()) {
        let ms = seconds * 1000.0;
        rt.with(move |w: &mut World| {
            pixie_kernel::timer::every(
                w,
                ms,
                Rc::new(move |w: &mut World| {
                    if let Some(f) = TIMER_FN.with(|c| c.get()) {
                        f(handler);
                    }
                    if let Some(hv) = CURRENT_VIEW.with(|c| c.get()) {
                        w.mark_view_dirty(hv);
                    }
                }),
            );
        });
    }
}

#[derive(Clone)]
struct CView {
    build: PixieBuildFn,
}

impl Component for CView {
    fn build(&self, _w: &World) -> Element {
        ARENA.with(|a| a.borrow_mut().clear());
        let root = (self.build)();
        take(root).unwrap_or_else(|| err_text("the view answered no element"))
    }
}

/// The headless run, as text: the tree's dump, then whatever the
/// script's steps produced. This is what `pixie_run` prints when
/// `PIXIE_SCRIPT` is set — kept as a function that ANSWERS the
/// transcript so a Rust test can drive the ABI without a window and
/// without reading the process's stdout.
fn headless(build: PixieBuildFn, script: &str, light: bool) -> String {
    let mut w = World::new();
    let h = mount(&mut w, CView { build }, &[]);
    CURRENT_VIEW.with(|c| c.set(Some(h.erase())));
    let rt = Runtime::new(w);
    install_timers(&rt);
    let _ = rt.with(|w| w.take_dirty_views());
    rt.with(|w: &mut World| pixie_kernel::theme::set_light(w, light));
    let mut tree = rt.with(|w| pixie_kernel::build_prepared(w, h));
    pixie_kernel::script::anim_settle(&rt, h, &mut tree);
    let mut out = rt.with(|w| tree.dump(w));
    out.push('\n');
    out.push_str(&pixie_kernel::script::run(&rt, h, &mut tree, script));
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Runs the app: opens a window titled `title` whose view is what
/// `build` answers, rebuilt after every event. `width` and `height` ask
/// for a window size (0 is the engine's own), and `padding` is the
/// inset between the window and the tree — a negative one leaves the
/// engine's, and 0 lets the app paint to the edge. With `PIXIE_SCRIPT`
/// set it never opens a window: the kernel's headless harness (the one
/// the tier gate and generated apps use) builds the tree, prints its
/// dump, replays the script and prints what the steps produced.
///
/// # Safety
/// `title` is NULL or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pixie_run(
    title: *const c_char,
    width: f64,
    height: f64,
    padding: f64,
    build: PixieBuildFn,
) -> i32 {
    let title = unsafe { text_arg(title) };
    if let Ok(script) = std::env::var("PIXIE_SCRIPT") {
        let light = std::env::var("PIXIE_THEME").is_ok_and(|v| v == "light");
        print!("{}", headless(build, &script, light));
        return 0;
    }
    let mut w = World::new();
    let h = mount(&mut w, CView { build }, &[]);
    CURRENT_VIEW.with(|c| c.set(Some(h.erase())));
    let rt = Runtime::new(w);
    install_timers(&rt);
    let win = if width > 0.0 || height > 0.0 {
        Some((width, height))
    } else {
        None
    };
    // The engine polls the file and calls back on the window's own
    // thread, so the door can read Ruby there as it does anywhere else.
    let watch = WATCH.with(|c| c.borrow_mut().take()).map(|(path, f)| ReloadWatch {
        path: std::path::PathBuf::from(path),
        reload: Box::new(move |_w: &mut World| f() != 0),
    });
    run_app(rt, h, &title, watch, win, (padding >= 0.0).then_some(padding));
    0
}

#[cfg(test)]
mod tests;
