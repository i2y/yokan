//! Shapes rpi-gen must bind — and shapes it must skip-and-report.

use std::path::Path;

/// Plain scalars in and out.
pub fn double(x: i64) -> i64 {
    x * 2
}

/// Strings by reference and by value.
pub fn shout(s: &str) -> String {
    s.to_uppercase()
}

/// Owned-String parameter.
pub fn consume(s: String) -> i64 {
    s.len() as i64
}

/// Fallible with a concrete error.
pub fn parse_flag(s: &str) -> Result<bool, std::num::ParseIntError> {
    Ok(s.parse::<i64>()? != 0)
}

/// The std::fs shape: AsRef generics, io::Result alias.
pub fn read_config<P: AsRef<Path>>(path: P) -> std::io::Result<String> {
    std::fs::read_to_string(path)
}

/// Two whitelisted generics (the fs::write shape).
pub fn write_config<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> std::io::Result<()> {
    std::fs::write(path, contents)
}

/// Floats and bools.
pub fn scale(v: f64, on: bool) -> f64 {
    if on { v * 2.0 } else { v }
}

/// List return (Vec<String> → List<String>).
pub fn list_names() -> std::io::Result<Vec<String>> {
    Ok(vec!["a".into(), "b".into()])
}

/// PathBuf return (lossy → String).
pub fn where_is<P: AsRef<Path>>(p: P) -> std::io::Result<std::path::PathBuf> {
    std::fs::canonicalize(p)
}

/// Non-i64 integer return (widened via `as i64`).
pub fn size_of(s: &str) -> u64 {
    s.len() as u64
}

// ---- must be skipped, each for a reported reason ----

/// Unsupported parameter type (slice of ints).
pub fn sum(xs: &[i64]) -> i64 {
    xs.iter().sum()
}

/// Unsupported return (Option).
pub fn maybe(x: i64) -> Option<i64> {
    (x > 0).then_some(x)
}

/// &mut parameter.
pub fn bump(x: &mut i64) {
    *x += 1;
}

/// Non-i64 integer width.
pub fn narrow(x: i32) -> i32 {
    x
}

/// Free generic with a non-whitelisted bound.
pub fn show<T: std::fmt::Debug>(t: T) -> String {
    format!("{t:?}")
}

/// Deprecated API (must be skipped).
#[deprecated(note = "use double")]
pub fn twice(x: i64) -> i64 {
    x * 2
}

pub mod inner {
    /// Nested module fn (filtered separately).
    pub fn ping() -> i64 {
        1
    }
}

/// Option return — the `T?` shape (§11.11).
pub fn find_flag(s: &str) -> Option<i64> {
    if s.is_empty() { None } else { Some(s.len() as i64) }
}

/// Option with a lossy inner (PathBuf → String?).
pub fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(Into::into)
}

/// Vec<u8> return — Bytes, not List<Int> (§11.10).
pub fn blob() -> Vec<u8> {
    vec![1, 2, 3]
}

/// &[u8] param — Bytes in, `.as_slice()` at the call site.
pub fn digest(data: &[u8]) -> i64 {
    data.iter().map(|b| *b as i64).sum()
}

/// A C-like enum: rpi-gen declares it with its Rust counterpart, and
/// the two fns below can then name it (§8.76).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Low,
    High,
}

/// Enum in the RETURN position.
pub fn level_of(x: i64) -> Level {
    if x > 10 { Level::High } else { Level::Low }
}

/// Enum in the ARGUMENT position.
pub fn level_name(l: Level) -> String {
    match l {
        Level::Low => "low".to_string(),
        Level::High => "high".to_string(),
    }
}

/// A payload-bearing enum is NOT declared: the correspondence is a
/// match over unit variants, and a payload would need its fields
/// related too. The fn below is skipped with a reason.
#[derive(Debug, Clone)]
pub enum Shape {
    Dot,
    Line(i64),
}

pub fn shape_of(n: i64) -> Shape {
    if n == 0 { Shape::Dot } else { Shape::Line(n) }
}

