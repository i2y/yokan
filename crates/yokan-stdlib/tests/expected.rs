//! CPython's own answers, against the twins the compiled run links.
//!
//! The gate proves the two runs AGREE. It cannot prove they agree
//! with Python: a twin that is wrong the same way in both runs still
//! passes it. That is what these tables are for — the interpreted run
//! is CPython itself, so holding a twin to CPython's printed answers
//! holds it to the other run's meaning too.
//!
//! Tables live in `tests/expected/` and are written by
//! `crates/yokan/tools/gen_expected.py`, never by hand. Each row is
//! `name arg… -> result`, values tagged by type, doubles in hex.
//! `~>` in place of `->` means the answer comes from the platform's
//! libm rather than from IEEE-754, so one ulp of slack is allowed.

use std::panic::{self, AssertUnwindSafe};
use yokan_stdlib::*;

#[derive(Debug, Clone, PartialEq)]
enum V {
    I(i64),
    F(f64),
    B(bool),
    S(String),
    /// What CPython raised: the class, and the message it carried.
    Raise(String, String),
}

impl V {
    fn f(&self) -> f64 {
        match self {
            V::F(x) => *x,
            other => panic!("wanted a float argument, table says {other:?}"),
        }
    }
}

fn unquote(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).expect("percent escape"));
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).expect("utf-8")
}

fn parse(cell: &str) -> V {
    if let Some(rest) = cell.strip_prefix('!') {
        let (class, msg) = rest.split_once(':').unwrap_or((rest, ""));
        return V::Raise(class.to_string(), unquote(msg));
    }
    let (tag, body) = cell.split_once(':').unwrap_or_else(|| panic!("untagged cell `{cell}`"));
    match tag {
        "i" => V::I(body.parse().expect("int")),
        "f" => V::F(f64::from_bits(u64::from_str_radix(body, 16).expect("hex double"))),
        "b" => V::B(body == "1"),
        "s" => V::S(unquote(body)),
        other => panic!("unknown tag `{other}` in `{cell}`"),
    }
}

/// Two doubles the table calls the same. Bit equality, except that
/// every NaN counts as the one NaN (the bit pattern is not specified)
/// and a libm row allows a single ulp.
fn same_float(got: f64, want: f64, ulp: bool) -> bool {
    if got.is_nan() && want.is_nan() {
        return true;
    }
    if got.to_bits() == want.to_bits() {
        return true;
    }
    if !ulp || got.is_nan() || want.is_nan() {
        return false;
    }
    let (a, b) = (got.to_bits() as i64, want.to_bits() as i64);
    got.is_sign_negative() == want.is_sign_negative() && (a - b).abs() <= 1
}

/// Run one table against a dispatcher. The dispatcher answers `None`
/// for a row it does not know, which fails the test rather than
/// passing quietly — a table row with no twin behind it is a gap.
fn check(table: &str, name: &str, call: impl Fn(&str, &[V]) -> Option<V>) {
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let mut failures: Vec<String> = Vec::new();
    for line in table.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let cells: Vec<&str> = line.split(' ').collect();
        let arrow = cells
            .iter()
            .position(|c| *c == "->" || *c == "~>")
            .unwrap_or_else(|| panic!("no arrow in `{line}`"));
        let fname = cells[0];
        let args: Vec<V> = cells[1..arrow].iter().map(|c| parse(c)).collect();
        let want = parse(cells[arrow + 1]);
        let ulp = cells[arrow] == "~>";
        let got = panic::catch_unwind(AssertUnwindSafe(|| {
            call(fname, &args).unwrap_or_else(|| panic!("__no_twin__"))
        }));
        let ok = match (&got, &want) {
            (Ok(V::F(g)), V::F(w)) => same_float(*g, *w, ulp),
            (Ok(g), w) => g == w,
            (Err(e), V::Raise(_class, msg)) => panic_msg(e) == *msg,
            (Err(_), _) => false,
        };
        if !ok {
            let shown = match &got {
                Ok(v) => format!("{v:?}"),
                Err(e) => format!("raised {:?}", panic_msg(e)),
            };
            failures.push(format!("  {line}\n      twin answered {shown}"));
        }
    }
    panic::set_hook(hook);
    assert!(
        failures.is_empty(),
        "{} of {name}'s rows disagree with CPython:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn panic_msg(e: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = e.downcast_ref::<&str>() {
        return (*s).to_string();
    }
    if let Some(s) = e.downcast_ref::<String>() {
        return s.clone();
    }
    "<non-string panic>".to_string()
}

#[test]
fn math_matches_cpython() {
    check(
        include_str!("expected/math.txt"),
        "math",
        |fname, a| match fname {
            "sqrt" => Some(V::F(math_sqrt(a[0].f()))),
            "sin" => Some(V::F(math_sin(a[0].f()))),
            "cos" => Some(V::F(math_cos(a[0].f()))),
            "fabs" => Some(V::F(math_fabs(a[0].f()))),
            "floor" => Some(V::I(math_floor(a[0].f()))),
            "ceil" => Some(V::I(math_ceil(a[0].f()))),
            "pow" => Some(V::F(math_pow(a[0].f(), a[1].f()))),
            "pi" => Some(V::F(math_pi())),
            _ => None,
        },
    );
}
