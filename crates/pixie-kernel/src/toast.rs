//! The toast countdown.
//!
//! A `Toast` with a positive `duration_ms` closes itself. Nothing in
//! the app schedules that: the element tree DECLARES it, the way a
//! `Behavior` declares an animation, and this pass turns the
//! declaration into a one-shot on the timer store every time the tree
//! is rebuilt.
//!
//! Three consequences, all of them the reason it is written here and
//! not in the app or the engine:
//!
//! - The clock is `anim::now` — the same input `advance:<ms>` moves —
//!   so a headless script stands a toast up, says the duration passed,
//!   and sees `on_close` run. No new script verb; the tier gate covers
//!   the countdown because it covers `advance:`.
//! - Both tiers get it for free. The compiled view and the interpreted
//!   view produce the same tree, and this pass runs on the tree.
//! - An app-side timer could not do it. `every` belongs to a store's
//!   lifetime, while a toast's countdown starts when the ELEMENT
//!   appears and must end when it leaves — an identity the view owns
//!   and the store has no name for.
//!
//! Identity is the element's index path from the root, which is what
//! positional identity means everywhere else in pixie. A toast that
//! MOVES in the tree is a different toast and starts its countdown
//! again; that is the same rule a `for` row's state follows.

use crate::{Element, Listener, Str, World};

/// The prefix every key this module arms carries, so a walk that
/// disarms what the tree no longer holds cannot reach a one-shot
/// somebody else armed.
const PREFIX: &str = "toast:";

/// Arm the countdowns the tree declares, and drop the ones it no
/// longer does. Called by `build_prepared`, after the animation
/// settle, so it sees the tree a frame would actually paint.
pub fn settle(w: &mut World, el: &Element) {
    let mut wanted: Vec<(Str, f64, Listener)> = Vec::new();
    collect(el, &mut Vec::new(), &mut wanted);
    // The overwhelmingly common frame: no toast on screen and no
    // countdown running. It touches no store and allocates nothing
    // past the empty vector above.
    if wanted.is_empty() && !crate::timer::any(w) {
        return;
    }
    // A toast that closed, moved or left the tree takes its countdown
    // with it: firing later would call a handler about a message the
    // user can no longer see.
    for key in crate::timer::armed_keys(w) {
        if key.as_str().starts_with(PREFIX) && !wanted.iter().any(|(k, ..)| *k == key) {
            crate::timer::disarm(w, &key);
        }
    }
    for (key, ms, cb) in wanted {
        // Idempotent: a toast that was already counting keeps the
        // deadline it had, so a rebuild triggered by anything else in
        // the app does not push its dismissal further away.
        crate::timer::arm_once(w, key, ms, Listener::clone(&cb));
    }
}

/// Every open, self-closing toast in the tree, paired with the key its
/// position gives it.
fn collect(el: &Element, path: &mut Vec<u32>, out: &mut Vec<(Str, f64, Listener)>) {
    if let Element::Toast {
        open: true,
        duration_ms,
        on_close: Some(cb),
        ..
    } = el
        && *duration_ms > 0.0
    {
        let mut key = String::from(PREFIX);
        for (i, seg) in path.iter().enumerate() {
            if i > 0 {
                key.push('.');
            }
            key.push_str(&seg.to_string());
        }
        out.push((Str::from(key), *duration_ms, Listener::clone(cb)));
    }
    for (i, c) in children_of(el).iter().enumerate() {
        path.push(i as u32);
        collect(c, path, out);
        path.pop();
    }
}

