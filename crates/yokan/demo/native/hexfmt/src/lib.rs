//! The user-crate demo target: plain Rust — no pyo3, no yokan types.
//! `[tool.yokan.crates]` binds it into the app, and BOTH runs call
//! these exact functions.

pub fn encode(s: &str) -> String {
    s.bytes().map(|b| format!("{b:02x}")).collect()
}

pub fn add(a: i64, b: i64) -> i64 {
    a + b
}

pub fn avg(xs: Vec<f64>) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    let n = xs.len() as f64;
    xs.iter().sum::<f64>() / n
}

pub fn halve(n: i64) -> Option<i64> {
    if n % 2 == 0 { Some(n / 2) } else { None }
}

pub fn greet(name: Option<&str>) -> String {
    match name {
        Some(n) => format!("hi {n}"),
        None => "hi anon".to_string(),
    }
}

pub fn parse_even(s: &str) -> Result<i64, String> {
    let n: i64 = s.parse().map_err(|_| format!("not a number: {s}"))?;
    if n % 2 != 0 {
        return Err(format!("odd: {n}"));
    }
    Ok(n)
}

pub struct Span {
    pub lo: i64,
    pub hi: i64,
}

pub fn width(s: Span) -> i64 {
    s.hi - s.lo
}

pub fn shift(s: Span, by: i64) -> Span {
    Span { lo: s.lo + by, hi: s.hi + by }
}

pub enum Grade {
    Fine,
    Odd,
}

pub fn judge(n: i64) -> Grade {
    if n % 2 == 0 { Grade::Fine } else { Grade::Odd }
}

pub fn describe(g: Grade) -> String {
    match g {
        Grade::Fine => "fine and even".to_string(),
        Grade::Odd => "odd one out".to_string(),
    }
}

pub struct Packed {
    pub id: u32,
    pub weight: i64,
}

pub fn pack(id: i64, weight: i64) -> Packed {
    Packed { id: id as u32, weight }
}

pub fn heavier(p: Packed, than: i64) -> bool {
    p.weight > than
}

pub fn parse_all(s: &str) -> Result<Vec<i64>, String> {
    s.split(',')
        .map(|p| p.trim().parse::<i64>().map_err(|_| format!("bad number: {p}")))
        .collect()
}

/// How many times each character occurs in `s`.
pub fn char_counts(s: &str) -> std::collections::HashMap<String, i64> {
    let mut m = std::collections::HashMap::new();
    for c in s.chars() {
        *m.entry(c.to_string()).or_insert(0) += 1;
    }
    m
}

/// Sum of all counts in the map.
pub fn total_counts(m: std::collections::HashMap<String, i64>) -> i64 {
    m.values().sum()
}

/// A span plus its payload — a struct holding structs.
pub struct Framed {
    pub span: Span,
    pub packed: Packed,
}

/// Wrap a span and a packed payload together.
pub fn frame(s: Span, p: Packed) -> Framed {
    Framed { span: s, packed: p }
}

/// Width of the span plus the payload's weight.
pub fn frame_sum(f: Framed) -> i64 {
    (f.span.hi - f.span.lo) + f.packed.weight
}
