//! pixie's kernel + gpui engine behind a C ABI.
//!
//! This is the substrate's C face, not any one language's: a door
//! written in another language opens the cdylib while developing and
//! links the staticlib when it ships, and both runs then drive exactly
//! the same engine — which is what makes comparing them worth doing.
//! Wakakusa (`wakakusa/`) is the first caller.
//!
//! The shape is the one `crates/yokan/src/lib.rs` gives CPython, with
//! the language-specific parts taken out: a Component whose `build`
//! calls a C function pointer that answers with an element handle, and
//! button listeners that call a C function pointer with the handler id
//! the app registered. State lives on the app's side of the ABI; every
//! event marks the view dirty and the engine rebuilds.
//!
//! Elements are built through the ABI as opaque integer handles into a
//! per-build arena (1-based; 0 is "no element"). A container consumes
//! its children's handles; the build callback returns the root's. The
//! arena is cleared at the start of every build, so a handle is only
//! meaningful inside the build that made it.
//!
//! Strings cross as NUL-terminated UTF-8 and are copied on entry: the
//! caller's buffer is only promised for the duration of the call
//! (spinel's rule for `:str`, and the right one for CRuby too).

use std::cell::{Cell, RefCell};
use std::ffi::{CStr, c_char};
use std::rc::Rc;

use pixie_engine_gpui::run_app;
use pixie_kernel::{Component, Element, ErasedHandle, Listener, Runtime, Str, World, mount};

/// The app's view: answers the root element's handle for this build.
pub type PixieBuildFn = extern "C" fn() -> i64;
/// The app's event dispatcher: receives the handler id a button carried.
pub type PixieEventFn = extern "C" fn(i64);

thread_local! {
    static ARENA: RefCell<Vec<Option<Element>>> = const { RefCell::new(Vec::new()) };
    static EVENT_FN: Cell<Option<PixieEventFn>> = const { Cell::new(None) };
    static CURRENT_VIEW: Cell<Option<ErasedHandle>> = const { Cell::new(None) };
}

fn put(el: Element) -> i64 {
    ARENA.with(|a| {
        let mut a = a.borrow_mut();
        a.push(Some(el));
        a.len() as i64
    })
}

fn take(h: i64) -> Option<Element> {
    if h <= 0 {
        return None;
    }
    ARENA.with(|a| a.borrow_mut().get_mut((h - 1) as usize).and_then(|s| s.take()))
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

/// # Safety
/// `ptr` points at `n` readable i64 handles, or `n` is 0.
unsafe fn children_arg(ptr: *const i64, n: usize) -> Vec<Element> {
    if ptr.is_null() || n == 0 {
        return Vec::new();
    }
    // SAFETY: the caller promises `n` readable handles at `ptr`.
    let handles = unsafe { std::slice::from_raw_parts(ptr, n) };
    handles
        .iter()
        .map(|&h| take(h).unwrap_or_else(|| Element::text("<consumed element>")))
        .collect()
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
// Element constructors (the same defaults `yokan`'s constructors write).

/// `text(s, size)`: `size` 0.0 is the engine's default.
///
/// # Safety
/// `text` is NULL or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pixie_text(text: *const c_char, size: f64) -> i64 {
    let text = unsafe { text_arg(text) };
    put(Element::Text {
        text: Str::from(text),
        font_size: size,
        color: Str::from(""),
        align: Str::from(""),
        grow: 0.0,
        bold: false,
        italic: false,
        mono: false,
        underline: false,
        wrap: Str::from(""),
        max_lines: 0,
        width: 0.0,
        background: Str::from(""),
        padding: 0.0,
        border_radius: 0.0,
        border_width: 0.0,
        border_color: Str::from(""),
    })
}

/// `button(label, handler)`: a click calls the registered event
/// function with `handler`, then marks the view dirty. A negative
/// handler is a button that does nothing.
///
/// # Safety
/// `label` is NULL or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pixie_button(label: *const c_char, handler: i64) -> i64 {
    let label = unsafe { text_arg(label) };
    let on_click: Listener = if handler < 0 {
        Rc::new(|_| {})
    } else {
        Rc::new(move |w: &mut World| {
            if let Some(f) = EVENT_FN.with(|c| c.get()) {
                f(handler);
            }
            if let Some(hv) = CURRENT_VIEW.with(|c| c.get()) {
                w.mark_view_dirty(hv);
            }
        })
    };
    put(Element::Button {
        label: Str::from(label),
        background: Str::from(""),
        hover_background: Str::from(""),
        active_background: Str::from(""),
        width: 0.0,
        height: 0.0,
        font_size: 0.0,
        color: Str::from(""),
        grow: 0.0,
        basis: 0.0,
        border_radius: 0.0,
        border_width: 0.0,
        border_color: Str::from(""),
        on_click,
    })
}

