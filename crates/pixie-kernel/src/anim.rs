//! Declarative animation (§8.35).
//!
//! The shape is Qt Quick's, not SwiftUI's: a `Behavior`-style
//! per-element declaration, no ambient transaction threaded down the
//! tree. That choice is what lets animation land WITHOUT a context
//! mechanism underneath it.
//!
//! Everything here runs on the KERNEL element tree, after `build` and
//! before the engine ever sees it. Three consequences, all deliberate:
//!
//! - `build` stays pure. Targets come out of the view; the current
//!   value comes out of this store, keyed by the element's path — the
//!   same positional identity the rest of pixie uses.
//! - Both tiers get it for free. The compiled and interpreted views
//!   produce the same tree, so the same pass runs over both and the
//!   tier gate covers interpolation and retention, not just parsing.
//! - Time is an input, not a hidden global. `set_now` / `advance` are
//!   the only clock, so a headless script can stand at t+120 ms and
//!   dump what a frame would have painted.

use crate::{Element, Str, World};
use std::collections::{HashMap, HashSet};

/// The element's index path from the root — pixie's positional
/// identity, spelled for the animation store.
type Path = Vec<u32>;

// ---------------------------------------------------------------------------
// Easing.

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Easing {
    Linear,
    In,
    /// The default: fast start, soft landing. Matches what both Qt
    /// Quick and Flutter reach for when nobody says otherwise.
    #[default]
    Out,
    InOut,
}

impl Easing {
    pub fn parse(s: &str) -> Option<Easing> {
        match s {
            "linear" => Some(Easing::Linear),
            "in" => Some(Easing::In),
            "out" => Some(Easing::Out),
            "inOut" => Some(Easing::InOut),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Easing::Linear => "linear",
            Easing::In => "in",
            Easing::Out => "out",
            Easing::InOut => "inOut",
        }
    }

    fn at(self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::In => t * t,
            Easing::Out => 1.0 - (1.0 - t) * (1.0 - t),
            Easing::InOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - 2.0 * (1.0 - t) * (1.0 - t)
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Values.

/// What a tween can interpolate. Colors ride as premultiplied-free
/// f64 channels so the arithmetic is the same as a number's.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Val {
    Num(f64),
    Rgba(f64, f64, f64, f64),
}

impl Val {
    fn lerp(self, to: Val, t: f64) -> Val {
        match (self, to) {
            (Val::Num(a), Val::Num(b)) => Val::Num(a + (b - a) * t),
            (Val::Rgba(a0, a1, a2, a3), Val::Rgba(b0, b1, b2, b3)) => Val::Rgba(
                a0 + (b0 - a0) * t,
                a1 + (b1 - a1) * t,
                a2 + (b2 - a2) * t,
                a3 + (b3 - a3) * t,
            ),
            // Mismatched kinds cannot be blended — snap, the same
            // answer an unparsable color gets.
            _ => to,
        }
    }
}

/// Parse the hex-color grammar the engine already accepts: `#rgb`,
/// `#rgba`, `#rrggbb`, `#rrggbbaa`. Anything else (a theme token name
/// like "accent", or empty) is NOT animatable here — token resolution
/// lives in the engine, so the kernel cannot know its endpoints and
/// snaps instead of guessing.
fn parse_hex(s: &str) -> Option<Val> {
    let h = s.strip_prefix('#')?;
    let n = |i: usize, w: usize| -> Option<f64> {
        let part = h.get(i..i + w)?;
        let v = u8::from_str_radix(part, 16).ok()?;
        Some(if w == 1 { (v * 17) as f64 } else { v as f64 })
    };
    match h.len() {
        3 => Some(Val::Rgba(n(0, 1)?, n(1, 1)?, n(2, 1)?, 255.0)),
        4 => Some(Val::Rgba(n(0, 1)?, n(1, 1)?, n(2, 1)?, n(3, 1)?)),
        6 => Some(Val::Rgba(n(0, 2)?, n(2, 2)?, n(4, 2)?, 255.0)),
        8 => Some(Val::Rgba(n(0, 2)?, n(2, 2)?, n(4, 2)?, n(6, 2)?)),
        _ => None,
    }
}

/// Interpolated values are rounded before they land in the tree:
/// four decimals is finer than any pixel, and it keeps a mid-flight
/// dump readable instead of printing float noise.
fn round4(v: f64) -> f64 {
    (v * 10_000.0).round() / 10_000.0
}

fn format_hex(v: Val) -> Str {
    let Val::Rgba(r, g, b, a) = v else {
        return Str::new();
    };
    let q = |c: f64| c.round().clamp(0.0, 255.0) as u8;
    if q(a) == 255 {
        Str::from(format!("#{:02x}{:02x}{:02x}", q(r), q(g), q(b)))
    } else {
        Str::from(format!("#{:02x}{:02x}{:02x}{:02x}", q(r), q(g), q(b), q(a)))
    }
}

// ---------------------------------------------------------------------------
// Tracks.

struct Track {
    from: Val,
    to: Val,
    /// The exact string a color target came from. Restored verbatim
    /// once the tween lands, so a settled animation dumps identically
    /// to one that never ran — the property the tier gate leans on.
    to_str: Option<Str>,
    start: f64,
    dur: f64,
    ease: Easing,
}

impl Track {
    fn done(&self, now: f64) -> bool {
        self.dur <= 0.0 || now >= self.start + self.dur
    }

