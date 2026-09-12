//! The headless `PIXIE_SCRIPT` harness, hoisted out of the generated
//! `main` template so every front end runs the SAME steps: the
//! compiled tier, the interpreted tier (build delegation makes the
//! tier gate meaningful), and embedders like pixie-py. The bodies are
//! transcribed verbatim from the template this replaced — dump bytes
//! are part of the gate's contract. `run` returns the final dump
//! instead of printing it, so callers keep stdout byte-identical by
//! printing the return value themselves.

use crate::{Component, Element, Handle, Runtime, Str, World, a11y, anim, build_prepared, theme};

/// Where a scripted run's transcript goes.
///
/// Stdout, unless `PIXIE_DUMP` names a file — and then the file, so
/// stdout belongs to the app and what it PRINTS is a channel of its
/// own. One writer for every front end: the generated `main`, the C
/// face, and an embedder that drives the harness itself. The first
/// write of a process truncates and the rest append, so a run that
/// dumps twice (the screen it started with, then the transcript)
/// leaves one file and not two.
pub fn emit(text: &str) {
    use std::io::Write;
    let Ok(path) = std::env::var("PIXIE_DUMP") else {
        println!("{text}");
        return;
    };
    static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let first = !STARTED.swap(true, std::sync::atomic::Ordering::Relaxed);
    let f = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(first)
        .append(!first)
        .open(&path);
    match f {
        Ok(mut f) => {
            let _ = writeln!(f, "{text}");
        }
        // A dump nobody can write is a broken run, not a quiet one.
        Err(e) => {
            eprintln!("pixie: cannot write the dump to {path}: {e}");
            std::process::exit(2);
        }
    }
}

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
/// completes should fail the run rather than hang it).
///
/// The bound is minutes, not seconds. It was five seconds while the
/// only tasks anyone wrote were a sleep and a file read; a task is
/// there for work that takes a while, and an app that spends a
/// minute in a model or a compiler is the case it was invented for.
/// What the bound is for is the task that will never finish at all.
///
/// This is also where a running task's reports are heard: the window
/// drains them on its pump, and a headless run has this loop instead.
pub fn settle<C: Component>(rt: &Runtime, view: Handle<C>, tree: &mut Element) {
    let mut spins = 0usize;
    while rt.has_tasks() {
        if crate::progress::any() {
            rt.with(|w: &mut World| crate::progress::drain(w));
        }
        rt.turn();
        flush(rt, view, tree);
        spins += 1;
        if spins > 300_000 {
            crate::script_refusal!("async tasks did not settle within ~5 minutes");
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

/// The step loop. Steps: `click[@n]:<label>` (a Button or a Link,
/// counted together in tree order; clicking a Link is accepted and
/// does nothing — opening a browser is not app state) ·
/// `input[@n]:<text>`
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
/// TabBar / Segmented / Table, counted together — picks the option with exactly
/// this text; a Table's options are its rows' first cells, so the
/// step picks a ROW and runs `onSelect` with its index) ·
/// `click:<column>` on a sorting Table (a header label matches like a
/// button and runs `onSort` with the column's index) ·
/// any of those verbs on a target inside a `Disabled` rider
/// (`disabled: true` on the element or on an ancestor) is ACCEPTED
/// AND DOES NOTHING — the target still counts toward `@n`, so the
/// numbering matches the window, but a person cannot press a
/// disabled control and neither can a script; the dump shows the
/// `Disabled[..]` wrapper, so the state itself is checked ·
/// `key:<chord>` (a keystroke: `key:cmd-s`, `key:escape` — it
/// delivers the chord AND taps the key, so an app reading the key
/// state sees a press and a release) ·
/// `keydown:<key>` / `keyup:<key>` (the two halves of that, for an app
/// that asks whether a key is held: `keydown:left` delivers the chord
/// too, and does not fail when nothing is bound to it) ·
/// `menu:<item>` (pick a menu item by name) ·
/// `file:<path>` (the answer the next file dialog gets) ·
/// `drop:<path>` (a file dragged onto the window) ·
/// `hover[@n]:<i>` (the pointer on the i-th point of the n-th chart —
/// BarChart and LineChart counted together — and `hover:` for the
/// pointer leaving; the dump then carries the chart's readout) ·
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
    let (parts, last) = run_parts(rt, view, tree, script);
    let mut log = String::new();
    for p in &parts {
        log.push_str(&p.text);
    }
    log.push_str(&last);
    log
}

/// One step of a script, and what it printed. A step that prints
/// nothing carries an empty text, so the order here is the script's
/// own and a caller can index it by the step it wrote.
pub struct StepOut {
    pub step: String,
    pub text: String,
}

/// The same run, with the transcript kept in pieces: every step
/// beside its own output, then the final dump. `run` is this joined,
/// so nothing a caller prints moves — what this adds is the ability
/// to ask WHICH step said a thing, which is what a test does and a
/// byte comparison does not need.
pub fn run_parts<C: Component>(
    rt: &Runtime,
    view: Handle<C>,
    tree: &mut Element,
    script: &str,
) -> (Vec<StepOut>, String) {
    let mut timed = false;
    let mut parts: Vec<StepOut> = Vec::new();
    for step in split_steps(script) {
        if step.is_empty() {
            continue;
        }
        let step = step.as_str();
        timed = false;
        let mut log = String::new();
        if let Some(name) = step.strip_prefix("theme:") {
            // §8.37: flipping the root palette is an ordinary
            // rebuild now, because the colors live in the tree.
            let light = match name {
                "light" => true,
                "dark" => false,
                _ => crate::script_refusal!("unknown theme `{name}`"),
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
                .unwrap_or_else(|_| crate::script_refusal!("bad advance step `{step}`"));
            rt.with(|w: &mut World| {
                anim::advance(w, ms);
                // The clock moved, so the timers that were due in that
                // span run — a script's `advance:` is how a headless
                // run says "a second passed".
                crate::timer::fire_due(w);
            });
            flush(rt, view, tree);
            settle(rt, view, tree);
            *tree = rt.with(|w| build_prepared(w, view));
            timed = true;
        } else if let Some(rest) = step.strip_prefix("click") {
            // Buttons and Links keep priority (counted together, one
            // shared index, by `find_button_nth`); when none carries
            // the label, a Checkbox/Switch answers (tree order among
            // toggles) and clicking it runs `onToggle` with the NEW
            // value. A Link's "click" is a no-op — opening a URL is
            // not app state — so it is accepted and changes nothing.
            // `@n` counts matches of the SAME label in tree order,
            // which is how a row of identical buttons is reached.
            let (n, label) = if let Some(r) = rest.strip_prefix('@') {
                let (a, b) = r
                    .split_once(':')
                    .unwrap_or_else(|| crate::script_refusal!("bad click step `{step}`"));
                let ix: usize = a
                    .parse()
                    .unwrap_or_else(|_| crate::script_refusal!("bad click index `{step}`"));
                (ix, b)
            } else if let Some(t) = rest.strip_prefix(':') {
                (0usize, t)
            } else {
                crate::script_refusal!("unknown script step `{step}`");
            };
            if let Some(f) = rt.with(|w| tree.find_button_nth(w, label, n)) {
                crate::contain("click handler", || rt.with(|w: &mut World| f(w)));
            } else {
                let (checked, on_toggle) = rt
                    .with(|w| tree.find_toggle_nth(w, label, n))
                    .unwrap_or_else(|| {
                        if n == 0 {
                            crate::script_refusal!("no button or toggle `{label}`")
                        } else {
                            crate::script_refusal!("no button or toggle `{label}` #{n}")
                        }
                    });
                let f = on_toggle
                    .unwrap_or_else(|| crate::script_refusal!("toggle `{label}` has no onToggle"));
                crate::contain("click handler", || {
                    rt.with(|w: &mut World| f(w, !checked))
                });
            }
        } else if let Some(rest) = step.strip_prefix("input") {
            let (n, text) = if let Some(r) = rest.strip_prefix('@') {
                let (a, b) = r
                    .split_once(':')
                    .unwrap_or_else(|| crate::script_refusal!("bad input step `{step}`"));
                let ix: usize = a
                    .parse()
                    .unwrap_or_else(|_| crate::script_refusal!("bad input index `{step}`"));
                (ix, b)
            } else if let Some(t) = rest.strip_prefix(':') {
                (0usize, t)
            } else {
                crate::script_refusal!("unknown script step `{step}`");
            };
            let target = rt
                .with(|w| tree.find_input(w, n))
                .unwrap_or_else(|| crate::script_refusal!("no input field #{n}"));
            match target {
                crate::InputTarget::Text { on_change, .. } => {
                    let f = on_change
                        .unwrap_or_else(|| crate::script_refusal!("TextField #{n} has no onTextChanged"));
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
                                .unwrap_or_else(|| crate::script_refusal!("NumberField #{n} has no onChange"));
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
                                .unwrap_or_else(|| crate::script_refusal!("IntField #{n} has no onChange"));
                            crate::contain("input handler", || {
                                rt.with(|w: &mut World| f(w, v))
                            });
                        }
                    }
                }
            }
        } else if let Some(key) = step.strip_prefix("keydown:") {
            // A key held down, which a window says two things about:
            // the chord reaches whatever declared it, and the key is
            // now DOWN until a `keyup:` says otherwise. Unlike `key:`
            // it does not fail when nothing is bound — an app that
            // only reads the key state binds nothing and still
            // presses keys.
            crate::contain("key handler", || {
                rt.with(|w: &mut World| {
                    crate::keys::press(key, false);
                    crate::keys::fire(w, key);
                })
            });
        } else if let Some(key) = step.strip_prefix("keyup:") {
            crate::keys::release(key);
        } else if let Some(chord) = step.strip_prefix("key:") {
            // A keystroke, spelled the way the platform spells it
            // (`cmd-s`, `shift-tab`); `+` reads the same. Nothing
            // bound to it is a script's typo, so it says so — the
            // rule `click` follows for a label no button carries.
            //
            // It is also a press and a release, because that is what
            // a finger does: an app reading the key state sees the tap
            // in the next tick, and no script step means less than the
            // hardware does.
            let fired = crate::contain("key handler", || {
                rt.with(|w: &mut World| crate::keys::fire(w, chord))
            });
            if !matches!(fired, Some(true)) {
                crate::script_refusal!("no shortcut or key handler for `{chord}`");
            }
            let tap = crate::keys::normalize(chord);
            let key = tap.rsplit('-').next().unwrap_or(&tap).to_string();
            crate::keys::press(&key, false);
            crate::keys::release(&key);
        } else if let Some(path) = step.strip_prefix("drop:") {
            // A file dragged onto the window. The drag is the
            // platform's; what the app does with the path is the
            // app's, and that is the part a script checks.
            let took = crate::contain("drop handler", || {
                rt.with(|w: &mut World| crate::drop::fire(w, path))
            });
            if !matches!(took, Some(true)) {
                crate::script_refusal!("nothing takes a dropped file (`on_file_drop`)");
            }
        } else if let Some(path) = step.strip_prefix("file:") {
            // The answer the next file dialog gets. A headless run has
            // no person to pick a file, so the script is the person.
            crate::dialog::push_answer(path);
        } else if let Some(name) = step.strip_prefix("menu:") {
            // Pick a menu item by the name it shows. Nothing under
            // that name is a script's typo, the way a missing button
            // label is.
            let picked = crate::contain("menu handler", || {
                rt.with(|w: &mut World| crate::menu::pick(w, name))
            });
            if !matches!(picked, Some(true)) {
                crate::script_refusal!("no menu item `{name}`");
            }
        } else if let Some(rest) = step.strip_prefix("hover") {
            // The pointer over a chart. `hover[@n]:<i>` puts it on the
            // i-th point of the n-th chart (BarChart and LineChart
            // counted together in tree order, default 0), and `hover:`
            // takes it away. A window reads the mouse; a script says
            // where the pointer is, and the dump carries the readout
            // the chart shows for that point — so what a person sees
            // on hover is a checked output, not a courtesy of the
            // window. Nothing rebuilds: the pointer is not app state.
            let (n, target) = if let Some(r) = rest.strip_prefix('@') {
                let (a, b) = r
                    .split_once(':')
                    .unwrap_or_else(|| crate::script_refusal!("bad hover step `{step}`"));
                let ix: usize = a
                    .parse()
                    .unwrap_or_else(|_| crate::script_refusal!("bad hover index `{step}`"));
                (ix, b)
            } else if let Some(t) = rest.strip_prefix(':') {
                (0usize, t)
            } else {
                crate::script_refusal!("unknown script step `{step}`");
            };
            if target.is_empty() {
                rt.with(|w: &mut World| crate::hover::clear(w));
            } else {
                let i: usize = target.parse().unwrap_or_else(|_| {
                    crate::script_refusal!(
                        "bad hover point `{step}` — a point's index, or nothing for the pointer leaving"
                    )
                });
                let points = rt
                    .with(|w| tree.find_chart_nth(w, n))
                    .unwrap_or_else(|| crate::script_refusal!("no chart #{n} (BarChart / LineChart)"));
                if i >= points {
                    crate::script_refusal!("chart #{n} has {points} point(s), so there is no #{i}");
                }
                rt.with(|w: &mut World| crate::hover::set(w, n, i));
            }
        } else if let Some(rest) = step.strip_prefix("submit") {
            let n: usize = if let Some(r) = rest.strip_prefix('@') {
                r.parse()
                    .unwrap_or_else(|_| crate::script_refusal!("bad submit index `{step}`"))
            } else if rest.is_empty() {
                0
            } else {
                crate::script_refusal!("unknown script step `{step}`");
            };
            let target = rt
                .with(|w| tree.find_input(w, n))
                .unwrap_or_else(|| crate::script_refusal!("no input field #{n}"));
            match target {
                crate::InputTarget::Text { value, on_submit, .. } => {
                    let f =
                        on_submit.unwrap_or_else(|| crate::script_refusal!("TextField #{n} has no onSubmitted"));
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
                    .unwrap_or_else(|| crate::script_refusal!("bad slide step `{step}`"));
                let ix: usize = a
                    .parse()
                    .unwrap_or_else(|_| crate::script_refusal!("bad slide index `{step}`"));
                (ix, b)
            } else if let Some(v) = rest.strip_prefix(':') {
                (0usize, v)
            } else {
                crate::script_refusal!("unknown script step `{step}`");
            };
            let val: f64 = raw
                .parse()
                .unwrap_or_else(|_| crate::script_refusal!("bad slide value `{step}`"));
            let (min, max, snap_step, change) = rt
                .with(|w| tree.find_slider(w, n))
                .unwrap_or_else(|| crate::script_refusal!("no Slider #{n}"));
            let f = change.unwrap_or_else(|| crate::script_refusal!("Slider #{n} has no onChange"));
            // The same clamp-and-snap the engine's pointer math runs,
            // so a scripted slide and a real drag land on identical
            // values.
            let v = crate::slider_snap(min, max, snap_step, val);
            crate::contain("slide handler", || rt.with(|w: &mut World| f(w, v)));
        } else if let Some(rest) = step.strip_prefix("select") {
            // `select:<label>` / `select@n:<label>` — the nth CHOOSER
            // (Select, RadioGroup, TabBar, Segmented or Table, counted together
            // in tree order; default 0) picks the option/label with
            // exactly this text, running `onSelect` with its 0-based
            // index. A Table's options are its rows' first cells, so
            // the index is the row's.
            let (n, label) = if let Some(r) = rest.strip_prefix('@') {
                let (a, b) = r
                    .split_once(':')
                    .unwrap_or_else(|| crate::script_refusal!("bad select step `{step}`"));
                let ix: usize = a
                    .parse()
                    .unwrap_or_else(|_| crate::script_refusal!("bad select index `{step}`"));
                (ix, b)
            } else if let Some(t) = rest.strip_prefix(':') {
                (0usize, t)
            } else {
                crate::script_refusal!("unknown script step `{step}`");
            };
            let (options, on_select) = rt
                .with(|w| tree.find_chooser(w, n))
                .unwrap_or_else(|| crate::script_refusal!("no chooser #{n} (Select / RadioGroup / TabBar / Segmented / Table / a ListView with onSelect)"));
            let ix = options
                .iter()
                .position(|o| o.as_str() == label)
                .unwrap_or_else(|| crate::script_refusal!("chooser #{n} has no option `{label}`"));
            let f = on_select.unwrap_or_else(|| crate::script_refusal!("chooser #{n} has no onSelect"));
            crate::contain("select handler", || {
                rt.with(|w: &mut World| f(w, ix as i64))
            });
        } else {
            crate::script_refusal!("unknown script step `{step}`");
        }
        flush(rt, view, tree);
        settle(rt, view, tree);
        // Whoever can draw gets this step's screen (`frames`); with
        // nobody listening this is one `is_none`.
        crate::frames::emit(tree);
        parts.push(StepOut { step: step.to_string(), text: log });
    }
    if !timed {
        anim_settle(rt, view, tree);
    }
    (parts, rt.with(|w| tree.dump(w)))
}