/// `column(children, n, spacing, padding)`: consumes the children.
/// `spacing` -1.0 is the engine's default.
///
/// # Safety
/// `children` points at `n` handles, or `n` is 0.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pixie_column(children: *const i64, n: usize, spacing: f64, padding: f64) -> i64 {
    let children = unsafe { children_arg(children, n) };
    put(Element::Column {
        spacing,
        padding,
        background: Str::from(""),
        grow: 0.0,
        border_radius: 0.0,
        border_width: 0.0,
        border_color: Str::from(""),
        children,
    })
}

/// `row(children, n, spacing, padding)`: consumes the children.
///
/// # Safety
/// `children` points at `n` handles, or `n` is 0.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pixie_row(children: *const i64, n: usize, spacing: f64, padding: f64) -> i64 {
    let children = unsafe { children_arg(children, n) };
    put(Element::Row {
        spacing,
        padding,
        background: Str::from(""),
        grow: 0.0,
        border_radius: 0.0,
        border_width: 0.0,
        border_color: Str::from(""),
        children,
    })
}

// ---------------------------------------------------------------------------
// The app's two callbacks, and the run.

/// Registers the function a button click reaches. Call before `pixie_run`.
#[unsafe(no_mangle)]
pub extern "C" fn pixie_set_event_handler(f: PixieEventFn) {
    EVENT_FN.with(|c| c.set(Some(f)));
}

#[derive(Clone)]
struct CView {
    build: PixieBuildFn,
}

impl Component for CView {
    fn build(&self, _w: &World) -> Element {
        ARENA.with(|a| a.borrow_mut().clear());
        let root = (self.build)();
        let el = take(root).unwrap_or_else(|| err_text("view returned no element"));
        ARENA.with(|a| a.borrow_mut().clear());
        el
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
/// `build` answers, rebuilt after every event. With `PIXIE_SCRIPT` set
/// it never opens a window: the kernel's headless harness (the one the
/// tier gate and generated apps use) builds the tree, prints its dump,
/// replays the script and prints what the steps produced.
///
/// # Safety
/// `title` is NULL or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pixie_run(title: *const c_char, build: PixieBuildFn) -> i32 {
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
    run_app(rt, h, &title, None, None, None);
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::sync::atomic::{AtomicI64, Ordering};

    /// The app's state, on the app's side of the ABI — where a door
    /// keeps it too.
    static COUNT: AtomicI64 = AtomicI64::new(0);

    /// A door's `view`, written in Rust: handles in, one handle out.
    extern "C" fn build() -> i64 {
        let label = CString::new(format!("count: {}", COUNT.load(Ordering::Relaxed))).unwrap();
        let plus = CString::new("+1").unwrap();
        // SAFETY: both pointers are NUL-terminated and outlive the calls.
        let kids = unsafe { [pixie_text(label.as_ptr(), 34.0), pixie_button(plus.as_ptr(), 7)] };
        // SAFETY: two readable handles, and the column consumes them.
        unsafe { pixie_column(kids.as_ptr(), kids.len(), 12.0, 16.0) }
    }

    extern "C" fn on_event(handler: i64) {
        assert_eq!(handler, 7, "the button carries the id the door registered");
        COUNT.fetch_add(1, Ordering::Relaxed);
    }

    /// The whole loop through the C face: build a tree, click the
    /// button the engine found by its label, watch the event reach the
    /// app's state, and see the rebuild carry the new value.
    #[test]
    fn a_click_reaches_the_app_and_the_rebuild_shows_it() {
        pixie_set_event_handler(on_event);
        assert_eq!(
            headless(build, "click:+1,dump", false),
            // The dump at the start, the `dump` step, and the one every
            // scripted run ends with.
            "Column(spacing=12, padding=16)[Text(count: 0, fontSize=34), Button(+1)]\n\
             Column(spacing=12, padding=16)[Text(count: 1, fontSize=34), Button(+1)]\n\
             Column(spacing=12, padding=16)[Text(count: 1, fontSize=34), Button(+1)]\n"
        );
    }
}
