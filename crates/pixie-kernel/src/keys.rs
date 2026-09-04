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

// ---- the keyboard as a device ---------------------------------------
//
// A chord is a message: it is delivered to whatever declared it and
// then it is over. A game asks something else — "is left held right
// now" — and that is not a message but the state of a device.
//
// It lives here rather than in the World for the reason the clipboard
// does: a key is the keyboard's, not the app's, and the standard-
// library functions that read it are ordinary `fn(&str) -> bool` with
// no World in reach, which is what lets one implementation serve the
// interpreted and the compiled run.

use std::cell::RefCell;

thread_local! {
    /// What the platform says is down right now.
    static HELD: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// What went down, and what came up, since the last tick. Emptied
    /// by the timer pass that fires one, so a tick sees every press
    /// since the previous tick and never sees one twice — in a window
    /// pumping frames at the display's rate and under a script's
    /// `advance:` alike, because both go through `timer::fire_due`.
    static PRESSED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static RELEASED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// One key's name: the way a chord spells its key, without the
/// modifiers. `left` is held whether or not shift is down too, which
/// is what a game means by the question; the modifiers answer under
/// their own names (`shift`, `cmd`, `ctrl`, `alt`).
pub fn key_name(name: &str) -> String {
    name.trim().to_lowercase()
}

fn add(cell: &'static std::thread::LocalKey<RefCell<Vec<String>>>, key: &str) {
    cell.with(|c| {
        let mut v = c.borrow_mut();
        if !v.iter().any(|k| k == key) {
            v.push(key.to_string());
        }
    });
}

fn has(cell: &'static std::thread::LocalKey<RefCell<Vec<String>>>, key: &str) -> bool {
    let key = key_name(key);
    cell.with(|c| c.borrow().iter().any(|k| *k == key))
}

/// A key went down. `repeat` is the platform's auto-repeat (gpui's
/// `KeyDownEvent::is_held`): it holds the key down, but it is not a
/// new press — otherwise a held key would fire `pressed` forever and
/// nothing could tell "just pressed" from "still down".
pub fn press(key: &str, repeat: bool) {
    let key = key_name(key);
    if key.is_empty() {
        return;
    }
    let already = has(&HELD, &key);
    add(&HELD, &key);
    if !already && !repeat {
        add(&PRESSED, &key);
    }
}

/// A key came up.
pub fn release(key: &str) {
    let key = key_name(key);
    if key.is_empty() {
        return;
    }
    HELD.with(|c| c.borrow_mut().retain(|k| *k != key));
    add(&RELEASED, &key);
}

/// The modifier keys, which the platform reports as a state rather
/// than as key events. Named like any other key, so `down("shift")`
/// is the same question as `down("left")`.
pub fn set_modifiers(cmd: bool, ctrl: bool, alt: bool, shift: bool) {
    for (on, name) in [
        (cmd, "cmd"),
        (ctrl, "ctrl"),
        (alt, "alt"),
        (shift, "shift"),
    ] {
        let held = has(&HELD, name);
        if on && !held {
            press(name, false);
        } else if !on && held {
            release(name);
        }
    }
}

/// Is this key down?
pub fn down(key: &str) -> bool {
    has(&HELD, key)
}

/// Did it go down since the last tick?
pub fn pressed(key: &str) -> bool {
    has(&PRESSED, key)
}

/// Did it come up since the last tick?
pub fn released(key: &str) -> bool {
    has(&RELEASED, key)
}

/// The tick has run: what it saw is spent. Called by `timer::fire_due`
/// after it runs its callbacks, and only when it ran one.
pub fn end_tick() {
    PRESSED.with(|c| c.borrow_mut().clear());
    RELEASED.with(|c| c.borrow_mut().clear());
}

/// Nothing is held any more. The engine calls this when the window
/// stops being the active one: a key held while the app is switched
/// away is never released, and would stay down forever.
pub fn release_all() {
    let held: Vec<String> = HELD.with(|c| c.borrow().clone());
    for k in held {
        release(&k);
    }
}

#[cfg(test)]
mod device_tests {
    use super::*;

    fn reset() {
        HELD.with(|c| c.borrow_mut().clear());
        end_tick();
    }

    #[test]
    fn a_press_is_held_until_it_is_released() {
        reset();
        press("Left", false);
        assert!(down("left"), "spelling is normalized like a chord's key");
        assert!(pressed("left"));
        release("left");
        assert!(!down("left"));
        assert!(released("left"));
    }

    /// The whole point of the tick boundary: a press survives every
    /// frame until the tick that reads it, and is gone after it.
    #[test]
    fn a_tick_spends_what_it_saw() {
        reset();
        press("space", false);
        assert!(pressed("space"));
        end_tick();
        assert!(!pressed("space"), "a tick never sees the same press twice");
        assert!(down("space"), "but the key is still down");
        reset();
    }

    /// Auto-repeat holds the key down without pressing it again.
    #[test]
    fn a_repeat_is_not_a_new_press() {
        reset();
        press("z", false);
        end_tick();
        press("z", true);
        assert!(down("z"));
        assert!(!pressed("z"));
        release("z");
        reset();
    }

    #[test]
    fn modifiers_answer_under_their_own_names() {
        reset();
        set_modifiers(false, false, false, true);
        assert!(down("shift"));
        assert!(!down("cmd"));
        set_modifiers(false, false, false, false);
        assert!(!down("shift"));
        assert!(released("shift"));
        reset();
    }

    #[test]
    fn losing_the_window_releases_everything() {
        reset();
        press("left", false);
        press("space", false);
        release_all();
        assert!(!down("left") && !down("space"));
        reset();
    }
}
