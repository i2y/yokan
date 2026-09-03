//! Key chords, and the handler a key reaches.
//!
//! A shortcut is a DECLARATION, the way a timer is: a store says
//! "this chord runs this handler" and the runtime binds it from the
//! moment the store exists. The chord is spelled the way the platform
//! spells it (`cmd-s`, `shift-tab`, `ctrl-cmd-space`), with `+`
//! accepted for the same thing, so nobody has to learn a second
//! vocabulary. A headless script's `key:<chord>` step fires exactly
//! what a window's keystroke would, which is what lets the tier gate
//! compare them.

use crate::{Str, World};
use std::rc::Rc;

type Fire = Rc<dyn Fn(&mut World)>;
type Typed = Rc<dyn Fn(&mut World, Str)>;

#[derive(Default)]
pub struct Keys {
    binds: Vec<(String, Fire)>,
    any: Vec<Typed>,
}

fn store(w: &mut World) -> crate::Handle<Keys> {
    w.singleton::<Keys>(Keys::default)
}

/// One chord, written once: modifiers in a fixed order and the key
/// last, so `cmd+shift+s`, `shift-cmd-s` and `Cmd-Shift-S` are one
/// chord and the two sides of a comparison cannot drift apart.
pub fn normalize(chord: &str) -> String {
    let mut mods: Vec<&str> = Vec::new();
    let mut key = String::new();
    for part in chord.split(['-', '+']).filter(|p| !p.is_empty()) {
        let p = part.trim().to_lowercase();
        match p.as_str() {
            "cmd" | "command" | "super" | "win" | "meta" | "platform" => mods.push("cmd"),
            "ctrl" | "control" => mods.push("ctrl"),
            "alt" | "opt" | "option" => mods.push("alt"),
            "shift" => mods.push("shift"),
            _ => key = p,
        }
    }
    let order = ["cmd", "ctrl", "alt", "shift"];
    let mut out = String::new();
    for m in order {
        if mods.contains(&m) {
            out.push_str(m);
            out.push('-');
        }
    }
    out.push_str(&key);
    out
}

/// Declare a shortcut.
pub fn bind(w: &mut World, chord: &str, cb: Fire) {
    let chord = normalize(chord);
    let h = store(w);
    w.get_mut(h).binds.push((chord, cb));
}

/// Declare a handler that sees every key, as the chord it was.
pub fn on_key(w: &mut World, cb: Typed) {
    let h = store(w);
    w.get_mut(h).any.push(cb);
}

/// Is anything listening? The engine asks before it installs a key
/// handler on the window.
pub fn any_bound(w: &World) -> bool {
    match w.try_singleton_ref::<Keys>() {
        Some(h) => {
            let k = w.get(h);
            !k.binds.is_empty() || !k.any.is_empty()
        }
        None => false,
    }
}

/// Deliver a chord. Shortcuts bound to it run first, in declaration
/// order, then every `on_key` handler. Answers whether anything ran,
/// which is how a script tells a typo from a key an app ignores.
pub fn fire(w: &mut World, chord: &str) -> bool {
    let chord = normalize(chord);
    let Some(h) = w.try_singleton_ref::<Keys>() else {
        return false;
    };
    let hit: Vec<Fire> = w
        .get(h)
        .binds
        .iter()
        .filter(|(c, _)| *c == chord)
        .map(|(_, f)| f.clone())
        .collect();
    let any: Vec<Typed> = w.get(h).any.clone();
    for f in &hit {
        f(w);
    }
    for f in &any {
        f(w, Str::from(chord.as_str()));
    }
    !hit.is_empty() || !any.is_empty()
}
