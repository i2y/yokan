//! The `Disabled` and `Sized` riders at the kernel: what they dump,
//! and what a headless script gets for a control inside a `Disabled`
//! — found and counted like any other, so `@n` matches the window,
//! with listeners that do nothing, because a person cannot press a
//! disabled control either.
use std::cell::Cell;
use std::rc::Rc;

use pixie_kernel::{Element, InputTarget, List, Listener, Str, World};

fn counted_button(label: &str, hits: &Rc<Cell<u32>>) -> Element {
    let hits = hits.clone();
    let f: Listener = Rc::new(move |_: &mut World| hits.set(hits.get() + 1));
    Element::button(label, f)
}

#[test]
fn a_disabled_button_is_counted_and_its_click_is_inert() {
    let mut w = World::new();
    let hits = [
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
    ];
    let tree = Element::column(vec![
        counted_button("save", &hits[0]),
        Element::Disabled {
            children: vec![counted_button("save", &hits[1])],
        },
        counted_button("save", &hits[2]),
    ]);
    // Three matches in tree order — the disabled one in the middle
    // is #1, exactly where a person would see it — and running each
    // reaches the two live listeners around it and nothing in it.
    for n in 0..3 {
        let f = tree
            .find_button_nth(&w, "save", n)
            .expect("every match counts");
        f(&mut w);
    }
    assert!(tree.find_button_nth(&w, "save", 3).is_none());
    assert_eq!(
        [hits[0].get(), hits[1].get(), hits[2].get()],
        [1, 0, 1],
        "the disabled button's click must be accepted and do nothing"
    );
}

#[test]
fn a_disabled_field_and_chooser_answer_with_inert_listeners() {
    let mut w = World::new();
    let typed = Rc::new(Cell::new(false));
    let on_change = {
        let t = typed.clone();
        Rc::new(move |_: &mut World, _: Str| t.set(true))
    };
    let field = Element::TextField {
        value: Str::from("v"),
        placeholder: Str::new(),
        on_change: Some(on_change),
        on_submit: None,
        multiline: false,
        rows: 0.0,
    };
    let picked = Rc::new(Cell::new(-1i64));
    let on_select = {
        let p = picked.clone();
        Rc::new(move |_: &mut World, i: i64| p.set(i))
    };
    let select = Element::Select {
        options: ["a", "b"].into_iter().map(Str::from).collect::<List<Str>>(),
        selected: 0,
        on_select: Some(on_select),
    };
    let tree = Element::column(vec![Element::Disabled {
        children: vec![Element::row(vec![field, select])],
    }]);

    // Found, with every listener present — `input:` and `submit` on
    // it are accepted (a field without `onSubmitted` would otherwise
    // fail the step) — and calling them changes nothing.
    match tree.find_input(&w, 0).expect("the field counts") {
        InputTarget::Text {
            value,
            on_change,
            on_submit,
        } => {
            assert_eq!(value.as_str(), "v");
            (on_change.expect("accepted"))(&mut w, Str::from("typed"));
            (on_submit.expect("accepted"))(&mut w, Str::from("typed"));
        }
        _ => panic!("a TextField"),
    }
    let (options, on_select) = tree.find_chooser(&w, 0).expect("the chooser counts");
    assert_eq!(options.iter().count(), 2);
    (on_select.expect("accepted"))(&mut w, 1);
    assert!(!typed.get(), "a disabled field's `onTextChanged` must not run");
    assert_eq!(picked.get(), -1, "a disabled chooser's `onSelect` must not run");
}

#[test]
fn the_riders_dump_their_props_only_when_set() {
    let w = World::new();
    let boxed = |width, height, min_width, max_width| Element::Sized {
        width,
        height,
        min_width,
        max_width,
        children: vec![Element::text("a")],
    };
    assert_eq!(
        boxed(200.0, 0.0, 120.0, 0.0).dump(&w),
        "Sized(width=200, minWidth=120)[Text(a)]"
    );
    assert_eq!(
        boxed(0.0, 40.0, 0.0, 400.0).dump(&w),
        "Sized(height=40, maxWidth=400)[Text(a)]"
    );
    // A bound size that reads zero this frame is a bare box, not a
    // `width=0` — the GridCell rule.
    assert_eq!(boxed(0.0, 0.0, 0.0, 0.0).dump(&w), "Sized[Text(a)]");
    let dimmed = Element::Disabled {
        children: vec![Element::text("a")],
    };
    assert_eq!(dimmed.dump(&w), "Disabled[Text(a)]");
    // `inner()` looks through both, so "what IS this" keeps working
    // under them.
    assert!(matches!(dimmed.inner(), Element::Text { .. }));
    assert!(matches!(boxed(1.0, 1.0, 1.0, 1.0).inner(), Element::Text { .. }));
}
