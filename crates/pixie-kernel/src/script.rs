//! The headless `PIXIE_SCRIPT` harness, hoisted out of the generated
//! `main` template so every front end runs the SAME steps: the
//! compiled tier, the interpreted tier (build delegation makes the
//! tier gate meaningful), and embedders like pixie-py. The bodies are
//! transcribed verbatim from the template this replaced — dump bytes
//! are part of the gate's contract. `run` returns the final dump
//! instead of printing it, so callers keep stdout byte-identical by
//! printing the return value themselves.

use crate::{Component, Element, Handle, Runtime, Str, World, a11y, anim, build_prepared, theme};

/// Flush queued signals; rebuild if any view dirtied.
pub fn flush<C: Component>(rt: &Runtime, view: Handle<C>, tree: &mut Element) {
    let next = rt.with(|w| {
        w.flush();
        if w.take_dirty_views().is_empty() {
            None
        } else {
            Some(build_prepared(w, view))
        }
    });
    if let Some(t) = next {
        *tree = t;
    }
}

/// Run every live tween to its end (§8.35). Animation must not
/// change what a script MEANS: a step that does not ask to stand
/// at a particular instant ends with time run forward, so a demo
/// that never mentions time dumps exactly as it did before
/// animation existed. `advance:<ms>` is the opt-in that leaves
/// the tree mid-flight.
pub fn anim_settle<C: Component>(rt: &Runtime, view: Handle<C>, tree: &mut Element) {
    for _ in 0..64 {
        let done = rt.with(|w| match anim::last_end(w) {
            None => true,
            Some(end) => {
                if end > anim::now(w) {
                    anim::set_now(w, end);
                }
                false
            }
        });
        if done {
            return;
        }
        *tree = rt.with(|w| build_prepared(w, view));
    }
}

