//! What a task says about itself while it runs.
//!
//! A task exists so the window keeps drawing while something slow
//! happens somewhere else. That leaves the app with nothing to show
//! but a spinner, because the only news it gets is the value at the
//! end. A report is the missing half: the work says how far it has
//! come, and a handler on the UI thread hears it.
//!
//! Two things make that safe. The report is DATA — a fraction and a
//! line of text — pushed onto a queue from whatever thread the work
//! runs on; nothing crosses the boundary but those. And the queue is
//! drained where a `&mut World` is already in hand: the window's
//! pump, the headless settle, and the moment an `await` comes back —
//! so the handler runs exactly where every other handler runs, and
//! the app is never touched from two threads at once.
//!
//! Reports are addressed to a task, not to the program: an `async fn`
//! carrying `@progress(handler)` gets an id when it starts, the work
//! it awaits inherits that id on the worker thread, and the drain
//! calls that task's handler. Two tasks reporting at once stay apart.

use crate::{Str, World};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

type Listener = Rc<dyn Fn(&mut World, f64, Str)>;

/// The handlers waiting to hear from a running task. Lives in the
/// World because a listener is an `Rc` closure over the UI thread's
/// objects — the queue is what crosses threads, never this.
#[derive(Default)]
pub struct Progress {
    listeners: Vec<(u64, Listener)>,
}

fn store(w: &mut World) -> crate::Handle<Progress> {
    w.singleton::<Progress>(Progress::default)
}

static QUEUE: OnceLock<Mutex<Vec<(u64, f64, String)>>> = OnceLock::new();
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

thread_local! {
    /// The task the code on THIS thread is running for. The UI thread
    /// sets it around an async body's turn; a worker inherits the id
    /// of the task that spawned it. Zero is "nobody is listening".
    static CURRENT: Cell<u64> = const { Cell::new(0) };
}

fn queue() -> &'static Mutex<Vec<(u64, f64, String)>> {
    QUEUE.get_or_init(|| Mutex::new(Vec::new()))
}

/// A fresh task id, claimed when an `async fn` with a progress
/// handler starts.
pub fn new_task() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::SeqCst)
}

/// Attach a task's handler. Called on the UI thread as the task
/// starts, and paired with `end`.
pub fn begin(w: &mut World, task: u64, cb: Listener) {
    let h = store(w);
    w.get_mut(h).listeners.push((task, cb));
}

/// The task is over: hand over its last words, then drop its
/// handler. Draining FIRST is what makes "every report is heard"
/// true — a report made by the final line of the work would
/// otherwise be queued behind the end of the task that made it.
pub fn end(w: &mut World, task: u64) {
    drain(w);
    let h = store(w);
    w.get_mut(h).listeners.retain(|(t, _)| *t != task);
    if current() == task {
        set_current(0);
    }
}

/// Say which task the code on this thread belongs to. The generated
/// async body calls it on the UI thread when it starts and after
/// every `await`; a worker calls it once, with the id it inherited.
pub fn set_current(task: u64) {
    CURRENT.with(|c| c.set(task));
}

/// The task this thread is running for, or zero.
pub fn current() -> u64 {
    CURRENT.with(|c| c.get())
}

/// The work's own voice: how far along, and what it is doing. Called
/// from the worker thread an `await` put the work on, from a `@py`
/// escape's Python, or from the async body itself between steps.
/// Outside a task it does nothing — there is no handler to reach, and
/// silence is the same answer in both runs.
pub fn report(fraction: f64, note: &str) {
    let task = current();
    if task == 0 {
        return;
    }
    queue()
        .lock()
        .unwrap()
        .push((task, fraction, note.to_string()));
}

/// Is anything waiting to be heard? The drain sites ask first, so a
/// frame that has no news pays one atomic read.
pub fn any() -> bool {
    !queue().lock().unwrap().is_empty()
}

/// Hand every queued report to its task's handler, in the order the
/// work made them. The handlers run here, on the UI thread, with the
/// World in hand — so a report is an ordinary state change and the
/// view rebuilds from it the way it does from a click.
pub fn drain(w: &mut World) {
    let pending: Vec<(u64, f64, String)> = std::mem::take(&mut *queue().lock().unwrap());
    if pending.is_empty() {
        return;
    }
    let Some(h) = w.try_singleton_ref::<Progress>() else {
        return;
    };
    for (task, fraction, note) in pending {
        let cb = w
            .get(h)
            .listeners
            .iter()
            .find(|(t, _)| *t == task)
            .map(|(_, c)| c.clone());
        if let Some(cb) = cb {
            crate::contain("progress handler", || cb(w, fraction, Str::from(note.as_str())));
        }
    }
}
