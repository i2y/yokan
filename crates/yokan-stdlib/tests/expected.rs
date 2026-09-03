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

fn parse(cell: &str) -> V {
    if let Some(body) = cell.strip_prefix('[').and_then(|c| c.strip_suffix(']')) {
        if body.is_empty() {
            return V::L(Vec::new());
        }
        return V::L(body.split(',').map(parse).collect());
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
