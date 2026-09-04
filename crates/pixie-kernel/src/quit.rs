//! Closing the window from inside the app.
//!
//! A device, not app state — the same shape as the clipboard and the
//! keyboard: the app asks, and whoever is holding a window answers.
//! The engine takes the request on its next frame and closes; a
//! headless run never takes it, so a script keeps running its steps
//! and the two runs print the same dumps. That is deliberate: a
//! window closing is not something a dump can show, and a compiled
//! binary that exited halfway through a script would differ from the
//! interpreted one for a reason the app never asked about.

use std::cell::Cell;

thread_local! {
    static ASKED: Cell<bool> = const { Cell::new(false) };
}

/// The app asks to close.
pub fn request() {
    ASKED.with(|c| c.set(true));
}

/// Has it asked? Taking the request clears it, so one `quit()` closes
/// one window and a second frame does not try again.
pub fn take() -> bool {
    ASKED.with(|c| c.replace(false))
}