/// Spin the async tier to completion (bounded — a task that never
/// completes is a harness misuse, not a hang).
pub fn settle<C: Component>(rt: &Runtime, view: Handle<C>, tree: &mut Element) {
    let mut spins = 0usize;
    while rt.has_tasks() {
        rt.turn();
        flush(rt, view, tree);
        spins += 1;
        if spins > 5000 {
            panic!("async tasks did not settle within ~5s");
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

/// Split a script into steps on commas, honouring `\\,` for a comma
/// that belongs to the step's text (`input:hello\\, world`) and
/// `\\\\` for a literal backslash. Without this a script could not
/// carry prose at all: the separator would eat it and the tail would
/// fail as an unknown step.
fn split_steps(script: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut it = script.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\\' => match it.peek() {
                Some(',') => {
                    cur.push(',');
                    it.next();
                }
                Some('\\') => {
                    cur.push('\\');
                    it.next();
                }
                _ => cur.push('\\'),
            },
            ',' => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// The step loop. Steps: `click[@n]:<label>` · `input[@n]:<text>`
/// (the n-th field a person types into — TextField, NumberField and
/// IntField counted TOGETHER in tree order, default 0, because they
/// share this verb. On a TextField the text runs `onTextChanged`; on
/// a numeric field the step COMMITS it, which is what `enter` or
/// leaving the field does in a window: parse with Python's `float()`
/// / `int()` rules, clamp, snap, and run `onChange` only when the
/// result differs from the bound value — text that is not a number
/// commits nothing) · `submit[@n]` (the same numbering; `onSubmitted`
/// on a TextField, accepted and inert on a numeric field, so
/// `input:3,submit` reads naturally) · `slide[@n]:<value>` (the n-th
/// Slider in tree order, default 0: clamp the value to `[min, max]`,
/// snap it to the nearest step multiple counted from min, run
/// `onChange`) ·
/// `select[@n]:<label>` (the n-th chooser — Select / RadioGroup /
/// TabBar — picks the option with exactly this text) ·
/// `advance:<ms>` · `theme:<light|dark>` · `a11y` ·
/// `mem` · `dump` (the element tree HERE — the run's own start and
/// end are printed by the caller, so a script that only drives is
/// checked at its endpoints; a `dump` between steps makes an
/// intermediate state a checked output too).
///
/// Steps that produce output (`a11y`, `mem`, `dump`) do not print:
/// they are collected into the returned transcript, ahead of the
/// final dump and in the order they ran. The bytes a caller prints
/// are unchanged from when those steps printed themselves — and an
/// embedder that CAPTURES the return value (yokan's CPython tier)
/// now sees them too, which is what makes such a step comparable
/// across tiers instead of silently one-sided. Every step settles the async tier before the next one
/// runs, so scripted runs stay deterministic. Time does not move
/// between steps: the clock only jumps at an explicit `advance:` or
/// once at the end, so a script that never mentions time dumps
/// exactly as it did before animation existed, and one that ENDS at
/// an `advance:` dumps the frame that instant would have painted.
pub fn run<C: Component>(
    rt: &Runtime,
    view: Handle<C>,
    tree: &mut Element,
    script: &str,
) -> String {
    let mut timed = false;
    let mut log = String::new();
    for step in split_steps(script) {
        if step.is_empty() {
            continue;
        }
        let step = step.as_str();
        timed = false;
        if let Some(name) = step.strip_prefix("theme:") {
            // §8.37: flipping the root palette is an ordinary
            // rebuild now, because the colors live in the tree.
            let light = match name {
                "light" => true,
                "dark" => false,
                _ => panic!("unknown theme `{name}`"),
            };
            rt.with(|w: &mut World| theme::set_light(w, light));
            *tree = rt.with(|w| build_prepared(w, view));
        } else if step == "mem" {
            // §8.44: how many objects the World is holding. A
            // checked output, the way the accessibility tree is.
            let n = rt.with(|w: &mut World| w.live_objects());
            log.push_str(&format!("live: {n}\n"));
        } else if step == "dump" {
            // The element tree at THIS point in the script. Same
            // bytes the run prints at its start and end, so a gate
            // comparing stdout compares the middle of a run too.
            log.push_str(&rt.with(|w| tree.dump(w)));
            log.push('\n');
        } else if step == "a11y" {
            // §8.36: the accessibility tree is a KERNEL output, so
            // a script can print exactly what a platform adapter
            // would be handed.
            let t = a11y::tree(tree);
            log.push_str(&t.dump());
            log.push('\n');
        } else if let Some(ms) = step.strip_prefix("advance:") {
            let ms: f64 = ms
                .parse()
                .unwrap_or_else(|_| panic!("bad advance step `{step}`"));
            rt.with(|w: &mut World| anim::advance(w, ms));
            *tree = rt.with(|w| build_prepared(w, view));
            timed = true;
        } else if let Some(rest) = step.strip_prefix("click") {
            // Buttons keep priority; when none carries the label, a
            // Checkbox/Switch answers (tree order among toggles) and
            // clicking it runs `onToggle` with the NEW value. `@n`
            // counts matches of the SAME label in tree order, which
            // is how a row of identical buttons is reached.
            let (n, label) = if let Some(r) = rest.strip_prefix('@') {
                let (a, b) = r
                    .split_once(':')
                    .unwrap_or_else(|| panic!("bad click step `{step}`"));
                let ix: usize = a
                    .parse()
                    .unwrap_or_else(|_| panic!("bad click index `{step}`"));
                (ix, b)
            } else if let Some(t) = rest.strip_prefix(':') {
                (0usize, t)
            } else {
                panic!("unknown script step `{step}`");
            };
            if let Some(f) = rt.with(|w| tree.find_button_nth(w, label, n)) {
                crate::contain("click handler", || rt.with(|w: &mut World| f(w)));
            } else {
                let (checked, on_toggle) = rt
                    .with(|w| tree.find_toggle_nth(w, label, n))
                    .unwrap_or_else(|| {
                        if n == 0 {
                            panic!("no button or toggle `{label}`")
                        } else {
                            panic!("no button or toggle `{label}` #{n}")
                        }
                    });
                let f = on_toggle
                    .unwrap_or_else(|| panic!("toggle `{label}` has no onToggle"));
                crate::contain("click handler", || {
                    rt.with(|w: &mut World| f(w, !checked))
                });
            }
        } else if let Some(rest) = step.strip_prefix("input") {
            let (n, text) = if let Some(r) = rest.strip_prefix('@') {
                let (a, b) = r
                    .split_once(':')
                    .unwrap_or_else(|| panic!("bad input step `{step}`"));
                let ix: usize = a
                    .parse()
                    .unwrap_or_else(|_| panic!("bad input index `{step}`"));
                (ix, b)
            } else if let Some(t) = rest.strip_prefix(':') {
                (0usize, t)
            } else {
                panic!("unknown script step `{step}`");
            };
            let target = rt
                .with(|w| tree.find_input(w, n))
                .unwrap_or_else(|| panic!("no input field #{n}"));
            match target {
                crate::InputTarget::Text { on_change, .. } => {
                    let f = on_change
                        .unwrap_or_else(|| panic!("TextField #{n} has no onTextChanged"));
                    crate::contain("input handler", || {
                        rt.with(|w: &mut World| f(w, Str::from(text)))
                    });
                }
                // A number field COMMITS what was typed, in one step:
                // a person presses `enter` or leaves the field, and
                // that runs exactly this — parse, clamp, snap, and
                // fire only on a real change. Text that is not a
                // number commits nothing (the field would put the
                // bound value back on screen, which no dump can see).
                crate::InputTarget::Number {
                    value,
                    min,
                    max,
                    step: snap_step,
                    on_change,
                } => {
                    if let Some(v) = crate::parse_float_text(text) {
                        let v = crate::number_snap(min, max, snap_step, v);
                        if v != value {
                            let f = on_change
                                .unwrap_or_else(|| panic!("NumberField #{n} has no onChange"));
                            crate::contain("input handler", || {
                                rt.with(|w: &mut World| f(w, v))
                            });
                        }
                    }
                }
                crate::InputTarget::Int {
                    value,
                    min,
                    max,
                    step: snap_step,
                    on_change,
                } => {
                    if let Some(v) = crate::parse_int_text(text) {
                        let v = crate::int_snap(min, max, snap_step, v);
                        if v != value {
                            let f = on_change
                                .unwrap_or_else(|| panic!("IntField #{n} has no onChange"));
                            crate::contain("input handler", || {
                                rt.with(|w: &mut World| f(w, v))
                            });
                        }
                    }
                }
            }
        } else if let Some(rest) = step.strip_prefix("submit") {
            let n: usize = if let Some(r) = rest.strip_prefix('@') {
                r.parse()
                    .unwrap_or_else(|_| panic!("bad submit index `{step}`"))
            } else if rest.is_empty() {
                0
            } else {
                panic!("unknown script step `{step}`");
            };
            let target = rt
                .with(|w| tree.find_input(w, n))
                .unwrap_or_else(|| panic!("no input field #{n}"));
            match target {
                crate::InputTarget::Text { value, on_submit, .. } => {
                    let f =
                        on_submit.unwrap_or_else(|| panic!("TextField #{n} has no onSubmitted"));
                    crate::contain("submit handler", || {
                        rt.with(|w: &mut World| f(w, value))
                    });
                }
                // `enter` on a number field commits, and `input:`
                // already did — so `input:3,submit` reads the way a
                // person works and means the same thing. Accepted,
                // does nothing, rather than a step that fails on the
                // wrong kind of field.
                crate::InputTarget::Number { .. } | crate::InputTarget::Int { .. } => {}
            }
        } else if let Some(rest) = step.strip_prefix("slide") {
            let (n, raw) = if let Some(r) = rest.strip_prefix('@') {
                let (a, b) = r
                    .split_once(':')
                    .unwrap_or_else(|| panic!("bad slide step `{step}`"));
                let ix: usize = a
                    .parse()
                    .unwrap_or_else(|_| panic!("bad slide index `{step}`"));
                (ix, b)
            } else if let Some(v) = rest.strip_prefix(':') {
                (0usize, v)
            } else {
                panic!("unknown script step `{step}`");
            };
            let val: f64 = raw
                .parse()
                .unwrap_or_else(|_| panic!("bad slide value `{step}`"));
            let (min, max, snap_step, change) = rt
                .with(|w| tree.find_slider(w, n))
                .unwrap_or_else(|| panic!("no Slider #{n}"));
            let f = change.unwrap_or_else(|| panic!("Slider #{n} has no onChange"));
            // The same clamp-and-snap the engine's pointer math runs,
            // so a scripted slide and a real drag land on identical
            // values.
            let v = crate::slider_snap(min, max, snap_step, val);
            crate::contain("slide handler", || rt.with(|w: &mut World| f(w, v)));
        } else if let Some(rest) = step.strip_prefix("select") {
            // `select:<label>` / `select@n:<label>` — the nth CHOOSER
            // (Select, RadioGroup or TabBar, counted together in tree
            // order; default 0) picks the option/label with exactly
            // this text, running `onSelect` with its 0-based index.
            let (n, label) = if let Some(r) = rest.strip_prefix('@') {
                let (a, b) = r
                    .split_once(':')
                    .unwrap_or_else(|| panic!("bad select step `{step}`"));
                let ix: usize = a
                    .parse()
                    .unwrap_or_else(|_| panic!("bad select index `{step}`"));
                (ix, b)
            } else if let Some(t) = rest.strip_prefix(':') {
                (0usize, t)
            } else {
                panic!("unknown script step `{step}`");
            };
            let (options, on_select) = rt
                .with(|w| tree.find_chooser(w, n))
                .unwrap_or_else(|| panic!("no chooser #{n} (Select / RadioGroup / TabBar)"));
            let ix = options
                .iter()
                .position(|o| o.as_str() == label)
                .unwrap_or_else(|| panic!("chooser #{n} has no option `{label}`"));
            let f = on_select.unwrap_or_else(|| panic!("chooser #{n} has no onSelect"));
            crate::contain("select handler", || {
                rt.with(|w: &mut World| f(w, ix as i64))
            });
        } else {
            panic!("unknown script step `{step}`");
        }
        flush(rt, view, tree);
        settle(rt, view, tree);
    }
    if !timed {
        anim_settle(rt, view, tree);
    }
    log.push_str(&rt.with(|w| tree.dump(w)));
    log
}
