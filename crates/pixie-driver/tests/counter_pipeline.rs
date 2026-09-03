//! Acceptance: the counter demo checks clean through the forked front end
//! and emits the expected Rust stereotypes. The full compile-and-run leg
//! (cargo on the generated crate + PIXIE_SCRIPT interaction) lives in the
//! `just accept` recipe — it needs a target dir and ~seconds, not unit
//! test time.

use std::path::Path;

#[test]
fn counter_demo_checks_and_emits() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/counter/counter.pix"
    );
    let outcome = pixie_driver::check_file(Path::new(path)).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().expect("module"), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        "pub struct Counter",
        "pub trait CounterRef: Copy",
        "pub const COUNTER_COUNT_CHANGED: SignalId =",
        "impl TodoRef for Handle<Todo>",
        // A NON-virtualized single-repeater list stays EAGER (§8.24):
        // rows are real children, so the window's clipped-viewport
        // path renders them. (Virtualized lists still go lazy —
        // biglist through the gate, LazyRows through the interp
        // tests.)
        "lazy: None",
        "for (__row_idx0, it) in __xs",
        "fn main() {",
        "PIXIE_SCRIPT",
        // The .rpi adapter path: bound Rust called inline, errors mapped.
        "std::fs::read_to_string",
        ".map_err(|e| Str::from(e.to_string()))",
        "Err(e) => {",
        "let __rt = Runtime::new(w);",
        "pixie_engine_gpui::run_app(__rt, __view, &__title, None, None);",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`");
    }
}

#[test]
fn values_demo_checks_and_emits() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/values/values.pix"
    );
    let outcome = pixie_driver::check_file(std::path::Path::new(path)).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code = pixie_codegen::emit_test_program(
        outcome.module.as_ref().expect("module"),
        outcome.binding_items,
    )
    .expect("emit succeeds");
    for needle in [
        "pub enum Color",
        "pub enum MathError",
        "impl std::fmt::Display for MathError",
        "pub struct Point",
        // Traits are REAL Rust traits now (§8.20): declaration,
        // handle impl, and a generic fn monomorphized by rustc.
        // `Clone`, not `Copy`: a handle is Copy and a value is not, so a
        // Copy supertrait would silently make traits object-only (§8.49).
        "pub trait Labeled: Clone {",
        "impl Labeled for Handle<Palette> {",
        "impl Labeled for Handle<Badge> {",
        "fn tag(self, w: &mut World)",
        "fn describe_tag<T: Labeled + Clone>(w: &mut World, x: T) -> Str {",
        // Trait-bound dispatch threads w through the real trait.
        "x.tag(w)",
        "return Err(MathError::divByZero);",
        "Color::Red => {",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`");
    }
}

#[test]
fn greeter_demo_checks_and_emits() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/greeter/greeter.pix"
    );
    let outcome = pixie_driver::check_file(Path::new(path)).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().expect("module"), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        // The controlled field: value bound to the store prop, both
        // handlers taking the implicit `text` argument.
        "Element::TextField { value: w.singleton_ref::<Session>().name(w)",
        "move |w: &mut World, text: Str|",
        ".update_name(w, __a0)",
        "Element::Row { spacing: -1f64, padding: 0f64, background: Str::new(), grow: 0f64, border_radius: 0f64, border_width: 0f64, border_color: Str::new(), children: ",
        // The uncontrolled field carries no value binding.
        "Element::TextField { value: Str::new()",
        // Headless script steps (text widgets included) go through
        // the shared kernel harness.
        "pixie_kernel::script::run(&__rt, __view, &mut __tree, &__script)",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`");
    }
}

#[test]
fn fetch_demo_checks_and_emits_async() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/fetch/fetch.pix"
    );
    let outcome = pixie_driver::check_file(Path::new(path)).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().expect("module"), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        // The async method keeps the sync call-site shape and spawns.
        "fn fetch(self, w: &mut World, path: Str) {",
        "let __ctx = w.async_ctx();",
        "w.spawn(async move {",
        // Sync statements run through re-entries; awaited binding calls
        // ship to a worker and convert back on the main side.
        "__ctx.with(|w: &mut World|",
        "pixie_kernel::spawn_worker(move ||",
        "__c1.await.map(|__v| Str::from(__v)).map_err(Str::from)",
        // The shared kernel harness settles the async tier between
        // steps (hoisted out of the emitted main).
        "pixie_kernel::script::run(&__rt, __view, &mut __tree, &__script)",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`");
    }
}

#[test]
fn genfs_demo_uses_generated_bindings() {
    // The .rpi next to this demo is rpi-gen output from the real
    // std.json (format 61) — the full §7 circle: generate → check →
    // emit against the same adapters the hand-written bindings use.
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/genfs/genfs.pix"
    );
    let outcome = pixie_driver::check_file(Path::new(path)).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().expect("module"), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        "std::fs::write((",
        "std::fs::read_to_string((",
        "std::fs::remove_file((",
        ".map_err(|e| Str::from(e.to_string()))",
        // The widened adapters: PathBuf→Str and Vec<String>→List<Str>.
        "std::fs::canonicalize((",
        "pixie_kernel::list_dir((",
        ".collect::<List<Str>>()",
        // `T?` (§11.11): the Option battery converts through a `let`
        // that pins the type (a closure applied in place would leave
        // its parameter untyped, E0282 — §8.77) and `when some/nil`
        // lowers to a Rust match.
        "pixie_kernel::env_var((",
        "__v.map(|__e| Str::from(__e))",
        "Some(v) => {",
        "None => {",
        // Bytes (§11.10): fs::read lands as the COW byte string, and
        // `.length` reads it like any other value.
        "std::fs::read((",
        ".map(|__v| Bytes::from(__v))",
        // §8.77: a STRUCT crosses field for field, in both
        // directions, and a list of them element by element.
        "pixie_kernel::dir_stats((",
        "|v: pixie_kernel::Entry| Entry { name: Str::from(v.name)",
        "pixie_kernel::stat_total((",
        "|x: &Entry| pixie_kernel::Entry {",
        // §8.78: reading a field widens on its own, writing it back
        // hits the width the `.rpi` named. `FileStat.len` is a `u64`.
        "FileStat { len: v.len as i64",
        "pixie_kernel::FileStat { len: ((&x.len).clone()) as u64",
        // And a TUPLE field crosses by position: `v.0` coming back,
        // `T { 0: .. }` going out, both valid Rust.
        "Perms { value: v.0 as i64 }",
        "pixie_kernel::Perms { 0: ((&x.value).clone()) as u32 }",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`");
    }
}

#[test]
fn reload_emission_wires_rung2() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/greeter/greeter.pix"
    );
    let outcome = pixie_driver::check_file(Path::new(path)).expect("driver runs");
    assert_eq!(outcome.error_count(), 0);
    let ri = pixie_codegen::ReloadInfo {
        source_path: "/tmp/greeter.pix".into(),
        fingerprint: 42,
        foreign_paths: Vec::new(),
    };
    let code = pixie_codegen::emit_program(
        outcome.module.as_ref().expect("module"),
        outcome.binding_items,
        Some(&ri),
    )
    .expect("emit succeeds");
    for needle in [
        // Statics + identity of the compiled program.
        "static __PIXIE_LIVE",
        "const __PIXIE_FINGERPRINT: u64 = 42u64;",
        // Reflection tables reach compiled classes.
        "fn __pixie_tables() -> pixie_interp::Tables",
        "t.getter(\"Session\", \"name\"",
        "t.method(\"Session\", \"updateName\"",
        "t.global(\"Session\", \"Session\"",
        // The build delegation and the engine watch hookup.
        "pixie_interp::build_view(&__lv, &__interp_env(self), &__tables, w)",
        "pixie_engine_gpui::ReloadWatch",
        "pixie_engine_gpui::run_app(__rt, __view, &__title, Some(__watch), None);",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`");
    }
}

#[test]
fn text_handler_shadows_state_cell() {
    // A state cell named `text` must not capture the handler's implicit
    // `text` argument — lexical inner scope wins.
    let dir = std::env::temp_dir().join("pixie-m0-gate");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("text_shadow.pix");
    std::fs::write(
        &f,
        "view Main {\n  state text : String = \"\"\n  state other : String = \"\"\n\n  Column {\n    TextField {\n      onTextChanged: { other = text }\n    }\n    Text { text: \"t: #{text}\" }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().unwrap(), outcome.binding_items, None)
            .expect("emit succeeds");
    assert!(
        code.contains("let __v = text.clone(); ") && code.contains("set_other(w, __v);"),
        "handler arg must stay the implicit param:\n{code}"
    );
    assert!(
        code.contains(".text(w)"),
        "display interpolation still reads the state cell:\n{code}"
    );
}

/// Styles splice before check/codegen: named entries land as element
/// props (right-wins through `+`), so the emission carries the merged
/// values and no `style:` member survives to the lowering.
#[test]
fn styles_splice_into_emission() {
    let dir = std::env::temp_dir().join("pixie-style-splice");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("styled.pix");
    std::fs::write(
        &f,
        "style Base {\n  fontSize: 18.0\n  color: \"#cdd6f4\"\n}\n\
         style Warn {\n  color: \"#f38ba8\"\n}\n\
         style Callout = Base + Warn\n\n\
         view Main {\n  Column {\n    style: Pad\n    Text { style: Callout; text: \"hi\" }\n  }\n}\n\n\
         style Pad {\n  spacing: 4.0\n  padding: 16.0\n  background: \"#181825\"\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert_eq!(outcome.error_count(), 0, "diags: {:?}", outcome.diagnostics);
    let code = pixie_codegen::emit_program(
        outcome.module.as_ref().expect("module"),
        outcome.binding_items,
        None,
    )
    .expect("emit succeeds");
    for needle in [
        // Base's size survives, Warn's color wins the merge.
        "font_size: 18f64",
        "color: Str::from(\"#f38ba8\")",
        // The container triple from `Pad`, declared BELOW the view —
        // the env is whole-module, not source-order.
        "spacing: 4f64, padding: 16f64, background: Str::from(\"#181825\")",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`:\n{code}");
    }
}

/// An unresolvable `style:` reference is a check-time error with the
/// property's span — never a silent fall-through.
#[test]
fn unknown_style_reference_errors() {
    let dir = std::env::temp_dir().join("pixie-style-unknown");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("bad.pix");
    std::fs::write(&f, "view Main {\n  Column {\n    style: Nope\n  }\n}\n").unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert!(outcome.error_count() > 0);
    assert!(
        outcome
            .diagnostics
            .iter()
            .any(|d| d.message.contains("unknown style `Nope`")),
        "diagnostics: {:?}",
        outcome.diagnostics
    );
}

#[test]
fn progress_demo_checks_and_emits() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/progress/progress.pix"
    );
    let outcome = pixie_driver::check_file(Path::new(path)).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().expect("module"), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        "Element::ProgressBar { value:",
        // The float lowering's Member arm: a Float state cell read
        // through the store's singleton handle.
        "w.singleton_ref::<Job>().ratio(w)",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`");
    }
}

#[test]
fn sliders_demo_checks_and_emits() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/sliders/sliders.pix"
    );
    let outcome = pixie_driver::check_file(Path::new(path)).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().expect("module"), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        // `value:` is a property READ through the store's singleton
        // handle; the range props are lowered literals with `step`
        // set (0f64 would mean continuous).
        "Element::Slider { value: w.singleton_ref::<Mix>().volume(w), min: 0f64, max: 10f64, step: 1f64, on_change: Some(",
        // The handler's implicit `value` argument carries the new
        // value — `onTextChanged`'s `text` machinery, one primitive
        // over.
        "move |w: &mut World, value: f64|",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`");
    }
}

#[test]
fn slider_value_must_be_a_property() {
    let dir = std::env::temp_dir().join("pixie-m0-gate");
    std::fs::create_dir_all(&dir).unwrap();

    // A literal `value:` could never reflect state across rebuilds —
    // a named error (the charts' `data:` rule), never a frozen
    // control.
    let f = dir.join("slider_literal_value.pix");
    std::fs::write(
        &f,
        "view Main {\n  Column {\n    Slider { value: 0.5 }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    let err =
        pixie_codegen::emit_program(outcome.module.as_ref().unwrap(), outcome.binding_items, None)
            .expect_err("a literal `value:` must not emit");
    assert!(
        err.message.contains("`value:`") && err.message.contains("property"),
        "error should say value must be a property: {}",
        err.message
    );

    // And the prop is required at all — the control IS its binding.
    let f = dir.join("slider_no_value.pix");
    std::fs::write(&f, "view Main {\n  Column {\n    Slider { min: 0.0 }\n  }\n}\n").unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    let err =
        pixie_codegen::emit_program(outcome.module.as_ref().unwrap(), outcome.binding_items, None)
            .expect_err("a valueless Slider must not emit");
    assert!(
        err.message.contains("Slider needs `value:`"),
        "error should name the missing prop: {}",
        err.message
    );
}

#[test]
fn scroll_demo_checks_and_emits() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/scroll/scroll.pix"
    );
    let outcome = pixie_driver::check_file(Path::new(path)).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().expect("module"), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        // Both scroll containers lower through `lower_children`, so a
        // `for` repeater inside one collects into the same Vec — but
        // only the vertical one carries a `height:` viewport prop.
        "Element::ScrollView { height: 240f64, children:",
        "Element::HScrollView(",
        "w.singleton_ref::<Feed>().items(w)",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`");
    }
}

#[test]
fn gallery_demo_checks_and_emits() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/gallery/gallery.pix"
    );
    let outcome = pixie_driver::check_file(Path::new(path)).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().expect("module"), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        "Element::Image { source: Str::from(\"examples/gallery/pixie.png\")",
        "width: 64f64",
        "height: 64f64",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`");
    }
}

#[test]
fn icons_demo_checks_and_emits() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/icons/icons.pix"
    );
    let outcome = pixie_driver::check_file(Path::new(path)).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().expect("module"), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        "Element::Svg { source: Str::from(\"examples/icons/star.svg\")",
        "width: 24f64",
        "height: 48f64",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`");
    }
}

#[test]
fn table_demo_checks_and_emits() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/table/table.pix"
    );
    let outcome = pixie_driver::check_file(Path::new(path)).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().expect("module"), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        "Element::DataTable(",
        "Element::Row { spacing: -1f64, padding: 0f64, background: Str::new(), grow: 0f64, border_radius: 0f64, border_width: 0f64, border_color: Str::new(), children: ",
        // The `for` repeater reads the store's list prop through its
        // singleton handle.
        "w.singleton_ref::<Roster>().rows(w)",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`");
    }
}

#[test]
fn dialog_demo_checks_and_emits() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/dialog/dialog.pix"
    );
    let outcome = pixie_driver::check_file(Path::new(path)).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().expect("module"), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        "Element::Modal { open:",
        // The bool lowering's Member arm: a Bool state cell read
        // through the store's singleton handle.
        "w.singleton_ref::<Dialog>().show(w)",
        // `open:` is a container property — `lower_children` must let
        // it past rather than rejecting the Modal's own member.
        "children:",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`");
    }
}

