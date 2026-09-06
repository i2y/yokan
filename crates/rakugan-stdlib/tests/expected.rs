//! What perl answers, against the twins the compiled run links.
//!
//! The gate proves the two runs AGREE. It cannot prove they agree with
//! perl: a twin that is wrong the same way in both runs still passes
//! it. The interpreted run IS perl, so holding a twin to what perl
//! printed holds it to the other run's meaning too.
//!
//! Tables live in `tests/expected/` and are written by
//! `rakugan/tools/gen_expected.pl`, never by hand. A row is
//! `name arg… -> result`, each value tagged by type, doubles in hex,
//! a list in brackets. `~>` in place of `->` allows one ulp, for an
//! answer the platform's own libm decides.

use rakugan_stdlib::*;

#[derive(Debug, Clone, PartialEq)]
enum V {
    I(i64),
    F(f64),
    B(bool),
    S(String),
    L(Vec<V>),
}

impl V {
    fn i(&self) -> i64 {
        match self {
            V::I(v) => *v,
            other => panic!("wanted a whole number, table says {other:?}"),
        }
    }
    fn f(&self) -> f64 {
        match self {
            V::F(v) => *v,
            other => panic!("wanted a number, table says {other:?}"),
        }
    }
    fn b(&self) -> bool {
        match self {
            V::B(v) => *v,
            other => panic!("wanted a bool, table says {other:?}"),
        }
    }
    fn s(&self) -> String {
        match self {
            V::S(v) => v.clone(),
            other => panic!("wanted a string, table says {other:?}"),
        }
    }
    fn is(&self) -> Vec<i64> {
        self.list().iter().map(V::i).collect()
    }
    fn fs(&self) -> Vec<f64> {
        self.list().iter().map(V::f).collect()
    }
    fn ss(&self) -> Vec<String> {
        self.list().iter().map(V::s).collect()
    }
    fn list(&self) -> &[V] {
        match self {
            V::L(v) => v,
            other => panic!("wanted a list, table says {other:?}"),
        }
    }
}

fn unquote(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() + 1 && i + 3 <= b.len() {
            out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).expect("percent escape"));
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).expect("utf-8")
}

