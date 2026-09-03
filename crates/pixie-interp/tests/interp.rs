//! The S6 proof, on the product crate: interpreted views drive real
//! World objects through hand-registered tables (standing in for the
//! emitter's), actions mutate through setters/methods, and a reloaded
//! body re-renders against preserved state.

use std::rc::Rc;

use pixie_interp::{
    FieldEnv, LiveView, Tables, Value, build_view, extract_view, module_fingerprint, parse_module,
    reload_from_source,
};
use pixie_kernel::{Element, ErasedHandle, Handle, List, SignalId, Str, World};

struct Counter {
    count: i64,
}
const COUNT_CHANGED: SignalId = 1;

trait CounterRef: Copy {
    fn count(self, w: &World) -> i64;
    fn set_count(self, w: &mut World, v: i64);
    fn increment(self, w: &mut World);
}
impl CounterRef for Handle<Counter> {
    fn count(self, w: &World) -> i64 {
        w.get(self).count
    }
    fn set_count(self, w: &mut World, v: i64) {
        if w.get(self).count != v {
            w.get_mut(self).count = v;
            w.notify(self.erase(), COUNT_CHANGED);
        }
    }
    fn increment(self, w: &mut World) {
        let v = self.count(w);
        self.set_count(w, v + 1);
    }
}

struct Todo {
    items: List<Str>,
}

trait TodoRef: Copy {
    fn items(self, w: &World) -> List<Str>;
    fn set_items(self, w: &mut World, v: List<Str>);
}
impl TodoRef for Handle<Todo> {
    fn items(self, w: &World) -> List<Str> {
        w.get(self).items.clone()
    }
    fn set_items(self, w: &mut World, v: List<Str>) {
        w.get_mut(self).items = v;
        w.notify(self.erase(), 2);
    }
}

struct Job {
    ratio: f64,
}

trait JobRef: Copy {
    fn ratio(self, w: &World) -> f64;
}
impl JobRef for Handle<Job> {
    fn ratio(self, w: &World) -> f64 {
        w.get(self).ratio
    }
}

struct Flag {
    on: bool,
}

trait FlagRef: Copy {
    fn on(self, w: &World) -> bool;
}
impl FlagRef for Handle<Flag> {
    fn on(self, w: &World) -> bool {
        w.get(self).on
    }
}

fn tables() -> Rc<Tables> {
    let mut t = Tables::new();
    t.getter("Counter", "count", |w, h| {
        Value::Int(h.typed::<Counter>().count(w))
    });
    t.setter("Counter", "count", |w, h, v| {
        h.typed::<Counter>().set_count(w, v.as_int()?);
        Ok(())
    });
    t.method("Counter", "increment", |w, h, args| {
        if !args.is_empty() {
            return Err("increment takes 0 argument(s)".into());
        }
        h.typed::<Counter>().increment(w);
        Ok(())
    });
    t.getter("Todo", "items", |w, h| {
        Value::List(
            h.typed::<Todo>()
                .items(w)
                .iter()
                .map(|x| Value::Str(x.clone()))
                .collect(),
        )
    });
    t.setter("Todo", "items", |w, h, v| {
        let Value::List(xs) = v else {
            return Err("expected List".into());
        };
        let mut out: List<Str> = List::new();
        for x in &xs {
            out.push(x.as_str_value()?);
        }
        h.typed::<Todo>().set_items(w, out);
        Ok(())
    });
    t.getter("Job", "ratio", |w, h| {
        Value::Float(h.typed::<Job>().ratio(w))
    });
    t.getter("Flag", "on", |w, h| Value::Bool(h.typed::<Flag>().on(w)));
    t.getter("Series", "values", |w, h| {
        Value::List(
            h.typed::<Series>()
                .values(w)
                .iter()
                .map(|x| Value::Float(*x))
                .collect(),
        )
    });
    Rc::new(t)
}

fn env(counter: ErasedHandle, todo: ErasedHandle) -> FieldEnv {
    FieldEnv {
        fields: vec![
            ("counter".into(), "Counter".into(), counter),
            ("todo".into(), "Todo".into(), todo),
        ],
    }
}

fn view_of(src: &str) -> LiveView {
    extract_view(&parse_module(src).expect("parses")).expect("has a view")
}

#[test]
fn interpreted_view_reads_props_and_repeats() {
    let mut w = World::new();
    let c = w.insert(Counter { count: 41 });
    let mut items: List<Str> = List::new();
    items.push(Str::from("a"));
    items.push(Str::from("b"));
    let t = w.insert(Todo { items });

    let lv = view_of(
        "view Main {\n  let counter = Counter()\n  let todo = Todo()\n  Column {\n    Text { text: \"Count: #{counter.count}\" }\n    ListView {\n      for it in todo.items {\n        Text { text: it }\n      }\n    }\n    Text { text: \"items: #{todo.items.length}\" }\n  }\n}\n",
    );
    let tree = build_view(&lv, &env(c.erase(), t.erase()), &tables(), &w).expect("builds");
    assert_eq!(
        tree.dump(&w),
        "Column[Text(Count: 41), ListView[Text(a), Text(b)], Text(items: 2)]"
    );
}

#[test]
fn interpreted_scroll_views_wrap_children() {
    let mut w = World::new();
    let c = w.insert(Counter { count: 7 });
    let mut items: List<Str> = List::new();
    items.push(Str::from("a"));
    items.push(Str::from("b"));
    let t = w.insert(Todo { items });

    let lv = view_of(
        "view Main {\n  let counter = Counter()\n  let todo = Todo()\n  Column {\n    ScrollView {\n      for it in todo.items {\n        Text { text: it }\n      }\n    }\n    HScrollView {\n      Text { text: \"x\" }\n      Text { text: \"Count: #{counter.count}\" }\n    }\n  }\n}\n",
    );
    let tree = build_view(&lv, &env(c.erase(), t.erase()), &tables(), &w).expect("builds");
    assert_eq!(
        tree.dump(&w),
        "Column[ScrollView[Text(a), Text(b)], HScrollView[Text(x), Text(Count: 7)]]"
    );
}