#[test]
fn biglist_demo_checks_and_emits() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/biglist/biglist.pix"
    );
    let outcome = pixie_driver::check_file(Path::new(path)).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().expect("module"), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        // A virtualized single-repeater list emits LazyRows: eager
        // len, on-demand row builder over the requested range —
        // §11.17's shape, with the container props alongside.
        "virtualized: true,",
        "item_height: 24f64,",
        "height: 280f64,",
        "lazy: Some(LazyRows {",
        "len: w.singleton_ref::<Rows>().rows(w).len(),",
        "build: Rc::new(move |w: &World, __range: std::ops::Range<usize>|",
        "let r = __xs.at(__row_idx0 as i64);",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`");
    }
}

#[test]
fn container_properties_are_allowlisted_per_element() {
    let dir = std::env::temp_dir().join("pixie-m0-gate");
    std::fs::create_dir_all(&dir).unwrap();

    // A key no container consumes: an error, never a silently dropped
    // property (D10 totality).
    let f = dir.join("listview_unknown_prop.pix");
    std::fs::write(
        &f,
        "view Main {\n  Column {\n    ListView {\n      spacing: 4.0\n      Text { text: \"hi\" }\n    }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    let err =
        pixie_codegen::emit_program(outcome.module.as_ref().unwrap(), outcome.binding_items, None)
            .expect_err("an unknown container property must not emit");
    assert!(
        err.message.contains("`spacing`") && err.message.contains("`ListView`"),
        "error should name the key and the element: {}",
        err.message
    );

    // The allowlist is per element: ListView's own keys are not
    // Column's.
    let f = dir.join("column_borrows_listview_prop.pix");
    std::fs::write(
        &f,
        "view Main {\n  Column {\n    virtualized: true\n    Text { text: \"hi\" }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    let err =
        pixie_codegen::emit_program(outcome.module.as_ref().unwrap(), outcome.binding_items, None)
            .expect_err("a ListView key on a Column must not emit");
    assert!(
        err.message.contains("`virtualized`") && err.message.contains("`Column`"),
        "error should name the key and the element: {}",
        err.message
    );

    // `virtualized:` is a Bool and stays strict. `itemHeight:` is a
    // Float, and an Int widens into it (§8.55) — 24 and 24.0 are the
    // same number and a row height is written as the first.
    let f = dir.join("listview_int_item_height.pix");
    std::fs::write(
        &f,
        "view Main {\n  Column {\n    ListView {\n      itemHeight: 24\n      Text { text: \"hi\" }\n    }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().unwrap(), outcome.binding_items, None)
            .expect("an Int `itemHeight:` widens");
    assert!(
        code.contains("item_height: 24f64"),
        "an Int itemHeight must widen: {code}"
    );

    // ScrollView's `height:` widens the same way, and belongs to the
    // vertical twin alone — HScrollView clips on width.
    let f = dir.join("scrollview_int_height.pix");
    std::fs::write(
        &f,
        "view Main {\n  Column {\n    ScrollView {\n      height: 240\n      Text { text: \"hi\" }\n    }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().unwrap(), outcome.binding_items, None)
            .expect("an Int `height:` widens");
    assert!(
        code.contains("height: 240f64"),
        "an Int height must widen: {code}"
    );

    let f = dir.join("hscrollview_height.pix");
    std::fs::write(
        &f,
        "view Main {\n  Column {\n    HScrollView {\n      height: 240.0\n      Text { text: \"hi\" }\n    }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    let err =
        pixie_codegen::emit_program(outcome.module.as_ref().unwrap(), outcome.binding_items, None)
            .expect_err("`height:` on HScrollView must not emit");
    assert!(
        err.message.contains("`height`") && err.message.contains("`HScrollView`"),
        "error should name the key and the element: {}",
        err.message
    );
}

#[test]
fn modal_requires_a_bool_open() {
    let dir = std::env::temp_dir().join("pixie-m0-gate");
    std::fs::create_dir_all(&dir).unwrap();

    // `open:` went optional when `if` landed in views: a bare Modal
    // (cute_ui's propless shape) emits open — the `if` wrapper is the
    // visibility switch now.
    let f = dir.join("modal_no_open.pix");
    std::fs::write(
        &f,
        "view Main {\n  Column {\n    Modal {\n      Text { text: \"hi\" }\n    }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    let code = pixie_codegen::emit_program(
        outcome.module.as_ref().unwrap(),
        outcome.binding_items,
        None,
    )
    .expect("a bare Modal emits, defaulting open");
    assert!(
        code.contains("Element::Modal { open: true, children:"),
        "bare Modal must default open:\n{code}"
    );

    // Present but Int-typed — same strictness as ProgressBar's Float.
    let f = dir.join("modal_int_open.pix");
    std::fs::write(
        &f,
        "store S {\n  state n : Int = 0\n}\n\nview Main {\n  Column {\n    Modal {\n      open: S.n\n      Text { text: \"hi\" }\n    }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    let err = pixie_codegen::emit_program(
        outcome.module.as_ref().unwrap(),
        outcome.binding_items,
        None,
    )
    .expect_err("an Int `open:` must not emit");
    assert!(
        err.message.contains("Bool"),
        "error should name the expected type: {}",
        err.message
    );
}

#[test]
fn list_literals_lower_in_methods_and_actions() {
    // §11.13: `[]` / `[a, b]` in assignment position (methods and
    // action blocks), not just prop defaults.
    let dir = std::env::temp_dir().join("pixie-m0-gate");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("list_lit.pix");
    std::fs::write(
        &f,
        "store S {\n  state items : List<String> = []\n\n  fn seed {\n    items = [\"a\", \"b\"]\n  }\n\n  fn reset {\n    items = []\n  }\n}\n\nview Main {\n  Column {\n    Text { text: \"n: #{S.items.length}\" }\n    Button { text: \"go\"; onClick: { S.items = [\"x\"] } }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().unwrap(), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        "__lit.push(Str::from(\"a\"))",
        "let __v = List::new(); ",
        "__lit.push(Str::from(\"x\"))",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`:\n{code}");
    }
}

#[test]
fn store_desugars_and_checks() {
    let dir = std::env::temp_dir().join("pixie-m0-gate");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("store_ok.pix");
    std::fs::write(
        &f,
        "store App {\n  state ticks : Int = 0\n\n  fn tick {\n    ticks = ticks + 1\n  }\n}\n\nview Main {\n  Column {\n    Text { text: \"t: #{App.ticks}\" }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().unwrap(), outcome.binding_items, None)
            .expect("emit succeeds");
    assert!(code.contains("w.singleton(App::new)"), "singleton init missing");
    assert!(
        code.contains("w.singleton_ref::<App>()"),
        "singleton_ref reads missing"
    );
}

#[test]
fn layers_demo_checks_and_emits() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/layers/layers.pix"
    );
    let outcome = pixie_driver::check_file(Path::new(path)).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().expect("module"), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        "Element::Stack(",
        // The base Column (child 0) lowers through the same
        // `lower_children` helper as every other container.
        "Element::Column { spacing: -1f64, padding: 0f64, background: Str::new(), grow: 0f64, border_radius: 0f64, border_width: 0f64, border_color: Str::new(), children: ",
        // The overlay Text and the top-layer Button are later
        // elements of the same children Vec — no separate wrapping on
        // the codegen side, that's engine-only (absolute/inset_0).
        "Element::Button { label: Str::from(\"bump\"), background: Str::new()",
        // Display interpolation inside the Stack's base still reads
        // the view's `state` cell through the synthesized holder.
        "__pixie_state.count(w)",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`");
    }
}

#[test]
fn calcgrid_demo_checks_and_emits() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/calcgrid/calcgrid.pix"
    );
    let outcome = pixie_driver::check_file(Path::new(path)).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().expect("module"), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        // `columns:`/`rows:` lower through `lower_view_int`; the rest
        // of the group is Column's, sentinels included.
        "Element::Grid { columns: 4i64, rows: 5i64, spacing: 8f64, padding: 0f64, background: Str::new(), grow: 5f64, border_radius: 0f64, border_width: 0f64, border_color: Str::new(), children: ",
        // `colSpan:` is stripped off the Button and re-emitted as the
        // grid-item wrapper around it.
        "Element::GridCell { col_span: 2i64, row_span: 1i64, children: vec![Element::Button { label: Str::from(\"0\")",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`");
    }
}

#[test]
fn grid_tracks_are_int_strict_and_spans_wrap_any_element() {
    let dir = std::env::temp_dir().join("pixie-m0-gate");
    std::fs::create_dir_all(&dir).unwrap();

    let emit = |name: &str, src: &str| -> Result<String, String> {
        let f = dir.join(name);
        std::fs::write(&f, src).unwrap();
        let outcome = pixie_driver::check_file(&f).expect("driver runs");
        pixie_codegen::emit_program(
            outcome.module.as_ref().unwrap(),
            outcome.binding_items,
            None,
        )
        .map_err(|e| e.message)
    };

    // A grid without tracks has no shape: named error, never a guess.
    let err = emit(
        "grid_no_columns.pix",
        "view Main {\n  Grid {\n    Text { text: \"a\" }\n  }\n}\n",
    )
    .expect_err("a Grid without `columns:` must not emit");
    assert!(
        err.contains("Grid needs `columns:`"),
        "error should name the missing prop: {err}"
    );

    // `columns:` is an Int — Float is an error, not a truncation.
    let err = emit(
        "grid_float_columns.pix",
        "view Main {\n  Grid {\n    columns: 2.0\n    Text { text: \"a\" }\n  }\n}\n",
    )
    .expect_err("a Float track count must not emit");
    assert!(
        err.contains("Int"),
        "error should name the expected type: {err}"
    );

    // The spans are element-independent: a CONTAINER takes them too,
    // and the wrapper lands outside it.
    let code = emit(
        "grid_container_span.pix",
        "view Main {\n  Grid {\n    columns: 2\n    Column {\n      colSpan: 2\n      Text { text: \"wide\" }\n    }\n  }\n}\n",
    )
    .expect("a spanned container emits");
    assert!(
        code.contains(
            "Element::GridCell { col_span: 2i64, row_span: 1i64, children: vec![Element::Column {"
        ),
        "the span wrapper should sit outside the container: {code}"
    );
}

#[test]
fn charts_demo_checks_and_emits() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/charts/charts.pix"
    );
    let outcome = pixie_driver::check_file(Path::new(path)).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().expect("module"), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        // `lower_view_float_list`'s Member arm: a List<Float> state
        // cell read through the store's singleton handle, plus
        // `lower_view_str_list`'s on the same element — and the demo's
        // explicit sizing, lowered by `lower_view_size`. Everything
        // the demo left unsaid lowers to the engine's "unset".
        "Element::BarChart { data: w.singleton_ref::<Charts>().values(w), \
         labels: w.singleton_ref::<Charts>().names(w), width: 260f64, height: 110f64, \
         min: 0f64, max: 0f64, axis: false, color: Str::new(), series: List::new(), \
         colors: List::new() }",
        // The unsized twin: both axes fall back to `0f64`, which the
        // engine reads as "full width, default plot height" — and a
        // single-series `color:` reaches it as a plain Str.
        "Element::LineChart { data: w.singleton_ref::<Charts>().values(w), \
         labels: w.singleton_ref::<Charts>().names(w), width: 0f64, height: 0f64, \
         min: 0f64, max: 0f64, axis: false, color: Str::from(\"#f9e2af\"), \
         series: List::new(), colors: List::new() }",
        // The negative-valued chart asks for an axis: `axis:` is a
        // plain bool, and the range stays at the data's own.
        "Element::BarChart { data: w.singleton_ref::<Charts>().pnl(w), labels: List::new(), \
         width: 0f64, height: 100f64, min: 0f64, max: 0f64, axis: true, color: Str::new(), \
         series: List::new(), colors: List::new() }",
        // The multi-series chart: `series:` is a List<List<Float>>
        // read (`lower_view_float_list2`), `colors:` a literal list
        // built in place, and the pinned range carries its sign.
        "Element::LineChart { data: List::new(), labels: List::new(), width: 0f64, \
         height: 100f64, min: (-2f64), max: 12f64, axis: true, color: Str::new(), \
         series: w.singleton_ref::<Charts>().pairs(w), colors: { let mut __c = List::new(); \
         __c.push(Str::from(\"accent\")); __c.push(Str::from(\"#f38ba8\")); __c } }",
        // Spinner's single square axis.
        "Element::Spinner { size: 32f64 }",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`");
    }
}

#[test]
fn a_chart_takes_data_or_series_and_refuses_a_nested_literal() {
    let dir = std::env::temp_dir().join("pixie-m0-gate");
    std::fs::create_dir_all(&dir).unwrap();
    let store = "store S {\n  state xs : List<Float> = []\n  \
                 state ss : List<List<Float>> = []\n}\n\n";
    let emit = |name: &str, body: &str| -> Result<String, String> {
        let f = dir.join(name);
        std::fs::write(
            &f,
            format!("{store}view Main {{\n  Column {{\n    {body}\n  }}\n}}\n"),
        )
        .unwrap();
        let outcome = pixie_driver::check_file(&f).expect("driver runs");
        pixie_codegen::emit_program(outcome.module.as_ref().unwrap(), outcome.binding_items, None)
            .map_err(|e| e.message)
    };

    // `series:` alone is a whole chart, and `data:` empties out.
    let code = emit("chart_series_only.pix", "BarChart { series: S.ss }").expect("emits");
    assert!(
        code.contains("data: List::new()") && code.contains("series: w.singleton_ref::<S>().ss(w)"),
        "a series-only chart binds the series and empties the data: {code}"
    );

    // A nested list literal has no lowering in a view — the refusal
    // names the field to declare instead of hinting at nothing.
    let err = emit("chart_series_literal.pix", "BarChart { series: [] }")
        .expect_err("a nested literal must not emit");
    assert!(
        err.contains("List<List<Float>>") && err.contains("store"),
        "error should name the shape and where to put it: {err}"
    );

    // A flat list in the series slot is a type error, not a silent
    // one-series chart.
    let err = emit("chart_series_flat.pix", "BarChart { series: S.xs }")
        .expect_err("a flat list is not a series list");
    assert!(
        err.contains("List<List<Float>>"),
        "error should name the expected type: {err}"
    );
}

#[test]
fn charts_require_a_typed_list_data() {
    let dir = std::env::temp_dir().join("pixie-m0-gate");
    std::fs::create_dir_all(&dir).unwrap();

    let emit_err = |name: &str, src: &str| -> String {
        let f = dir.join(name);
        std::fs::write(&f, src).unwrap();
        let outcome = pixie_driver::check_file(&f).expect("driver runs");
        pixie_codegen::emit_program(
            outcome.module.as_ref().unwrap(),
            outcome.binding_items,
            None,
        )
        .expect_err("this chart must not emit")
        .message
    };

    // Missing `data:` — a chart with nothing to plot is a mistake, not
    // an empty chart (that is a bound empty list).
    for widget in ["BarChart", "LineChart"] {
        let err = emit_err(
            &format!("chart_no_data_{widget}.pix"),
            &format!("view Main {{\n  Column {{\n    {widget} {{ }}\n  }}\n}}\n"),
        );
        assert!(
            err.contains(&format!("{widget} needs `data:`")),
            "error should name the missing prop: {err}"
        );
    }

    // Present but wrongly typed: same strictness as ProgressBar's
    // Float — `data:` is List<Float>, `labels:` is List<String>.
    let store = "store S {\n  state xs : List<Float> = []\n  state ns : List<String> = []\n}\n\n";
    let err = emit_err(
        "chart_str_data.pix",
        &format!("{store}view Main {{\n  Column {{\n    BarChart {{ data: S.ns }}\n  }}\n}}\n"),
    );
    assert!(
        err.contains("List<Float>"),
        "error should name the expected type: {err}"
    );
    let err = emit_err(
        "chart_float_labels.pix",
        &format!(
            "{store}view Main {{\n  Column {{\n    LineChart {{ data: S.xs; labels: S.xs }}\n  }}\n}}\n"
        ),
    );
    assert!(
        err.contains("List<String>"),
        "error should name the expected type: {err}"
    );

    // A list literal has no lowering yet; the error says so and points
    // at the binding that does work.
    let err = emit_err(
        "chart_literal_data.pix",
        "view Main {\n  Column {\n    BarChart { data: [] }\n  }\n}\n",
    );
    assert!(
        err.contains("List<Float> property"),
        "error should point at the property binding: {err}"
    );
}

#[test]
fn chart_and_spinner_sizing_is_optional_and_float_typed() {
    let dir = std::env::temp_dir().join("pixie-m0-gate");
    std::fs::create_dir_all(&dir).unwrap();
    let store = "store S {\n  state xs : List<Float> = []\n}\n\n";
    let emit = |name: &str, body: &str| -> Result<String, String> {
        let f = dir.join(name);
        std::fs::write(
            &f,
            format!("{store}view Main {{\n  Column {{\n    {body}\n  }}\n}}\n"),
        )
        .unwrap();
        let outcome = pixie_driver::check_file(&f).expect("driver runs");
        pixie_codegen::emit_program(outcome.module.as_ref().unwrap(), outcome.binding_items, None)
            .map_err(|e| e.message)
    };

    // Unset sizing lowers to `0f64` on every axis — the Image rule, so
    // an untouched chart keeps its full-width default rendering.
    let code = emit("chart_unsized.pix", "BarChart { data: S.xs }").expect("emits");
    assert!(
        code.contains("width: 0f64, height: 0f64"),
        "unset chart sizing must lower to 0f64: {code}"
    );
    let code = emit("spinner_unsized.pix", "Spinner { }").expect("emits");
    assert!(
        code.contains("Element::Spinner { size: 0f64 }"),
        "unset Spinner size must lower to 0f64: {code}"
    );

    // Present but Int-typed: widened, not rejected (§8.55). These
    // three were errors until the emitter learned what the checker
    // had always allowed — `width: 200` is what anyone writes.
    for (name, body, needle) in [
        (
            "chart_int_width.pix",
            "BarChart { data: S.xs; width: 200 }",
            "width: 200f64",
        ),
        (
            "chart_int_height.pix",
            "LineChart { data: S.xs; height: 90 }",
            "height: 90f64",
        ),
        (
            "spinner_int_size.pix",
            "Spinner { size: 32 }",
            "Element::Spinner { size: 32f64 }",
        ),
    ] {
        let code = emit(name, body).expect("an Int size widens");
        assert!(code.contains(needle), "missing `{needle}`: {code}");
    }

    // A String is still a String — widening is numeric, not a general
    // coercion, so this must still name the type.
    let err = emit("spinner_str_size.pix", "Spinner { size: \"big\" }")
        .expect_err("a String size must not emit");
    assert!(
        err.contains("numeric"),
        "error should say what the slot takes: {err}"
    );
}

#[test]
fn toggles_demo_checks_and_emits() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/toggles/toggles.pix"
    );
    let outcome = pixie_driver::check_file(Path::new(path)).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().expect("module"), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        // The bool lowering's Member arm: a Bool state cell read
        // through the store's singleton handle — and the handler's
        // implicit `checked: bool` (the NEW value) in the closure
        // signature, TextField's `text` convention one type over.
        "Element::Checkbox { label: Str::from(\"Dark mode\"), \
         checked: w.singleton_ref::<App>().dark(w), \
         on_toggle: Some(Rc::new(move |w: &mut World, checked: bool|",
        "Element::Switch { label: Str::from(\"Wi-Fi\"), \
         checked: w.singleton_ref::<App>().wifi(w), \
         on_toggle: Some(Rc::new(move |w: &mut World, checked: bool|",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`");
    }
    // The toggles OWN `label:` — no `Element::Semantics` wrapper may
    // ride in from the universal a11y rider of the same name.
    assert!(
        !code.contains("Element::Semantics"),
        "a toggle's `label:` must not spawn a Semantics wrapper:\n{code}"
    );
}

