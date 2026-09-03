//! File dialogs: a request from the app, a panel from the platform,
//! and a canned answer in a headless run.
//!
//! A dialog waits for a person, so it belongs inside a task: the
//! calling thread blocks on the answer while the window keeps
//! drawing and drains the request on its next frame. A headless run
//! has no window and no person, so it answers from a queue a script
//! filled with `file:<path>` steps — which is what lets a flow that
//! opens a file be replayed and compared like any other.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock, mpsc};

/// Which panel the app asked for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Open,
    Save,
}

pub struct Request {
    pub id: u64,
    pub kind: Kind,
    /// The panel's title (open) or the suggested file name (save).
    pub label: String,
    reply: mpsc::Sender<String>,
}

impl Request {
    /// Hand the app its answer: a path, or the empty string when the
    /// person cancelled.
    pub fn answer(self, path: &str) {
        let _ = self.reply.send(path.to_string());
    }
}

static QUEUE: OnceLock<Mutex<Vec<Request>>> = OnceLock::new();
static SCRIPTED: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
static WINDOWED: AtomicBool = AtomicBool::new(false);
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn queue() -> &'static Mutex<Vec<Request>> {
    QUEUE.get_or_init(|| Mutex::new(Vec::new()))
}

fn scripted() -> &'static Mutex<Vec<String>> {
    SCRIPTED.get_or_init(|| Mutex::new(Vec::new()))
}

/// The engine says a window is there to open panels.
pub fn windowed() {
    WINDOWED.store(true, Ordering::SeqCst);
}

/// A script's answer to the next dialog (`file:<path>`), or an empty
/// path for "the person cancelled".
pub fn push_answer(path: &str) {
    scripted().lock().unwrap().push(path.to_string());
}

/// Ask for a panel and wait for the answer. Called from a task's
/// thread; a headless run answers from the script's queue instead.
pub fn ask(kind: Kind, label: &str) -> String {
    if !WINDOWED.load(Ordering::SeqCst) {
        let mut q = scripted().lock().unwrap();
        if q.is_empty() {
            return String::new();
        }
        return q.remove(0);
    }
    let (tx, rx) = mpsc::channel();
    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
    queue().lock().unwrap().push(Request {
        id,
        kind,
        label: label.to_string(),
        reply: tx,
    });
    rx.recv().unwrap_or_default()
}

/// Anything waiting? The engine asks before it touches the platform.
pub fn any() -> bool {
    !queue().lock().unwrap().is_empty()
}

/// The requests the window has yet to open, taken for opening.
pub fn take() -> Vec<Request> {
    std::mem::take(&mut *queue().lock().unwrap())
}