#[test]
fn scroll_view_takes_a_float_height() {
    // The `height:` container prop: optional, strict Float (codegen's
    // rule), `0.0` = unset so an untouched ScrollView keeps the bare
    // `ScrollView[..]` dump the tier gate has always compared.
    let mut w = World::new();
    let c = w.insert(Counter { count: 7 });
    let t = w.insert(Todo { items: List::new() });
    let e = env(c.erase(), t.erase());
    let tb = tables();

    let lv = view_of(
        "view Main {\n  Column {\n    ScrollView {\n      height: 240.0\n      Text { text: \"hi\" }\n    }\n  }\n}\n",
    );
    let tree = build_view(&lv, &e, &tb, &w).expect("builds");
    assert_eq!(tree.dump(&w), "Column[ScrollView(height=240)[Text(hi)]]");

    let lv = view_of(
        "view Main {\n  Column {\n    ScrollView {\n      Text { text: \"hi\" }\n    }\n  }\n}\n",
    );
    let tree = build_view(&lv, &e, &tb, &w).expect("builds");
    assert_eq!(tree.dump(&w), "Column[ScrollView[Text(hi)]]");

    // An Int-typed prop widens, exactly like `itemHeight:` (§8.55).
    let lv = view_of(
        "view Main {\n  let counter = Counter()\n  Column {\n    ScrollView {\n      height: counter.count\n      Text { text: \"hi\" }\n    }\n  }\n}\n",
    );
    let tree = build_view(&lv, &e, &tb, &w).expect("an Int height widens");
    assert_eq!(tree.dump(&w), "Column[ScrollView(height=7)[Text(hi)]]");

    // And it is the vertical twin's prop alone.
    let lv = view_of(
        "view Main {\n  Column {\n    HScrollView {\n      height: 240.0\n      Text { text: \"hi\" }\n    }\n  }\n}\n",
    );
    match build_view(&lv, &e, &tb, &w) {
        Ok(_) => panic!("`height:` on HScrollView must error"),
        Err(err) => assert!(
            err.contains("`height`") && err.contains("`HScrollView`"),
            "error should name the key and the element: {err}"
        ),
    }
}

#[test]
fn list_view_takes_a_float_height() {
    // ListView's viewport height rides the same machinery, and joins
    // the parenthesized dump group only when it is set — so a list that
    // only sets `virtualized:`/`itemHeight:` dumps as it always did.
    let mut w = World::new();
    let c = w.insert(Counter { count: 3 });
    let mut items: List<Str> = List::new();
    items.push(Str::from("a"));
    let t = w.insert(Todo { items });
    let e = env(c.erase(), t.erase());
    let tb = tables();

    let lv = view_of(
        "view Main {\n  let todo = Todo()\n  Column {\n    ListView {\n      virtualized: true\n      itemHeight: 24.0\n      height: 280.0\n      for it in todo.items {\n        Text { text: it }\n      }\n    }\n  }\n}\n",
    );
    let tree = build_view(&lv, &e, &tb, &w).expect("builds");
    assert_eq!(
        tree.dump(&w),
        "Column[ListView(virtualized=true, itemHeight=24, height=280)[Text(a)]]"
    );

    // `height:` alone still leaves the other two at their defaults.
    let lv = view_of(
        "view Main {\n  let todo = Todo()\n  Column {\n    ListView {\n      height: 280.0\n      for it in todo.items {\n        Text { text: it }\n      }\n    }\n  }\n}\n",
    );
    let tree = build_view(&lv, &e, &tb, &w).expect("builds");
    assert_eq!(
        tree.dump(&w),
        "Column[ListView(virtualized=false, itemHeight=0, height=280)[Text(a)]]"
    );

    let lv = view_of(
        "view Main {\n  let counter = Counter()\n  Column {\n    ListView {\n      height: counter.count\n      Text { text: \"hi\" }\n    }\n  }\n}\n",
    );
    let tree = build_view(&lv, &e, &tb, &w).expect("an Int height widens");
    assert_eq!(
        tree.dump(&w),
        "Column[ListView(virtualized=false, itemHeight=0, height=3)[Text(hi)]]"
    );
}

#[test]
fn interpreted_actions_mutate_and_rerender() {
    let mut w = World::new();
    let c = w.insert(Counter { count: 0 });
    let t = w.insert(Todo { items: List::new() });
    let e = env(c.erase(), t.erase());
    let tb = tables();

    let lv = view_of(
        "view Main {\n  let counter = Counter()\n  let todo = Todo()\n  Column {\n    Text { text: \"Count: #{counter.count}\" }\n    Button { text: \"+1\"; onClick: counter.increment() }\n    Button { text: \"+10\"; onClick: { counter.count = counter.count + 10 } }\n    Button { text: \"add\"; onClick: { todo.items.push(\"x#{counter.count}\") } }\n  }\n}\n",
    );
    let tree = build_view(&lv, &e, &tb, &w).expect("builds");

    tree.find_button(&w, "+1").expect("button")(&mut w);
    tree.find_button(&w, "+10").expect("button")(&mut w);
    tree.find_button(&w, "add").expect("button")(&mut w);
    w.flush();

    let tree2 = build_view(&lv, &e, &tb, &w).expect("rebuilds");
    assert_eq!(
        tree2.dump(&w),
        "Column[Text(Count: 11), Button(+1), Button(+10), Button(add)]"
    );
    assert_eq!(w.get(t).items.at(0).as_str(), "x11");
}

#[test]
fn reloaded_body_renders_against_preserved_state() {
    let mut w = World::new();
    let c = w.insert(Counter { count: 7 });
    let t = w.insert(Todo { items: List::new() });
    let e = env(c.erase(), t.erase());
    let tb = tables();

    let v1 = "view Main {\n  let counter = Counter()\n  let todo = Todo()\n  Column {\n    Text { text: \"Count: #{counter.count}\" }\n  }\n}\n";
    let v2 = "view Main {\n  let counter = Counter()\n  let todo = Todo()\n  Column {\n    Text { text: \"THE COUNT IS NOW #{counter.count}!\" }\n    Text { text: \"and a second line\" }\n  }\n}\n";
    // Same fingerprint: only the view body differs.
    assert_eq!(
        module_fingerprint(&parse_module(v1).unwrap()),
        module_fingerprint(&parse_module(v2).unwrap())
    );

    let t1 = build_view(&view_of(v1), &e, &tb, &w).unwrap();
    assert_eq!(t1.dump(&w), "Column[Text(Count: 7)]");
    let t2 = build_view(&view_of(v2), &e, &tb, &w).unwrap();
    assert_eq!(
        t2.dump(&w),
        "Column[Text(THE COUNT IS NOW 7!), Text(and a second line)]"
    );

    // Mid-edit garbage: parse fails, the caller keeps the last tree.
    assert!(parse_module("view Main {\n  Column {\n").is_err());
}

