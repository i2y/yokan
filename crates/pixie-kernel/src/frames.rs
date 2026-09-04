//! Handing each step's screen to whoever can draw it.
//!
//! A headless run has the whole screen as a value — that is what the
//! dump prints — but the kernel cannot turn a canvas into pixels: the
//! rasterizer lives in the engine, which is the half that knows about
//! displays. So the kernel holds a SINK, and whoever links a
//! rasterizer installs one. The script harness calls it after every
//! step, which is what lets a scripted run leave a picture of each
//! frame behind without opening a window.
//!
//! Nothing is installed by default, so a run that was not asked for
//! frames pays one `is_none` per step.

use crate::Element;
use std::cell::RefCell;

type Sink = Box<dyn Fn(&Element)>;

thread_local! {
    static SINK: RefCell<Option<Sink>> = const { RefCell::new(None) };
}

/// Take every step's screen from here on.
pub fn install(sink: Sink) {
    SINK.with(|s| *s.borrow_mut() = Some(sink));
}

/// Is anyone taking them?
pub fn wanted() -> bool {
    SINK.with(|s| s.borrow().is_some())
}

/// One step's screen. Called by the script harness; a run with no
/// sink installed does nothing.
pub fn emit(el: &Element) {
    SINK.with(|s| {
        if let Some(f) = s.borrow().as_ref() {
            f(el);
        }
    });
}
