//! Callbacks on the animation clock — repeating, and one-shot.
//!
//! A timer is a DECLARATION: a store says "run this every N ms" and
//! the runtime does, from the moment the store exists. Time is the
//! same input animation reads — `anim::now` — so a headless script
//! stepping the clock with `advance:<ms>` fires exactly the ticks a
//! window would have fired in that span, and the tier gate can compare
//! them. Nothing here polls a wall clock of its own.
//!
//! The one-shot half is younger and comes from the other direction: a
//! `Toast` in the element tree wants to close itself once, some
//! milliseconds from when it appeared. It rides here rather than in a
//! store of its own because everything that drives this file — the
//! engine's frame pump and the script's `advance:` arm — already asks
//! this module what is due. A second clock consumer would have to be
//! wired into both, and would be a second answer to "is anything
//! pending?" for the pump to get wrong.

use crate::{Str, World};
use std::rc::Rc;

type Tick = Rc<dyn Fn(&mut World)>;

struct Entry {
    period: f64,
    next: f64,
    cb: Tick,
    /// A one-shot's name, which is what lets the arming side ask
    /// whether its countdown is already running instead of restarting
    /// it on every rebuild. `None` on a repeating timer: `every` has no
    /// identity beyond the store that declared it.
    key: Option<Str>,
    once: bool,
}

#[derive(Default)]
pub struct Timers {
    entries: Vec<Entry>,
}

fn store(w: &mut World) -> crate::Handle<Timers> {
    w.singleton::<Timers>(Timers::default)
}

/// Declare a repeating callback. The first tick lands one period from
/// now, which is what `every` means in the languages that have it.
pub fn every(w: &mut World, period_ms: f64, cb: Tick) {
    let now = crate::anim::now(w);
    let period = if period_ms > 0.0 { period_ms } else { 1.0 };
    let h = store(w);
    w.get_mut(h).entries.push(Entry {
        period,
        next: now + period,
        cb,
        key: None,
        once: false,
    });
}

/// Arm a named one-shot `delay_ms` from now, unless that name is
/// already armed.
///
/// Idempotent on purpose: the caller is a pass over the element tree
/// that runs on EVERY rebuild, so "arm it again" has to mean "leave
/// the countdown where it is" — a toast that restarted its three
/// seconds every time the app touched anything would never close.
pub fn arm_once(w: &mut World, key: Str, delay_ms: f64, cb: Tick) {
    if armed(w, &key) {
        return;
    }
    let now = crate::anim::now(w);
    let h = store(w);
    w.get_mut(h).entries.push(Entry {
        // A one-shot never repeats, so its period is only the length
        // of the wait. Zero would divide by zero in the catch-up
        // arithmetic below, so it is floored the way `every`'s is.
        period: if delay_ms > 0.0 { delay_ms } else { 1.0 },
        next: now + delay_ms.max(0.0),
        cb,
        key: Some(key),
        once: true,
    });
}

/// Is this one-shot's countdown running?
pub fn armed(w: &World, key: &Str) -> bool {
    match w.try_singleton_ref::<Timers>() {
        Some(h) => w
            .get(h)
            .entries
            .iter()
            .any(|e| e.key.as_ref().is_some_and(|k| k == key)),
        None => false,
    }
}

/// Drop a named one-shot before it fires. What a toast leaving the
/// tree does: the countdown belonged to an element that is no longer
/// on screen, and firing it later would call a handler about something
/// the user cannot see.
pub fn disarm(w: &mut World, key: &Str) {
    let Some(h) = w.try_singleton_ref::<Timers>() else {
        return;
    };
    w.get_mut(h)
        .entries
        .retain(|e| !e.key.as_ref().is_some_and(|k| k == key));
}

/// Every armed one-shot name, for a caller that knows which of them
/// should still exist. Allocates, and is called once per rebuild only
/// while something is armed.
pub fn armed_keys(w: &World) -> Vec<Str> {
    match w.try_singleton_ref::<Timers>() {
        Some(h) => w
            .get(h)
            .entries
            .iter()
            .filter_map(|e| e.key.clone())
            .collect(),
        None => Vec::new(),
    }
}

/// Is anything waiting to tick? The engine asks this to keep the
/// frame pump alive, exactly as it does for a running animation.
pub fn any(w: &World) -> bool {
    match w.try_singleton_ref::<Timers>() {
        Some(h) => !w.get(h).entries.is_empty(),
        None => false,
    }
}

/// The moment the earliest timer is due, or `None` when none is.
pub fn next_due(w: &World) -> Option<f64> {
    let h = w.try_singleton_ref::<Timers>()?;
    w.get(h)
        .entries
        .iter()
        .map(|e| e.next)
        .fold(None, |acc: Option<f64>, n| Some(acc.map_or(n, |a| a.min(n))))
}

/// Run every callback the clock has passed, and answer whether any
/// ran. A callback may itself take the World, so the schedule is read
/// out first and the entries are put back after — the same shape the
/// task queue uses.
///
/// The answer is what lets a window tell a frame that DID something
/// from one that only arrived: a display refreshes far faster than an
/// app ticks, and a frame with no tick in it has nothing new to build.
///
/// A tick that is late by more than one period does NOT repeat: the
/// clock jumped (a slow frame, or a script advancing a minute at
/// once), and running a minute of ticks would be surprising where
/// catching up is what nobody asked for.
pub fn fire_due(w: &mut World) -> bool {
    let now = crate::anim::now(w);
    let Some(h) = w.try_singleton_ref::<Timers>() else {
        return false;
    };
    let mut due: Vec<Tick> = Vec::new();
    // Whether a REPEATING timer ran, which is not the same question as
    // whether anything ran — see the keys note below.
    let mut repeated = false;
    {
        let t = w.get_mut(h);
        let mut spent: Vec<usize> = Vec::new();
        for (i, e) in t.entries.iter_mut().enumerate() {
            if now >= e.next {
                due.push(e.cb.clone());
                if e.once {
                    spent.push(i);
                } else {
                    let missed = ((now - e.next) / e.period).floor() + 1.0;
                    e.next += e.period * missed;
                    repeated = true;
                }
            }
        }
        // A one-shot is spent by firing. Dropped BEFORE the callbacks
        // run, so a handler that puts the same toast back up arms a
        // fresh countdown instead of racing the one that just went
        // off. Back to front, so the earlier indexes stay valid.
        for i in spent.into_iter().rev() {
            t.entries.remove(i);
        }
    }
    let ticked = !due.is_empty();
    for cb in due {
        crate::contain("timer tick", || cb(w));
    }
    // The keys a tick saw are spent (`keys::end_tick`). Here, and only
    // when a tick actually ran: a window pumps frames at the display's
    // rate while a game ticks at thirty, so clearing them per FRAME
    // would take a press away before the tick that was meant to read
    // it. A script's `advance:` runs this same pass, which is what
    // makes the two runs agree about a keystroke.
    //
    // A ONE-SHOT does not spend them: it is not the app's tick, and a
    // toast closing itself in the same frame a key went down must not
    // eat that press from the tick that was going to read it.
    if repeated {
        crate::keys::end_tick();
    }
    ticked
}
