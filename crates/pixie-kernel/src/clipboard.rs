//! The system clipboard, as one value both runs can see.
//!
//! An app copies and pastes through here; a window syncs this with
//! the platform's own clipboard once per frame, and a headless run
//! keeps the value to itself. That is what makes copy-and-paste a
//! checked interaction: the two tiers of a script agree because
//! neither of them reaches a machine-wide buffer, while a real window
//! still exchanges text with every other application.

use crate::Str;
use std::cell::RefCell;

thread_local! {
    static TEXT: RefCell<String> = const { RefCell::new(String::new()) };
    /// Set by the app and not yet handed to the platform.
    static PENDING: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Put text on the clipboard.
pub fn set(text: &str) {
    TEXT.with(|t| *t.borrow_mut() = text.to_string());
    PENDING.with(|p| *p.borrow_mut() = Some(text.to_string()));
}

/// What is on the clipboard.
pub fn get() -> Str {
    TEXT.with(|t| Str::from(t.borrow().as_str()))
}

/// Take what the app copied but the platform has not been told about.
/// The engine calls this; a headless run never does, which is why a
/// script's clipboard stays its own.
pub fn take_pending() -> Option<String> {
    PENDING.with(|p| p.borrow_mut().take())
}

/// What the platform says the clipboard holds. Not pending: adopting
/// is how the outside world gets in, and pushing it back out would
/// be an echo.
pub fn adopt(text: String) {
    TEXT.with(|t| *t.borrow_mut() = text);
}