#[test]
fn state_cells_resolve_through_the_holder() {
    let mut w = World::new();
    let c = w.insert(Counter { count: 5 });
    // The holder is just another world class (the emitter synthesizes
    // it); reuse Counter as the stand-in holder with a `count` cell.
    let holder = w.insert(Counter { count: 100 });
    let e = FieldEnv {
        fields: vec![
            ("counter".into(), "Counter".into(), c.erase()),
            ("__pixie_state".into(), "Counter".into(), holder.erase()),
        ],
    };
    let tb = tables();
    // `count` appears both as a state cell and as a prop of `counter`
    // — the bare name goes to the holder.
    let src = "view Main {\n  state count : Int = 100\n  Column {\n    Text { text: \"cell: #{count} obj: #{counter.count}\" }\n    Button { text: \"bump\"; onClick: { count = count + 1 } }\n  }\n}\n";
    let lv = view_of(src);
    let tree = build_view(&lv, &e, &tb, &w).unwrap();
    assert_eq!(
        tree.dump(&w),
        "Column[Text(cell: 100 obj: 5), Button(bump)]"
    );
    tree.find_button(&w, "bump").unwrap()(&mut w);
    w.flush();
    let tree2 = build_view(&lv, &e, &tb, &w).unwrap();
    assert_eq!(
        tree2.dump(&w),
        "Column[Text(cell: 101 obj: 5), Button(bump)]"
    );
}

#[test]
fn grid_dumps_its_tracks_and_span_cells() {
    // `columns:`/`rows:` are Int-strict, `spacing:` follows Column's
    // sentinel, and `colSpan:` on a child materializes the GridCell
    // wrapper — the compiled tier emits exactly this shape.
    let w = World::new();
    let e = FieldEnv { fields: vec![] };
    let tb = tables();
    let lv = view_of(
        "view Main {\n  Grid {\n    columns: 2\n    rows: 3\n    spacing: 4.0\n    Text { text: \"a\" }\n    Text { text: \"b\"; colSpan: 2 }\n  }\n}\n",
    );
    let tree = build_view(&lv, &e, &tb, &w).expect("builds");
    assert_eq!(
        tree.dump(&w),
        "Grid(columns=2, rows=3, spacing=4)[Text(a), GridCell(colSpan=2)[Text(b)]]"
    );
}

#[test]
fn grid_without_span_props_builds_no_cells() {
    // The "no span props, no wrapper" rule: a plain grid dumps its
    // children directly, so pre-Grid demos stay byte-identical.
    let w = World::new();
    let e = FieldEnv { fields: vec![] };
    let tb = tables();
    let lv = view_of(
        "view Main {\n  Grid {\n    columns: 3\n    Text { text: \"a\" }\n    Text { text: \"b\" }\n  }\n}\n",
    );
    let tree = build_view(&lv, &e, &tb, &w).expect("builds");
    assert_eq!(tree.dump(&w), "Grid(columns=3)[Text(a), Text(b)]");
}

#[test]
fn grid_spans_wrap_containers_too() {
    // The spans belong to the parent grid, not to an element's own
    // vocabulary, so a Column takes them and the cell lands outside.
    let w = World::new();
    let e = FieldEnv { fields: vec![] };
    let tb = tables();
    let lv = view_of(
        "view Main {\n  Grid {\n    columns: 2\n    Column {\n      colSpan: 2\n      Text { text: \"wide\" }\n    }\n  }\n}\n",
    );
    let tree = build_view(&lv, &e, &tb, &w).expect("builds");
    assert_eq!(
        tree.dump(&w),
        "Grid(columns=2)[GridCell(colSpan=2)[Column[Text(wide)]]]"
    );
}

#[test]
fn grid_requires_a_columns_count() {
    let w = World::new();
    let e = FieldEnv { fields: vec![] };
    let tb = tables();
    let lv = view_of("view Main {\n  Grid {\n    Text { text: \"a\" }\n  }\n}\n");
    match build_view(&lv, &e, &tb, &w) {
        Ok(_) => panic!("a Grid without `columns:` must error"),
        Err(err) => assert!(err.contains("columns"), "error should name the prop: {err}"),
    }
}

#[test]
fn grid_rejects_a_float_columns_count() {
    // Mirrors codegen's strictness: `columns:` is an Int, no silent
    // Float truncation.
    let w = World::new();
    let e = FieldEnv { fields: vec![] };
    let tb = tables();
    let lv = view_of(
        "view Main {\n  Grid {\n    columns: 2.0\n    Text { text: \"a\" }\n  }\n}\n",
    );
    match build_view(&lv, &e, &tb, &w) {
        Ok(_) => panic!("Float columns: must error"),
        Err(err) => assert!(err.contains("Int"), "error should name the type: {err}"),
    }
}

#[test]
fn progress_bar_dumps_its_float_value() {
    let mut w = World::new();
    let job = w.insert(Job { ratio: 0.5 });
    let e = FieldEnv {
        fields: vec![("job".into(), "Job".into(), job.erase())],
    };
    let tb = tables();
    let lv = view_of(
        "view Main {\n  let job = Job()\n  Column {\n    ProgressBar { value: job.ratio }\n  }\n}\n",
    );
    let tree = build_view(&lv, &e, &tb, &w).expect("builds");
    assert_eq!(tree.dump(&w), "Column[ProgressBar(0.5)]");
}