#[test]
fn toggles_require_label_and_a_bool_checked() {
    let dir = std::env::temp_dir().join("pixie-m0-gate");
    std::fs::create_dir_all(&dir).unwrap();
    let store = "store S {\n  state on : Bool = false\n  state n : Int = 0\n}\n\n";
    let emit = |name: &str, body: &str| -> Result<String, String> {
        let f = dir.join(name);
        std::fs::write(
            &f,
            format!("{store}view Main {{\n  Column {{\n    {body}\n  }}\n}}\n"),
        )
        .unwrap();
        let outcome = pixie_driver::check_file(&f).expect("driver runs");
        pixie_codegen::emit_program(outcome.module.as_ref().unwrap(), outcome.binding_items, None)
            .map_err(|e| e.message)
    };

    for widget in ["Checkbox", "Switch"] {
        // Both props are required — `checked:` is bound state the app
        // owns, so omitting it is a mistake, never "defaults false".
        let err = emit(
            &format!("toggle_no_label_{widget}.pix"),
            &format!("{widget} {{ checked: S.on }}"),
        )
        .expect_err("a label-less toggle must not emit");
        assert!(
            err.contains(&format!("{widget} needs `label:`")),
            "error should name the missing prop: {err}"
        );
        let err = emit(
            &format!("toggle_no_checked_{widget}.pix"),
            &format!("{widget} {{ label: \"x\" }}"),
        )
        .expect_err("a checked-less toggle must not emit");
        assert!(
            err.contains(&format!("{widget} needs `checked:`")),
            "error should name the missing prop: {err}"
        );
        // Present but Int-typed: same strictness as Modal's `open:` —
        // assert on the type name, not the full message.
        let err = emit(
            &format!("toggle_int_checked_{widget}.pix"),
            &format!("{widget} {{ label: \"x\"; checked: S.n }}"),
        )
        .expect_err("an Int `checked:` must not emit");
        assert!(err.contains("Bool"), "error should name the type: {err}");
    }

    // `onToggle:` is the optional half of the contract.
    let code = emit(
        "toggle_no_handler.pix",
        "Checkbox { label: \"quiet\"; checked: S.on }",
    )
    .expect("a handler-less toggle emits");
    assert!(
        code.contains("Element::Checkbox { label: Str::from(\"quiet\"), checked: w.singleton_ref::<S>().on(w), on_toggle: None }"),
        "missing the None handler: {code}"
    );
}

#[test]
fn choosers_demo_checks_and_emits() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/choosers/choosers.pix"
    );
    let outcome = pixie_driver::check_file(Path::new(path)).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().expect("module"), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        // The chooser contract: `options:`/`labels:` through
        // `lower_view_str_list`, `selected:`/`active:` through
        // `lower_view_int`, and `onSelect` binding the implicit
        // `index: i64` (TextField's `text`, one primitive over).
        "Element::Select { options: w.singleton_ref::<App>().fruits(w), \
         selected: w.singleton_ref::<App>().ix(w), on_select: Some(",
        "Element::RadioGroup { options: w.singleton_ref::<App>().fruits(w), \
         selected: w.singleton_ref::<App>().pick(w), on_select: Some(",
        "Element::TabBar { labels: w.singleton_ref::<App>().tabs(w), \
         active: w.singleton_ref::<App>().tab(w), on_select: Some(",
        "move |w: &mut World, index: i64|",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`");
    }
}

#[test]
fn choosers_require_their_props() {
    let dir = std::env::temp_dir().join("pixie-m0-gate");
    std::fs::create_dir_all(&dir).unwrap();

    let emit_err = |name: &str, src: &str| -> String {
        let f = dir.join(name);
        std::fs::write(&f, src).unwrap();
        let outcome = pixie_driver::check_file(&f).expect("driver runs");
        pixie_codegen::emit_program(
            outcome.module.as_ref().unwrap(),
            outcome.binding_items,
            None,
        )
        .expect_err("this chooser must not emit")
        .message
    };

    // Both required props, named per element.
    for widget in ["Select", "RadioGroup"] {
        let err = emit_err(
            &format!("chooser_no_options_{widget}.pix"),
            &format!("view Main {{\n  Column {{\n    {widget} {{ }}\n  }}\n}}\n"),
        );
        assert!(
            err.contains(&format!("{widget} needs `options:`")),
            "error should name the missing prop: {err}"
        );
    }
    let err = emit_err(
        "chooser_no_labels.pix",
        "view Main {\n  Column {\n    TabBar { }\n  }\n}\n",
    );
    assert!(
        err.contains("TabBar needs `labels:`"),
        "error should name the missing prop: {err}"
    );
    let store = "store S {\n  state ns : List<String> = []\n  state xs : List<Float> = []\n  state i : Int = 0\n}\n\n";
    let err = emit_err(
        "chooser_no_selected.pix",
        &format!("{store}view Main {{\n  Column {{\n    Select {{ options: S.ns }}\n  }}\n}}\n"),
    );
    assert!(
        err.contains("Select needs `selected:`"),
        "error should name the missing prop: {err}"
    );
    let err = emit_err(
        "chooser_no_active.pix",
        &format!("{store}view Main {{\n  Column {{\n    TabBar {{ labels: S.ns }}\n  }}\n}}\n"),
    );
    assert!(
        err.contains("TabBar needs `active:`"),
        "error should name the missing prop: {err}"
    );

    // Wrongly typed options; assert on the type name, not the whole
    // message (the playbook's rule).
    let err = emit_err(
        "chooser_float_options.pix",
        &format!(
            "{store}view Main {{\n  Column {{\n    Select {{ options: S.xs; selected: S.i }}\n  }}\n}}\n"
        ),
    );
    assert!(
        err.contains("List<String>"),
        "error should name the expected type: {err}"
    );

    // A literal list of strings is an option list people write by
    // hand, so it lowers; a literal of anything else still points at
    // the property binding that does work (the charts' rule).
    let f = dir.join("chooser_literal_options.pix");
    std::fs::write(
        &f,
        format!(
            "{store}view Main {{\n  Column {{\n    RadioGroup {{ options: [\"a\", \"b\"]; selected: S.i }}\n  }}\n}}\n"
        ),
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().unwrap(), outcome.binding_items, None)
            .expect("a literal option list emits");
    assert!(
        code.contains("__lit.push(Str::from(\"a\"))"),
        "the literal list should be built in place:\n{code}"
    );
    let err = emit_err(
        "chooser_float_literal_options.pix",
        &format!(
            "{store}view Main {{\n  Column {{\n    RadioGroup {{ options: [1.0]; selected: S.i }}\n  }}\n}}\n"
        ),
    );
    assert!(
        err.contains("String"),
        "error should point at the property binding: {err}"
    );
}

#[test]
fn if_in_views_lowers_both_branches() {
    let dir = std::env::temp_dir().join("pixie-m0-gate");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("if_views.pix");
    std::fs::write(
        &f,
        "store S {\n  state show : Bool = false\n  state n : Int = 0\n}\n\nview Main {\n  Column {\n    if S.show {\n      Text { text: \"visible\" }\n    }\n    if S.n > 2 {\n      Text { text: \"big\" }\n    } else {\n      Text { text: \"small\" }\n    }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().unwrap(), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        // Bare `if`: pushes only when the Bool prop reads true.
        "if w.singleton_ref::<S>().show(w) {",
        // Comparison condition through the action-expression grammar.
        "if (w.singleton_ref::<S>().n(w) > 2i64) {",
        "} else {",
        "__c0.push(Element::Text { text: Str::from(\"small\"), font_size: 0f64, color: Str::new(), align: Str::new(), grow: 0f64 });",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`:\n{code}");
    }
}

/// `Bytes` is a first-class prim now (§8.19): its member set is
/// closed (`length` only), and props of it are a named M2 error
/// rather than a rustc surprise.
#[test]
fn bytes_is_a_strict_prim() {
    let dir = std::env::temp_dir().join("pixie-bytes-prim");
    std::fs::create_dir_all(&dir).unwrap();

    // Bad member: strict error, not a soft-pass.
    let f = dir.join("member.pix");
    std::fs::write(
        &f,
        "store S {\n  state n : Int = 0\n\n  fn go {\n    case Fs.read(\"/tmp/x\") {\n      when ok(b) { n = b.size }\n      when err(e) { n = 0 }\n    }\n  }\n}\n\nview Main {\n  Column {\n    Text { text: \"#{S.n}\" }\n  }\n}\n",
    )
    .unwrap();
    let rpi = dir.join("fs.rpi");
    std::fs::write(
        &rpi,
        "class Fs {\n  static fn read(path: String) !Bytes @rust(\"std::fs::read\")\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert!(
        outcome
            .diagnostics
            .iter()
            .any(|d| d.message.contains("no member `size` on `Bytes`")),
        "diagnostics: {:?}",
        outcome.diagnostics
    );

    // Bytes prop: named M2 gate at emission.
    let f2 = dir.join("prop.pix");
    std::fs::write(
        &f2,
        "class C {\n  pub prop data : Bytes, default: 0\n}\n\nview Main {\n  Column {\n    Text { text: \"x\" }\n  }\n}\n",
    )
    .unwrap();
    let outcome2 = pixie_driver::check_file(&f2).expect("driver runs");
    if outcome2.error_count() == 0 {
        let err = pixie_codegen::emit_program(
            outcome2.module.as_ref().expect("module"),
            outcome2.binding_items,
            None,
        )
        .unwrap_err();
        assert!(
            format!("{err:?}").contains("`Bytes` props are not lowerable yet"),
            "unexpected: {err:?}"
        );
    }
}

