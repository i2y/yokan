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
//! `name arg… -> result`, values tagged by type, doubles in hex; a
//! list is `[v,v,…]`.
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
    L(Vec<V>),
    /// An object, in the order its keys went in.
    O(Vec<(String, V)>),
    /// What a function answers when it answers nothing.
    Unit,
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
    fn i(&self) -> i64 {
        match self {
            V::I(x) => *x,
            other => panic!("wanted an int argument, table says {other:?}"),
        }
    }
    fn fs(&self) -> Vec<f64> {
        match self {
            V::L(xs) => xs.iter().map(|x| x.f()).collect(),
            other => panic!("wanted a list of floats, table says {other:?}"),
        }
    }
    fn s(&self) -> String {
        match self {
            V::S(x) => x.clone(),
            other => panic!("wanted a str argument, table says {other:?}"),
        }
    }
    fn b(&self) -> bool {
        match self {
            V::B(x) => *x,
            other => panic!("wanted a bool argument, table says {other:?}"),
        }
    }
    fn list(&self) -> &[V] {
        match self {
            V::L(xs) => xs,
            other => panic!("wanted a list, table says {other:?}"),
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

/// Split on commas at depth zero — a list inside a list carries
/// commas of its own.
fn split_top(body: &str) -> Vec<&str> {
    let (mut out, mut depth, mut start) = (Vec::new(), 0i32, 0usize);
    for (i, c) in body.char_indices() {
        match c {
            '[' | '{' => depth += 1,
            ']' | '}' => depth -= 1,
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
    if let Some(body) = cell.strip_prefix('{').and_then(|c| c.strip_suffix('}')) {
        if body.is_empty() {
            return V::O(Vec::new());
        }
        return V::O(
            split_top(body)
                .into_iter()
                .map(|pair| {
                    let (k, v) = pair.split_once('=').expect("key=value");
                    match parse(k) {
                        V::S(k) => (k, parse(v)),
                        other => panic!("an object key is a str, table says {other:?}"),
                    }
                })
                .collect(),
        );
    }
    if let Some(rest) = cell.strip_prefix('!') {
        let (class, msg) = rest.split_once(':').unwrap_or((rest, ""));
        return V::Raise(class.to_string(), unquote(msg));
    }
    let (tag, body) = cell.split_once(':').unwrap_or_else(|| panic!("untagged cell `{cell}`"));
    match tag {
        "u" => V::Unit,
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
    // The panic hook is process-wide, so the tables run one at a
    // time rather than shouting over each other's silence.
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _lock = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
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
    check(include_str!("expected/math.txt"), "math", |fname, a| {
        let f1 = |g: fn(f64) -> f64| Some(V::F(g(a[0].f())));
        let f2 = |g: fn(f64, f64) -> f64| Some(V::F(g(a[0].f(), a[1].f())));
        let i1 = |g: fn(f64) -> i64| Some(V::I(g(a[0].f())));
        let b1 = |g: fn(f64) -> bool| Some(V::B(g(a[0].f())));
        let n2 = |g: fn(i64, i64) -> i64| Some(V::I(g(a[0].i(), a[1].i())));
        match fname {
            "sqrt" => f1(math_sqrt),
            "sin" => f1(math_sin),
            "cos" => f1(math_cos),
            "tan" => f1(math_tan),
            "sinh" => f1(math_sinh),
            "cosh" => f1(math_cosh),
            "tanh" => f1(math_tanh),
            "asin" => f1(math_asin),
            "acos" => f1(math_acos),
            "atan" => f1(math_atan),
            "asinh" => f1(math_asinh),
            "acosh" => f1(math_acosh),
            "atanh" => f1(math_atanh),
            "cbrt" => f1(math_cbrt),
            "exp" => f1(math_exp),
            "exp2" => f1(math_exp2),
            "expm1" => f1(math_expm1),
            "log1p" => f1(math_log1p),
            "log2" => f1(math_log2),
            "log10" => f1(math_log10),
            "degrees" => f1(math_degrees),
            "radians" => f1(math_radians),
            "fabs" => f1(math_fabs),
            "ulp" => f1(math_ulp),
            "log" if a.len() == 1 => f1(math_log),
            "log" => f2(math_log_base),
            "floor" => i1(math_floor),
            "ceil" => i1(math_ceil),
            "trunc" => i1(math_trunc),
            "isnan" => b1(math_isnan),
            "isinf" => b1(math_isinf),
            "isfinite" => b1(math_isfinite),
            "atan2" => f2(math_atan2),
            "copysign" => f2(math_copysign),
            "fmod" => f2(math_fmod),
            "remainder" => f2(math_remainder),
            "hypot" => f2(math_hypot),
            "nextafter" => f2(math_nextafter),
            "pow" => f2(math_pow),
            "isclose" => Some(V::B(math_isclose(a[0].f(), a[1].f()))),
            "ldexp" => Some(V::F(math_ldexp(a[0].f(), a[1].i()))),
            "fma" => Some(V::F(math_fma(a[0].f(), a[1].f(), a[2].f()))),
            "factorial" => Some(V::I(math_factorial(a[0].i()))),
            "isqrt" => Some(V::I(math_isqrt(a[0].i()))),
            "comb" => n2(math_comb),
            "perm" => n2(math_perm),
            "gcd" => n2(math_gcd),
            "lcm" => n2(math_lcm),
            "fsum" => Some(V::F(math_fsum(a[0].fs()))),
            "dist" => Some(V::F(math_dist(a[0].fs(), a[1].fs()))),
            "pi" => Some(V::F(math_pi())),
            "e" => Some(V::F(math_e())),
            "tau" => Some(V::F(math_tau())),
            "inf" => Some(V::F(math_inf())),
            "nan" => Some(V::F(math_nan())),
            _ => None,
        }
    });
}


/// `random` is stateful, so the rows run in file order and the twin's
/// generator carries between them — the same way CPython's
/// module-level one does. Every sequence in the table starts from a
/// `seed`, which is the only way a sequence can be written down.
#[test]
fn random_matches_cpython() {
    check(include_str!("expected/random.txt"), "random", |fname, a| {
        // `choice` and `sample` answer an element, so which twin runs
        // follows what the list holds — the same choice the
        // translator makes from the declared type.
        fn pick<T: Clone>(xs: &[V], get: fn(&V) -> T) -> Vec<T> {
            xs.iter().map(get).collect()
        }
        match fname {
            "seed" => {
                random_seed(a[0].i());
                Some(V::Unit)
            }
            "random" => Some(V::F(random_random())),
            "getrandbits" => Some(V::I(random_getrandbits(a[0].i()))),
            "randint" => Some(V::I(random_randint(a[0].i(), a[1].i()))),
            "randrange" if a.len() == 1 => Some(V::I(random_randrange(a[0].i()))),
            "randrange" if a.len() == 2 => Some(V::I(random_randrange_from(a[0].i(), a[1].i()))),
            "randrange" => Some(V::I(random_randrange_step(a[0].i(), a[1].i(), a[2].i()))),
            "uniform" => Some(V::F(random_uniform(a[0].f(), a[1].f()))),
            "gauss" => Some(V::F(random_gauss(a[0].f(), a[1].f()))),
            "choice" => Some(match a[0].list().first() {
                Some(V::S(_)) => V::S(random_choice_str(pick(a[0].list(), |v| match v {
                    V::S(s) => s.clone(),
                    _ => unreachable!(),
                }))),
                Some(V::F(_)) => V::F(random_choice_float(a[0].fs())),
                // An empty list has no element type; either twin
                // refuses it the same way.
                _ => V::I(random_choice_int(pick(a[0].list(), |v| v.i()))),
            }),
            "sample" => Some(match a[0].list().first() {
                Some(V::S(_)) => V::L(
                    random_sample_str(
                        pick(a[0].list(), |v| match v {
                            V::S(s) => s.clone(),
                            _ => unreachable!(),
                        }),
                        a[1].i(),
                    )
                    .into_iter()
                    .map(V::S)
                    .collect(),
                ),
                Some(V::F(_)) => V::L(
                    random_sample_float(a[0].fs(), a[1].i())
                        .into_iter()
                        .map(V::F)
                        .collect(),
                ),
                _ => V::L(
                    random_sample_int(pick(a[0].list(), |v| v.i()), a[1].i())
                        .into_iter()
                        .map(V::I)
                        .collect(),
                ),
            }),
            _ => None,
        }
    });
}

/// `statistics` over `list[float]`. Where CPython adds the data as
/// exact rationals and rounds once, so does the twin — being close is
/// not the same number.
#[test]
fn statistics_matches_cpython() {
    check(
        include_str!("expected/statistics.txt"),
        "statistics",
        |fname, a| {
            let xs = a[0].fs();
            Some(V::F(match fname {
                "mean" => statistics_mean(xs),
                "fmean" => statistics_fmean(xs),
                "median" => statistics_median(xs),
                "mode" => statistics_mode(xs),
                "variance" => statistics_variance(xs),
                "pvariance" => statistics_pvariance(xs),
                "stdev" => statistics_stdev(xs),
                "pstdev" => statistics_pstdev(xs),
                _ => return None,
            }))
        },
    );
}


/// `json.dumps` with CPython's defaults. The writers compose the way
/// the translator composes them: a value renders to its text, a
/// container joins texts. A list of one scalar type goes through the
/// `_each` writer, which is the path a list the app is HOLDING takes.
fn json_render(v: &V) -> String {
    match v {
        V::Unit => json_null(),
        V::B(b) => json_bool(*b),
        V::I(n) => json_int(*n),
        V::F(f) => json_float(*f),
        V::S(s) => json_text(s),
        V::L(xs) => json_array(match xs.first() {
            Some(V::I(_)) if xs.iter().all(|x| matches!(x, V::I(_))) => {
                json_ints(xs.iter().map(|x| x.i()).collect())
            }
            Some(V::F(_)) if xs.iter().all(|x| matches!(x, V::F(_))) => {
                json_floats(xs.iter().map(|x| x.f()).collect())
            }
            Some(V::S(_)) if xs.iter().all(|x| matches!(x, V::S(_))) => {
                json_texts(xs.iter().map(|x| x.s()).collect())
            }
            Some(V::B(_)) if xs.iter().all(|x| matches!(x, V::B(_))) => {
                json_bools(xs.iter().map(|x| x.b()).collect())
            }
            _ => xs.iter().map(json_render).collect(),
        }),
        V::O(pairs) => json_object(
            pairs.iter().map(|(k, _)| k.clone()).collect(),
            pairs.iter().map(|(_, v)| json_render(v)).collect(),
        ),
        V::Raise(..) => unreachable!("an expected value, not an error"),
    }
}

#[test]
fn json_matches_cpython() {
    check(include_str!("expected/json.txt"), "json", |fname, a| match fname {
        "dumps" => Some(V::S(json_render(&a[0]))),
        _ => None,
    });
}

/// `datetime` in the integers the dialect carries it as: a date is
/// its ordinal, a datetime is microseconds from the same origin, a
/// timedelta is microseconds. Naive only — an aware datetime carries
/// a zone, which is what the plan keeps out for now.
#[test]
fn datetime_matches_cpython() {
    check(include_str!("expected/datetime.txt"), "datetime", |fname, a| {
        let n1 = |g: fn(i64) -> i64| Some(V::I(g(a[0].i())));
        let s1 = |g: fn(i64) -> String| Some(V::S(g(a[0].i())));
        let n2 = |g: fn(i64, i64) -> i64| Some(V::I(g(a[0].i(), a[1].i())));
        match fname {
            "date_new" => Some(V::I(date_new(a[0].i(), a[1].i(), a[2].i()))),
            "date_from_iso" => Some(V::I(date_from_iso(&a[0].s()))),
            "date_isoformat" => s1(date_isoformat),
            "date_str" => s1(date_str),
            "date_year" => n1(date_year),
            "date_month" => n1(date_month),
            "date_day" => n1(date_day),
            "date_weekday" => n1(date_weekday),
            "date_isoweekday" => n1(date_isoweekday),
            "date_strftime" => Some(V::S(date_strftime(a[0].i(), &a[1].s()))),
            "date_add_delta" => n2(date_add_delta),
            "date_sub_date" => n2(date_sub_date),
            "datetime_new" => Some(V::I(datetime_new(
                a[0].i(), a[1].i(), a[2].i(), a[3].i(), a[4].i(), a[5].i(), a[6].i(),
            ))),
            "datetime_from_iso" => Some(V::I(datetime_from_iso(&a[0].s()))),
            "datetime_isoformat" => s1(datetime_isoformat),
            "datetime_str" => s1(datetime_str),
            "datetime_date" => n1(datetime_date),
            "datetime_year" => n1(datetime_year),
            "datetime_month" => n1(datetime_month),
            "datetime_day" => n1(datetime_day),
            "datetime_hour" => n1(datetime_hour),
            "datetime_minute" => n1(datetime_minute),
            "datetime_second" => n1(datetime_second),
            "datetime_microsecond" => n1(datetime_microsecond),
            "datetime_weekday" => n1(datetime_weekday),
            "datetime_strftime" => Some(V::S(datetime_strftime(a[0].i(), &a[1].s()))),
            "datetime_add_delta" => n2(datetime_add_delta),
            "datetime_sub_datetime" => n2(datetime_sub_datetime),
            "datetime_of_date" => n1(datetime_of_date),
            "delta_new" => Some(V::I(delta_new(
                a[0].i(), a[1].i(), a[2].i(), a[3].i(), a[4].i(), a[5].i(), a[6].i(),
            ))),
            "delta_days" => n1(delta_days),
            "delta_seconds" => n1(delta_seconds),
            "delta_microseconds" => n1(delta_microseconds),
            "delta_total_seconds" => Some(V::F(delta_total_seconds(a[0].i()))),
            "delta_str" => s1(delta_str),
            _ => None,
        }
    });
}

/// `re`, asked the way the dialect asks: the pattern arrives as the
/// array CPython's own compiler produced, so what this checks is the
/// ENGINE running CPython's bytes rather than a rewriting of the
/// language.
#[test]
fn re_matches_cpython() {
    check(include_str!("expected/re.txt"), "re", |fname, a| {
        let ints = |v: &V| v.list().iter().map(|x| x.i()).collect::<Vec<i64>>();
        let strs = |v: &V| v.list().iter().map(|x| x.s()).collect::<Vec<String>>();
        match fname {
            "re_search" => Some(V::B(re_search(ints(&a[0]), &a[1].s()))),
            "re_match" => Some(V::B(re_match(ints(&a[0]), &a[1].s()))),
            "re_fullmatch" => Some(V::B(re_fullmatch(ints(&a[0]), &a[1].s()))),
            "re_findall" => Some(V::L(
                re_findall(ints(&a[0]), &a[1].s(), a[2].i())
                    .into_iter()
                    .map(V::S)
                    .collect(),
            )),
            "re_split" => Some(V::L(
                re_split(ints(&a[0]), &a[1].s(), a[2].i())
                    .into_iter()
                    .map(V::S)
                    .collect(),
            )),
            "re_sub" => Some(V::S(re_sub(
                ints(&a[0]),
                ints(&a[1]),
                strs(&a[2]),
                &a[3].s(),
                a[4].i(),
                a[5].i(),
            ))),
            "re_escape" => Some(V::S(re_escape(&a[0].s()))),
            _ => None,
        }
    });
}

/// The pure small modules: `string`, `textwrap`, `bisect`, `heapq`
/// and the rest of `str`.
#[test]
fn small_modules_match_cpython() {
    check(include_str!("expected/small.txt"), "small", |fname, a| {
        let s1 = |g: fn(&str) -> String| Some(V::S(g(&a[0].s())));
        let b1 = |g: fn(&str) -> bool| Some(V::B(g(&a[0].s())));
        let s2 = |g: fn(&str, &str) -> String| Some(V::S(g(&a[0].s(), &a[1].s())));
        let n2 = |g: fn(&str, &str) -> i64| Some(V::I(g(&a[0].s(), &a[1].s())));
        let pad = |g: fn(&str, i64, &str) -> String| Some(V::S(g(&a[0].s(), a[1].i(), &a[2].s())));
        let ints = |v: &V| v.list().iter().map(|x| x.i()).collect::<Vec<i64>>();
        let strs = |v: &V| v.list().iter().map(|x| x.s()).collect::<Vec<String>>();
        let is_str_list = |v: &V| matches!(v.list().first(), Some(V::S(_)));
        match fname {
            "string_ascii_letters" => Some(V::S(string_ascii_letters())),
            "string_ascii_lowercase" => Some(V::S(string_ascii_lowercase())),
            "string_ascii_uppercase" => Some(V::S(string_ascii_uppercase())),
            "string_digits" => Some(V::S(string_digits())),
            "string_hexdigits" => Some(V::S(string_hexdigits())),
            "string_octdigits" => Some(V::S(string_octdigits())),
            "string_punctuation" => Some(V::S(string_punctuation())),
            "string_whitespace" => Some(V::S(string_whitespace())),
            "string_printable" => Some(V::S(string_printable())),
            "textwrap_dedent" => s1(textwrap_dedent),
            "textwrap_indent" => s2(textwrap_indent),
            "bisect_left" if is_str_list(&a[0]) => {
                Some(V::I(bisect_left_str(strs(&a[0]), a[1].s())))
            }
            "bisect_right" if is_str_list(&a[0]) => {
                Some(V::I(bisect_right_str(strs(&a[0]), a[1].s())))
            }
            "bisect_left" => Some(V::I(bisect_left_int(ints(&a[0]), a[1].i()))),
            "bisect_right" => Some(V::I(bisect_right_int(ints(&a[0]), a[1].i()))),
            "heapq_nsmallest" if is_str_list(&a[1]) => Some(V::L(
                heapq_nsmallest_str(a[0].i(), strs(&a[1])).into_iter().map(V::S).collect(),
            )),
            "heapq_nlargest" if is_str_list(&a[1]) => Some(V::L(
                heapq_nlargest_str(a[0].i(), strs(&a[1])).into_iter().map(V::S).collect(),
            )),
            "heapq_nsmallest" => Some(V::L(
                heapq_nsmallest_int(a[0].i(), ints(&a[1])).into_iter().map(V::I).collect(),
            )),
            "heapq_nlargest" => Some(V::L(
                heapq_nlargest_int(a[0].i(), ints(&a[1])).into_iter().map(V::I).collect(),
            )),
            "math_frexp_m" => Some(V::F(math_frexp_m(a[0].f()))),
            "math_frexp_e" => Some(V::I(math_frexp_e(a[0].f()))),
            "math_modf_frac" => Some(V::F(math_modf_frac(a[0].f()))),
            "math_modf_int" => Some(V::F(math_modf_int(a[0].f()))),
            "py_str_partition_before" => s2(py_str_partition_before),
            "py_str_partition_sep" => s2(py_str_partition_sep),
            "py_str_partition_after" => s2(py_str_partition_after),
            "py_str_rpartition_before" => s2(py_str_rpartition_before),
            "py_str_rpartition_sep" => s2(py_str_rpartition_sep),
            "py_str_rpartition_after" => s2(py_str_rpartition_after),
            "py_str_title" => s1(py_str_title),
            "py_str_capitalize" => s1(py_str_capitalize),
            "py_str_swapcase" => s1(py_str_swapcase),
            "py_str_isupper" => b1(py_str_isupper),
            "py_str_islower" => b1(py_str_islower),
            "py_str_isalpha" => b1(py_str_isalpha),
            "py_str_isdigit" => b1(py_str_isdigit),
            "py_str_isalnum" => b1(py_str_isalnum),
            "py_str_isspace" => b1(py_str_isspace),
            "py_str_isascii" => b1(py_str_isascii),
            "py_str_zfill" => Some(V::S(py_str_zfill(&a[0].s(), a[1].i()))),
            "py_str_ljust" => pad(py_str_ljust),
            "py_str_rjust" => pad(py_str_rjust),
            "py_str_center" => pad(py_str_center),
            "py_str_removeprefix" => s2(py_str_removeprefix),
            "py_str_removesuffix" => s2(py_str_removesuffix),
            "py_str_rfind" => n2(py_str_rfind),
            "py_str_index_of" => n2(py_str_index_of),
            "py_str_rindex" => n2(py_str_rindex),
            "py_str_splitlines" => Some(V::L(
                py_str_splitlines(&a[0].s()).into_iter().map(V::S).collect(),
            )),
            "py_str_expandtabs" => Some(V::S(py_str_expandtabs(&a[0].s(), a[1].i()))),
            "py_str_strip_chars" => s2(py_str_strip_chars),
            "py_str_lstrip_chars" => s2(py_str_lstrip_chars),
            "py_str_rstrip_chars" => s2(py_str_rstrip_chars),
            _ => None,
        }
    });
}