#[test]
fn modal_dumps_its_open_flag_and_children() {
    // `open` is a bound Bool, so both states produce the same subtree
    // — only the flag in the dump (and the engine's paint) changes.
    for on in [true, false] {
        let mut w = World::new();
        let flag = w.insert(Flag { on });
        let e = FieldEnv {
            fields: vec![("flag".into(), "Flag".into(), flag.erase())],
        };
        let lv = view_of(
            "view Main {\n  let flag = Flag()\n  Column {\n    Modal {\n      open: flag.on\n      Text { text: \"hi\" }\n    }\n  }\n}\n",
        );
        let tree = build_view(&lv, &e, &tables(), &w).expect("builds");
        assert_eq!(tree.dump(&w), format!("Column[Modal({on})[Text(hi)]]"));
    }
}

#[test]
fn modal_rejects_a_non_bool_open() {
    // Matches codegen's strictness: `open:` must be a Bool, no
    // truthiness on an Int.
    let mut w = World::new();
    let c = w.insert(Counter { count: 3 });
    let t = w.insert(Todo { items: List::new() });
    let e = env(c.erase(), t.erase());
    let tb = tables();
    let lv = view_of(
        "view Main {\n  let counter = Counter()\n  let todo = Todo()\n  Column {\n    Modal {\n      open: counter.count\n      Text { text: \"hi\" }\n    }\n  }\n}\n",
    );
    match build_view(&lv, &e, &tb, &w) {
        Ok(_) => panic!("Int open: must error, not coerce"),
        Err(err) => assert!(
            err.contains("Bool"),
            "error should name the expected type: {err}"
        ),
    }
}

#[test]
fn progress_bar_widens_an_int_typed_value() {
    // Matches codegen: `value:` is a Float slot and an Int widens
    // into it (§8.55).
    let mut w = World::new();
    let c = w.insert(Counter { count: 3 });
    let t = w.insert(Todo { items: List::new() });
    let e = env(c.erase(), t.erase());
    let tb = tables();
    let lv = view_of(
        "view Main {\n  let counter = Counter()\n  let todo = Todo()\n  Column {\n    ProgressBar { value: counter.count }\n  }\n}\n",
    );
    let tree = build_view(&lv, &e, &tb, &w).expect("an Int value widens");
    assert_eq!(tree.dump(&w), "Column[ProgressBar(3)]");
}

#[test]
fn slider_dumps_value_range_and_optional_step() {
    // Defaults mirror codegen exactly: min 0.0, max 1.0, step 0.0 —
    // and a continuous slider (step unset) keeps `step=` out of the
    // dump (the per-prop join rule).
    let mut w = World::new();
    let job = w.insert(Job { ratio: 0.5 });
    let e = FieldEnv {
        fields: vec![("job".into(), "Job".into(), job.erase())],
    };
    let tb = tables();
    let lv = view_of(
        "view Main {\n  let job = Job()\n  Column {\n    Slider { value: job.ratio }\n    Slider { value: job.ratio; min: 0.0; max: 10.0; step: 1.0 }\n  }\n}\n",
    );
    let tree = build_view(&lv, &e, &tb, &w).expect("builds");
    assert_eq!(
        tree.dump(&w),
        "Column[Slider(value=0.5, min=0, max=1), Slider(value=0.5, min=0, max=10, step=1)]"
    );
}

#[test]
fn slider_requires_a_value_binding() {
    let w = World::new();
    let e = FieldEnv { fields: vec![] };
    let tb = tables();
    let lv = view_of("view Main {\n  Column {\n    Slider { min: 0.0 }\n  }\n}\n");
    match build_view(&lv, &e, &tb, &w) {
        Ok(_) => panic!("a valueless Slider must error"),
        Err(err) => assert!(
            err.contains("Slider needs `value:`"),
            "error should name the missing prop: {err}"
        ),
    }
}

#[test]
fn image_dumps_source_and_dimensions() {
    let w = World::new();
    let e = FieldEnv { fields: vec![] };
    let tb = tables();
    let lv = view_of(
        "view Main {\n  Column {\n    Image { source: \"pixie.png\" }\n    Image { source: \"pixie.png\"; width: 64.0; height: 32.0 }\n  }\n}\n",
    );
    let tree = build_view(&lv, &e, &tb, &w).expect("builds");
    assert_eq!(
        tree.dump(&w),
        "Column[Image(pixie.png 0x0), Image(pixie.png 64x32)]"
    );
}

#[test]
fn image_widens_an_int_typed_width() {
    // Matches codegen: `width:`/`height:` take a number, and an Int
    // widens into the Float slot (§8.55).
    let w = World::new();
    let e = FieldEnv { fields: vec![] };
    let tb = tables();
    let lv = view_of(
        "view Main {\n  Column {\n    Image { source: \"pixie.png\"; width: 64 }\n  }\n}\n",
    );
    let tree = build_view(&lv, &e, &tb, &w).expect("an Int width widens");
    assert_eq!(tree.dump(&w), "Column[Image(pixie.png 64x0)]");
}

#[test]
fn list_view_dumps_its_container_props() {
    let mut w = World::new();
    let mut items: List<Str> = List::new();
    items.push(Str::from("a"));
    items.push(Str::from("b"));
    let t = w.insert(Todo { items });
    let flag = w.insert(Flag { on: true });
    let e = FieldEnv {
        fields: vec![
            ("todo".into(), "Todo".into(), t.erase()),
            ("flag".into(), "Flag".into(), flag.erase()),
        ],
    };
    let tb = tables();

    // Literal props, and a bound Bool through a prop read.
    let lv = view_of(
        "view Main {\n  let todo = Todo()\n  let flag = Flag()\n  Column {\n    ListView {\n      virtualized: flag.on\n      itemHeight: 24.0\n      for it in todo.items {\n        Text { text: it }\n      }\n    }\n  }\n}\n",
    );
    let tree = build_view(&lv, &e, &tb, &w).expect("builds");
    assert_eq!(
        tree.dump(&w),
        "Column[ListView(virtualized=true, itemHeight=24)[Text(a), Text(b)]]"
    );

    // Unset props keep the bare `ListView[..]` dump, so every existing
    // demo compares byte-identically across the tier gate.
    let lv = view_of(
        "view Main {\n  let todo = Todo()\n  Column {\n    ListView {\n      for it in todo.items {\n        Text { text: it }\n      }\n    }\n  }\n}\n",
    );
    let tree = build_view(&lv, &e, &tb, &w).expect("builds");
    assert_eq!(tree.dump(&w), "Column[ListView[Text(a), Text(b)]]");
}

