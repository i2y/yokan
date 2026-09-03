//! Files dragged onto the window.
//!
//! A drop is a declaration like a shortcut: the app says what to do
//! with a path, and the runtime hands it one whenever a file lands on
//! the window. A headless script drops with `drop:<path>`, so an app
//! that accepts files is replayable — the platform's drag is the only
//! part a script cannot bring, and it is the part that carries no
//! meaning of its own.

use crate::{Str, World};
use std::rc::Rc;

type Dropped = Rc<dyn Fn(&mut World, Str)>;

#[derive(Default)]
pub struct Drops {
    handlers: Vec<Dropped>,
}

fn store(w: &mut World) -> crate::Handle<Drops> {
    w.singleton::<Drops>(Drops::default)
}

/// Declare what happens to a file dropped on the window.
pub fn on_file(w: &mut World, cb: Dropped) {
    let h = store(w);
    w.get_mut(h).handlers.push(cb);
}

/// Is anything listening? The engine asks before it accepts a drop.
pub fn any(w: &World) -> bool {
    match w.try_singleton_ref::<Drops>() {
        Some(h) => !w.get(h).handlers.is_empty(),
        None => false,
    }
}

/// Deliver one path. Answers whether anything took it, which is how a
/// script tells a typo from an app that ignores files.
pub fn fire(w: &mut World, path: &str) -> bool {
    let Some(h) = w.try_singleton_ref::<Drops>() else {
        return false;
    };
    let hs: Vec<Dropped> = w.get(h).handlers.clone();
    for f in &hs {
        f(w, Str::from(path));
    }
    !hs.is_empty()
}
