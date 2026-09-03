//! The application menu bar, declared beside the shortcuts.
//!
//! A menu item is a DECLARATION on a store — the menu it sits in, the
//! name it shows, and the handler it runs — registered the moment the
//! store exists. A window hands the list to the platform; a headless
//! script picks an item by name with `menu:<item>`, which runs the
//! same handler the platform would. That is what makes a menu
//! something the gate can check rather than something only a mouse
//! can reach.

use crate::World;
use std::rc::Rc;

type Pick = Rc<dyn Fn(&mut World)>;

struct Entry {
    menu: String,
    item: String,
    cb: Pick,
}

#[derive(Default)]
pub struct Menus {
    entries: Vec<Entry>,
}

fn store(w: &mut World) -> crate::Handle<Menus> {
    w.singleton::<Menus>(Menus::default)
}

/// Declare one item: which menu it belongs to, what it says, and what
/// it does. Declaration order is menu order.
pub fn item(w: &mut World, menu: &str, item: &str, cb: Pick) {
    let h = store(w);
    w.get_mut(h).entries.push(Entry {
        menu: menu.to_string(),
        item: item.to_string(),
        cb,
    });
}

/// The menus as the platform wants them: each menu once, in the order
/// its first item was declared, with `(menu, item, index)` triples.
/// The index is what a dispatched menu command carries back.
pub fn layout(w: &World) -> Vec<(String, Vec<(String, usize)>)> {
    let Some(h) = w.try_singleton_ref::<Menus>() else {
        return Vec::new();
    };
    let mut out: Vec<(String, Vec<(String, usize)>)> = Vec::new();
    for (i, e) in w.get(h).entries.iter().enumerate() {
        match out.iter_mut().find(|(m, _)| *m == e.menu) {
            Some((_, items)) => items.push((e.item.clone(), i)),
            None => out.push((e.menu.clone(), vec![(e.item.clone(), i)])),
        }
    }
    out
}

/// Is anything declared? The engine asks before it touches the
/// platform's menu bar, so an app that declares none keeps whatever
/// the platform gives it.
pub fn any(w: &World) -> bool {
    match w.try_singleton_ref::<Menus>() {
        Some(h) => !w.get(h).entries.is_empty(),
        None => false,
    }
}

/// Run the item at this index — what a window's menu dispatches.
pub fn pick_at(w: &mut World, index: usize) -> bool {
    let Some(h) = w.try_singleton_ref::<Menus>() else {
        return false;
    };
    let Some(cb) = w.get(h).entries.get(index).map(|e| e.cb.clone()) else {
        return false;
    };
    cb(w);
    true
}

/// Run the item with this name — what a script picks. Names are
/// unique across the bar in practice; the first match wins, which is
/// the rule `click:<label>` follows.
pub fn pick(w: &mut World, item: &str) -> bool {
    let Some(h) = w.try_singleton_ref::<Menus>() else {
        return false;
    };
    let Some(cb) = w
        .get(h)
        .entries
        .iter()
        .find(|e| e.item == item)
        .map(|e| e.cb.clone())
    else {
        return false;
    };
    cb(w);
    true
}