#[test]
fn list_view_rejects_mistyped_container_props() {
    // Matches codegen exactly: `virtualized:` must be a Bool — no
    // truthiness — while `itemHeight:` takes an Int by widening.
    let mut w = World::new();
    let c = w.insert(Counter { count: 3 });
    let t = w.insert(Todo { items: List::new() });
    let e = env(c.erase(), t.erase());
    let tb = tables();

    let lv = view_of(
        "view Main {\n  let counter = Counter()\n  let todo = Todo()\n  Column {\n    ListView {\n      virtualized: counter.count\n      Text { text: \"hi\" }\n    }\n  }\n}\n",
    );
    match build_view(&lv, &e, &tb, &w) {
        Ok(_) => panic!("Int virtualized: must error, not coerce"),
        Err(err) => assert!(
            err.contains("Bool"),
            "error should name the expected type: {err}"
        ),
    }

    let lv = view_of(
        "view Main {\n  let counter = Counter()\n  let todo = Todo()\n  Column {\n    ListView {\n      itemHeight: counter.count\n      Text { text: \"hi\" }\n    }\n  }\n}\n",
    );
    let tree = build_view(&lv, &e, &tb, &w).expect("an Int itemHeight widens");
    assert_eq!(
        tree.dump(&w),
        "Column[ListView(virtualized=false, itemHeight=3)[Text(hi)]]"
    );
}

#[test]
fn unknown_container_property_errors_like_codegen() {
    // The §11.12 closure: `build_children` used to ignore every
    // container-level property, so a view codegen rejects would still
    // reload happily through rung 2. Now both tiers refuse it, with the
    // same message, and the allowlist is per element.
    let mut w = World::new();
    let c = w.insert(Counter { count: 3 });
    let t = w.insert(Todo { items: List::new() });
    let e = env(c.erase(), t.erase());
    let tb = tables();

    let lv = view_of(
        "view Main {\n  Column {\n    ListView {\n      spacing: 4.0\n      Text { text: \"hi\" }\n    }\n  }\n}\n",
    );
    match build_view(&lv, &e, &tb, &w) {
        Ok(_) => panic!("an unknown container property must error, not be ignored"),
        Err(err) => assert!(
            err.contains("`spacing`") && err.contains("`ListView`"),
            "error should name the key and the element: {err}"
        ),
    }

    // ListView's keys are not Column's.
    let lv = view_of(
        "view Main {\n  Column {\n    itemHeight: 24.0\n    Text { text: \"hi\" }\n  }\n}\n",
    );
    match build_view(&lv, &e, &tb, &w) {
        Ok(_) => panic!("a ListView key on a Column must error"),
        Err(err) => assert!(
            err.contains("`itemHeight`") && err.contains("`Column`"),
            "error should name the key and the element: {err}"
        ),
    }

    // ...and the keys a container really does consume still pass.
    let lv = view_of(
        "view Main {\n  Column {\n    Modal {\n      open: false\n      Text { text: \"hi\" }\n    }\n  }\n}\n",
    );
    build_view(&lv, &e, &tb, &w).expect("Modal's own `open:` stays legal");
}

#[test]
fn data_table_dumps_header_and_repeated_rows() {
    let mut w = World::new();
    let mut items: List<Str> = List::new();
    items.push(Str::from("a"));
    items.push(Str::from("b"));
    let t = w.insert(Todo { items });
    let e = FieldEnv {
        fields: vec![("todo".into(), "Todo".into(), t.erase())],
    };
    let tb = tables();
    let lv = view_of(
        "view Main {\n  let todo = Todo()\n  Column {\n    DataTable {\n      Row {\n        Text { text: \"Name\" }\n      }\n      for it in todo.items {\n        Row { Text { text: it } }\n      }\n    }\n  }\n}\n",
    );
    let tree = build_view(&lv, &e, &tb, &w).expect("builds");
    // The interp arm mirrors codegen exactly: DataTable just joins
    // children like Column — header vs. zebra styling is engine-side.
    assert_eq!(
        tree.dump(&w),
        "Column[DataTable[Row[Text(Name)], Row[Text(a)], Row[Text(b)]]]"
    );
}

#[test]
fn stack_dumps_children_in_paint_order() {
    let mut w = World::new();
    // Same holder trick as `state_cells_resolve_through_the_holder`:
    // reuse Counter as the stand-in for the emitter-synthesized state
    // holder.
    let holder = w.insert(Counter { count: 0 });
    let e = FieldEnv {
        fields: vec![("__pixie_state".into(), "Counter".into(), holder.erase())],
    };
    let tb = tables();
    // Base (child 0) is a Column that sizes the Stack; the overlay
    // Text and the top-layer Button are later children, dumped (and
    // painted) above it in list order.
    let src = "view Main {\n  state count : Int = 0\n  Column {\n    Stack {\n      Column {\n        Text { text: \"base\" }\n        Text { text: \"count: #{count}\" }\n      }\n      Text { text: \"overlay\" }\n      Button { text: \"bump\"; onClick: { count = count + 1 } }\n    }\n  }\n}\n";
    let lv = view_of(src);
    let tree = build_view(&lv, &e, &tb, &w).unwrap();
    assert_eq!(
        tree.dump(&w),
        "Column[Stack[Column[Text(base), Text(count: 0)], Text(overlay), Button(bump)]]"
    );
    // Stack must join the `find_button` recursion (kernel catalog
    // rule) so a headless script can reach a button nested inside one.
    tree.find_button(&w, "bump").unwrap()(&mut w);
    w.flush();
    let tree2 = build_view(&lv, &e, &tb, &w).unwrap();
    assert_eq!(
        tree2.dump(&w),
        "Column[Stack[Column[Text(base), Text(count: 1)], Text(overlay), Button(bump)]]"
    );
}

