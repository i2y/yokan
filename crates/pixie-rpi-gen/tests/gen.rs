//! The measured acceptance for rpi-gen: real rustdoc JSON (committed
//! fixture, format 61 — must match the pinned rustdoc-types) →
//! generated `.rpi` → parsed back by pixie-binding. Regenerate with:
//!   cd tests/fixture && \
//!     RUSTDOCFLAGS="-Z unstable-options --output-format json" \
//!     cargo +nightly-2026-08-22 doc -q --no-deps && \
//!     cp target/doc/rpi_fixture.json ..

use pixie_rpi_gen::{BindSpec, generate, parse_crate};

fn fixture() -> rustdoc_types::Crate {
    let json = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/rpi_fixture.json"
    ))
    .expect("fixture json (see module docs to regenerate)");
    parse_crate(&json).expect("fixture parses with the pinned rustdoc-types")
}

#[test]
fn binds_the_adapter_surface_and_skips_the_rest() {
    let krate = fixture();
    let (text, reports) = generate(
        &krate,
        &[
            BindSpec {
                module: "rpi_fixture".into(),
                class: "Fixture".into(),
            },
            BindSpec {
                module: "rpi_fixture::inner".into(),
                class: "Inner".into(),
            },
        ],
    )
    .expect("generates");

    for needle in [
        // Scalars, strings (by ref and by value), floats/bools.
        "static fn double(x: Int) Int @rust(\"rpi_fixture::double\")",
        "static fn shout(s: String) String @rust(\"rpi_fixture::shout\")",
        "static fn consume(s: String) Int @rust(\"rpi_fixture::consume\")",
        "static fn scale(v: Float, on: Bool) Float @rust(\"rpi_fixture::scale\")",
        // Fallibles: a concrete error and the io::Result alias.
        "static fn parseFlag(s: String) !Bool @rust(\"rpi_fixture::parse_flag\")",
        "static fn readConfig(path: String) !String @rust(\"rpi_fixture::read_config\")",
        // Two whitelisted AsRef generics (the fs::write shape).
        "static fn writeConfig(path: String, contents: String) !Void @rust(\"rpi_fixture::write_config\")",
        // The adapter-widened shapes: Vec<String> return, PathBuf
        // return (lossy), non-i64 integer return.
        "static fn listNames() !List<String> @rust(\"rpi_fixture::list_names\")",
        "static fn whereIs(p: String) !String @rust(\"rpi_fixture::where_is\")",
        "static fn sizeOf(s: String) Int @rust(\"rpi_fixture::size_of\")",
        // Option returns → `T?` (§11.11), lossy inner included.
        "static fn findFlag(s: String) Int? @rust(\"rpi_fixture::find_flag\")",
        "static fn maybe(x: Int) Int? @rust(\"rpi_fixture::maybe\")",
        "static fn homeDir() String? @rust(\"rpi_fixture::home_dir\")",
        // Bytes (§11.10): Vec<u8> return and &[u8] param.
        "static fn blob() Bytes @rust(\"rpi_fixture::blob\")",
        "static fn digest(data: Bytes) Int @rust(\"rpi_fixture::digest\")",
        // Nested module in its own class.
        "class Inner {",
        "static fn ping() Int @rust(\"rpi_fixture::inner::ping\")",
        // §8.76: a C-like enum is DECLARED with its Rust counterpart,
        // and every variant keeps its own name — so the
        // correspondence needs one attribute, not one per variant.
        "enum Level @rust(\"rpi_fixture::Level\") {",
        "  Low",
        "  High",
        // ...which is what lets these two name it, in both positions.
        "static fn levelOf(x: Int) Level @rust(\"rpi_fixture::level_of\")",
        "static fn levelName(l: Level) String @rust(\"rpi_fixture::level_name\")",
        // §8.76: `Vec<T>` and `Option<T>` PARAMETERS, which the
        // call-site adapter has taken since §8.73.
        "static fn joinAll(parts: List<String>) String @rust(\"rpi_fixture::join_all\")",
        "static fn orDefault(v: Int?) Int @rust(\"rpi_fixture::or_default\")",
        // §8.77: a plain struct is DECLARED field for field. The
        // names camel-case, and only a field whose two conventions
        // disagree carries its own attribute.
        "struct Stat @rust(\"rpi_fixture::Stat\") {",
        "  var byteLen : Int @rust(\"byte_len\")",
        "  var name : String",
        "  var level : Level",
        // A field crosses by the same rule the whole value does, so a
        // struct may hold a struct, or a list of one.
        "struct Report @rust(\"rpi_fixture::Report\") {",
        "  var head : Stat",
        "  var rest : List<Stat>",
        // ...and the fns can then name them, in both positions and
        // inside a list or an optional.
        "static fn statOf(s: String) Stat @rust(\"rpi_fixture::stat_of\")",
        "static fn statLine(st: Stat) String @rust(\"rpi_fixture::stat_line\")",
        "static fn reportOf(s: String) Report @rust(\"rpi_fixture::report_of\")",
        "static fn statLines(sts: List<Stat>) List<String> @rust(\"rpi_fixture::stat_lines\")",
        "static fn levelOr(l: Level?) String @rust(\"rpi_fixture::level_or\")",
        // §8.78: a field names its Rust TYPE when reading it back is
        // not enough. A `u64` widens into `Int` on the way here and
        // has to hit the width exactly on the way out.
        "struct Wide @rust(\"rpi_fixture::Wide\") {",
        "  var count : Int @rust(\"count: u64\")",
        // ...and a TUPLE struct names positions. A newtype's one
        // field is `value`; a wider one numbers them.
        "struct Meters @rust(\"rpi_fixture::Meters\") {",
        "  var value : Float @rust(\"0\")",
        "struct Span @rust(\"rpi_fixture::Span\") {",
        "  var field0 : Int @rust(\"0\")",
        "  var field1 : String @rust(\"1\")",
        "static fn metersOf(v: Float) Meters @rust(\"rpi_fixture::meters_of\")",
        "static fn metersShow(m: Meters) String @rust(\"rpi_fixture::meters_show\")",
        "static fn wideOf(n: Int) Wide @rust(\"rpi_fixture::wide_of\")",
    ] {
        assert!(text.contains(needle), "generated .rpi lacks `{needle}`:\n{text}");
    }

    let fx = &reports[0];
    assert_eq!(fx.bound.len(), 28, "bound: {:?}", fx.bound);
    let skipped: Vec<&str> = fx.skipped.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        skipped,
        [
            "bump", "holder_of", "narrow", "opaque_of", "sealed_of", "shape_of", "show",
            "sum", "twice", "widths_of"
        ],
        "skip set changed: {:?}",
        fx.skipped
    );
    // A PAYLOAD-bearing enum is not declared, so a fn returning one
    // is skipped rather than mis-declared: the correspondence §8.74
    // generates is a match over unit variants.
    assert!(
        !text.contains("enum Shape"),
        "a payload enum must not be declared:\n{text}"
    );
    // Nor is a struct pixie could not build: `Opaque` has a private
    // field, and `Holder`'s only field is that payload enum. Each
    // reaches the report through the fn that returns it.
    assert!(
        !text.contains("struct Opaque") && !text.contains("struct Holder"),
        "a struct with an uncrossable field must not be declared:\n{text}"
    );
    // Nor a TUPLE struct with a private field (position matters, so
    // pixie could not build one), nor a field pixie cannot write at
    // all: `Widths.counts` is a `Vec<u32>`, and the per-field
    // attribute names ONE type, so an element needing its own has
    // nowhere to say it.
    assert!(
        !text.contains("struct Sealed") && !text.contains("struct Widths"),
        "a field pixie cannot fill or write must not be declared:\n{text}"
    );
    // Reasons are human-readable and name the offending piece.
    assert!(fx.skipped.iter().any(|(n, r)| n == "bump" && r.contains("&mut")));
    assert!(fx.skipped.iter().any(|(n, r)| n == "narrow" && r.contains("i32")));
    assert!(fx.skipped.iter().any(|(n, r)| n == "twice" && r.contains("deprecated")));

    // Round trip: the emitted file is a valid binding module.
    let mut sm = pixie_syntax::SourceMap::default();
    let module = pixie_binding::parse_rpi(&mut sm, "generated.rpi", &text)
        .expect("generated .rpi parses as a binding module");
    let classes: Vec<_> = module
        .items
        .iter()
        .filter_map(|i| match i {
            pixie_syntax::ast::Item::Class(c) => Some(c.name.name.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(classes, ["Fixture", "Inner"]);
}

#[test]
fn unknown_module_reports_the_available_ones() {
    let krate = fixture();
    let err = generate(
        &krate,
        &[BindSpec {
            module: "rpi_fixture::nope".into(),
            class: "X".into(),
        }],
    )
    .unwrap_err();
    assert!(err.contains("not found"), "{err}");
    assert!(err.contains("rpi_fixture::inner"), "{err}");
}