    fn value(&self, now: f64) -> Val {
        if self.done(now) {
            return self.to;
        }
        let t = self.ease.at((now - self.start) / self.dur);
        self.from.lerp(self.to, t)
    }
}

/// A retained subtree on its way out. It is NOT part of the built
/// tree any more — the settle pass re-inserts it at its old index
/// each frame until the fade finishes.
struct Retained {
    parent: Path,
    index: usize,
    start: f64,
    dur: f64,
    ease: Easing,
    el: Element,
}

impl Retained {
    fn opacity(&self, now: f64) -> f64 {
        if self.dur <= 0.0 {
            return 0.0;
        }
        let t = self.ease.at((now - self.start) / self.dur);
        1.0 - t
    }
}

// ---------------------------------------------------------------------------
// The store.

#[derive(Default)]
pub struct AnimStore {
    now: f64,
    /// The platform asked for reduced motion: every duration reads as
    /// zero, so values snap and nothing requests another frame.
    reduced: bool,
    tracks: HashMap<(Path, &'static str), Track>,
    /// Enter fades, keyed by the `Anim` node's path.
    fades: HashMap<Path, Track>,
    /// `Anim` paths the previous settle walked — the "did this just
    /// appear?" test that drives `enter:`.
    seen_last: HashSet<Path>,
    /// Children of every container that held an exiting child last
    /// frame. Snapshotting only those keeps the cost proportional to
    /// what the view opted into.
    kids_last: HashMap<Path, Vec<Element>>,
    retained: Vec<Retained>,
    active: bool,
}

impl AnimStore {
    fn dur_of(&self, d: f64) -> f64 {
        if self.reduced {
            0.0
        } else {
            d
        }
    }
}

fn store(w: &mut World) -> crate::Handle<AnimStore> {
    w.singleton::<AnimStore>(AnimStore::default)
}

/// Set the animation clock, in milliseconds since an arbitrary epoch.
/// The engine calls this once per frame with the wall clock; the
/// headless harness steps it instead.
pub fn set_now(w: &mut World, ms: f64) {
    let h = store(w);
    w.get_mut(h).now = ms;
}

/// Step the clock forward. `advance:<ms>` in a `PIXIE_SCRIPT` is this.
pub fn advance(w: &mut World, ms: f64) {
    let h = store(w);
    w.get_mut(h).now += ms;
}

pub fn now(w: &World) -> f64 {
    match w.try_singleton_ref::<AnimStore>() {
        Some(h) => w.get(h).now,
        None => 0.0,
    }
}

pub fn set_reduced(w: &mut World, on: bool) {
    let h = store(w);
    w.get_mut(h).reduced = on;
}

/// True while any tween, fade, or retained subtree is still running
/// at the current clock. The engine asks for another frame on it; the
/// headless harness settles on it.
pub fn active(w: &World) -> bool {
    match w.try_singleton_ref::<AnimStore>() {
        Some(h) => w.get(h).active,
        None => false,
    }
}

/// The latest moment any live animation finishes, or `None` when
/// nothing is running. The headless settle jumps straight here rather
/// than stepping frame by frame.
pub fn last_end(w: &World) -> Option<f64> {
    let h = w.try_singleton_ref::<AnimStore>()?;
    let st = w.get(h);
    let mut end: Option<f64> = None;
    let mut bump = |e: f64| {
        end = Some(match end {
            Some(cur) if cur >= e => cur,
            _ => e,
        });
    };
    for t in st.tracks.values() {
        if !t.done(st.now) {
            bump(t.start + t.dur);
        }
    }
    for f in st.fades.values() {
        if !f.done(st.now) {
            bump(f.start + f.dur);
        }
    }
    for r in &st.retained {
        bump(r.start + r.dur);
    }
    end
}

// ---------------------------------------------------------------------------
// Animatable slots.

enum Slot<'a> {
    Num(&'a mut f64),
    Color(&'a mut Str),
}

/// The v0 animatable surface, by element. Every entry is a value the
/// engine already reads straight off the element, so a tween is a
/// value substitution and nothing downstream has to know.
fn slots(el: &mut Element) -> Vec<(&'static str, Slot<'_>)> {
    match el {
        Element::Text {
            font_size, color, ..
        } => vec![
            ("fontSize", Slot::Num(font_size)),
            ("color", Slot::Color(color)),
        ],
        Element::Button {
            background,
            width,
            height,
            font_size,
            color,
            grow,
            basis,
            border_radius,
            border_width,
            border_color,
            ..
        } => vec![
            ("background", Slot::Color(background)),
            ("width", Slot::Num(width)),
            ("height", Slot::Num(height)),
            ("fontSize", Slot::Num(font_size)),
            ("color", Slot::Color(color)),
            ("grow", Slot::Num(grow)),
            ("basis", Slot::Num(basis)),
            ("borderRadius", Slot::Num(border_radius)),
            ("borderWidth", Slot::Num(border_width)),
            ("borderColor", Slot::Color(border_color)),
        ],
        Element::Column {
            spacing,
            padding,
            background,
            grow,
            border_radius,
            border_width,
            border_color,
            ..
        }
        | Element::Row {
            spacing,
            padding,
            background,
            grow,
            border_radius,
            border_width,
            border_color,
            ..
        } => vec![
            ("spacing", Slot::Num(spacing)),
            ("padding", Slot::Num(padding)),
            ("background", Slot::Color(background)),
            ("grow", Slot::Num(grow)),
            ("borderRadius", Slot::Num(border_radius)),
            ("borderWidth", Slot::Num(border_width)),
            ("borderColor", Slot::Color(border_color)),
        ],
        Element::Grid {
            spacing,
            padding,
            background,
            grow,
            border_radius,
            border_width,
            border_color,
            ..
        } => vec![
            ("spacing", Slot::Num(spacing)),
            ("padding", Slot::Num(padding)),
            ("background", Slot::Color(background)),
            ("grow", Slot::Num(grow)),
            ("borderRadius", Slot::Num(border_radius)),
            ("borderWidth", Slot::Num(border_width)),
            ("borderColor", Slot::Color(border_color)),
        ],
        Element::Image { width, height, .. } | Element::Svg { width, height, .. } => {
            vec![("width", Slot::Num(width)), ("height", Slot::Num(height))]
        }
        Element::BarChart { width, height, .. } | Element::LineChart { width, height, .. } => {
            vec![("width", Slot::Num(width)), ("height", Slot::Num(height))]
        }
        // cute_ui's eased fill-toward-value, finally: a ProgressBar
        // under `animate:` sweeps to its new value instead of jumping.
        Element::ProgressBar { value } => vec![("value", Slot::Num(value))],
        Element::Spinner { size } => vec![("size", Slot::Num(size))],
        Element::ScrollView { height, .. } => vec![("height", Slot::Num(height))],
        Element::ListView {
            item_height, height, ..
        } => vec![
            ("itemHeight", Slot::Num(item_height)),
            ("height", Slot::Num(height)),
        ],
        _ => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// The settle pass.

/// Reconcile the tree against the store at the current clock and hand
/// back what a frame should paint. Called by `build_prepared`, so
/// every rebuild path — windowed, headless, both tiers — goes through
/// it exactly once.
pub fn settle(w: &mut World, mut el: Element) -> Element {
    let h = store(w);
    // The walk needs the store mutably while it also holds the tree;
    // taking it out keeps both borrows honest and costs one move.
    let mut st = std::mem::take(w.get_mut(h));
    st.active = false;

    let mut seen: HashSet<Path> = HashSet::new();
    let mut kids: HashMap<Path, Vec<Element>> = HashMap::new();
    let mut fresh_exits: Vec<Retained> = Vec::new();
    let mut path: Path = Vec::new();
    walk(&mut st, &mut el, &mut path, &mut seen, &mut kids, &mut fresh_exits);

    // Retire finished exits, admit new ones. A path that leaves twice
    // in a row restarts its fade — the second departure is a new one.
    let now = st.now;
    st.retained.retain(|r| now < r.start + r.dur);
    st.retained.extend(fresh_exits);

    // Drop tracks whose element is gone: positional identity means a
    // vanished path is a vanished element, and keeping its track
    // would hand stale state to whatever lands there next.
    st.tracks.retain(|(p, _), _| seen.contains(p));
    st.fades.retain(|p, f| seen.contains(p) && !f.done(now));

    st.active = st.tracks.values().any(|t| !t.done(now))
        || st.fades.values().any(|f| !f.done(now))
        || !st.retained.is_empty();

    st.seen_last = seen;
    st.kids_last = kids;

    // Re-insert the leaving subtrees LAST, after every path has been
    // computed. A retained child is a paint-time addition only, so it
    // never shifts a live sibling's path out from under its tracks.
    let mut pending: Vec<(Path, usize, Element)> = st
        .retained
        .iter()
        .map(|r| {
            let mut ghost = r.el.clone();
            if let Element::Anim { opacity, .. } = &mut ghost {
                *opacity = round4(r.opacity(now));
            }
            (r.parent.clone(), r.index, ghost)
        })
        .collect();
    pending.sort_by_key(|(p, i, _)| (p.clone(), *i));
    for (parent, index, ghost) in pending {
        insert_at(&mut el, &parent, index, ghost);
    }

    *w.get_mut(h) = st;
    el
}

fn walk(
    st: &mut AnimStore,
    el: &mut Element,
    path: &mut Path,
    seen: &mut HashSet<Path>,
    kids: &mut HashMap<Path, Vec<Element>>,
    exits: &mut Vec<Retained>,
) {
    if let Element::Anim {
        duration,
        easing,
        enter,
        opacity,
        children,
        ..
    } = el
    {
        seen.insert(path.clone());
        let dur = st.dur_of(*duration);
        let ease = *easing;
        if *enter && dur > 0.0 && !st.seen_last.contains(path) && !st.fades.contains_key(path) {
            st.fades.insert(
                path.clone(),
                Track {
                    from: Val::Num(0.0),
                    to: Val::Num(1.0),
                    to_str: None,
                    start: st.now,
                    dur,
                    ease,
                },
            );
        }
        *opacity = match st.fades.get(path) {
            Some(f) => match f.value(st.now) {
                Val::Num(v) => round4(v),
                _ => 1.0,
            },
            None => 1.0,
        };
        if dur > 0.0 {
            if let Some(child) = children.first_mut() {
                // Through any semantics/theme wrapper: `animate:` and
                // `role:` on one element must not disable each other.
                tween_props(st, child.inner_mut(), path, dur, ease);
            }
        }
    }
    let Some(cs) = children_of(el) else {
        return;
    };
    for (i, c) in cs.iter_mut().enumerate() {
        path.push(i as u32);
        walk(st, c, path, seen, kids, exits);
        path.pop();
    }
    // Removal detection, positional like everything else: the
    // children this container lost are the TAIL of last frame's list.
    // Losing a MIDDLE child renumbers its followers, so what reads as
    // leaving is the last one — Flutter's no-key rule taken at its
    // word rather than papered over with a heuristic.
    if let Some(prev) = st.kids_last.get(path) {
        if prev.len() > cs.len() {
            for (i, old) in prev.iter().enumerate().skip(cs.len()) {
                let Element::Anim {
                    exit: true,
                    duration,
                    easing,
                    ..
                } = old
                else {
                    continue;
                };
                let dur = st.dur_of(*duration);
                if dur > 0.0 {
                    exits.push(Retained {
                        parent: path.clone(),
                        index: i,
                        start: st.now,
                        dur,
                        ease: *easing,
                        el: old.clone(),
                    });
                }
            }
        }
    }
    if cs
        .iter()
        .any(|c| matches!(c, Element::Anim { exit: true, .. }))
    {
        kids.insert(path.clone(), cs.clone());
    }
}

fn tween_props(st: &mut AnimStore, child: &mut Element, path: &Path, dur: f64, ease: Easing) {
    for (name, slot) in slots(child) {
        let key = (path.clone(), name);
        match slot {
            Slot::Num(v) => {
                let target = Val::Num(*v);
                let cur = reconcile(st, key, target, None, dur, ease);
                if let Val::Num(x) = cur {
                    *v = round4(x);
                }
            }
            Slot::Color(s) => {
                let Some(target) = parse_hex(s.as_str()) else {
                    // A theme token or an unset color: nothing the
                    // kernel can interpolate, so it snaps and no
                    // track is kept for it.
                    st.tracks.remove(&(path.clone(), name));
                    continue;
                };
                let cur = reconcile(st, key, target, Some(s.clone()), dur, ease);
                if let Some(t) = st.tracks.get(&(path.clone(), name)) {
                    if t.done(st.now) {
                        if let Some(exact) = &t.to_str {
                            *s = exact.clone();
                            continue;
                        }
                    }
                }
                *s = format_hex(cur);
            }
        }
    }
}

/// Fold one property against its track and return the value to paint.
/// A first sighting records the target and does NOT animate: an
/// element's initial value is where it starts, not something to slide
/// into from zero.
fn reconcile(
    st: &mut AnimStore,
    key: (Path, &'static str),
    target: Val,
    to_str: Option<Str>,
    dur: f64,
    ease: Easing,
) -> Val {
    let now = st.now;
    match st.tracks.get(&key) {
        None => {
            st.tracks.insert(
                key,
                Track {
                    from: target,
                    to: target,
                    to_str,
                    start: now,
                    dur: 0.0,
                    ease,
                },
            );
            target
        }
        Some(t) if t.to == target => t.value(now),
        Some(t) => {
            let from = t.value(now);
            st.tracks.insert(
                key,
                Track {
                    from,
                    to: target,
                    to_str,
                    start: now,
                    dur,
                    ease,
                },
            );
            from
        }
    }
}

/// The one child list of a container, or `None` for a leaf. Lazy
/// ListView rows are deliberately absent: materializing them here
/// would build every row of a virtualized list on every frame, which
/// is the whole thing virtualization exists to avoid.
fn children_of(el: &mut Element) -> Option<&mut Vec<Element>> {
    match el {
        Element::Column { children, .. }
        | Element::Row { children, .. }
        | Element::Grid { children, .. }
        | Element::GridCell { children, .. }
        | Element::Anim { children, .. }
        | Element::Semantics { children, .. }
        | Element::Themed { children, .. }
        | Element::ListView { children, .. }
        | Element::ScrollView { children, .. }
        | Element::Modal { children, .. } => Some(children),
        Element::Stack(cs) | Element::HScrollView(cs) | Element::DataTable(cs) => Some(cs),
        _ => None,
    }
}

fn insert_at(el: &mut Element, parent: &[u32], index: usize, ghost: Element) {
    match parent.split_first() {
        None => {
            if let Some(cs) = children_of(el) {
                let at = index.min(cs.len());
                cs.insert(at, ghost);
            }
        }
        Some((head, rest)) => {
            if let Some(cs) = children_of(el) {
                if let Some(next) = cs.get_mut(*head as usize) {
                    insert_at(next, rest, index, ghost);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anim(dur: f64, exit: bool, child: Element) -> Element {
        Element::Anim {
            duration: dur,
            easing: Easing::Linear,
            enter: false,
            exit,
            opacity: 1.0,
            children: vec![child],
        }
    }

    fn column(children: Vec<Element>) -> Element {
        Element::Column {
            spacing: 0.0,
            padding: 0.0,
            background: Str::new(),
            grow: 0.0,
            border_radius: 0.0,
            border_width: 0.0,
            border_color: Str::new(),
            children,
        }
    }

    fn bar(v: f64) -> Element {
        Element::ProgressBar { value: v }
    }

    fn bar_value(el: &Element) -> f64 {
        match el {
            Element::ProgressBar { value } => *value,
            Element::Anim { children, .. } | Element::Column { children, .. } => {
                bar_value(&children[0])
            }
            _ => panic!("no ProgressBar here"),
        }
    }

    #[test]
    fn hex_round_trips_every_width() {
        assert_eq!(parse_hex("#f00"), Some(Val::Rgba(255.0, 0.0, 0.0, 255.0)));
        assert_eq!(parse_hex("#ff0000"), Some(Val::Rgba(255.0, 0.0, 0.0, 255.0)));
        assert_eq!(parse_hex("#ff000080").map(format_hex).unwrap().as_str(), "#ff000080");
        assert_eq!(format_hex(parse_hex("#3355ff").unwrap()).as_str(), "#3355ff");
        // Theme tokens and junk are not colors the kernel can blend.
        assert!(parse_hex("accent").is_none());
        assert!(parse_hex("#12345").is_none());
    }

    #[test]
    fn easing_curves_pin_their_midpoints() {
        assert_eq!(Easing::Linear.at(0.5), 0.5);
        assert_eq!(Easing::In.at(0.5), 0.25);
        assert_eq!(Easing::Out.at(0.5), 0.75);
        assert_eq!(Easing::InOut.at(0.5), 0.5);
        // Out of range clamps rather than overshooting.
        assert_eq!(Easing::Linear.at(-1.0), 0.0);
        assert_eq!(Easing::Linear.at(2.0), 1.0);
    }

    /// The first sighting of a value is where the element STARTS —
    /// only a later change tweens.
    #[test]
    fn first_value_lands_without_animating() {
        let mut w = World::default();
        let out = settle(&mut w, anim(100.0, false, bar(0.4)));
        assert_eq!(bar_value(&out), 0.4);
        assert!(!active(&w));
    }

    #[test]
    fn a_changed_value_sweeps_and_lands() {
        let mut w = World::default();
        settle(&mut w, anim(100.0, false, bar(0.0)));

        // The change starts at the old value, not the new one.
        let out = settle(&mut w, anim(100.0, false, bar(1.0)));
        assert_eq!(bar_value(&out), 0.0);
        assert!(active(&w));

        advance(&mut w, 40.0);
        let out = settle(&mut w, anim(100.0, false, bar(1.0)));
        assert_eq!(bar_value(&out), 0.4);

        advance(&mut w, 60.0);
        let out = settle(&mut w, anim(100.0, false, bar(1.0)));
        assert_eq!(bar_value(&out), 1.0);
        assert!(!active(&w), "a landed tween stops asking for frames");
    }

    /// Reduced motion is not a slower animation — it is none.
    #[test]
    fn reduced_motion_snaps() {
        let mut w = World::default();
        set_reduced(&mut w, true);
        settle(&mut w, anim(100.0, false, bar(0.0)));
        let out = settle(&mut w, anim(100.0, false, bar(1.0)));
        assert_eq!(bar_value(&out), 1.0);
        assert!(!active(&w));
    }

    /// The invariant animation exists to break: `build` stopped
    /// emitting the child, and it is still in the tree.
    #[test]
    fn an_exiting_child_is_retained_then_dropped() {
        let mut w = World::default();
        settle(&mut w, column(vec![anim(100.0, true, bar(0.5))]));

        let out = settle(&mut w, column(vec![]));
        let Element::Column { children, .. } = &out else {
            panic!("column")
        };
        assert_eq!(children.len(), 1, "the leaving child is retained");
        let Element::Anim { opacity, .. } = &children[0] else {
            panic!("the retained child keeps its wrapper")
        };
        assert_eq!(*opacity, 1.0, "the fade starts fully opaque");

        advance(&mut w, 50.0);
        let out = settle(&mut w, column(vec![]));
        let Element::Column { children, .. } = &out else {
            panic!("column")
        };
        let Element::Anim { opacity, .. } = &children[0] else {
            panic!("still retained")
        };
        assert_eq!(*opacity, 0.5);

        advance(&mut w, 60.0);
        let out = settle(&mut w, column(vec![]));
        let Element::Column { children, .. } = &out else {
            panic!("column")
        };
        assert!(children.is_empty(), "an expired exit stops painting");
        assert!(!active(&w));
    }

    /// A child that never asked to animate leaves the instant `build`
    /// stops emitting it — retention is opt-in, not a new default.
    #[test]
    fn a_plain_child_is_not_retained() {
        let mut w = World::default();
        settle(&mut w, column(vec![anim(100.0, false, bar(0.5))]));
        let out = settle(&mut w, column(vec![]));
        let Element::Column { children, .. } = &out else {
            panic!("column")
        };
        assert!(children.is_empty());
    }
}