#[test]
fn svg_dumps_source_and_dimensions() {
    let w = World::new();
    let e = FieldEnv { fields: vec![] };
    let tb = tables();
    let lv = view_of(
        "view Main {\n  Column {\n    Svg { source: \"star.svg\" }\n    Svg { source: \"star.svg\"; width: 24.0; height: 48.0 }\n  }\n}\n",
    );
    let tree = build_view(&lv, &e, &tb, &w).expect("builds");
    assert_eq!(
        tree.dump(&w),
        "Column[Svg(star.svg 0x0), Svg(star.svg 24x48)]"
    );
}

#[test]
fn svg_widens_an_int_typed_width() {
    // Matches codegen: `width:`/`height:` take a number, and an Int
    // widens into the Float slot (§8.55).
    let w = World::new();
    let e = FieldEnv { fields: vec![] };
    let tb = tables();
    let lv = view_of(
        "view Main {\n  Column {\n    Svg { source: \"star.svg\"; width: 24 }\n  }\n}\n",
    );
    let tree = build_view(&lv, &e, &tb, &w).expect("an Int width widens");
    assert_eq!(tree.dump(&w), "Column[Svg(star.svg 24x0)]");
}

#[test]
fn charts_dump_their_data_and_labels() {
    // The interp builds the same kernel Lists the emitter does, so this
    // dump is exactly what the compiled tier prints (the tier gate).
    let (w, e) = charts_world();
    let tree = build_view(
        &chart_view(
            "BarChart { data: series.values; labels: todo.items }\n    \
             LineChart { data: series.values; labels: todo.items }\n    \
             Spinner { }",
        ),
        &e,
        &tables(),
        &w,
    )
    .expect("builds");
    assert_eq!(
        tree.dump(&w),
        "Column[BarChart([0.5, 1.5] [\"a\", \"b\"]), \
         LineChart([0.5, 1.5] [\"a\", \"b\"]), Spinner]"
    );
}

#[test]
fn charts_default_to_no_labels() {
    // `labels:` is optional and defaults to an empty List — same
    // default as codegen's `List::new()`.
    let (w, e) = charts_world();
    let tree = build_view(
        &chart_view("BarChart { data: series.values }"),
        &e,
        &tables(),
        &w,
    )
    .expect("builds");
    assert_eq!(tree.dump(&w), "Column[BarChart([0.5, 1.5] [])]");
}

#[test]
fn charts_require_data() {
    // Mirrors codegen's required-prop error, which the emission tests
    // assert on the other tier.
    let (w, e) = charts_world();
    for widget in ["BarChart", "LineChart"] {
        match build_view(&chart_view(&format!("{widget} {{ }}")), &e, &tables(), &w) {
            Ok(_) => panic!("{widget} without `data:` must error"),
            Err(err) => assert!(
                err.contains(&format!("{widget} needs `data:`")),
                "error should name the missing prop: {err}"
            ),
        }
    }
}

#[test]
fn charts_reject_wrongly_typed_lists() {
    // Mirrors codegen's strictness: `data:` is List<Float> and
    // `labels:` is List<String>, with no coercion in either direction
    // — and a non-List binding is rejected outright.
    let (w, e) = charts_world();
    for (body, expected) in [
        ("BarChart { data: todo.items }", "Float"),
        ("BarChart { data: counter.count }", "Float"),
        (
            "LineChart { data: series.values; labels: series.values }",
            "String",
        ),
    ] {
        match build_view(&chart_view(body), &e, &tables(), &w) {
            Ok(_) => panic!("`{body}` must error, not coerce"),
            Err(err) => assert!(
                err.contains(expected),
                "error should name the expected type: {err}"
            ),
        }
    }
}

#[test]
fn chart_and_spinner_sizing_joins_the_dump_only_when_set() {
    // The kernel's non-default rule (ListView's): unset sizing keeps
    // the bare rendering `charts_dump_their_data_and_labels` asserts,
    // and each set axis appends its own token — so the tier gate sees
    // exactly what the window is sized by.
    let (w, e) = charts_world();
    let tree = build_view(
        &chart_view(
            "BarChart { data: series.values; width: 260.0; height: 110.0 }\n    \
             LineChart { data: series.values; height: 90.0 }\n    \
             Spinner { size: 32.0 }",
        ),
        &e,
        &tables(),
        &w,
    )
    .expect("builds");
    assert_eq!(
        tree.dump(&w),
        "Column[BarChart([0.5, 1.5] [] w=260 h=110), \
         LineChart([0.5, 1.5] [] h=90), Spinner(32)]"
    );
}

#[test]
fn chart_and_spinner_sizing_widens_int_literals() {
    // Mirrors codegen's `lower_view_size` / `lower_view_float`:
    // an Int widens into the Float slot (§8.55).
    let (w, e) = charts_world();
    for (body, dumped) in [
        (
            "BarChart { data: series.values; width: 260 }",
            "Column[BarChart([0.5, 1.5] [] w=260)]",
        ),
        (
            "LineChart { data: series.values; height: 90 }",
            "Column[LineChart([0.5, 1.5] [] h=90)]",
        ),
        ("Spinner { size: 32 }", "Column[Spinner(32)]"),
    ] {
        let tree = build_view(&chart_view(body), &e, &tables(), &w)
            .unwrap_or_else(|err| panic!("`{body}` must widen: {err}"));
        assert_eq!(tree.dump(&w), dumped);
    }
}

#[test]
fn choosers_dump_options_and_the_current_index() {
    // The chooser contract in the interp tier: `options:`/`labels:`
    // are a List<String> read, `selected:`/`active:` an Int read, and
    // the dump renders every choice with the index always shown —
    // out of range included (the index is the widget's state, shown
    // verbatim). The tier gate replays this on the demo.
    let (w, e) = charts_world();
    let tree = build_view(
        &chart_view(
            "Select { options: todo.items; selected: counter.count }\n    \
             RadioGroup { options: todo.items; selected: counter.count }\n    \
             TabBar { labels: todo.items; active: counter.count }",
        ),
        &e,
        &tables(),
        &w,
    )
    .expect("builds");
    assert_eq!(
        tree.dump(&w),
        "Column[Select(selected=3)[a, b], RadioGroup(selected=3)[a, b], \
         TabBar(active=3)[a, b]]"
    );
}