/// The children this pass descends into. A fourth copy of the kernel's
/// read-only child walk (`a11y`, `anim` and `theme` each keep one);
/// unifying them is a refactor of its own, not something a new widget
/// should do to three passes it did not come to change.
///
/// A virtualized `ListView` hands over the children it materialized,
/// like every other container: a toast declared inside a lazy row that
/// is scrolled out of view is not on screen, and should not be
/// counting down.
fn children_of(el: &Element) -> &[Element] {
    match el {
        Element::Column { children, .. }
        | Element::Row { children, .. }
        | Element::Grid { children, .. }
        | Element::GridCell { children, .. }
        | Element::Anim { children, .. }
        | Element::Semantics { children, .. }
        | Element::Tooltip { children, .. }
        | Element::ContextMenu { children, .. }
        | Element::Disabled { children }
        | Element::Sized { children, .. }
        | Element::Themed { children, .. }
        | Element::ListView { children, .. }
        | Element::ScrollView { children, .. }
        | Element::Modal { children, .. }
        | Element::Table { children, .. } => children,
        Element::Stack(cs) | Element::HScrollView(cs) | Element::DataTable(cs) => cs,
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    fn toast(ms: f64, open: bool, hits: &Rc<Cell<u32>>) -> Element {
        let h = Rc::clone(hits);
        Element::Toast {
            message: Str::from("saved"),
            open,
            duration_ms: ms,
            on_close: Some(Rc::new(move |_: &mut World| h.set(h.get() + 1))),
        }
    }

    #[test]
    fn a_duration_closes_itself_once_the_clock_passes_it() {
        let mut w = World::new();
        let hits = Rc::new(Cell::new(0));
        let tree = Element::column(vec![toast(1500.0, true, &hits)]);
        settle(&mut w, &tree);
        crate::anim::advance(&mut w, 1000.0);
        crate::timer::fire_due(&mut w);
        assert_eq!(hits.get(), 0, "closed before its duration was up");
        crate::anim::advance(&mut w, 500.0);
        crate::timer::fire_due(&mut w);
        assert_eq!(hits.get(), 1);
        // Spent: a one-shot does not repeat, however far the clock
        // runs afterwards.
        crate::anim::advance(&mut w, 10_000.0);
        crate::timer::fire_due(&mut w);
        assert_eq!(hits.get(), 1);
        assert!(!crate::timer::any(&w));
    }

    #[test]
    fn a_rebuild_does_not_push_the_deadline_away() {
        let mut w = World::new();
        let hits = Rc::new(Cell::new(0));
        for _ in 0..4 {
            let tree = Element::column(vec![toast(1000.0, true, &hits)]);
            settle(&mut w, &tree);
            crate::anim::advance(&mut w, 250.0);
            crate::timer::fire_due(&mut w);
        }
        assert_eq!(hits.get(), 1, "the countdown restarted on a rebuild");
    }

    #[test]
    fn leaving_the_tree_takes_the_countdown_with_it() {
        let mut w = World::new();
        let hits = Rc::new(Cell::new(0));
        let up = Element::column(vec![toast(1000.0, true, &hits)]);
        settle(&mut w, &up);
        assert!(crate::timer::any(&w));
        settle(&mut w, &Element::column(vec![]));
        assert!(!crate::timer::any(&w), "the countdown outlived the toast");
        crate::anim::advance(&mut w, 5000.0);
        crate::timer::fire_due(&mut w);
        assert_eq!(hits.get(), 0);
    }

    #[test]
    fn a_closed_or_undying_toast_arms_nothing() {
        let mut w = World::new();
        let hits = Rc::new(Cell::new(0));
        // Closed; open with no duration; open with a duration that is
        // not a duration. None of the three is counting down.
        settle(
            &mut w,
            &Element::column(vec![
                toast(1000.0, false, &hits),
                toast(0.0, true, &hits),
                toast(-1.0, true, &hits),
            ]),
        );
        assert!(!crate::timer::any(&w));
        crate::anim::advance(&mut w, 5000.0);
        crate::timer::fire_due(&mut w);
        assert_eq!(hits.get(), 0);
    }

    #[test]
    fn two_toasts_count_down_separately() {
        let mut w = World::new();
        let a = Rc::new(Cell::new(0));
        let b = Rc::new(Cell::new(0));
        let tree = Element::column(vec![toast(500.0, true, &a), toast(1500.0, true, &b)]);
        settle(&mut w, &tree);
        crate::anim::advance(&mut w, 600.0);
        crate::timer::fire_due(&mut w);
        assert_eq!((a.get(), b.get()), (1, 0));
        crate::anim::advance(&mut w, 1000.0);
        crate::timer::fire_due(&mut w);
        assert_eq!((a.get(), b.get()), (1, 1));
    }
}