/// Cross-module styles (§8.19): `pub style` in a used module resolves
/// in the entry's splice; a non-`pub` one stays invisible.
#[test]
fn pub_styles_cross_modules() {
    let dir = std::env::temp_dir().join("pixie-pub-style");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("theme.pix"),
        "pub style Big {\n  fontSize: 30.0\n}\n\nstyle Hidden {\n  fontSize: 9.0\n}\n",
    )
    .unwrap();

    let ok = dir.join("ok.pix");
    std::fs::write(
        &ok,
        "use theme\n\nview Main {\n  Column {\n    Text { style: Big; text: \"x\" }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&ok).expect("driver runs");
    assert_eq!(outcome.error_count(), 0, "diags: {:?}", outcome.diagnostics);
    assert!(
        outcome.foreign_styles_src.contains("style Big"),
        "foreign snippet: {}",
        outcome.foreign_styles_src
    );
    let code = pixie_codegen::emit_program(
        outcome.module.as_ref().expect("module"),
        outcome.binding_items,
        None,
    )
    .expect("emit succeeds");
    assert!(code.contains("font_size: 30f64"), "spliced size missing");

    let bad = dir.join("bad.pix");
    std::fs::write(
        &bad,
        "use theme\n\nview Main {\n  Column {\n    Text { style: Hidden; text: \"x\" }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&bad).expect("driver runs");
    assert!(
        outcome
            .diagnostics
            .iter()
            .any(|d| d.message.contains("unknown style `Hidden`")),
        "diagnostics: {:?}",
        outcome.diagnostics
    );
}

/// §12.1: aliases, nested paths, and selective imports — plus the
/// hard errors that keep the flat emitter namespace honest.
#[test]
fn module_system_slices() {
    let dir = std::env::temp_dir().join("pixie-mod-system");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("ui")).unwrap();
    std::fs::write(
        dir.join("model.pix"),
        "pub fn decorate(t: String) String {\n  \"[#{t}]\"\n}\n\npub fn hidden String {\n  \"h\"\n}\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("ui/card.pix"),
        "use model.{decorate}\n\npub fn cardTitle(t: String) String {\n  \"<#{decorate(t)}>\"\n}\n",
    )
    .unwrap();

    // Alias + nested-dir import, qualified refs erased for codegen.
    let ok = dir.join("ok.pix");
    std::fs::write(
        &ok,
        "use model as m\nuse ui.card\n\nstore S {\n  state out : String = \"\"\n\n  fn go {\n    let inner : String = m.decorate(\"x\")\n    out = card.cardTitle(inner)\n  }\n}\n\nview Main {\n  Column {\n    Text { text: \"#{S.out}\" }\n    Button { text: \"go\"; onClick: S.go() }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&ok).expect("driver runs");
    assert_eq!(outcome.error_count(), 0, "diags: {:?}", outcome.diagnostics);
    let code = pixie_codegen::emit_program(
        outcome.module.as_ref().expect("module"),
        outcome.binding_items,
        None,
    )
    .expect("emit succeeds");
    // Qualifiers erased; the nested-w args hoist (§11.20 fixed).
    assert!(code.contains("decorate(w, "), "qualified call not erased:\n{code}");
    assert!(code.contains("let __a0 = "), "call args not hoisted:\n{code}");

    // A selective import reaches ONLY the listed names.
    let sel = dir.join("sel.pix");
    std::fs::write(
        &sel,
        "use model.{decorate}\n\nfn misuse String {\n  hidden()\n}\n\nview Main {\n  Column {\n    Text { text: misuse() }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&sel).expect("driver runs");
    assert!(
        outcome.error_count() > 0,
        "selective import must not leak `hidden`"
    );

    // Selective renames rebind the local name (§8.22).
    let ren = dir.join("ren.pix");
    std::fs::write(
        &ren,
        "use model.{decorate as d}\n\nstore S {\n  state out : String = \"\"\n\n  fn go {\n    out = d(\"x\")\n  }\n}\n\nview Main {\n  Column {\n    Text { text: \"#{S.out}\" }\n    Button { text: \"go\"; onClick: S.go() }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&ren).expect("driver runs");
    assert_eq!(outcome.error_count(), 0, "diags: {:?}", outcome.diagnostics);
    let code = pixie_codegen::emit_program(
        outcome.module.as_ref().expect("module"),
        outcome.binding_items,
        None,
    )
    .expect("emit succeeds");
    assert!(
        code.contains("decorate(w, "),
        "renamed selective must call the target:\n{code}"
    );

    // A qualifier that collides with a local name refuses to erase.
    let shadow = dir.join("shadow.pix");
    std::fs::write(
        &shadow,
        "use model as m\n\nfn f(m: Int) Int {\n  m\n}\n\nview Main {\n  Column {\n    Text { text: \"x\" }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&shadow).expect("driver runs");
    assert!(
        outcome
            .diagnostics
            .iter()
            .any(|d| d.message.contains("collides with a declared name")),
        "diags: {:?}",
        outcome.diagnostics
    );

    // Same-name items across modules COEXIST now (§8.22): each
    // declaration mangles per module; qualified refs resolve to the
    // right one.
    std::fs::write(
        dir.join("other.pix"),
        "pub fn decorate(t: String) String {\n  \"(#{t})\"\n}\n",
    )
    .unwrap();
    let dup = dir.join("dup.pix");
    std::fs::write(
        &dup,
        "use model as m\nuse other as o\n\nstore S {\n  state out : String = \"\"\n\n  fn go {\n    let a : String = m.decorate(\"x\")\n    let b : String = o.decorate(\"x\")\n    out = a + b\n  }\n}\n\nview Main {\n  Column {\n    Text { text: \"#{S.out}\" }\n    Button { text: \"go\"; onClick: S.go() }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&dup).expect("driver runs");
    assert_eq!(outcome.error_count(), 0, "diags: {:?}", outcome.diagnostics);
    let code = pixie_codegen::emit_program(
        outcome.module.as_ref().expect("module"),
        outcome.binding_items,
        None,
    )
    .expect("emit succeeds");
    assert!(code.contains("decorate__model(w, "), "mangled model fn:\n{code}");
    assert!(code.contains("decorate__other(w, "), "mangled other fn:\n{code}");

    // A bare reference to the contested name is ambiguous — refused
    // with both origins named.
    let amb = dir.join("amb.pix");
    std::fs::write(
        &amb,
        "use model\nuse other\n\nfn bad(t: String) String {\n  decorate(t)\n}\n\nview Main {\n  Column {\n    Text { text: \"x\" }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&amb).expect("driver runs");
    assert!(
        outcome
            .diagnostics
            .iter()
            .any(|d| d.message.contains("ambiguous")),
        "diags: {:?}",
        outcome.diagnostics
    );

    // `pub use` re-exports: importers of the face module reach the
    // item bare AND qualified without importing its home.
    std::fs::write(
        dir.join("face.pix"),
        "pub use ui.card.{cardTitle}\n\npub fn faceOnly String {\n  \"f\"\n}\n",
    )
    .unwrap();
    let reexp = dir.join("reexp.pix");
    std::fs::write(
        &reexp,
        "use face\n\nstore S {\n  state out : String = \"\"\n\n  fn go {\n    let a : String = cardTitle(\"x\")\n    let b : String = face.cardTitle(\"x\")\n    out = a + b\n  }\n}\n\nview Main {\n  Column {\n    Text { text: \"#{S.out}\" }\n    Button { text: \"go\"; onClick: S.go() }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&reexp).expect("driver runs");
    assert_eq!(outcome.error_count(), 0, "diags: {:?}", outcome.diagnostics);
    let code = pixie_codegen::emit_program(
        outcome.module.as_ref().expect("module"),
        outcome.binding_items,
        None,
    )
    .expect("emit succeeds");
    assert!(
        code.contains("card_title(w, "),
        "re-exported fn must resolve to its home:\n{code}"
    );
}

/// §12.3: the HTTP battery — worker-pool awaits, `!Bytes` body, and
/// `Map<String, String>` headers crossing the Send boundary via the
/// kernel pair helpers.
#[test]
fn http_demo_emits_the_client_adapters() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/http/http.pix");
    let outcome = pixie_driver::check_file(Path::new(path)).expect("driver runs");
    assert_eq!(outcome.error_count(), 0, "diags: {:?}", outcome.diagnostics);
    let code = pixie_codegen::emit_program(
        outcome.module.as_ref().expect("module"),
        outcome.binding_items,
        None,
    )
    .expect("emit succeeds");
    for needle in [
        // Awaited battery calls ship to workers…
        "pixie_kernel::http::get(",
        "pixie_kernel::http::get_bytes(",
        "pixie_kernel::http::post(",
        "pixie_kernel::http::get_with(",
        // …errors come back as Str, bytes as Bytes.
        ".map_err(|e| e.to_string())",
        ".map(|__v| Bytes::from(__v)).map_err(Str::from)",
        // Map headers cross the thread as plain pairs.
        "pixie_kernel::map_to_send(&(",
        "pixie_kernel::map_from_send(&__args",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`:\n{code}");
    }
}

/// §11.9: a release emission (reload = None) carries NO rung-2
/// machinery — no embedded source path, no fingerprint, no
/// interpreter tables, no interp-tier harness.
#[test]
fn release_emission_strips_rung2() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/counter/counter.pix"
    );
    let outcome = pixie_driver::check_file(Path::new(path)).expect("driver runs");
    assert_eq!(outcome.error_count(), 0);
    let code = pixie_codegen::emit_program(
        outcome.module.as_ref().expect("module"),
        outcome.binding_items,
        None,
    )
    .expect("emit succeeds");
    for forbidden in [
        "__PIXIE_SRC",
        "__PIXIE_FINGERPRINT",
        "__PIXIE_FOREIGN_STYLES",
        "pixie_interp",
        "__pixie_tables",
        "PIXIE_TIER",
    ] {
        assert!(
            !code.contains(forbidden),
            "release emission must not contain `{forbidden}`"
        );
    }
    // The compiled behavior is still whole.
    assert!(code.contains("pixie_engine_gpui::run_app"));
    assert!(code.contains("PIXIE_SCRIPT"));
}

/// Method-body loops (§8.27): `for` over a list hoists the list into
/// `__xs` first (the `w` borrow ends at the `let`, and iterating a
/// temporary would drop it while borrowed), ranges lower to native
/// Rust ranges, `while` re-evaluates its condition each pass, and
/// `break` / `continue` pass through inside a loop.
#[test]
fn method_body_loops_lower() {
    let dir = std::env::temp_dir().join("pixie-loops");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("loops.pix");
    std::fs::write(
        &f,
        "store S {\n  state total : Int = 0\n\n  fn crunch {\n    let xs = [1, 2, 3]\n    var sum = 0\n    for x in xs {\n      if x == 2 {\n        continue\n      }\n      sum += x\n    }\n    for i in 0..3 {\n      sum += i\n    }\n    for i in 1..=2 {\n      sum += i\n    }\n    var n = 2\n    while n > 0 {\n      sum += n\n      n -= 1\n      if sum > 100 {\n        break\n      }\n    }\n    total = sum\n  }\n}\n\nview Main {\n  Column {\n    Text { text: \"#{S.total}\" }\n    Button { text: \"go\"; onClick: S.crunch() }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().unwrap(), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        "{ let __xs = ",
        "for __it in __xs.iter() {",
        "let x = __it.clone();",
        "for i in (0i64)..(3i64) {",
        "for i in (1i64)..=(2i64) {",
        "while (n.clone() > 0i64) {",
        "continue;",
        "break;",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`:\n{code}");
    }

    // `break` outside a loop is a named error, not a rustc surprise.
    let f2 = dir.join("stray_break.pix");
    std::fs::write(
        &f2,
        "store S {\n  state n : Int = 0\n\n  fn go {\n    break\n  }\n}\n\nview Main {\n  Column {\n    Text { text: \"#{S.n}\" }\n  }\n}\n",
    )
    .unwrap();
    let outcome2 = pixie_driver::check_file(&f2).expect("driver runs");
    let has_err = outcome2.error_count() > 0
        || pixie_codegen::emit_program(
            outcome2.module.as_ref().unwrap(),
            outcome2.binding_items,
            None,
        )
        .is_err();
    assert!(has_err, "stray break must not compile");
}

/// Non-exhaustive `case` over a declared enum is a hard typed error
/// (§8.27) — codegen's unmatched tail is a silent no-op, so a missed
/// variant would swallow behavior. Full coverage or `when _` passes.
#[test]
fn enum_case_exhaustiveness_is_an_error() {
    let dir = std::env::temp_dir().join("pixie-exhaustive");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("missing.pix");
    std::fs::write(
        &f,
        "enum Color {\n  Red\n  Green\n  Blue\n}\n\nstore S {\n  state n : Int = 0\n\n  fn pick(c: Color) {\n    case c {\n      when Red { n = 1 }\n      when Green { n = 2 }\n    }\n  }\n}\n\nview Main {\n  Column {\n    Text { text: \"#{S.n}\" }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert!(
        outcome.error_count() > 0,
        "missing variant must be an error: {:?}",
        outcome.diagnostics
    );
    assert!(
        outcome
            .diagnostics
            .iter()
            .any(|d| d.message.contains("missing Blue")),
        "the error names the missing variant: {:?}",
        outcome.diagnostics
    );

    // A `when _` catch-all restores exhaustiveness.
    let f2 = dir.join("wildcard.pix");
    std::fs::write(
        &f2,
        "enum Color {\n  Red\n  Green\n  Blue\n}\n\nstore S {\n  state n : Int = 0\n\n  fn pick(c: Color) {\n    case c {\n      when Red { n = 1 }\n      when _ { n = 0 }\n    }\n  }\n}\n\nview Main {\n  Column {\n    Text { text: \"#{S.n}\" }\n  }\n}\n",
    )
    .unwrap();
    let outcome2 = pixie_driver::check_file(&f2).expect("driver runs");
    assert_eq!(
        outcome2.error_count(),
        0,
        "wildcard case must pass: {:?}",
        outcome2.diagnostics
    );
}

/// View components (§8.29): use sites inline at compile time, params
/// substitute, per-instance state hoists under deterministic names,
/// `Slot` carries use-site children. The emitted crate contains only
/// catalog elements plus the hoisted holder fields.
#[test]
fn components_splice_into_the_root_view() {
    let dir = std::env::temp_dir().join("pixie-components");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("cards.pix");
    std::fs::write(
        &f,
        "view Counter(label: String, step: Int) {\n  state n : Int = 0\n\n  Row {\n    Text { text: \"#{label}: #{n}\" }\n    Button { text: \"+#{step}\"; onClick: { n = n + step } }\n  }\n}\n\nview Card(title: String) {\n  Column {\n    Text { text: title }\n    Slot { }\n  }\n}\n\nview Main {\n  Column {\n    Card {\n      title: \"counters\"\n      Counter { label: \"a\"; step: 1 }\n      Counter { label: \"b\"; step: 10 }\n    }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().unwrap(), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        // Two instances, two independent hoisted holders.
        "__c1___pixie_state",
        "__c2___pixie_state",
        // Param substitution + literal folding into the interp string.
        "format!(\"a: {}\", __c1___pixie_state.n(w))",
        "format!(\"b: {}\", __c2___pixie_state.n(w))",
        // Step params reached the actions.
        "+ 1i64",
        "+ 10i64",
        // Slot content landed inside the Card column.
        "Str::from(\"counters\")",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`:\n{code}");
    }
}

/// The component splice's named errors, one probe each.
#[test]
fn component_splice_errors() {
    let dir = std::env::temp_dir().join("pixie-components-err");
    std::fs::create_dir_all(&dir).unwrap();
    let probes: &[(&str, &str, &str)] = &[
        (
            "cycle.pix",
            "view A {\n  Column {\n    B { }\n  }\n}\n\nview B {\n  Column {\n    A { }\n  }\n}\n\nview Main {\n  Column {\n    A { }\n  }\n}\n",
            "component cycle",
        ),
        (
            "missing.pix",
            "view Chip(label: String) {\n  Text { text: label }\n}\n\nview Main {\n  Column {\n    Chip { }\n  }\n}\n",
            "needs `label:`",
        ),
        (
            "unknown.pix",
            "view Chip(label: String) {\n  Text { text: label }\n}\n\nview Main {\n  Column {\n    Chip { label: \"x\"; extra: 1 }\n  }\n}\n",
            "has no `extra:`",
        ),
        (
            "object_in_repeat.pix",
            "store S {\n  state xs : List<String> = []\n}\n\nclass Counter {\n  pub prop count : Int, default: 0\n}\n\nview Chip {\n  let c = Counter()\n\n  Text { text: \"#{c.count}\" }\n}\n\nview Main {\n  Column {\n    for x in S.xs {\n      Chip { }\n    }\n  }\n}\n",
            "per-row object graphs inside a repeater are M2",
        ),
        (
            "two_slots.pix",
            "view Chip {\n  Column {\n    Slot { }\n    Slot { }\n  }\n}\n\nview Main {\n  Column {\n    Chip {\n      Text { text: \"x\" }\n    }\n  }\n}\n",
            "more than one `Slot`",
        ),
        (
            "no_slot.pix",
            "view Chip {\n  Text { text: \"c\" }\n}\n\nview Main {\n  Column {\n    Chip {\n      Text { text: \"x\" }\n    }\n  }\n}\n",
            "takes no children",
        ),
        (
            "builtin.pix",
            "view Text {\n  Column {\n    Spinner { }\n  }\n}\n\nview Main {\n  Column {\n    Spinner { }\n  }\n}\n",
            "shadows a built-in",
        ),
        (
            "shadow.pix",
            "store S {\n  state xs : List<String> = []\n}\n\nview Chip(label: String) {\n  Column {\n    for label in S.xs {\n      Text { text: label }\n    }\n  }\n}\n\nview Main {\n  Column {\n    Chip { label: \"x\" }\n  }\n}\n",
            "shadows a param",
        ),
    ];
    for (fname, src, needle) in probes {
        let f = dir.join(fname);
        std::fs::write(&f, src).unwrap();
        let outcome = pixie_driver::check_file(&f).expect("driver runs");
        assert!(
            outcome
                .diagnostics
                .iter()
                .any(|d| d.message.contains(needle)),
            "{fname}: expected `{needle}` in {:?}",
            outcome.diagnostics
        );
    }
}

/// Per-row component state (§8.30): a stateful component in a
/// depth-1 repeater hoists a ROW SEAT — the emitted crate carries the
/// seat field, the `prepare` phase sizing it to the driving list, and
/// the `__PixieRowScope` lowering that binds each row's handle.
#[test]
fn per_row_state_lowers_through_a_seat() {
    let dir = std::env::temp_dir().join("pixie-rowstate");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("rows.pix");
    std::fs::write(
        &f,
        "store Board {\n  state names : List<String> = []\n}\n\nview Tally(who: String) {\n  state n : Int = 0\n\n  Row {\n    Text { text: \"#{who}: #{n}\" }\n    Button { text: who; onClick: { n = n + 1 } }\n  }\n}\n\nview Main {\n  Column {\n    for who in Board.names {\n      Tally { who: who }\n    }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().unwrap(), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        "Handle<pixie_kernel::RowSeat<__TallyState>>",
        "fn prepare(&self, w: &mut World)",
        "pixie_kernel::ensure_row_grid(w, self.__c0___pixie_rows, &[__n0],",
        "let __c0___pixie_row = pixie_kernel::row_at(w, __c0___pixie_rows, &[__row_idx0]);",
        "for (__row_idx0, who) in",
        "w.connect(h.erase(), ___TALLY_STATE_N_CHANGED,",
        "(__c0___pixie_rows.erase(), __PIXIE_SEAT_SIG_0)",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`:\n{code}");
    }
}

/// §8.34: the same seat, one level deeper and behind a lazy list.
/// A stateful component under NESTED repeaters keys its seat by the
/// whole index path, so `prepare` sizes one dimension per enclosing
/// `for`; inside a VIRTUALIZED ListView the lazy row closure supplies
/// the innermost index, and `prepare` still sizes the seat from the
/// full list, not the visible range. Both shapes used to be named
/// errors in the component splice.
#[test]
fn nested_and_lazy_per_row_state_lower_through_one_seat() {
    let dir = std::env::temp_dir().join("pixie-rowstate-deep");
    std::fs::create_dir_all(&dir).unwrap();

    let nested = dir.join("nested.pix");
    std::fs::write(
        &nested,
        "store S {\n  state xs : List<String> = []\n  state ys : List<String> = []\n}\n\nview Chip {\n  state n : Int = 0\n\n  Button { text: \"#{n}\"; onClick: { n = n + 1 } }\n}\n\nview Main {\n  Column {\n    for x in S.xs {\n      Row {\n        for y in S.ys {\n          Chip { }\n        }\n      }\n    }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&nested).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "nested diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().unwrap(), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        // One dimension per enclosing repeater, outermost first.
        "let __n0 = w.singleton_ref::<S>().xs(w).len();",
        "let __n1 = w.singleton_ref::<S>().ys(w).len();",
        "pixie_kernel::ensure_row_grid(w, self.__c0___pixie_rows, &[__n0, __n1],",
        "for (__row_idx0, x) in",
        "for (__row_idx1, y) in",
        "pixie_kernel::row_at(w, __c0___pixie_rows, &[__row_idx0, __row_idx1]);",
    ] {
        assert!(code.contains(needle), "nested code lacks `{needle}`:\n{code}");
    }

    let lazy = dir.join("lazy.pix");
    std::fs::write(
        &lazy,
        "store S {\n  state xs : List<String> = []\n}\n\nview Chip {\n  state n : Int = 0\n\n  Button { text: \"#{n}\"; onClick: { n = n + 1 } }\n}\n\nview Main {\n  ListView {\n    virtualized: true\n    itemHeight: 24.0\n    for x in S.xs {\n      Chip { }\n    }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&lazy).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "lazy diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().unwrap(), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        // Sized from the whole list — the lazy range only decides what
        // gets BUILT, never how many seats exist.
        "let __n0 = w.singleton_ref::<S>().xs(w).len();",
        "pixie_kernel::ensure_row_grid(w, self.__c0___pixie_rows, &[__n0],",
        "for __row_idx0 in __range {",
        "pixie_kernel::row_at(w, __c0___pixie_rows, &[__row_idx0]);",
    ] {
        assert!(code.contains(needle), "lazy code lacks `{needle}`:\n{code}");
    }
}

/// §11.23: a class is World-resident, so its name as a TYPE means a
/// `Handle` — in a prop, a parameter, a return, a list element. The
/// emitter used to lay the STRUCT out in all four places, which broke
/// the generated crate; these are the four shapes it must emit now.
#[test]
fn a_class_typed_field_is_a_handle() {
    let dir = std::env::temp_dir().join("pixie-objgraph");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("graph.pix");
    std::fs::write(
        &f,
        "class Leaf {\n  pub prop v : Int, default: 0\n}\n\nclass Owner {\n  pub prop kid : Leaf\n\n  init(k: Leaf) {\n    kid = k\n  }\n}\n\nstore S {\n  state hits : Int = 0\n  state ls : List<Leaf> = []\n\n  fn take(l: Leaf) {\n    hits = l.v\n  }\n\n  fn make Leaf {\n    let x = Leaf()\n    return x\n  }\n\n  fn go {\n    let o = Owner(Leaf())\n    o.kid.v = 7\n    hits = o.kid.v\n  }\n}\n\nview Main {\n  Column { Text { text: \"#{S.hits}\" } }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().unwrap(), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        // The field, the init parameter, and the accessors.
        "kid: Handle<Leaf>",
        "pub fn new(k: Handle<Leaf>)",
        "fn take(self, w: &mut World, l: Handle<Leaf>)",
        "fn make(self, w: &mut World) -> Handle<Leaf>",
        // A list OF objects is a list of handles.
        "ls: List<Handle<Leaf>>",
        // Reading and writing THROUGH the chain go via the World.
        "(o.kid(w)).v(w)",
        "__o.set_v(w, __v);",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`:\n{code}");
    }
}

/// §8.41: a VIEW can hold and read an object graph. Two shapes, both
/// gated until now — a `state` field whose constructor argument is
/// itself an object (nested, or an earlier field, which is how two
/// fields come to SHARE one object), and a `List<Class>` prop read
/// from the view, which needs the interpreter's reflection tables to
/// carry a handle.
#[test]
fn a_view_can_own_and_read_an_object_graph() {
    let dir = std::env::temp_dir().join("pixie-viewgraph");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("g.pix");
    std::fs::write(
        &f,
        "class Leaf {\n  pub prop v : Int, default: 0\n}\n\nclass Owner {\n  pub prop kid : Leaf\n\n  init(k: Leaf) {\n    kid = k\n  }\n}\n\nstore S {\n  state notes : List<Leaf> = []\n}\n\nview Main {\n  let shared = Leaf()\n  let a = Owner(shared)\n  let b = Owner(Leaf())\n\n  Column {\n    Text { text: \"#{a.kid.v}/#{b.kid.v}\" }\n    for n in S.notes {\n      Text { text: \"#{n.v}\" }\n    }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().unwrap(), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        // An earlier field is passed by name — one object, two owners.
        "let a = w.insert(Owner::new(shared));",
        // A nested construction hoists: nesting the two `w.insert`
        // calls would take two mutable borrows of the World at once.
        "let __ctor0 = w.insert(Leaf::new());",
        "let b = w.insert(Owner::new(__ctor0));",
        // Reading through the chain, compiled side. (The interpreted
        // side reads the same graph through the reflection tables,
        // which are only emitted under reload — `examples/graph` in
        // the tier gate is what checks that half, byte for byte.)
        "((a).kid(w)).v(w)",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`:\n{code}");
    }
}

/// §8.42: the escape analysis, checked from BOTH sides. The first
/// half proves a temporary is reclaimed; the second — the one that
/// matters — proves every way an object can get out of a scope
/// suppresses the reclaim. A false reclaim would be a "stale handle"
/// panic at run time, so these are the assertions that make the
/// optimization safe to have.
#[test]
fn escape_analysis_reclaims_only_what_cannot_get_out() {
    let dir = std::env::temp_dir().join("pixie-escape");
    std::fs::create_dir_all(&dir).unwrap();

    let head = "class Leaf {\n  pub prop v : Int, default: 0\n}\n\nclass Box2 {\n  pub prop kid : Leaf\n\n  init(k: Leaf) {\n    kid = k\n  }\n}\n\n";
    let tail = "\n\nview Main {\n  Column { Text { text: \"#{S.hits}\" } }\n}\n";
    let emit = |body: &str| -> String {
        let f = dir.join("e.pix");
        std::fs::write(&f, format!("{head}{body}{tail}")).unwrap();
        let outcome = pixie_driver::check_file(&f).expect("driver runs");
        assert_eq!(
            outcome.error_count(),
            0,
            "diagnostics for `{body}`: {:?}",
            outcome.diagnostics
        );
        pixie_codegen::emit_program(
            outcome.module.as_ref().unwrap(),
            outcome.binding_items,
            None,
        )
        .expect("emit succeeds")
    };

    // Reclaimed: created, written through, never handed anywhere.
    let code = emit(
        "store S {\n  state hits : Int = 0\n\n  fn go {\n    let n = Leaf()\n    n.v = 1\n    hits = n.v\n  }\n}",
    );
    assert!(
        code.contains("let _ = w.remove(n);"),
        "a purely local object was not reclaimed:\n{code}"
    );

    // Reclaimed PER ITERATION — the shape the measured leak had.
    let code = emit(
        "store S {\n  state hits : Int = 0\n\n  fn go {\n    for i in 0..10 {\n      let n = Leaf()\n      n.v = i\n    }\n  }\n}",
    );
    assert!(
        code.contains("let _ = w.remove(n);"),
        "a loop temporary was not reclaimed:\n{code}"
    );

    // Reclaimed even though its property value is handed away: a
    // string built from `n.v` carries the VALUE, not the object.
    // This is the most ordinary thing done with a local object, so
    // treating any mention as an escape made the analysis nearly
    // useless in practice.
    let code = emit(
        "store S {\n  state hits : Int = 0\n  state seen : List<String> = []\n\n  fn go {\n    let n = Leaf()\n    n.v = 3\n    seen.push(\"v=#{n.v}\")\n  }\n}",
    );
    assert!(
        code.contains("let _ = w.remove(n);"),
        "an object whose property value was passed on was not reclaimed:\n{code}"
    );

    // Every one of these lets the object out, and none may reclaim.
    for (why, body) in [
        (
            "returned",
            "store S {\n  state hits : Int = 0\n\n  fn make Leaf {\n    let n = Leaf()\n    return n\n  }\n}",
        ),
        (
            "returned as the trailing expression",
            "store S {\n  state hits : Int = 0\n\n  fn make Leaf {\n    let n = Leaf()\n    n\n  }\n}",
        ),
        (
            "stored into another object",
            "store S {\n  state hits : Int = 0\n\n  fn go {\n    let n = Leaf()\n    let b = Box2(n)\n    hits = b.kid.v\n  }\n}",
        ),
        (
            "assigned into another object's property",
            "store S {\n  state hits : Int = 0\n\n  fn go {\n    let b = Box2(Leaf())\n    let n = Leaf()\n    b.kid = n\n  }\n}",
        ),
        (
            "pushed into a list",
            "store S {\n  state hits : Int = 0\n  state keep : List<Leaf> = []\n\n  fn go {\n    let n = Leaf()\n    keep.push(n)\n  }\n}",
        ),
        (
            "aliased to a second local",
            "store S {\n  state hits : Int = 0\n  state keep : List<Leaf> = []\n\n  fn go {\n    let n = Leaf()\n    let m = n\n    keep.push(m)\n  }\n}",
        ),
        (
            "escaping from inside a loop",
            "store S {\n  state hits : Int = 0\n  state keep : List<Leaf> = []\n\n  fn go {\n    for i in 0..3 {\n      let n = Leaf()\n      keep.push(n)\n    }\n  }\n}",
        ),
        (
            "escaping from inside an if",
            "store S {\n  state hits : Int = 0\n  state keep : List<Leaf> = []\n\n  fn go {\n    let n = Leaf()\n    if hits > 0 {\n      keep.push(n)\n    }\n  }\n}",
        ),
    ] {
        let code = emit(body);
        assert!(
            !code.contains("w.remove(n)"),
            "an object that {why} was reclaimed anyway:\n{code}"
        );
    }
}

/// §8.45: the three limits §8.44 named, closed. A list property has
/// a real `push` — one operation instead of read-modify-write — so
/// filling a list is linear, appending through an OBJECT works at
/// all, and an object list retains only what arrives. And a generic
/// class registers an edge table per instantiation.
#[test]
fn a_list_property_has_a_real_push() {
    let dir = std::env::temp_dir().join("pixie-push");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("p.pix");
    std::fs::write(
        &f,
        "class Node {\n  pub prop v : Int, default: 0\n  pub prop kids : List<Node>, default: []\n}\n\nclass Bag<T> {\n  pub prop items : List<T>, default: []\n}\n\nstore S {\n  state roots : List<Node> = []\n  state ns : List<Int> = []\n\n  fn go {\n    let top = Node()\n    let kid = Node()\n    top.kids.push(kid)\n    roots.push(top)\n    ns.push(1)\n  }\n}\n\nview Main {\n  let bag = Bag<Node>()\n\n  Column { Text { text: \"#{S.ns.length}\" } }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().unwrap(), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        // Taking the list OUT is what makes the push land in place;
        // the old read-modify-write cloned the whole vector each time.
        "let mut __xs = std::mem::take(&mut w.get_mut(self).kids);",
        // An object list retains exactly the arriving element.
        "fn push_kids(self, w: &mut World, v: Handle<Node>) {",
        "w.retain((v).erase());",
        // Appending THROUGH an object — the case that used to be a
        // named error — and through a store property.
        "__h.push_kids(w, __v);",
        "__h.push_roots(w, __v);",
        // A plain value list gets the same one-call shape, no retain.
        "fn push_ns(self, w: &mut World, v: i64) {",
        // The generic class registers its edges per INSTANTIATION.
        "w.register_edges::<Bag<Handle<Node>>>(",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`:\n{code}");
    }
    // A value list must not pay for counting.
    let push_ns = code
        .split("fn push_ns(self, w: &mut World, v: i64) {")
        .nth(1)
        .expect("push_ns exists");
    let body = push_ns.split("    }").next().unwrap();
    assert!(
        !body.contains("retain"),
        "a list of values counts nothing:\n{body}"
    );
}

/// §8.46: a value cannot hold a reference. Copying a `struct` copies
/// its fields, and a copied handle is a second reference to one
/// object — which is exactly what refcounted edges must not have
/// happen behind their back. This used to pass the checker and break
/// rustc inside the generated crate, which D10 says is our bug.
///
/// The sibling positions were already named errors; this pins that
/// none of them regresses into a rustc failure.
#[test]
fn a_value_cannot_hold_a_reference() {
    // §8.68 narrowed this: a `T?` PROPERTY and a `Map` property of
    // class type hold references and are now allowed — `edge_push_expr`
    // and `retain_expr` have walked both shapes since §8.44, and the
    // reflection tables carry them now. What remains here is the
    // rule: a VALUE (a struct field, an enum payload) cannot.
    let dir = std::env::temp_dir().join("pixie-valueref");
    std::fs::create_dir_all(&dir).unwrap();
    let head = "class Leaf {\n  pub prop v : Int, default: 1\n}\n\n";
    let tail = "\n\nstore S {\n  state hits : Int = 0\n}\n\nview Main {\n  Column { Text { text: \"#{S.hits}\" } }\n}\n";
    for (why, decl, needle) in [
        (
            "a struct field",
            "struct Box2 {\n  var l: Leaf\n}",
            "a `struct` holds values",
        ),
        (
            "a struct field holding a LIST of objects",
            "struct Box2 {\n  var ls: List<Leaf>\n}",
            "a `struct` holds values",
        ),
        (
            "an enum payload",
            "enum Slot {\n  empty\n  full(Leaf)\n}",
            "",
        ),
    ] {
        let f = dir.join("v.pix");
        std::fs::write(&f, format!("{head}{decl}{tail}")).unwrap();
        let outcome = pixie_driver::check_file(&f).expect("driver runs");
        let emitted = outcome.module.as_ref().and_then(|m| {
            pixie_codegen::emit_program(m, outcome.binding_items, None).ok()
        });
        assert!(
            outcome.error_count() > 0 || emitted.is_none(),
            "{why} was accepted — it would break the generated crate"
        );
        if !needle.is_empty() {
            let msg = outcome
                .module
                .as_ref()
                .and_then(|m| {
                    pixie_codegen::emit_program(m, outcome.binding_items, None).err()
                })
                .map(|e| e.message)
                .unwrap_or_default();
            assert!(
                msg.contains(needle),
                "{why}: expected `{needle}`, got `{msg}`"
            );
        }
    }

    // A struct METHOD may still take an object: the handle is used
    // during the call and stored nowhere, so nothing is copied into a
    // value that outlives it.
    let f = dir.join("ok.pix");
    std::fs::write(
        &f,
        format!(
            "{head}struct P {{\n  var x: Int\n\n  fn plus(l: Leaf) Int {{\n    self.x\n  }}\n}}{tail}"
        ),
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "a struct method taking an object is fine: {:?}",
        outcome.diagnostics
    );
    assert!(
        pixie_codegen::emit_program(
            outcome.module.as_ref().unwrap(),
            outcome.binding_items,
            None
        )
        .is_ok()
    );
}

/// §8.47: a VIEW is a counted holder, and so is a row seat. Their
/// objects used to be uncounted, which was safe only because nothing
/// could give one of them a second, counted edge — and that rested on
/// an unrelated limit rather than on a decision. These are the tables
/// that make the model right on its own terms.
#[test]
fn a_view_and_its_row_seats_are_counted_holders() {
    let dir = std::env::temp_dir().join("pixie-viewedges");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("v.pix");
    std::fs::write(
        &f,
        "class Tally {\n  pub prop hits : Int, default: 0\n\n  pub fn bump {\n    hits = hits + 1\n  }\n}\n\nstore Bin {\n  state kept : List<Tally> = []\n  state names : List<String> = []\n\n  fn take(t: Tally) {\n    kept.push(t)\n  }\n}\n\nview Row2(who: String) {\n  state n : Int = 0\n\n  Button {\n    text: who\n    onClick: {\n      n = n + 1\n    }\n  }\n}\n\nview Main {\n  let mine = Tally()\n\n  Column {\n    Button { text: \"stash\"; onClick: Bin.take(mine) }\n    Button { text: \"bump\"; onClick: mine.bump() }\n    for w in Bin.names {\n      Row2 { who: w }\n    }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().unwrap(), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        // The view's fields are edges, installed before it is built.
        "w.register_edges::<MainView>(",
        "__pixie_register_view_edges(&mut w);",
        // ... and so are the rows a seat holds, at any depth.
        "w.register_edges::<pixie_kernel::RowSeat<",
        ".edges());",
        // The thing that used to be unwritable: an action naming a
        // state object and handing it to a store method.
        // Handler call arguments hoist now (§8.53), so the call
        // reads `{ let __a0 = mine; ....take(w, __a0) }`.
        "let __a0 = mine;",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`:\n{code}");
    }
}

/// §8.49: one trait abstracts over BOTH halves of the type system.
/// A value implementing a declared trait used to splice its methods
/// in as inherent ones and emit no `impl Trait for P` block, so it
/// could not satisfy a bound — the checker accepted it and rustc
/// rejected the generated crate.
#[test]
fn a_trait_covers_objects_and_values_alike() {
    let dir = std::env::temp_dir().join("pixie-traits");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("t.pix");
    std::fs::write(
        &f,
        "trait Labeled {\n  fn tag String\n}\n\nclass Dog {\n  pub prop n : String, default: \"rex\"\n}\n\nstruct Tag2 {\n  var s: String\n\n  fn shout String {\n    self.s\n  }\n}\n\nimpl Labeled for Dog {\n  fn tag String {\n    \"dog:#{n}\"\n  }\n}\n\nimpl Labeled for Tag2 {\n  fn tag String {\n    \"tag:#{self.s}\"\n  }\n}\n\nfn describe<T: Labeled>(v: T) String {\n  \"<#{v.tag()}>\"\n}\n\nstore S {\n  state out : String = \"\"\n\n  fn go {\n    let d = Dog()\n    let t = Tag2(\"hi\")\n    out = \"#{describe(d)}#{describe(t)}#{t.shout()}\"\n  }\n}\n\nview Main {\n  Column { Text { text: S.out } }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().unwrap(), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        // A handle is Copy and a value is not, so the supertrait has
        // to be the one both satisfy.
        "pub trait Labeled: Clone {",
        // Both halves get a real impl block, so both satisfy a bound.
        "impl Labeled for Handle<Dog> {",
        "impl Labeled for Tag2 {",
        // The value's impl takes the World and ignores it — that is
        // what lets ONE signature serve both.
        "fn tag(self, _w: &mut World) -> Str {",
        // A trait impl does not swallow the value's own methods.
        "pub fn shout(&self) -> Str {",
        // One generic function, monomorphized by rustc for each.
        "fn describe<T: Labeled + Clone>(w: &mut World, v: T) -> Str",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`:\n{code}");
    }
}

/// §8.53: a handler body is a method body written at the use site,
/// so the statements are the same ones. Control flow, locals that
/// hold objects, construction — each was an `(M1)` error whose
/// reason nobody had written down, which is a different thing from a
/// constraint.
#[test]
fn a_handler_body_is_a_method_body() {
    let dir = std::env::temp_dir().join("pixie-handler");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("h.pix");
    std::fs::write(
        &f,
        "class C {\n  pub prop n : Int, default: 0\n\n  pub fn bump {\n    n = n + 1\n  }\n}\n\nstore S {\n  state hits : Int = 0\n\n  fn add(x: Int) {\n    hits = hits + x\n  }\n\n  fn take(c: C) {\n    hits = hits + c.n\n  }\n}\n\nview Main {\n  Column {\n    Text { text: \"#{S.hits}\" }\n    Button {\n      text: \"go\"\n      onClick: {\n        var i = 0\n        while i < 3 {\n          i = i + 1\n          if i > 2 {\n            break\n          }\n        }\n        for k in 0..2 {\n          S.add(k)\n        }\n        let c = C()\n        c.n = 5\n        c.bump()\n        S.take(c)\n      }\n    }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().unwrap(), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        // Control flow, and a local the handler assigns to.
        "while (i.clone() < 3i64) {",
        "for k in (0i64)..(2i64) {",
        "break;",
        "i = (i.clone() + 1i64);",
        // A local that holds an OBJECT: constructed, written through,
        // called on, and handed to a method.
        "let c = w.insert(C::new());",
        "let __o = c.clone(); let __v = 5i64; __o.set_n(w, __v);",
        "(c.clone()).bump(w);",
        "let __a0 = c.clone();",
        // (The interpreted tier's constructor table only emits under
        // reload, so the demo in the tier gate is what checks that
        // half — byte for byte against this one.)
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`:\n{code}");
    }
}

/// A view body genuinely cannot call a method, and now says why.
/// `build` takes `&World` so a rebuild cannot change what it rebuilds
/// from; a class method takes `&mut World`. That is a design
/// constraint, and the message names it and the two ways around.
#[test]
fn a_view_body_explains_why_it_cannot_call_a_method() {
    let dir = std::env::temp_dir().join("pixie-viewcall");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("v.pix");
    std::fs::write(
        &f,
        "store S {\n  state n : Int = 0\n\n  pub fn label String {\n    \"n=#{n}\"\n  }\n}\n\nview Main {\n  Column { Text { text: S.label() } }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    let msg = pixie_codegen::emit_program(
        outcome.module.as_ref().unwrap(),
        outcome.binding_items,
        None,
    )
    .err()
    .map(|e| e.message)
    .unwrap_or_default();
    assert!(msg.contains("only READS the World"), "message was `{msg}`");
    assert!(
        !msg.contains("not lowerable"),
        "a design constraint should not read as a missing feature: `{msg}`"
    );
}

/// §8.54: three `(M0)` limits that were gaps in the lowerer rather
/// than rules about the language — arithmetic inside a view's
/// interpolation, format specs, and `static fn`.
#[test]
fn interpolation_formats_and_static_fns() {
    let dir = std::env::temp_dir().join("pixie-m0");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("m.pix");
    std::fs::write(
        &f,
        "class Temp {\n  pub prop c : Float, default: -1.0\n\n  pub static fn fromF(f: Float) Float {\n    (f - 32.0) * 5.0 / 9.0\n  }\n}\n\nstore S {\n  state n : Int = 7\n  state v : Float = 3.14159\n  state out : Float = 0.0\n\n  fn go {\n    out = Temp.fromF(212.0)\n  }\n}\n\nview Main {\n  Column {\n    Text { text: \"#{S.n * 2} of #{S.n + 1}\" }\n    Text { text: \"#{S.v:.2f} #{S.n:>6} #{S.n:04}\" }\n    Button { text: \"go\"; onClick: S.go() }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().unwrap(), outcome.binding_items, None)
            .expect("emit succeeds");
    for needle in [
        // Arithmetic reaches the format arguments.
        "* 2i64)",
        "+ 1i64)",
        // The spec rides through to `format!`, minus the type letter.
        "{:.2}",
        "{:>6}",
        "{:04}",
        // An associated fn: no receiver, no World.
        "pub fn from_f(f: f64) -> f64 {",
        "Temp::from_f(212f64)",
        // And a negative constant default, which used to be an error.
        "c: (-1f64),",
    ] {
        assert!(code.contains(needle), "generated code lacks `{needle}`:\n{code}");
    }
}

/// A bad format spec is a pixie error naming the spec, not a
/// `format!` that fails inside the generated crate (D10).
#[test]
fn a_bad_format_spec_is_a_named_error() {
    let dir = std::env::temp_dir().join("pixie-badspec");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("b.pix");
    std::fs::write(
        &f,
        "store S {\n  state v : Float = 3.0\n}\n\nview Main {\n  Column { Text { text: \"#{S.v:zz}\" } }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    let msg = pixie_codegen::emit_program(
        outcome.module.as_ref().unwrap(),
        outcome.binding_items,
        None,
    )
    .err()
    .map(|e| e.message)
    .unwrap_or_default();
    assert!(
        msg.contains("`zz` is not a format spec"),
        "message was `{msg}`"
    );
}

/// Cross-module components (§8.29): qualified, aliased-selective, and
/// private-sibling references expand from an imported module; the
/// guard rails answer with named errors.
#[test]
fn cross_module_components() {
    let dir = std::env::temp_dir().join("pixie-xmod-comp");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("ui.pix"),
        "pub view Chip(label: String) {\n  state hits : Int = 0\n\n  Row {\n    Text { text: \"#{label}/#{hits}\" }\n    Button { text: label; onClick: { hits = hits + 1 } }\n  }\n}\n\nview Frame {\n  Column {\n    Slot { }\n  }\n}\n\npub view Badge(tag: String) {\n  Frame {\n    Text { text: \"[#{tag}]\" }\n  }\n}\n\nview Hidden {\n  Text { text: \"no\" }\n}\n",
    )
    .unwrap();
    let f = dir.join("main.pix");
    std::fs::write(
        &f,
        "use ui\nuse ui.{Chip as Pin}\n\nview Main {\n  Column {\n    Pin { label: \"a\" }\n    ui.Chip { label: \"b\" }\n    ui.Badge { tag: \"ok\" }\n  }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code =
        pixie_codegen::emit_program(outcome.module.as_ref().unwrap(), outcome.binding_items, None)
            .expect("emit succeeds");
    // Two independent foreign stateful instances + the private
    // sibling's body all landed in the entry view.
    for needle in ["__c0___pixie_state", "__c1___pixie_state", "[ok]"] {
        assert!(code.contains(needle), "generated code lacks `{needle}`:\n{code}");
    }

    // The baked foreign snippet keeps `pub` (the decl span starts at
    // `view`) and reload-splices cleanly — the rung-2 leg of §8.29.
    // The reload reads the import from disk now (§8.72) and hands the
    // views over parsed, so the check is on what it produced rather
    // than on a source snippet.
    let foreign = pixie_interp::foreign_reload_from_paths(&outcome.foreign_paths);
    let ui = &foreign
        .views
        .iter()
        .find(|(n, _)| n == "ui")
        .expect("the import's views")
        .1;
    assert!(
        ui.iter().any(|v| v.name.name == "Chip" && v.is_pub),
        "the exported component must cross: {ui:?}"
    );
    let entry_src = std::fs::read_to_string(&f).unwrap();
    pixie_interp::reload_from_source_with(&entry_src, &foreign)
        .expect("the reload path splices foreign components");

    // Guard rails.
    let probes: &[(&str, &str, &str)] = &[
        (
            "private.pix",
            "use ui\n\nview Main {\n  Column {\n    ui.Hidden { }\n  }\n}\n",
            "is private",
        ),
        (
            "noimport.pix",
            "view Main {\n  Column {\n    nowhere.Chip { }\n  }\n}\n",
            "not an imported module",
        ),
        (
            "nomember.pix",
            "use ui\n\nview Main {\n  Column {\n    ui.Nope { }\n  }\n}\n",
            "has no `view Nope`",
        ),
    ];
    for (fname, src, needle) in probes {
        let pf = dir.join(fname);
        std::fs::write(&pf, src).unwrap();
        let outcome = pixie_driver::check_file(&pf).expect("driver runs");
        assert!(
            outcome
                .diagnostics
                .iter()
                .any(|d| d.message.contains(needle)),
            "{fname}: expected `{needle}` in {:?}",
            outcome.diagnostics
        );
    }
}

#[test]
fn int_widens_to_float_wherever_the_checker_widens_it() {
    // §8.55. The checker has always accepted an Int where a Float is
    // expected — `is_subtype` widens. The emitter has no types, so it
    // emitted `f64 * i64` and rustc rejected generated code, which is
    // a compiler bug by D10. The checker now records which operand it
    // widened and the emitter casts there.
    let dir = std::env::temp_dir().join("pixie-m0-gate");
    std::fs::create_dir_all(&dir).unwrap();

    let f = dir.join("widen.pix");
    std::fs::write(
        &f,
        concat!(
            "fn scale(v: Float) Float { v * 2.0 }\n",
            "\n",
            "store S {\n",
            "  state n : Int = 3\n",
            "  state out : Float = 0\n",
            "  state half : Float = 0.0\n",
            "\n",
            "  fn go {\n",
            "    out = 30.0 * n\n",
            "    half = n / 2.0\n",
            "    out += n\n",
            "    let k : Float = n\n",
            "    half += k\n",
            "    out = scale(n)\n",
            "  }\n",
            "}\n",
            "\n",
            "view Main {\n",
            "  Column {\n",
            "    Button { text: \"go\"; onClick: S.go() }\n",
            "    Text { text: \"#{S.out}\"; fontSize: 14 }\n",
            "    Spinner { size: 20 }\n",
            "  }\n",
            "}\n",
        ),
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "mixed arithmetic must check clean: {:?}",
        outcome.diagnostics
    );
    let code = pixie_codegen::emit_program_with(
        outcome.module.as_ref().unwrap(),
        outcome.binding_items,
        None,
        &outcome.check_info,
    )
    .expect("mixed arithmetic emits");

    // Every widened site casts. `as f64` on an already-f64 operand
    // would compile, so these assertions check the *count* of casts
    // indirectly by naming each shape.
    for needle in [
        // binary operand
        "(30f64 * ((self.n(w)) as f64))",
        // the other side of the operator
        "(((self.n(w)) as f64) / 2f64)",
        // compound assignment
        "self.out(w) + ((self.n(w)) as f64)",
        // an annotated `let`
        "let k: f64 = ((self.n(w)) as f64)",
        // a call argument
        "let __a0 = ((self.n(w)) as f64); scale(w, __a0)",
        // an Int default in a Float prop
        "out: 0f64",
        // an Int literal in a Float view slot
        "font_size: 14f64",
        "Element::Spinner { size: 20f64 }",
    ] {
        assert!(code.contains(needle), "missing `{needle}`:\n{code}");
    }
}

#[test]
fn for_bodies_and_if_branches_hold_a_run_of_items() {
    // §8.56. `for` bodies and `if` branches held exactly one element,
    // which was the parser building a one-element Block and both
    // lowerers insisting on it. Neither was a rule about views: a
    // container's body already holds a run of children, repeaters and
    // conditionals, and these are the same run.
    let dir = std::env::temp_dir().join("pixie-m0-gate");
    std::fs::create_dir_all(&dir).unwrap();

    let f = dir.join("runs.pix");
    std::fs::write(
        &f,
        concat!(
            "store S {\n",
            "  state names : List<String> = []\n",
            "  state on : Bool = true\n",
            "}\n",
            "\n",
            "view Main {\n",
            "  Column {\n",
            "    for n in S.names {\n",
            "      Text { text: n }\n",
            "      Text { text: \"-\" }\n",
            "      if S.on {\n",
            "        Text { text: \"on\" }\n",
            "      }\n",
            "    }\n",
            "    if S.on {\n",
            "      Text { text: \"a\" }\n",
            "      Text { text: \"b\" }\n",
            "    } else {\n",
            "      for n in S.names {\n",
            "        Text { text: n }\n",
            "      }\n",
            "    }\n",
            "  }\n",
            "}\n",
        ),
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code = pixie_codegen::emit_program(
        outcome.module.as_ref().unwrap(),
        outcome.binding_items,
        None,
    )
    .expect("a run of items emits");

    // Three pushes inside one loop body, and a nested `for` inside the
    // `else` — the repeater's list binding is named by repeater depth,
    // so the inner one does not shadow the outer.
    assert_eq!(code.matches("__c0.push(Element::Text").count(), 6, "{code}");
    assert!(code.contains("__xs0"), "{code}");

    // A VIRTUALIZED list is the one place the rule survives, because
    // a lazy row IS one element.
    let f = dir.join("runs_virtual.pix");
    std::fs::write(
        &f,
        concat!(
            "store S {\n",
            "  state names : List<String> = []\n",
            "}\n",
            "\n",
            "view Main {\n",
            "  Column {\n",
            "    ListView {\n",
            "      virtualized: true\n",
            "      itemHeight: 20\n",
            "      for n in S.names {\n",
            "        Text { text: n }\n",
            "        Text { text: \"-\" }\n",
            "      }\n",
            "    }\n",
            "  }\n",
            "}\n",
        ),
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    let err = pixie_codegen::emit_program(
        outcome.module.as_ref().unwrap(),
        outcome.binding_items,
        None,
    )
    .expect_err("a virtualized row must be one element");
    assert!(
        err.message.contains("one element per row"),
        "error should say why: {}",
        err.message
    );
}

#[test]
fn plain_let_and_var_fields_are_class_state() {
    // §8.58. `let x : T` / `var x : T` on a class said "plain fields
    // are not lowerable yet (M0); use `prop`" — but the parser, the
    // AST and the type table all knew them, and `let` carries
    // something `prop` cannot: init-once.
    let dir = std::env::temp_dir().join("pixie-m0-gate");
    std::fs::create_dir_all(&dir).unwrap();

    let src = |body: &str| {
        format!(
            "class Node {{\n  pub let id : Int\n  pub var hits : Int = 0\n\n  init(n: Int) {{\n    id = n\n  }}\n\n{body}}}\n\nview Main {{\n  let n = Node(5)\n  Column {{\n    Text {{ text: \"#{{n.id}} #{{n.hits}}\" }}\n  }}\n}}\n"
        )
    };
    let f = dir.join("fields.pix");
    std::fs::write(&f, src("  pub fn bump {\n    hits = hits + 1\n  }\n")).unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code = pixie_codegen::emit_program(
        outcome.module.as_ref().unwrap(),
        outcome.binding_items,
        None,
    )
    .expect("plain fields emit");

    // A field carries the same reactive machinery a prop does, so a
    // view bound to one still rebuilds.
    assert!(code.contains("pub const NODE_HITS_CHANGED: SignalId ="), "{code}");
    assert!(code.contains("pub const NODE_ID_CHANGED: SignalId ="), "{code}");
    assert!(code.contains("fn set_hits(self, w: &mut World, v: i64)"), "{code}");
    // `let` is init-once, so no setter is emitted for it at all —
    // `init` writes the struct field directly.
    assert!(!code.contains("fn set_id"), "{code}");

    // Writing a `let` field outside `init` is a named error, not a
    // rustc one.
    let f = dir.join("fields_write_let.pix");
    std::fs::write(&f, src("  pub fn bump {\n    id = id + 1\n  }\n")).unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    let err = pixie_codegen::emit_program(
        outcome.module.as_ref().unwrap(),
        outcome.binding_items,
        None,
    )
    .expect_err("a `let` field takes no write");
    assert!(
        err.message.contains("`let` field") && err.message.contains("var id"),
        "error should name the fix: {}",
        err.message
    );
}

#[test]
fn constant_interpolated_defaults_and_in_place_interp_spans() {
    // §8.59, two halves of one probe.
    //
    // A `default:` runs before the object exists, so it has to be
    // constant — but an interpolation OVER constants is constant, and
    // `default: "v#{MAJOR}.#{MINOR}"` said "prop defaults must be
    // literals (M0)".
    //
    // Finding that turned up the larger bug: an interpolation's
    // expression is re-parsed from its own slice, so every span
    // inside `#{ .. }` pointed at byte 0 of the file. Every
    // diagnostic in every interpolation, not just this one.
    let dir = std::env::temp_dir().join("pixie-m0-gate");
    std::fs::create_dir_all(&dir).unwrap();

    let f = dir.join("interp_default.pix");
    std::fs::write(
        &f,
        "store S {\n  state tag : String = \"v#{1}.#{2 * 3} #{true} #{1.5:.2f}\"\n}\n\nview Main {\n  Column { Text { text: S.tag } }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    let code = pixie_codegen::emit_program(
        outcome.module.as_ref().unwrap(),
        outcome.binding_items,
        None,
    )
    .expect("a constant interpolation is a constant");
    assert!(
        code.contains(r#"tag: Str::from(format!("v{}.{} {} {}", 1i64, (2i64 * 3i64), true, { let __f = 1.5f64; if __f.is_nan() { "nan".to_string() } else { format!("{:.2}", __f) } }))"#),
        "{code}"
    );

    // A piece that reads the World is not constant, and the error
    // points AT THE PIECE.
    let f = dir.join("interp_default_bad.pix");
    let src = "store S {\n  state n : Int = 1\n  state bad : String = \"x#{n}\"\n}\n\nview Main {\n  Column { Text { text: S.bad } }\n}\n";
    std::fs::write(&f, src).unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    let err = pixie_codegen::emit_program(
        outcome.module.as_ref().unwrap(),
        outcome.binding_items,
        None,
    )
    .expect_err("a World read is not a constant");
    assert!(err.message.contains("constant"), "{}", err.message);
    // The span covers exactly the `n` inside `#{n}` — the whole point
    // of the second half. Byte 0 would be the old behavior.
    let at = &src[err.span.range()];
    assert_eq!(at, "n", "span should cover the interpolated piece, got {at:?}");
}

#[test]
fn derived_props_compute_and_the_retired_modifiers_say_why() {
    // §8.61. `bindable / fresh / model / constant props are not
    // lowerable yet (M0)` covered four different things. One of them
    // — `bind { .. }` — is a real language feature; the other three
    // are Qt storage decisions with a pixie spelling that does the
    // job, so they get a message instead of a lowering.
    let dir = std::env::temp_dir().join("pixie-m0-gate");
    std::fs::create_dir_all(&dir).unwrap();

    let f = dir.join("derived.pix");
    std::fs::write(
        &f,
        concat!(
            "class Cart {\n",
            "  pub prop unit : Float, default: 2.5\n",
            "  pub prop qty : Int, default: 2\n",
            "  pub prop total : Float, bind { unit * qty }\n",
            "  pub prop label : String, bind { \"#{qty} @ #{total:.2f}\" }\n",
            "}\n",
            "\n",
            "view Main {\n",
            "  let c = Cart()\n",
            "  Column { Text { text: c.label } }\n",
            "}\n",
        ),
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code = pixie_codegen::emit_program_with(
        outcome.module.as_ref().unwrap(),
        outcome.binding_items,
        None,
        &outcome.check_info,
    )
    .expect("a derived property emits");

    // Nothing is stored: the struct has the two real fields only.
    assert!(code.contains("pub struct Cart {\n    unit: f64,\n    qty: i64,\n}"), "{code}");
    // The getter computes, and the §8.55 widening reaches it — which
    // it only does because the checker now VISITS the body.
    assert!(
        code.contains("(self.unit(w) * ((self.qty(w)) as f64))"),
        "{code}"
    );
    // A derivation may read another derivation.
    assert!(code.contains(r#"format!("{} @ {}", self.qty(w), { let __f = self.total(w); if __f.is_nan() { "nan".to_string() } else { format!("{:.2}", __f) } })"#), "{code}");
    // No setter, and no signal of its own — it changes exactly when
    // what it reads changes, and that already fires.
    assert!(!code.contains("fn set_total"), "{code}");
    assert!(!code.contains("CART_TOTAL_CHANGED"), "{code}");
    assert!(code.contains("CART_QTY_CHANGED"), "{code}");

    // The three retired modifiers each name their pixie spelling.
    for (src, needle) in [
        (
            "class C {\n  pub prop n : Int, bindable, default: 1\n}\n",
            "one reactive loop",
        ),
        (
            "class C {\n  pub prop n : Int, constant, default: 1\n}\n",
            "pub let n",
        ),
        (
            "class C {\n  pub prop n : Int, default: 1\n  pub prop d : Int, fresh { n * 2 }\n}\n",
            "one spelling",
        ),
    ] {
        let f = dir.join("retired.pix");
        std::fs::write(
            &f,
            format!("{src}\nview Main {{ Column {{ Text {{ text: \"x\" }} }} }}\n"),
        )
        .unwrap();
        let outcome = pixie_driver::check_file(&f).expect("driver runs");
        let err = pixie_codegen::emit_program(
            outcome.module.as_ref().unwrap(),
            outcome.binding_items,
            None,
        )
        .expect_err("a retired modifier must not emit");
        assert!(err.message.contains(needle), "missing `{needle}`: {}", err.message);
    }
}

#[test]
fn inherited_syntax_says_what_pixie_does_instead() {
    // §8.62. Four `(M0)`/`(M1)` messages on constructs pixie inherited
    // from cute and never designed for. One is a real feature
    // (`return` in a handler); three are things pixie deliberately
    // does differently, and each now says what to write instead of
    // promising a lowering that would add a second way to say
    // something the language already says.
    let dir = std::env::temp_dir().join("pixie-m0-gate");
    std::fs::create_dir_all(&dir).unwrap();

    let emit = |name: &str, src: &str| -> Result<String, String> {
        let f = dir.join(name);
        std::fs::write(&f, src).unwrap();
        let outcome = pixie_driver::check_file(&f).expect("driver runs");
        pixie_codegen::emit_program(
            outcome.module.as_ref().unwrap(),
            outcome.binding_items,
            None,
        )
        .map_err(|e| e.message)
    };

    // `return` in a handler: an early exit, and it leaves the whole
    // handler rather than just an enclosing loop.
    let code = emit(
        "handler_return.pix",
        concat!(
            "store S {\n  state hits : Int = 0\n  state stop : Int = 3\n}\n\n",
            "view Main {\n",
            "  Column {\n",
            "    Button { text: \"scan\"; onClick: {\n",
            "      for i in 0..10 {\n",
            "        if i == S.stop { return }\n",
            "        S.hits = S.hits + 1\n",
            "      }\n",
            "      S.hits = S.hits + 100\n",
            "    } }\n",
            "  }\n",
            "}\n",
        ),
    )
    .expect("a bare return emits");
    assert!(code.contains("return;"), "{code}");

    // A value has nowhere to go.
    let err = emit(
        "handler_return_value.pix",
        "store S {\n  state n : Int = 0\n}\n\nview Main {\n  Column {\n    Button { text: \"x\"; onClick: { return 1 } }\n  }\n}\n",
    )
    .expect_err("a returned value must not emit");
    assert!(err.contains("returns nothing"), "{err}");

    // A block-taking call — pixie has no block-passing at all, so the
    // message covers both readings: iterating, and grouping
    // statements (`batch { .. }` is that second one, and stopped
    // being a keyword in §8.71).
    let err = emit(
        "block_call.pix",
        "store S {\n  state xs : List<Int> = []\n  state n : Int = 0\n\n  fn go {\n    xs.each { n = n + 1 }\n  }\n}\n\nview Main {\n  Column {\n    Button { text: \"go\"; onClick: S.go() }\n  }\n}\n",
    )
    .expect_err("a block call must not emit");
    assert!(err.contains("`each`") && err.contains("for v in"), "{err}");
    assert!(err.contains("already run together"), "{err}");
}

#[test]
fn this_is_the_receiver() {
    // §8.63. `this` reached codegen as an ordinary identifier that
    // resolved to nothing — no crate knew the name. It is the object
    // whose method is running, which is what `self` already is in the
    // emitted code.
    let dir = std::env::temp_dir().join("pixie-m0-gate");
    std::fs::create_dir_all(&dir).unwrap();

    let emit = |name: &str, src: &str| -> Result<String, String> {
        let f = dir.join(name);
        std::fs::write(&f, src).unwrap();
        let outcome = pixie_driver::check_file(&f).expect("driver runs");
        if outcome.error_count() > 0 {
            return Err(format!("{:?}", outcome.diagnostics));
        }
        pixie_codegen::emit_program(
            outcome.module.as_ref().unwrap(),
            outcome.binding_items,
            None,
        )
        .map_err(|e| e.message)
    };

    let code = emit(
        "this.pix",
        concat!(
            "class Node {\n",
            "  pub prop name : String\n",
            "  pub prop kids : List<Node>, default: []\n",
            "  init(n: String) { name = n }\n",
            "\n",
            "  pub fn adopt(k: Node) {\n",
            "    kids.push(k)\n",
            "    k.attach(this)\n",
            "  }\n",
            "\n",
            "  pub fn attach(p: Node) { name = p.name }\n",
            "  pub fn me Node { this }\n",
            "}\n",
            "\n",
            "store S {\n",
            "  state kept : List<Node> = []\n",
            "  fn go {\n",
            "    for n in kept {\n",
            "      n.attach(n.me())\n",
            "    }\n",
            "  }\n",
            "}\n",
            "\n",
            "view Main {\n  Column { Text { text: \"x\" } }\n}\n",
        ),
    )
    .expect("`this` emits");
    // As an argument, and as a return.
    assert!(code.contains("let __a0 = self; k.attach(w, __a0)"), "{code}");
    assert!(code.contains("fn me(self, w: &mut World) -> Handle<Node> {"), "{code}");
    // A method call on a `for` ROW threads the World — the loop
    // variable carries its element class now, which it did not before.
    assert!(code.contains("n.attach(w, __a0)"), "{code}");

    // `init` runs before the object exists.
    let err = emit(
        "this_init.pix",
        "class C {\n  pub prop n : Int, default: 0\n  pub prop me : List<C>, default: []\n  init(k: Int) { n = k; me = [this] }\n}\n\nview Main { Column { Text { text: \"x\" } } }\n",
    )
    .expect_err("`this` in init must not emit");
    assert!(err.contains("no `this` yet"), "{err}");

    // And `Self` is not a pixie type — it happened to mean the right
    // thing inside the emitted trait impl while every USE of the
    // result failed to type-check.
    let err = emit(
        "this_self.pix",
        "class C {\n  pub prop n : Int, default: 0\n  pub fn me Self { this }\n}\n\nview Main { Column { Text { text: \"x\" } } }\n",
    )
    .expect_err("`Self` must not emit");
    assert!(err.contains("name the class"), "{err}");
}

#[test]
fn a_store_can_own_an_object() {
    // §8.64. `state root : Node = Node("r")` reported the constant
    // rule, which was true of `C::new()` and not of the language: a
    // store's fields are initialized where the World exists.
    let dir = std::env::temp_dir().join("pixie-m0-gate");
    std::fs::create_dir_all(&dir).unwrap();

    let f = dir.join("owned.pix");
    std::fs::write(
        &f,
        concat!(
            "class Node {\n",
            "  pub prop name : String\n",
            "  pub prop n : Int, default: 0\n",
            "  init(s: String, k: Int) { name = s; n = k }\n",
            "}\n",
            "\n",
            "store S {\n",
            "  state root : Node = Node(\"r\", 3)\n",
            "  state out : String = \"\"\n",
            "  fn go { out = root.name }\n",
            "}\n",
            "\n",
            "view Main {\n",
            "  Column { Text { text: \"#{S.root.name}:#{S.root.n}\" } }\n",
            "}\n",
        ),
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code = pixie_codegen::emit_program(
        outcome.module.as_ref().unwrap(),
        outcome.binding_items,
        None,
    )
    .expect("an owned object emits");

    // The slot starts empty, because `new()` has no World...
    assert!(code.contains("root: Handle::<Node>::PENDING"), "{code}");
    // ...and `main` fills it through the ordinary setter, which counts
    // the edge.
    assert!(
        code.contains(r#"let __o = w.insert(Node::new(Str::from("r"), 3i64)); __g0.set_root(&mut w, __o);"#),
        "{code}"
    );
    // A view reads THROUGH it: the chain used to stop at the store,
    // because only view fields counted as objects.
    assert!(
        code.contains("singleton_ref::<S>()).root(w)).name(w)"),
        "{code}"
    );
}

#[test]
fn a_repeater_iterates_any_list_it_can_reach() {
    // §8.65. `for` in a view took a list off a view field or a store
    // and nothing else, which meant a row could not repeat over its
    // OWN list — the shape a table is.
    let dir = std::env::temp_dir().join("pixie-m0-gate");
    std::fs::create_dir_all(&dir).unwrap();

    let f = dir.join("reach.pix");
    std::fs::write(
        &f,
        concat!(
            "class Tag { pub prop items : List<String>, default: [] }\n",
            "class Row {\n",
            "  pub prop name : String\n",
            "  pub prop tag : Tag\n",
            "  init(n: String, t: Tag) { name = n; tag = t }\n",
            "}\n",
            "\n",
            "store S {\n",
            "  state rows : List<Row> = []\n",
            "  state hub : Tag = Tag()\n",
            "}\n",
            "\n",
            "view Main {\n",
            "  Column {\n",
            "    for h in S.hub.items {\n",
            "      Text { text: h }\n",
            "    }\n",
            "    for r in S.rows {\n",
            "      Text { text: r.name }\n",
            "      for c in r.tag.items {\n",
            "        Text { text: c }\n",
            "      }\n",
            "    }\n",
            "  }\n",
            "}\n",
        ),
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code = pixie_codegen::emit_program(
        outcome.module.as_ref().unwrap(),
        outcome.binding_items,
        None,
    )
    .expect("a reached list emits");

    // A chain rooted at a store...
    assert!(
        code.contains("let __xs0 = ((w.singleton_ref::<S>()).hub(w)).items(w);"),
        "{code}"
    );
    // ...and one rooted at the enclosing loop variable, two hops in.
    assert!(code.contains("let __xs1 = ((r).tag(w)).items(w);"), "{code}");
}

#[test]
fn writing_through_an_index_and_reserved_type_names() {
    // §8.67, two things a demo turned up.
    //
    // `xs[0] = 5` said "only plain-name assignment is lowerable yet
    // (M0)" while `xs[0]` had read since §8.38. And a class named
    // `Box` emitted `pub struct Box`, which shadows the real one and
    // fails hundreds of lines away inside machinery the author never
    // wrote.
    let dir = std::env::temp_dir().join("pixie-m0-gate");
    std::fs::create_dir_all(&dir).unwrap();

    let emit = |name: &str, src: &str| -> Result<String, String> {
        let f = dir.join(name);
        std::fs::write(&f, src).unwrap();
        let outcome = pixie_driver::check_file(&f).expect("driver runs");
        if outcome.error_count() > 0 {
            return Err(format!("{:?}", outcome.diagnostics));
        }
        pixie_codegen::emit_program(
            outcome.module.as_ref().unwrap(),
            outcome.binding_items,
            None,
        )
        .map_err(|e| e.message)
    };

    let code = emit(
        "index_write.pix",
        concat!(
            "class Crate { pub prop xs : List<Int>, default: [] }\n",
            "\n",
            "store S {\n",
            "  state xs : List<Int> = [1, 2, 3]\n",
            "  state b : Crate = Crate()\n",
            "  fn go {\n",
            "    xs[0] = 5\n",
            "    b.xs[1] = 7\n",
            "  }\n",
            "}\n",
            "\n",
            "view Main { Column { Button { text: \"go\"; onClick: S.go() } } }\n",
        ),
    )
    .expect("an index write emits");
    // The list comes OUT of the object before the write, so the
    // copy-on-write finds a single owner — the `push` stereotype.
    assert!(
        code.contains("let mut __xs = __h.xs(w); __xs.set(0i64, 5i64); __h.set_xs(w, __xs);"),
        "{code}"
    );
    assert!(
        code.contains("__xs.set(1i64, 7i64);"),
        "{code}"
    );

    // A name the generated program already uses.
    for (src, kind) in [
        ("class Box { pub prop n : Int, default: 0 }\n", "class"),
        ("struct Vec { var n : Int }\n", "struct"),
    ] {
        let err = emit(
            "reserved.pix",
            &format!("{src}\nview Main {{ Column {{ Text {{ text: \"x\" }} }} }}\n"),
        )
        .expect_err("a reserved type name must not emit");
        assert!(
            err.contains("already uses") && err.contains(kind),
            "{err}"
        );
    }
}

#[test]
fn map_optional_bytes_and_struct_properties() {
    // §8.68. `Map` / `T?` / `Bytes` properties were labelled `(M2)`
    // — deferred by design — when what they actually needed was a
    // `Value` variant in the reflection table each. Struct field
    // defaults and `T?` struct fields carried the same label for the
    // same reason.
    let dir = std::env::temp_dir().join("pixie-m0-gate");
    std::fs::create_dir_all(&dir).unwrap();

    let f = dir.join("shapes.pix");
    std::fs::write(
        &f,
        concat!(
            "struct Row {\n",
            "  var name : String\n",
            "  var score : Int = 0\n",
            "  var note : String? = nil\n",
            "}\n",
            "\n",
            "store S {\n",
            "  state rows : List<Row> = []\n",
            "  state tally : Map<String, Int> = {}\n",
            "  state raw : Bytes = []\n",
            "  state picked : String? = nil\n",
            "\n",
            "  fn seed {\n",
            "    rows = [Row(\"ada\"), Row(\"grace\", 2, \"n\")]\n",
            "    tally = { ada: 1 }\n",
            "    picked = \"ada\"\n",
            "  }\n",
            "}\n",
            "\n",
            "view Main {\n",
            "  Column {\n",
            "    Text { text: \"[#{S.picked}] #{S.raw.length}\" }\n",
            "    for k in S.tally.keys {\n",
            "      Text { text: \"#{k}=#{S.tally[k]}\" }\n",
            "    }\n",
            "    for r in S.rows {\n",
            "      Text { text: \"#{r.name} #{r.score} #{r.note}\" }\n",
            "    }\n",
            "  }\n",
            "}\n",
        ),
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code = pixie_codegen::emit_program(
        outcome.module.as_ref().unwrap(),
        outcome.binding_items,
        Some(&pixie_codegen::ReloadInfo {
            source_path: f.to_string_lossy().into_owned(),
            fingerprint: 0,
            foreign_paths: Vec::new(),
        }),
    )
    .expect("the four shapes emit");

    // Defaults fill the omitted trailing fields, and `nil` / a bare
    // value both land in the `T?` one.
    assert!(
        code.contains(r#"Row { name: Str::from("ada"), score: 0i64, note: None }"#),
        "{code}"
    );
    assert!(
        code.contains(r#"note: Some(Str::from("n"))"#),
        "{code}"
    );
    // An absent optional prints as nothing, in both tiers.
    assert!(code.contains("__pixie_show_opt("), "{code}");
    // And each of the four crosses to the reflection table.
    for needle in [
        "pixie_interp::Value::Map(",
        "pixie_interp::Value::Bytes(",
        "pixie_interp::Value::Nil",
        "pixie_interp::Value::Struct(\"Row\"",
    ] {
        assert!(code.contains(needle), "missing `{needle}`:\n{code}");
    }
}

#[test]
fn if_let_is_the_case_it_already_was() {
    // §8.69. `if let` parsed since the fork and no lowerer read it,
    // so it answered "(M2)" in four places. It is a two-armed `case`
    // spelled shorter, and `case` lowers in all four — so the fix is
    // one desugar in `pixie_syntax`, ahead of the checker.
    let dir = std::env::temp_dir().join("pixie-m0-gate");
    std::fs::create_dir_all(&dir).unwrap();

    let f = dir.join("iflet.pix");
    std::fs::write(
        &f,
        concat!(
            "enum Mode { idle  busy }\n",
            "\n",
            "store S {\n",
            "  state who : String? = nil\n",
            "  state note : String = \"-\"\n",
            "  state m : Mode = Mode.idle\n",
            "\n",
            "  fn refresh {\n",
            "    if let some(w) = who {\n",
            "      note = \"hi #{w}\"\n",
            "    } else {\n",
            "      note = \"nobody\"\n",
            "    }\n",
            "  }\n",
            "\n",
            "  fn quiet {\n",
            "    if let some(w) = who { note = w }\n",
            "  }\n",
            "}\n",
            "\n",
            "view Main {\n",
            "  Column {\n",
            "    Button { text: \"x\"; onClick: { if let some(w) = S.who { S.note = w } } }\n",
            "    if let some(w) = S.who {\n",
            "      Text { text: w }\n",
            "      Text { text: \"!\" }\n",
            "    } else {\n",
            "      Text { text: \"none\" }\n",
            "    }\n",
            "    case S.m {\n",
            "      when idle { Text { text: \"idle\" } }\n",
            "      when _ { Text { text: \"other\" } }\n",
            "    }\n",
            "  }\n",
            "}\n",
        ),
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code = pixie_codegen::emit_program(
        outcome.module.as_ref().unwrap(),
        outcome.binding_items,
        None,
    )
    .expect("`if let` emits");

    // A method body, an `else`-less one (the empty half is implied),
    // a handler, and a view — every position the `(M2)` covered.
    // (The count is higher than four: the emitted preamble matches
    // too, so this asserts the shape, not an exact tally.)
    assert!(code.matches("match ").count() >= 4, "{code}");
    // The view arms push elements, and both take as many as written.
    // `w_`, not `w`: the binding is renamed away from the World
    // parameter (§8.68).
    assert!(code.contains("Some(w_) => {"), "{code}");
    assert!(code.contains("Mode::idle => {"), "{code}");
    // An unlisted variant contributes nothing rather than panicking
    // mid-frame.
    assert!(code.contains("#[allow(unreachable_patterns)] _ => {}"), "{code}");
}

#[test]
fn a_foreign_style_or_component_edit_is_a_view_slice_edit() {
    // §8.72. Another module's `pub style` — and a component body it
    // exports — used to be baked into the binary as text, which froze
    // them: editing one meant a rebuild even though both are
    // view-slice material. The binary rereads its imports now.
    let dir = std::env::temp_dir().join("pixie-foreign-reload");
    std::fs::create_dir_all(&dir).unwrap();
    let ui = dir.join("ui.pix");
    let entry = dir.join("app.pix");

    let ui_src = |bg: &str, badge: &str| {
        format!(
            concat!(
                "pub style Card {{\n  background: \"{}\"\n}}\n",
                "\n",
                "style Secret {{\n  background: \"#000000\"\n}}\n",
                "\n",
                "view Frame {{\n  Column {{ padding: 2.0; Slot {{ }} }}\n}}\n",
                "\n",
                "pub view Badge(tag: String) {{\n  Frame {{ Text {{ text: \"{}#{{tag}}\" }} }}\n}}\n",
            ),
            bg, badge
        )
    };
    std::fs::write(&ui, ui_src("#313244", "[")).unwrap();
    std::fs::write(
        &entry,
        "use ui\n\nview Main {\n  Column {\n    style: Card\n    ui.Badge { tag: \"x\" }\n  }\n}\n",
    )
    .unwrap();

    let outcome = pixie_driver::check_file(&entry).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    // The imports are handed over as paths, not as frozen text.
    assert_eq!(outcome.foreign_paths.len(), 1, "{:?}", outcome.foreign_paths);
    assert_eq!(outcome.foreign_paths[0].0, "ui");

    let paths = outcome.foreign_paths.clone();
    let entry_text = std::fs::read_to_string(&entry).unwrap();
    let fp0 = pixie_interp::program_fingerprint_of(&entry_text, &paths).unwrap();

    // A `pub style` body in the import: view-slice material.
    std::fs::write(&ui, ui_src("#ff0000", "[")).unwrap();
    assert_eq!(
        pixie_interp::program_fingerprint_of(&entry_text, &paths).unwrap(),
        fp0,
        "a foreign style edit must not ask for a rebuild"
    );
    let styles = pixie_interp::foreign_reload_from_paths(&paths).styles;
    assert!(styles.contains("#ff0000"), "the reload reads it fresh: {styles}");
    // Only `pub` styles cross.
    assert!(!styles.contains("style Secret"), "a private style must not cross: {styles}");

    // An exported component's body: also view-slice material.
    std::fs::write(&ui, ui_src("#ff0000", "<")).unwrap();
    assert_eq!(
        pixie_interp::program_fingerprint_of(&entry_text, &paths).unwrap(),
        fp0,
        "a foreign component body edit must not ask for a rebuild"
    );
    // Its PRIVATE sibling travels with it — `Badge` uses `Frame`, and
    // a foreign body resolves in its home module (§8.30).
    let views = pixie_interp::foreign_reload_from_paths(&paths).views;
    let ui_views = &views.iter().find(|(n, _)| n == "ui").expect("the import's views").1;
    assert!(ui_views.iter().any(|v| v.name.name == "Frame" && !v.is_pub), "{ui_views:?}");
    assert!(ui_views.iter().any(|v| v.name.name == "Badge" && v.is_pub), "{ui_views:?}");

    // A class in the import is the compiled half's business.
    std::fs::write(
        &ui,
        format!("{}\npub class Extra {{ pub prop k : Int, default: 1 }}\n", ui_src("#ff0000", "<")),
    )
    .unwrap();
    assert_ne!(
        pixie_interp::program_fingerprint_of(&entry_text, &paths).unwrap(),
        fp0,
        "a foreign class edit must ask for a rebuild"
    );
}

#[test]
fn a_binding_takes_a_list_and_an_optional() {
    // §8.73. Binding RETURNS have crossed as lists and optionals
    // since the start; arguments took scalars only, so the
    // conversion knowledge existed and only ran one way.
    let dir = std::env::temp_dir().join("pixie-binding-args");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/genfs/batteries.rpi"),
        dir.join("batteries.rpi"),
    )
    .unwrap();

    let f = dir.join("args.pix");
    std::fs::write(
        &f,
        concat!(
            "store S {\n",
            "  state note : String = \"-\"\n",
            "  state who : String? = nil\n",
            "\n",
            "  fn go {\n",
            "    note = Kernel.joinPath([\"a\", \"b\"])\n",
            "    note = Kernel.orElse(who, \"nobody\")\n",
            "  }\n",
            "}\n",
            "\n",
            "view Main { Column { Text { text: S.note } } }\n",
        ),
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code = pixie_codegen::emit_program(
        outcome.module.as_ref().unwrap(),
        outcome.binding_items,
        None,
    )
    .expect("list and optional arguments emit");

    // A list arrives owned, element-converted — the `Vec<T>` shape
    // the return side already produced.
    assert!(
        code.contains(".iter().map(|x: &Str| x.as_str().to_string()).collect::<Vec<_>>()"),
        "{code}"
    );
    // An optional keeps its shape.
    assert!(code.contains(").as_ref().map(|x: &Str| x.as_str().to_string())"), "{code}");

    // A list of something a binding cannot carry says so by name.
    let f = dir.join("bad.pix");
    std::fs::write(
        &f,
        "class C { pub prop n : Int, default: 0 }\n\nstore S {\n  state note : String = \"-\"\n  state xs : List<C> = []\n  fn go { note = Kernel.joinPath(xs) }\n}\n\nview Main { Column { Text { text: S.note } } }\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    // The checker rejects it first — a `List<C>` is not a
    // `List<String>` — which is the better error of the two.
    assert!(outcome.error_count() > 0, "a class list must not cross");
}

#[test]
fn a_mapped_enum_crosses_a_binding_both_ways() {
    // §8.74. pixie's `enum Color` and a foreign Rust `Color` are two
    // different types, so crossing needs a declared correspondence
    // rather than a lowering. A `.rpi` writes it: `@rust` on the enum
    // names the Rust type, `@rust` on a variant names its counterpart,
    // and a variant without one uses its own name.
    let dir = std::env::temp_dir().join("pixie-enum-binding");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("k.rpi"),
        concat!(
            "enum PathKind @rust(\"pixie_kernel::PathKind\") {\n",
            "  missing @rust(\"Missing\")\n",
            "  file    @rust(\"File\")\n",
            "  dir     @rust(\"Dir\")\n",
            "}\n",
            "\n",
            "class K {\n",
            "  static fn pathKind(path: String) PathKind @rust(\"pixie_kernel::path_kind\")\n",
            "  static fn kindName(kind: PathKind) String @rust(\"pixie_kernel::kind_name\")\n",
            "}\n",
        ),
    )
    .unwrap();

    let f = dir.join("app.pix");
    std::fs::write(
        &f,
        "store S {\n  state note : String = \"-\"\n  fn go {\n    let k = K.pathKind(\"/tmp\")\n    note = K.kindName(k)\n  }\n}\n\nview Main { Column { Text { text: S.note } } }\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "diagnostics: {:?}",
        outcome.diagnostics
    );
    let code = pixie_codegen::emit_program(
        outcome.module.as_ref().unwrap(),
        outcome.binding_items,
        None,
    )
    .expect("a mapped enum emits");

    // Foreign → pixie on the way out...
    assert!(
        code.contains("pixie_kernel::PathKind::Dir => PathKind::dir"),
        "{code}"
    );
    // ...and pixie → foreign on the way in, the same correspondence
    // read right to left.
    assert!(
        code.contains("PathKind::dir => pixie_kernel::PathKind::Dir"),
        "{code}"
    );

    // A PAYLOAD variant cannot correspond variant-for-variant, and
    // saying so beats emitting Rust that does not compile (§8.76 —
    // the unit form for a payload variant was exactly that).
    std::fs::write(
        dir.join("k.rpi"),
        concat!(
            "enum Shape @rust(\"some::Shape\") {\n",
            "  dot @rust(\"Dot\")\n",
            "  line(n: Int) @rust(\"Line\")\n",
            "}\n",
            "\n",
            "class K {\n  static fn shapeOf(n: Int) Shape @rust(\"some::shape_of\")\n}\n",
        ),
    )
    .unwrap();
    std::fs::write(
        &f,
        "store S {\n  state note : String = \"-\"\n  fn go {\n    case K.shapeOf(1) {\n      when dot { note = \"dot\" }\n      when line(n) { note = \"#{n}\" }\n    }\n  }\n}\n\nview Main { Column { Text { text: S.note } } }\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    let err = pixie_codegen::emit_program(
        outcome.module.as_ref().unwrap(),
        outcome.binding_items,
        None,
    )
    .expect_err("a payload enum must not cross");
    assert!(err.message.contains("payload"), "{}", err.message);

    // Without the mapping there is nothing to generate, and the
    // message says what to write.
    std::fs::write(
        dir.join("k.rpi"),
        "enum Loose { a  b }\n\nclass K {\n  static fn take(v: Loose) String @rust(\"probe_take\")\n}\n",
    )
    .unwrap();
    std::fs::write(
        &f,
        "store S {\n  state note : String = \"-\"\n  fn go { note = K.take(Loose.a) }\n}\n\nview Main { Column { Text { text: S.note } } }\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    let err = pixie_codegen::emit_program(
        outcome.module.as_ref().unwrap(),
        outcome.binding_items,
        None,
    )
    .expect_err("an unmapped enum must not cross");
    assert!(err.message.contains("@rust(..)"), "{}", err.message);

    // §8.78: a field may name its Rust TYPE, and one pixie cannot
    // write into is wrong wherever it appears — including a program
    // that only READS the struct, where nothing would have gone back.
    std::fs::write(
        dir.join("k.rpi"),
        concat!(
            "struct Bad @rust(\"some::Bad\") {\n",
            "  var len : Int @rust(\"len: MyThing\")\n",
            "}\n",
            "\n",
            "class K {\n  static fn badOf() Bad @rust(\"some::bad_of\")\n}\n",
        ),
    )
    .unwrap();
    std::fs::write(
        &f,
        "store S {\n  state n : Int = 0\n  fn go { n = K.badOf().len }\n}\n\nview Main { Column { Text { text: \"#{S.n}\" } } }\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&f).expect("driver runs");
    let err = pixie_codegen::emit_program(
        outcome.module.as_ref().unwrap(),
        outcome.binding_items,
        None,
    )
    .expect_err("a field type pixie cannot write must not cross");
    assert!(err.message.contains("MyThing"), "{}", err.message);
    // The message names the PIXIE type, not the Rust spelling: an
    // author who wrote `Int` should not be told about an `i64`.
    assert!(err.message.contains("`Int`"), "{}", err.message);
}

#[test]
fn a_private_style_travels_with_the_component_that_uses_it() {
    // §8.75. A module's own `style Inner` worked in its own view and
    // stopped working the moment that view was used from another
    // module: *unknown style `Inner`*, at build time. The component
    // splice ran BEFORE the style pass, so an exported body had
    // already moved into the importer by the time styles resolved —
    // and the importer has the exporter's `pub` styles only.
    let dir = std::env::temp_dir().join("pixie-private-style");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("ui.pix"),
        concat!(
            "style Inner { padding: 2.0 }\n",
            "\n",
            "view Frame {\n  Column { style: Inner; Slot { } }\n}\n",
            "\n",
            "pub view Badge(tag: String) {\n  Frame { Text { text: \"[#{tag}]\" } }\n}\n",
        ),
    )
    .unwrap();
    let entry = dir.join("app.pix");
    std::fs::write(
        &entry,
        "use ui\n\nview Main {\n  Column { ui.Badge { tag: \"a\" } }\n}\n",
    )
    .unwrap();

    let outcome = pixie_driver::check_file(&entry).expect("driver runs");
    assert_eq!(
        outcome.error_count(),
        0,
        "a private style must reach the component that uses it: {:?}",
        outcome.diagnostics
    );
    let code = pixie_codegen::emit_program(
        outcome.module.as_ref().unwrap(),
        outcome.binding_items,
        None,
    )
    .expect("emits");
    assert!(code.contains("padding: 2f64"), "the style landed: {code}");

    // And the importer still may not NAME it — resolving in the
    // exporter's scope is not the same as exporting.
    std::fs::write(
        &entry,
        "use ui\n\nview Main {\n  Column { style: Inner; Text { text: \"x\" } }\n}\n",
    )
    .unwrap();
    let outcome = pixie_driver::check_file(&entry).expect("driver runs");
    assert!(
        outcome
            .diagnostics
            .iter()
            .any(|d| d.message.contains("unknown style `Inner`")),
        "a private style must stay private: {:?}",
        outcome.diagnostics
    );
}