/// `Vec<T>` parameter — the adapter has taken one since §8.73.
pub fn join_all(parts: Vec<String>) -> String {
    parts.join("/")
}

/// `Option<T>` parameter.
pub fn or_default(v: Option<i64>) -> i64 {
    v.unwrap_or(-1)
}

/// A plain struct: rpi-gen declares it field for field, camel-casing
/// the names and writing `@rust(..)` only where the two conventions
/// disagree (§8.77).
#[derive(Debug, Clone, PartialEq)]
pub struct Stat {
    pub byte_len: i64,
    pub name: String,
    pub level: Level,
}

/// Struct in the RETURN position.
pub fn stat_of(s: &str) -> Stat {
    Stat {
        byte_len: s.len() as i64,
        name: s.to_string(),
        level: level_of(s.len() as i64),
    }
}

/// Struct in the ARGUMENT position.
pub fn stat_line(st: Stat) -> String {
    format!("{} {}", st.name, st.byte_len)
}

/// A struct holding a struct, and a list of them — a field crosses by
/// the same rule the whole value does.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub title: String,
    pub head: Stat,
    pub rest: Vec<Stat>,
}

pub fn report_of(s: &str) -> Report {
    Report {
        title: s.to_string(),
        head: stat_of(s),
        rest: vec![stat_of("a")],
    }
}

/// A struct with a PRIVATE field cannot be declared: pixie could not
/// fill it, so the fn returning one is skipped with a reason.
#[derive(Debug, Clone)]
pub struct Opaque {
    pub shown: i64,
    hidden: i64,
}

pub fn opaque_of(n: i64) -> Opaque {
    Opaque {
        shown: n,
        hidden: n,
    }
}

/// A struct whose FIELD cannot cross is not declared either — the
/// payload enum above is exactly such a field.
#[derive(Debug, Clone)]
pub struct Holder {
    pub shape: Shape,
}

pub fn holder_of(n: i64) -> Holder {
    Holder { shape: shape_of(n) }
}

/// A LIST of a declared type, both ways — the element rule is the
/// whole-value rule (§8.77).
pub fn stat_lines(sts: Vec<Stat>) -> Vec<String> {
    sts.iter().map(|s| stat_line(s.clone())).collect()
}

/// An OPTIONAL declared type in the argument position.
pub fn level_or(l: Option<Level>) -> String {
    level_name(l.unwrap_or(Level::Low))
}

/// A field that crosses ONE way only: `u64` widens into `Int` on the
/// way back, but the argument side produces an `i64`, and a struct
/// field is written in both directions. Skipped with a reason rather
/// than declared into a rustc error.
#[derive(Debug, Clone)]
pub struct Wide {
    pub count: u64,
}

pub fn wide_of(n: i64) -> Wide {
    Wide { count: n as u64 }
}

/// A newtype — one unnamed field, which pixie reaches as `value`
/// (§8.78). Rust's own `.0` is what the correspondence writes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Meters(pub f64);

pub fn meters_of(v: f64) -> Meters {
    Meters(v)
}

pub fn meters_show(m: Meters) -> String {
    format!("{} m", m.0)
}

/// A wider tuple struct: fields are numbered on this side.
#[derive(Debug, Clone, PartialEq)]
pub struct Span(pub i64, pub String);

pub fn span_of(n: i64) -> Span {
    Span(n, "span".to_string())
}

/// A tuple struct with a PRIVATE field cannot be built from pixie, so
/// the fn returning one is skipped.
#[derive(Debug, Clone)]
pub struct Sealed(pub i64, i64);

pub fn sealed_of(n: i64) -> Sealed {
    Sealed(n, n)
}

/// A field whose type pixie cannot write AT ALL: an element that
/// would need its own annotation, and the per-field attribute names
/// one type.
#[derive(Debug, Clone)]
pub struct Widths {
    pub counts: Vec<u32>,
}

pub fn widths_of(n: i64) -> Widths {
    Widths {
        counts: vec![n as u32],
    }
}
