//! Repeating callbacks on the animation clock.
//!
//! A timer is a DECLARATION: a store says "run this every N ms" and
//! the runtime does, from the moment the store exists. Time is the
//! same input animation reads — `anim::now` — so a headless script
//! stepping the clock with `advance:<ms>` fires exactly the ticks a
//! window would have fired in that span, and the tier gate can compare
//! them. Nothing here polls a wall clock of its own.

use crate::World;
use std::rc::Rc;

type Tick = Rc<dyn Fn(&mut World)>;

struct Entry {
    period: f64,
    next: f64,
    cb: Tick,
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
    });
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
    {
        let t = w.get_mut(h);
        for e in t.entries.iter_mut() {
            if now >= e.next {
                due.push(e.cb.clone());
                let missed = ((now - e.next) / e.period).floor() + 1.0;
                e.next += e.period * missed;
            }
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
    if ticked {
        crate::keys::end_tick();
    }
    ticked
}