/// Split on commas at depth zero — a list inside a list has its own.
fn split_top(body: &str) -> Vec<&str> {
    let (mut out, mut depth, mut start) = (Vec::new(), 0i32, 0usize);
    for (i, c) in body.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => depth -= 1,
            ',' if depth == 0 => {
                out.push(&body[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&body[start..]);
    out
}

fn parse(cell: &str) -> V {
    if let Some(body) = cell.strip_prefix('[').and_then(|c| c.strip_suffix(']')) {
        if body.is_empty() {
            return V::L(Vec::new());
        }
        return V::L(split_top(body).into_iter().map(parse).collect());
    }
    let (tag, body) = cell.split_once(':').unwrap_or_else(|| panic!("untagged cell `{cell}`"));
    match tag {
        "i" => V::I(body.parse().expect("whole number")),
        "f" => V::F(f64::from_bits(u64::from_str_radix(body, 16).expect("hex double"))),
        "b" => V::B(body == "1"),
        "s" => V::S(unquote(body)),
        other => panic!("unknown tag `{other}` in `{cell}`"),
    }
}

/// Two doubles the table calls the same: bit equality, with one ulp of
/// slack on a row the platform's libm decided.
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

fn check(table: &str, call: impl Fn(&str, &[V]) -> V) {
    let text = std::fs::read_to_string(format!("tests/expected/{table}"))
        .unwrap_or_else(|e| panic!("tests/expected/{table}: {e}"));
    let mut n = 0;
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let (ulp, arrow) = if line.contains(" ~> ") { (true, " ~> ") } else { (false, " -> ") };
        let (call_text, want_text) = line
            .split_once(arrow)
            .unwrap_or_else(|| panic!("no arrow in `{line}`"));
        let mut cells = call_text.split(' ');
        let name = cells.next().expect("a name");
        let args: Vec<V> = cells.map(parse).collect();
        let want = parse(want_text);
        let got = call(name, &args);
        let same = match (&got, &want) {
            (V::F(a), V::F(b)) => same_float(*a, *b, ulp),
            (a, b) => a == b,
        };
        assert!(same, "{table}: {name} answered {got:?}, perl says {want:?}\n  {line}");
        n += 1;
    }
    assert!(n > 0, "{table} has no rows");
}

fn dispatch(name: &str, a: &[V]) -> V {
    match name {
        // strings
        "length_of" => V::I(length_of(&a[0].s())),
        "uc" => V::S(uc(&a[0].s())),
        "lc" => V::S(lc(&a[0].s())),
        "ucfirst" => V::S(ucfirst(&a[0].s())),
        "lcfirst" => V::S(lcfirst(&a[0].s())),
        "reverse_str" => V::S(reverse_str(&a[0].s())),
        "substr_from" => V::S(substr_from(&a[0].s(), a[1].i())),
        "substr_len" => V::S(substr_len(&a[0].s(), a[1].i(), a[2].i())),
        "index_of" => V::I(index_of(&a[0].s(), &a[1].s())),
        "index_from" => V::I(index_from(&a[0].s(), &a[1].s(), a[2].i())),
        "rindex_of" => V::I(rindex_of(&a[0].s(), &a[1].s())),
        "rindex_from" => V::I(rindex_from(&a[0].s(), &a[1].s(), a[2].i())),
        "join" => V::S(join(&a[0].s(), a[1].ss())),
        "split_on" => V::L(split_on(&a[0].s(), &a[1].s()).into_iter().map(V::S).collect()),
        "split_words" => V::L(split_words(&a[0].s()).into_iter().map(V::S).collect()),
        // numbers
        "abs_num" => V::F(abs_num(a[0].f())),
        "abs_int" => V::I(abs_int(a[0].i())),
        "sqrt_of" => V::F(sqrt_of(a[0].f())),
        "floor_of" => V::F(floor_of(a[0].f())),
        "ceil_of" => V::F(ceil_of(a[0].f())),
        "fmod_of" => V::F(fmod_of(a[0].f(), a[1].f())),
        "mod_int" => V::I(mod_int(a[0].i(), a[1].i())),
        "div_int" => V::F(div_int(a[0].i(), a[1].i())),
        "int_of" => V::I(int_of(a[0].f())),
        "num_of" => V::F(num_of(&a[0].s())),
        "num_text" => V::S(num_text(a[0].f())),
        "bool_text" => V::S(bool_text(a[0].b())),
        // List::Util
        "sum_int" => V::I(sum_int(a[0].is())),
        "sum_num" => V::F(sum_num(a[0].fs())),
        "max_int" => V::I(max_int(a[0].is())),
        "min_int" => V::I(min_int(a[0].is())),
        "max_num" => V::F(max_num(a[0].fs())),
        "min_num" => V::F(min_num(a[0].fs())),
        "uniq_int" => V::L(uniq_int(a[0].is()).into_iter().map(V::I).collect()),
        "uniq_str" => V::L(uniq_str(a[0].ss()).into_iter().map(V::S).collect()),
        // sprintf
        "fmt_num" => V::S(fmt_num(&a[0].s(), a[1].f())),
        "fmt_int" => V::S(fmt_int(&a[0].s(), a[1].i())),
        "fmt_str" => V::S(fmt_str(&a[0].s(), &a[1].s())),
        // POSIX
        "strftime_utc" => V::S(strftime_utc(&a[0].s(), a[1].i())),
        other => panic!("the table names `{other}`, which no twin answers"),
    }
}

#[test]
fn strings_match_perl() {
    check("strings.txt", dispatch);
}

#[test]
fn numbers_match_perl() {
    check("numbers.txt", dispatch);
}

#[test]
fn list_util_matches_perl() {
    check("list_util.txt", dispatch);
}

#[test]
fn sprintf_matches_perl() {
    check("sprintf.txt", dispatch);
}

#[test]
fn time_matches_perl() {
    check("time.txt", dispatch);
}