#[test]
fn choosers_require_their_props() {
    // Mirrors codegen's required-prop errors, which the emission
    // tests assert on the other tier.
    let (w, e) = charts_world();
    for (body, needle) in [
        ("Select { }", "Select needs `options:`"),
        ("RadioGroup { }", "RadioGroup needs `options:`"),
        ("TabBar { }", "TabBar needs `labels:`"),
        ("Select { options: todo.items }", "Select needs `selected:`"),
        ("TabBar { labels: todo.items }", "TabBar needs `active:`"),
    ] {
        match build_view(&chart_view(body), &e, &tables(), &w) {
            Ok(_) => panic!("`{body}` must error"),
            Err(err) => assert!(
                err.contains(needle),
                "error should name the missing prop: {err}"
            ),
        }
    }
    // Wrongly typed options are rejected, not coerced (the charts'
    // rule); assert on the type name, never the full message.
    match build_view(
        &chart_view("Select { options: series.values; selected: counter.count }"),
        &e,
        &tables(),
        &w,
    ) {
        Ok(_) => panic!("Float options must error, not coerce"),
        Err(err) => assert!(
            err.contains("String"),
            "error should name the expected type: {err}"
        ),
    }
}

fn charts_world() -> (World, FieldEnv) {
    let mut w = World::new();
    let mut values: List<f64> = List::new();
    values.push(0.5);
    values.push(1.5);
    let s = w.insert(Series { values });
    let mut items: List<Str> = List::new();
    items.push(Str::from("a"));
    items.push(Str::from("b"));
    let t = w.insert(Todo { items });
    let c = w.insert(Counter { count: 3 });
    let e = FieldEnv {
        fields: vec![
            ("series".into(), "Series".into(), s.erase()),
            ("todo".into(), "Todo".into(), t.erase()),
            ("counter".into(), "Counter".into(), c.erase()),
        ],
    };
    (w, e)
}

fn chart_view(body: &str) -> LiveView {
    view_of(&format!("{CHART_PRELUDE}{body}\n  }}\n}}\n"))
}

struct Series {
    values: List<f64>,
}

trait SeriesRef: Copy {
    fn values(self, w: &World) -> List<f64>;
}
impl SeriesRef for Handle<Series> {
    fn values(self, w: &World) -> List<f64> {
        w.get(self).values.clone()
    }
}

const CHART_PRELUDE: &str =
    "view Main {\n  let series = Series()\n  let todo = Todo()\n  let counter = Counter()\n  Column {\n    ";

#[test]
fn lazy_rows_build_only_the_requested_range() {
    // A virtualized single-repeater ListView carries LazyRows: the
    // closure materializes exactly the asked-for index window against
    // the live World — §11.17's contract, provable without an engine.
    let mut w = World::new();
    let mut items: List<Str> = List::new();
    for i in 0..100 {
        items.push(Str::from(format!("item {i}")));
    }
    let t = w.insert(Todo { items });
    let e = FieldEnv {
        fields: vec![("todo".into(), "Todo".into(), t.erase())],
    };
    let tb = tables();
    let lv = view_of(
        "view Main {\n  let todo = Todo()\n  Column {\n    ListView {\n      virtualized: true\n      itemHeight: 24.0\n      for it in todo.items {\n        Text { text: it }\n      }\n    }\n  }\n}\n",
    );
    let tree = build_view(&lv, &e, &tb, &w).expect("builds");
    let Element::Column { children, .. } = &tree else {
        panic!("root is a Column");
    };
    let Element::ListView {
        lazy: Some(rows),
        children: static_children,
        ..
    } = &children[0]
    else {
        panic!("virtualized single-repeater list must carry LazyRows");
    };
    assert!(static_children.is_empty());
    assert_eq!(rows.len, 100);
    let window = (rows.build)(&w, 40..43);
    let dumps: Vec<String> = window.iter().map(|c| c.dump(&w)).collect();
    assert_eq!(dumps, ["Text(item 40)", "Text(item 41)", "Text(item 42)"]);
    // Out-of-range asks clamp instead of panicking.
    assert_eq!((rows.build)(&w, 98..200).len(), 2);
    // And the full dump still materializes every row for the gate.
    assert!(tree.dump(&w).contains("Text(item 99)"));
}

#[test]
fn if_in_views_toggles_and_takes_else() {
    // Mirrors codegen: a false bare `if` contributes nothing, an
    // if/else takes exactly one branch, and the condition is strict
    // Bool (reload-time parity — the interpreter is the checker's
    // stand-in during rung 2).
    let mut w = World::new();
    let flag = w.insert(Flag { on: false });
    let c = w.insert(Counter { count: 1 });
    let e = FieldEnv {
        fields: vec![
            ("flag".into(), "Flag".into(), flag.erase()),
            ("counter".into(), "Counter".into(), c.erase()),
        ],
    };
    let tb = tables();
    let src = "view Main {\n  let flag = Flag()\n  let counter = Counter()\n  Column {\n    if flag.on {\n      Text { text: \"visible\" }\n    }\n    if counter.count > 2 {\n      Text { text: \"big\" }\n    } else {\n      Text { text: \"small\" }\n    }\n  }\n}\n";
    let lv = view_of(src);
    assert_eq!(
        build_view(&lv, &e, &tb, &w).expect("builds").dump(&w),
        "Column[Text(small)]"
    );
    w.get_mut(flag).on = true;
    w.get_mut(c).count = 5;
    assert_eq!(
        build_view(&lv, &e, &tb, &w).expect("rebuilds").dump(&w),
        "Column[Text(visible), Text(big)]"
    );

    // Strictness: an Int condition errors instead of truthiness.
    let bad = view_of(
        "view Main {\n  let counter = Counter()\n  Column {\n    if counter.count {\n      Text { text: \"x\" }\n    }\n  }\n}\n",
    );
    assert!(build_view(&bad, &e, &tb, &w).is_err());
}

