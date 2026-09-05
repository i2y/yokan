//! What the C face promises, checked from Rust: one whole round of the
//! protocol, and the table against the arms.

use super::*;
use std::sync::atomic::{AtomicI64, Ordering};

/// The app's state, on the app's side of the ABI — where a door keeps
/// it too.
static COUNT: AtomicI64 = AtomicI64::new(0);

fn put_str(el: i64, key: i32, v: &str) {
    let s = CString::new(v).unwrap();
    // SAFETY: NUL-terminated, and it outlives the call.
    unsafe { pixie_str(el, key, s.as_ptr()) };
}

/// A door's `view`, written in Rust: open, write, close, and hand the
/// root's handle back.
extern "C" fn build() -> i64 {
    let t = pixie_el(KIND_TEXT);
    put_str(t, K_TEXT, &format!("count: {}", COUNT.load(Ordering::Relaxed)));
    pixie_num(t, K_SIZE, 34.0);
    pixie_end(t);

    let b = pixie_el(KIND_BUTTON);
    put_str(b, K_LABEL, "+1");
    pixie_on(b, K_ON_CLICK, 7);
    put_str(b, K_TOOLTIP, "one more");
    pixie_end(b);

    let col = pixie_el(KIND_COLUMN);
    pixie_num(col, K_SPACING, 12.0);
    pixie_num(col, K_PADDING, 16.0);
    let kids = [t, b];
    // SAFETY: two readable handles, and the column consumes them.
    unsafe { pixie_children(col, kids.as_ptr(), kids.len()) };
    pixie_end(col)
}

extern "C" fn on_event(handler: i64, kind: i64) {
    assert_eq!(kind, PAY_NONE, "a button carries nothing");
    assert_eq!(handler, 7, "the button carries the id the door registered");
    COUNT.fetch_add(1, Ordering::Relaxed);
}

/// The whole loop through the C face: build a tree, click the button
/// the engine found by its label, watch the event reach the app's state,
/// and see the rebuild carry the new value. The tooltip on the button
/// is there to prove the riders are applied on this path too.
#[test]
fn a_click_reaches_the_app_and_the_rebuild_shows_it() {
    pixie_set_event_handler(on_event);
    assert_eq!(
        headless(build, "click:+1,dump", false),
        // The dump at the start, the `dump` step, and the one every
        // scripted run ends with.
        "Column(spacing=12, padding=16)[Text(count: 0, fontSize=34), Tooltip(one more)[Button(+1)]]\n\
         Column(spacing=12, padding=16)[Text(count: 1, fontSize=34), Tooltip(one more)[Button(+1)]]\n\
         Column(spacing=12, padding=16)[Text(count: 1, fontSize=34), Tooltip(one more)[Button(+1)]]\n"
    );
}

fn id_of(table: &[(&str, i32)], name: &str) -> i32 {
    table
        .iter()
        .find(|(n, _)| *n == name)
        .unwrap_or_else(|| panic!("`{name}` is in elements.toml but not in gen.rs"))
        .1
}

/// Every keyword the table declares has to reach the arm that builds
/// its element. A property nobody reads is a property an app can write
/// and watch vanish, which is exactly the kind of difference the gate
/// would then have to catch instead.
#[test]
fn every_keyword_in_the_table_reaches_an_arm() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../wakakusa/elements.toml");
    let text = std::fs::read_to_string(path).expect("wakakusa/elements.toml");
    let doc: toml::Value = toml::from_str(&text).expect("elements.toml parses");

    for el in doc["element"].as_array().expect("[[element]]") {
        let name = el["name"].as_str().unwrap();
        let kind = id_of(KINDS, name);
        let mut bag = Bag {
            kind,
            ..Default::default()
        };
        let mut want: Vec<(&str, i32)> = Vec::new();
        for p in el["props"].as_array().expect("props") {
            let pname = p["name"].as_str().unwrap();
            let key = id_of(KEYS, pname);
            want.push((pname, key));
            if p.get("handler").is_some() {
                bag.handlers.push((key, 1));
                continue;
            }
            match p["type"].as_str().unwrap() {
                "str" => bag.strs.push((key, String::from("x"))),
                "num" => bag.nums.push((key, 1.0)),
                "int" => bag.ints.push((key, 1)),
                "bool" => bag.bools.push((key, true)),
                "strs" => bag.str_lists.push((key, vec![vec![String::from("x")]])),
                "nums" | "nums2" => bag.num_lists.push((key, vec![vec![1.0]])),
                "rows" => bag.rows.push((key, 1)),
                other => panic!("{name}: no such type `{other}`"),
            }
        }
        let built = materialize(&bag, Vec::new());
        let _ = apply_riders(kind, built, &bag);
        let read = bag.read.borrow();
        for (pname, key) in want {
            assert!(
                read.contains(&key),
                "{name}: `{pname}` is in the table and never reached the arm"
            );
        }
    }

    // And every keyword that rides on every element.
    let bag = Bag {
        kind: KIND_COLUMN,
        ..Default::default()
    };
    let built = materialize(&bag, Vec::new());
    let _ = apply_riders(KIND_COLUMN, built, &bag);
    let read = bag.read.borrow().clone();
    bag.read.borrow_mut().clear();
    for r in doc["rider"].as_array().expect("[[rider]]") {
        let rname = r["name"].as_str().unwrap();
        let key = id_of(KEYS, rname);
        assert!(read.contains(&key), "the rider `{rname}` is never read");
    }
}