/// Styles splice on the reload path exactly like the compiled tier
/// (the same `pixie_syntax::style` pass), and a style edit does NOT
/// flip the module fingerprint — it hot-reloads at rung 2 like a
/// view-body edit, while a non-view item edit still flips it.
#[test]
fn styles_splice_at_reload_and_stay_rung_two() {
    let src_a = "style Accent {\n  fontSize: 24.0\n  color: \"#f38ba8\"\n}\n\nview Main {\n  Column {\n    style: Pad\n    Text { style: Accent; text: \"hello\" }\n  }\n}\n\nstyle Pad {\n  spacing: 0.0\n  padding: 12.0\n}\n";
    let (fp_a, lv_a) = reload_from_source(src_a).expect("a reloads");
    let w = World::new();
    let e = FieldEnv { fields: vec![] };
    let tb = tables();
    let tree = build_view(&lv_a, &e, &tb, &w).expect("builds");
    assert_eq!(
        tree.dump(&w),
        "Column(spacing=0, padding=12)[Text(hello, fontSize=24, color=#f38ba8)]"
    );

    // Style-only edit: fingerprint unchanged (rung 2), new value live.
    let src_b = src_a.replace("#f38ba8", "#a6e3a1");
    let (fp_b, lv_b) = reload_from_source(&src_b).expect("b reloads");
    assert_eq!(fp_a, fp_b, "a style edit must classify as rung 2");
    let tree_b = build_view(&lv_b, &e, &tb, &w).expect("builds");
    assert!(tree_b.dump(&w).contains("color=#a6e3a1"));

    // A non-view item edit still flips the fingerprint (rung 1).
    let src_c = format!("{src_a}\nfn helper {{\n}}\n");
    let (fp_c, _) = reload_from_source(&src_c).expect("c reloads");
    assert_ne!(fp_a, fp_c, "an item edit must classify as rung 1");

    // An unknown style at reload is a reported error, not a panic.
    let src_d = src_a.replace("style: Accent", "style: Nope");
    assert!(reload_from_source(&src_d).is_err());
}

/// §8.56, the interpreted half: a `for` body and an `if` branch each
/// hold a run of items, and the two tiers accept the same programs.
/// A divergence HERE is invisible to the tier gate — a program one
/// tier rejects produces no dump to compare.
#[test]
fn for_bodies_and_if_branches_hold_a_run_of_items() {
    let mut w = World::new();
    let c = w.insert(Counter { count: 3 });
    let mut items: List<Str> = List::new();
    items.push(Str::from("a"));
    items.push(Str::from("b"));
    let t = w.insert(Todo { items });
    let e = env(c.erase(), t.erase());
    let tb = tables();

    let lv = view_of(
        "view Main {\n  let counter = Counter()\n  let todo = Todo()\n  Column {\n    for it in todo.items {\n      Text { text: it }\n      Text { text: \"-\" }\n    }\n    if counter.count > 2 {\n      Text { text: \"big\" }\n      Text { text: \"!\" }\n    } else {\n      Text { text: \"small\" }\n    }\n  }\n}\n",
    );
    let tree = build_view(&lv, &e, &tb, &w).expect("builds");
    assert_eq!(
        tree.dump(&w),
        "Column[Text(a), Text(-), Text(b), Text(-), Text(big), Text(!)]"
    );

    // A conditional inside a repeater body, reading the loop variable.
    let lv = view_of(
        "view Main {\n  let todo = Todo()\n  Column {\n    for it in todo.items {\n      Text { text: it }\n      if it == \"a\" {\n        Text { text: \"first\" }\n      }\n    }\n  }\n}\n",
    );
    let tree = build_view(&lv, &e, &tb, &w).expect("builds");
    assert_eq!(tree.dump(&w), "Column[Text(a), Text(first), Text(b)]");

    // A virtualized list still builds one element per row.
    let lv = view_of(
        "view Main {\n  let todo = Todo()\n  Column {\n    ListView {\n      virtualized: true\n      itemHeight: 20.0\n      for it in todo.items {\n        Text { text: it }\n        Text { text: \"-\" }\n      }\n    }\n  }\n}\n",
    );
    match build_view(&lv, &e, &tb, &w) {
        Ok(_) => panic!("a virtualized row must be one element"),
        Err(err) => assert!(
            err.contains("one element per row"),
            "error should say why: {err}"
        ),
    }
}

#[test]
fn text_typography_wrapping_and_box_join_the_dump_only_when_set() {
    // The per-prop rule (ListView's) on the element with the most
    // props: a plain Text dumps exactly as it did before they
    // existed, a flag dumps as its bare name, and an explicitly
    // false flag is the same as an absent one.
    let (w, e) = charts_world();
    let tree = build_view(
        &chart_view(
            "Text { text: \"plain\" }\n    \
             Text { text: \"loud\"; bold: true; italic: true; mono: true; underline: true }\n    \
             Text { text: \"long\"; wrap: \"ellipsis\"; width: 260; maxLines: 2 }\n    \
             Text { text: \"pill\"; background: \"#2fa84f\"; padding: 4; borderRadius: 10; \
             borderWidth: 1; borderColor: \"#11111b\" }\n    \
             Text { text: \"off\"; bold: false; wrap: \"\" }",
        ),
        &e,
        &tables(),
        &w,
    )
    .expect("builds");
    assert_eq!(
        tree.dump(&w),
        "Column[Text(plain), Text(loud, bold, italic, mono, underline), \
         Text(long, wrap=ellipsis, maxLines=2, width=260), \
         Text(pill, bg=#2fa84f, padding=4, radius=10, border=1, borderColor=#11111b), \
         Text(off)]"
    );

    // A value prop is a value: an Int property reaches `maxLines:`,
    // and the same read widens into the Float slot next to it.
    let tree = build_view(
        &chart_view("Text { text: \"bound\"; maxLines: counter.count; width: counter.count }"),
        &e,
        &tables(),
        &w,
    )
    .expect("builds");
    assert_eq!(tree.dump(&w), "Column[Text(bound, maxLines=3, width=3)]");

    // A flag that is not a Bool is refused rather than coerced —
    // assert on the type name, never the whole message.
    match build_view(
        &chart_view("Text { text: \"x\"; bold: counter.count }"),
        &e,
        &tables(),
        &w,
    ) {
        Ok(_) => panic!("an Int in a Bool slot must error"),
        Err(err) => assert!(
            err.contains("Bool"),
            "error should name the expected type: {err}"
        ),
    }
}
