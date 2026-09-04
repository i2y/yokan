//! yokan-stdlib: the standard library's native core. ONE implementation per
//! function; tier A reaches it through the yokan extension's pyo3
//! door, the compiled tier through the `.rpi` binding — the same
//! code either way, so the cross-tier gate arbitrates one truth.
//! Errors PANIC with a clear message: under the containment regime
//! both tiers abort the failing statement and keep running.

/// The fallible twin: `!String` natively, a catchable RuntimeError
/// through the door — ONE message string serves the panic, the
/// raise, and the native `err(e)` payload, so `f"{e}"` renders
/// identically in both tiers.
pub fn fs_read_text_result(path: &str) -> std::io::Result<String> {
    std::fs::read_to_string(path)
        .map_err(|e| std::io::Error::other(format!("fs.read_text {path}: {e}")))
}

pub fn fs_read_text(path: &str) -> String {
    match fs_read_text_result(path) {
        Ok(s) => s,
        Err(e) => panic!("{e}"),
    }
}

pub fn fs_write_text(path: &str, text: &str) -> i64 {
    if let Some(dir) = std::path::Path::new(path).parent() {
        if !dir.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(dir);
        }
    }
    match std::fs::write(path, text) {
        Ok(()) => text.len() as i64,
        Err(e) => panic!("fs.write_text {path}: {e}"),
    }
}

pub fn fs_exists(path: &str) -> bool {
    std::path::Path::new(path).exists()
}

// ---- sqlite ---------------------------------------------------------

pub fn sqlite_exec(path: &str, sql: &str) -> i64 {
    let conn = match rusqlite::Connection::open(path) {
        Ok(c) => c,
        Err(e) => panic!("sqlite.exec open {path}: {e}"),
    };
    match conn.execute(sql, []) {
        Ok(n) => n as i64,
        Err(e) => panic!("sqlite.exec {path}: {e}"),
    }
}

/// Column 0 of every row, rendered as text — the deterministic v1
/// read surface (shape your row with SQL, order with ORDER BY).
pub fn sqlite_query_text(path: &str, sql: &str) -> Vec<String> {
    let conn = match rusqlite::Connection::open(path) {
        Ok(c) => c,
        Err(e) => panic!("sqlite.query_text open {path}: {e}"),
    };
    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(e) => panic!("sqlite.query_text {path}: {e}"),
    };
    let rows = stmt.query_map([], |row| {
        Ok(match row.get::<_, rusqlite::types::Value>(0) {
            Ok(rusqlite::types::Value::Integer(i)) => i.to_string(),
            Ok(rusqlite::types::Value::Real(f)) => f.to_string(),
            Ok(rusqlite::types::Value::Text(s)) => s,
            Ok(rusqlite::types::Value::Null) => String::new(),
            Ok(rusqlite::types::Value::Blob(_)) => "<blob>".to_string(),
            Err(e) => panic!("sqlite.query_text column 0: {e}"),
        })
    });
    match rows {
        Ok(it) => it
            .map(|r| match r {
                Ok(s) => s,
                Err(e) => panic!("sqlite.query_text row: {e}"),
            })
            .collect(),
        Err(e) => panic!("sqlite.query_text {path}: {e}"),
    }
}

// ---- http -----------------------------------------------------------

/// Blocking GET, body as text. Deliberately synchronous in v1: both
/// tiers block the same statement the same way; the async rider can
/// come later without changing the meaning of this one.
pub fn http_get_text_result(url: &str) -> std::io::Result<String> {
    match ureq::get(url).call() {
        Ok(resp) => resp
            .into_string()
            .map_err(|e| std::io::Error::other(format!("http.get_text {url}: {e}"))),
        Err(e) => Err(std::io::Error::other(format!("http.get_text {url}: {e}"))),
    }
}

pub fn http_get_text(url: &str) -> String {
    match http_get_text_result(url) {
        Ok(s) => s,
        Err(e) => panic!("{e}"),
    }
}

// ---- math -----------------------------------------------------------
// Python's `math`, as a twin: the interpreted run calls CPython's own
// module, so these have to answer what CPython answers — including
// where CPython raises and IEEE-754 would shrug. The gate holds the
// two runs together; `tests/expected/math.txt`, printed by CPython
// itself, holds this half to CPython.

pub fn math_sqrt(v: f64) -> f64 {
    // Python's domain, not IEEE's: a negative input is an error here,
    // not a quiet NaN. Contained as one failing statement in both runs.
    assert!(!(v < 0.0), "expected a nonnegative input, got {}", py_float_repr(v));
    v.sqrt()
}
/// The three that take an angle refuse an infinite one: there is no
/// angle at infinity, and CPython says so rather than answering NaN.
fn finite(v: f64) -> f64 {
    if v.is_infinite() {
        domain("expected a finite input", v);
    }
    v
}
pub fn math_sin(v: f64) -> f64 { finite(v).sin() }
pub fn math_cos(v: f64) -> f64 { finite(v).cos() }

/// `math.pow` — CPython's domain and range rules laid over libm's
/// `pow`. Rust's `powf` answers NaN where Python raises and infinity
/// where Python overflows, so the special cases are written out; the
/// order is CPython's `m_pow` (Modules/mathmodule.c).
pub fn math_pow(x: f64, y: f64) -> f64 {
    if !x.is_finite() || !y.is_finite() {
        if x.is_nan() {
            return if y == 0.0 { 1.0 } else { x };
        }
        if y.is_nan() {
            return if x == 1.0 { 1.0 } else { y };
        }
        if x.is_infinite() {
            let odd_y = y.is_finite() && (y.abs() % 2.0) == 1.0;
            return if y > 0.0 {
                if odd_y { x } else { x.abs() }
            } else if y == 0.0 {
                1.0
            } else if odd_y {
                0.0_f64.copysign(x)
            } else {
                0.0
            };
        }
        // y is the infinite one.
        if x.abs() == 1.0 {
            return 1.0;
        }
        if y > 0.0 && x.abs() > 1.0 {
            return y;
        }
        if y < 0.0 && x.abs() < 1.0 {
            assert!(x != 0.0, "math domain error");
            return -y;
        }
        return 0.0;
    }
    let r = x.powf(y);
    if r.is_nan() {
        // A negative base under a fractional exponent.
        panic!("math domain error");
    }
    if r.is_infinite() {
        // Zero under a negative exponent is a domain error in Python;
        // anything else that reached infinity overflowed.
        assert!(x != 0.0, "math domain error");
        panic!("math range error");
    }
    r
}
pub fn math_fabs(v: f64) -> f64 { v.abs() }
pub fn math_floor(v: f64) -> i64 { float_to_int("floor", v.floor()) }
pub fn math_ceil(v: f64) -> i64 { float_to_int("ceil", v.ceil()) }
pub fn math_pi() -> f64 { std::f64::consts::PI }
pub fn math_e() -> f64 { std::f64::consts::E }
pub fn math_tau() -> f64 { std::f64::consts::TAU }
pub fn math_inf() -> f64 { f64::INFINITY }
pub fn math_nan() -> f64 { f64::NAN }

/// CPython 3.14 names the domain it wanted and the value it got, one
/// message per function rather than a shared "math domain error".
/// The text is part of what a twin has to reproduce, so it lives here
/// once and the ground-truth table checks it.
fn domain(want: &str, got: f64) -> ! {
    panic!("{want}, got {}", py_float_repr(got));
}

/// CPython's `math_1`: a libm call, then errno read back as an error.
/// A NaN out of a non-NaN input is a domain error; an infinity out of
/// a finite one is a range error where the function can overflow and
/// a domain error where it cannot (a pole).
fn math_1(x: f64, r: f64, can_overflow: bool) -> f64 {
    if r.is_nan() && !x.is_nan() {
        panic!("math domain error");
    }
    if r.is_infinite() && x.is_finite() {
        panic!("{}", if can_overflow { "math range error" } else { "math domain error" });
    }
    r
}

macro_rules! math_ranged {
    ($($name:ident => $call:ident, $lo:expr, $hi:expr, $want:expr;)*) => {$(
        pub fn $name(v: f64) -> f64 {
            if !(v.is_nan() || ($lo..=$hi).contains(&v)) {
                domain($want, v);
            }
            v.$call()
        }
    )*};
}

// The inverse trigonometric functions and their domains, which
// CPython checks by name.
math_ranged! {
    math_acos => acos, -1.0, 1.0, "expected a number in range from -1 up to 1";
    math_asin => asin, -1.0, 1.0, "expected a number in range from -1 up to 1";
}

pub fn math_atanh(v: f64) -> f64 {
    if !(v.is_nan() || (-1.0 < v && v < 1.0)) {
        domain("expected a number between -1 and 1", v);
    }
    v.atanh()
}
pub fn math_acosh(v: f64) -> f64 {
    if !(v.is_nan() || v >= 1.0) {
        domain("expected argument value not less than 1", v);
    }
    v.acosh()
}
pub fn math_asinh(v: f64) -> f64 { v.asinh() }
pub fn math_atan(v: f64) -> f64 { v.atan() }
pub fn math_atan2(y: f64, x: f64) -> f64 { y.atan2(x) }
pub fn math_tan(v: f64) -> f64 { finite(v).tan() }
pub fn math_sinh(v: f64) -> f64 { math_1(v, v.sinh(), true) }
pub fn math_cosh(v: f64) -> f64 { math_1(v, v.cosh(), true) }
pub fn math_tanh(v: f64) -> f64 { v.tanh() }
pub fn math_cbrt(v: f64) -> f64 { v.cbrt() }
pub fn math_exp(v: f64) -> f64 { math_1(v, v.exp(), true) }
pub fn math_exp2(v: f64) -> f64 { math_1(v, v.exp2(), true) }
pub fn math_expm1(v: f64) -> f64 { math_1(v, v.exp_m1(), true) }
pub fn math_degrees(v: f64) -> f64 { v.to_degrees() }
pub fn math_radians(v: f64) -> f64 { v.to_radians() }
pub fn math_trunc(v: f64) -> i64 { float_to_int("trunc", v.trunc()) }
pub fn math_isnan(v: f64) -> bool { v.is_nan() }
pub fn math_isinf(v: f64) -> bool { v.is_infinite() }
pub fn math_isfinite(v: f64) -> bool { v.is_finite() }
pub fn math_copysign(a: f64, b: f64) -> f64 { a.copysign(b) }
pub fn math_ulp(v: f64) -> f64 {
    if v.is_nan() {
        return v;
    }
    let v = v.abs();
    if v.is_infinite() {
        return v;
    }
    if v == f64::MAX {
        // The step past the largest finite double is the one below it.
        return v - f64::from_bits(v.to_bits() - 1);
    }
    f64::from_bits(v.to_bits() + 1) - v
}

/// The logarithms share one domain: strictly positive.
fn log_of(v: f64, f: impl Fn(f64) -> f64) -> f64 {
    if !(v.is_nan() || v > 0.0) {
        domain("expected a positive input", v);
    }
    f(v)
}
pub fn math_log(v: f64) -> f64 { log_of(v, f64::ln) }
pub fn math_log2(v: f64) -> f64 { log_of(v, f64::log2) }
pub fn math_log10(v: f64) -> f64 { log_of(v, f64::log10) }
/// `log(x, base)` is a ratio of two logs in CPython too, not a
/// separate libm call, so the last bit lands the same way.
pub fn math_log_base(v: f64, base: f64) -> f64 { math_log(v) / math_log(base) }
pub fn math_log1p(v: f64) -> f64 {
    if !(v.is_nan() || v > -1.0) {
        domain("expected argument value > -1", v);
    }
    v.ln_1p()
}

pub fn math_fmod(x: f64, y: f64) -> f64 {
    if x.is_infinite() && !y.is_nan() {
        panic!("math domain error");
    }
    let r = x % y;
    if r.is_nan() && !x.is_nan() && !y.is_nan() {
        panic!("math domain error");
    }
    r
}

/// IEEE 754 remainder — x minus the nearest multiple of y, ties to
/// even. Neither Rust nor the `%` operator answers this one.
pub fn math_remainder(x: f64, y: f64) -> f64 {
    if x.is_nan() || y.is_nan() {
        return f64::NAN;
    }
    if x.is_infinite() || y == 0.0 {
        panic!("math domain error");
    }
    if y.is_infinite() {
        return x;
    }
    let ay = y.abs();
    let mut r = x % y;
    // `%` truncates toward zero; the remainder rounds to nearest,
    // so a residue past half a step moves one multiple over.
    let half = ay * 0.5;
    if r.abs() > half || (r.abs() == half && (x / y).abs().rem_euclid(2.0) >= 1.0) {
        r -= ay.copysign(r);
    }
    if r == 0.0 { r.copysign(x) } else { r }
}

/// `ldexp` that answers an infinity instead of stopping, for callers
/// that report the overflow in their own words.
fn math_ldexp_unchecked(x: f64, n: i64) -> f64 {
    if x == 0.0 || !x.is_finite() {
        return x;
    }
    let mut r = x;
    let mut n = n;
    while n > 1023 {
        r *= f64::from_bits((1023u64 + 1023) << 52);
        n -= 1023;
        if r.is_infinite() {
            return r;
        }
    }
    while n < -1022 {
        r *= f64::from_bits((1023u64 - 1022) << 52);
        n += 1022;
        if r == 0.0 {
            return r;
        }
    }
    r * f64::from_bits(((1023i64 + n) as u64) << 52)
}

pub fn math_ldexp(x: f64, n: i64) -> f64 {
    if x == 0.0 || !x.is_finite() {
        return x;
    }
    // Scale in steps a double can hold, so a huge exponent neither
    // overflows the shift nor loses the value on the way.
    let mut r = x;
    let mut n = n;
    while n > 1023 {
        r *= f64::from_bits((1023u64 + 1023) << 52);
        n -= 1023;
        if r.is_infinite() {
            panic!("math range error");
        }
    }
    while n < -1022 {
        r *= f64::from_bits((1023u64 - 1022) << 52);
        n += 1022;
        if r == 0.0 {
            return r;
        }
    }
    let scale = f64::from_bits(((1023i64 + n) as u64) << 52);
    let out = r * scale;
    if out.is_infinite() {
        panic!("math range error");
    }
    out
}

pub fn math_nextafter(x: f64, y: f64) -> f64 {
    if x.is_nan() || y.is_nan() {
        return f64::NAN;
    }
    if x == y {
        return y;
    }
    if x < y { x.next_up() } else { x.next_down() }
}

pub fn math_fma(x: f64, y: f64, z: f64) -> f64 {
    let r = x.mul_add(y, z);
    let all_finite = x.is_finite() && y.is_finite() && z.is_finite();
    if r.is_nan() && !x.is_nan() && !y.is_nan() && !z.is_nan() {
        panic!("invalid operation in fma");
    }
    if r.is_infinite() && all_finite {
        panic!("overflow in fma");
    }
    r
}

/// The exponent `frexp` answers: `x == f * 2**e` with `0.5 <= |f| < 1`.
fn frexp_exp(x: f64) -> i32 {
    let bits = x.abs().to_bits();
    let raw = ((bits >> 52) & 0x7ff) as i32;
    if raw != 0 {
        return raw - 1022;
    }
    // Subnormal: the leading one is somewhere in the mantissa.
    let m = bits & 0x000f_ffff_ffff_ffff;
    -1022 - (m.leading_zeros() as i32 - 11)
}

/// Two doubles whose sum is `a + b` exactly, for `|a| >= |b|`.
fn dl_fast_sum(a: f64, b: f64) -> (f64, f64) {
    let x = a + b;
    (x, b - (x - a))
}

/// Two doubles whose sum is `x * y` exactly.
fn dl_mul(x: f64, y: f64) -> (f64, f64) {
    let z = x * y;
    (z, x.mul_add(y, -z))
}

/// The Euclidean norm, as CPython computes it (`vector_norm`) rather
/// than as libm's `hypot` does. CPython does not call the platform
/// here, so a twin that did would be right on this machine and wrong
/// on the next: the vector is scaled so nothing overflows, squared
/// losslessly, summed with the low halves carried alongside, and
/// finished with a differential correction that makes the result the
/// correctly rounded one.
fn vector_norm(vec: &mut [f64], max: f64, found_nan: bool) -> f64 {
    if max.is_infinite() {
        return max;
    }
    if found_nan {
        return f64::NAN;
    }
    if max == 0.0 || vec.len() <= 1 {
        return max;
    }
    let max_e = frexp_exp(max);
    if max_e < -1023 {
        // Subnormal inputs: lift the whole vector out of the
        // subnormal range and scale the answer back.
        for v in vec.iter_mut() {
            *v /= f64::MIN_POSITIVE;
        }
        let lifted = max / f64::MIN_POSITIVE;
        return f64::MIN_POSITIVE * vector_norm(vec, lifted, found_nan);
    }
    let scale = math_ldexp(1.0, -(max_e as i64));
    let mut csum = 1.0f64;
    let mut frac1 = 0.0f64;
    let mut frac2 = 0.0f64;
    for v in vec.iter() {
        let x = *v * scale;
        let pr = dl_mul(x, x);
        let sm = dl_fast_sum(csum, pr.0);
        csum = sm.0;
        frac1 += pr.1;
        frac2 += sm.1;
    }
    let mut h = (csum - 1.0 + (frac1 + frac2)).sqrt();
    let pr = dl_mul(-h, h);
    let sm = dl_fast_sum(csum, pr.0);
    csum = sm.0;
    frac1 += pr.1;
    frac2 += sm.1;
    let x = csum - 1.0 + (frac1 + frac2);
    h += x / (2.0 * h);
    h / scale
}

/// `hypot(x, y)` and `dist(p, q)`, both over `vector_norm`.
pub fn math_hypot(x: f64, y: f64) -> f64 {
    let mut v = [x.abs(), y.abs()];
    norm_of(&mut v)
}
pub fn math_dist(a: Vec<f64>, b: Vec<f64>) -> f64 {
    assert!(
        a.len() == b.len(),
        "both points must have the same number of dimensions"
    );
    let mut v: Vec<f64> = a.iter().zip(b.iter()).map(|(x, y)| (x - y).abs()).collect();
    norm_of(&mut v)
}

fn norm_of(v: &mut [f64]) -> f64 {
    let mut max = 0.0f64;
    let mut found_nan = false;
    for x in v.iter() {
        if x.is_nan() {
            found_nan = true;
        } else if *x > max {
            max = *x;
        }
    }
    vector_norm(v, max, found_nan)
}

/// `isclose` with CPython's default tolerances, which are part of the
/// answer rather than of the call.
pub fn math_isclose(a: f64, b: f64) -> bool {
    const REL: f64 = 1e-9;
    if a == b {
        return true;
    }
    if a.is_infinite() || b.is_infinite() {
        return false;
    }
    let diff = (b - a).abs();
    diff <= (REL * b).abs() || diff <= (REL * a).abs()
}

/// Shewchuk's exact summation, the algorithm CPython's `fsum` is: the
/// partial sums are kept as a list of non-overlapping doubles, so the
/// answer is the correctly rounded total rather than the accumulated
/// drift of adding left to right.
pub fn math_fsum(xs: Vec<f64>) -> f64 {
    let mut partials: Vec<f64> = Vec::new();
    // An infinity or a NaN leaves the partials behind and lands here
    // instead; two infinities of opposite sign are an error rather
    // than a NaN, which is CPython's call and not IEEE's.
    let mut special_sum = 0.0f64;
    let mut inf_sum = 0.0f64;
    for xin in xs {
        let mut x = xin;
        let mut i = 0;
        for j in 0..partials.len() {
            let mut y = partials[j];
            if x.abs() < y.abs() {
                std::mem::swap(&mut x, &mut y);
            }
            let hi = x + y;
            let lo = y - (hi - x);
            if lo != 0.0 {
                partials[i] = lo;
                i += 1;
            }
            x = hi;
        }
        partials.truncate(i);
        if x != 0.0 {
            if !x.is_finite() {
                // Either the running sum overflowed, or this summand
                // was not finite to begin with. The two are different
                // errors and CPython tells them apart.
                assert!(!xin.is_finite(), "intermediate overflow in fsum");
                if xin.is_infinite() {
                    inf_sum += xin;
                }
                special_sum += xin;
                partials.clear();
            } else {
                partials.push(x);
            }
        }
    }
    if special_sum != 0.0 {
        assert!(!inf_sum.is_nan(), "-inf + inf in fsum");
        return special_sum;
    }
    // Add the partials from the smallest up, with one round-to-even
    // correction at the top — CPython's own tail.
    let mut hi = 0.0f64;
    let n = partials.len();
    if n > 0 {
        hi = partials[n - 1];
        let mut lo = 0.0f64;
        for j in (0..n - 1).rev() {
            let x = hi;
            let y = partials[j];
            hi = x + y;
            lo = y - (hi - x);
            if lo != 0.0 {
                break;
            }
        }
        if n >= 2 && ((lo < 0.0 && partials[n - 2] < 0.0) || (lo > 0.0 && partials[n - 2] > 0.0)) {
            let y = lo * 2.0;
            let x = hi + y;
            if y == x - hi {
                hi = x;
            }
        }
    }
    hi
}

pub fn math_factorial(n: i64) -> i64 {
    assert!(n >= 0, "factorial() not defined for negative values");
    let mut out: i64 = 1;
    for k in 2..=n {
        out = out
            .checked_mul(k)
            .expect("`math.factorial` overflows the 64-bit int range");
    }
    out
}

pub fn math_isqrt(n: i64) -> i64 {
    assert!(n >= 0, "isqrt() argument must be nonnegative");
    if n == 0 {
        return 0;
    }
    // Newton from a power-of-two seed: integer-only, so the answer is
    // the exact floor rather than a rounded square root.
    let mut x = 1i64 << ((64 - (n as u64).leading_zeros()) / 2 + 1);
    loop {
        let y = (x + n / x) / 2;
        if y >= x {
            return x;
        }
        x = y;
    }
}

pub fn math_comb(n: i64, k: i64) -> i64 {
    assert!(n >= 0, "n must be a non-negative integer");
    assert!(k >= 0, "k must be a non-negative integer");
    if k > n {
        return 0;
    }
    let k = k.min(n - k);
    let mut out: i64 = 1;
    for i in 0..k {
        out = out
            .checked_mul(n - i)
            .expect("`math.comb` overflows the 64-bit int range")
            / (i + 1);
    }
    out
}

pub fn math_perm(n: i64, k: i64) -> i64 {
    assert!(n >= 0, "n must be a non-negative integer");
    assert!(k >= 0, "k must be a non-negative integer");
    if k > n {
        return 0;
    }
    let mut out: i64 = 1;
    for i in 0..k {
        out = out
            .checked_mul(n - i)
            .expect("`math.perm` overflows the 64-bit int range");
    }
    out
}

pub fn math_gcd(a: i64, b: i64) -> i64 {
    let (mut a, mut b) = (a.unsigned_abs(), b.unsigned_abs());
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a as i64
}

pub fn math_lcm(a: i64, b: i64) -> i64 {
    if a == 0 || b == 0 {
        return 0;
    }
    let g = math_gcd(a, b);
    (a / g)
        .checked_mul(b)
        .expect("`math.lcm` overflows the 64-bit int range")
        .abs()
}

/// A whole double as an int, with CPython's two refusals — it tells
/// infinity and NaN apart, and so does the message. Python's int is
/// unbounded and this one is 64 bits wide (a decided constraint), so
/// a value past the range stops the statement rather than wrapping.
fn float_to_int(what: &str, v: f64) -> i64 {
    if v.is_nan() {
        panic!("cannot convert float NaN to integer");
    }
    if v.is_infinite() {
        panic!("cannot convert float infinity to integer");
    }
    assert!(
        v >= -9223372036854775808.0 && v < 9223372036854775808.0,
        "`{what}` overflows the 64-bit int range"
    );
    v as i64
}

// ---- json -----------------------------------------------------------
// Typed extractors over a dotted path ("users.1.name"); a missing or
// mistyped node PANICS with the path — contained as one failing
// statement in both tiers.

fn json_at(src: &str, path: &str) -> serde_json::Value {
    let mut v: serde_json::Value = match serde_json::from_str(src) {
        Ok(v) => v,
        Err(e) => panic!("json: invalid document: {e}"),
    };
    if path.is_empty() {
        return v;
    }
    for seg in path.split('.') {
        v = match (&v, seg.parse::<usize>()) {
            (serde_json::Value::Array(xs), Ok(i)) => match xs.get(i) {
                Some(x) => x.clone(),
                None => panic!("json: index `{seg}` out of range in `{path}`"),
            },
            (serde_json::Value::Object(m), _) => match m.get(seg) {
                Some(x) => x.clone(),
                None => panic!("json: no key `{seg}` in `{path}`"),
            },
            _ => panic!("json: `{seg}` does not index into `{path}`"),
        };
    }
    v
}

pub fn json_get_text(src: &str, path: &str) -> String {
    match json_at(src, path) {
        serde_json::Value::String(s) => s,
        other => panic!("json: `{path}` is not a string (got {other})"),
    }
}

pub fn json_get_int(src: &str, path: &str) -> i64 {
    match json_at(src, path).as_i64() {
        Some(n) => n,
        None => panic!("json: `{path}` is not an integer"),
    }
}

pub fn json_get_float(src: &str, path: &str) -> f64 {
    match json_at(src, path).as_f64() {
        Some(f) => f,
        None => panic!("json: `{path}` is not a number"),
    }
}

pub fn json_get_bool(src: &str, path: &str) -> bool {
    match json_at(src, path).as_bool() {
        Some(b) => b,
        None => panic!("json: `{path}` is not a bool"),
    }
}

pub fn json_length(src: &str, path: &str) -> i64 {
    match json_at(src, path) {
        serde_json::Value::Array(xs) => xs.len() as i64,
        serde_json::Value::Object(m) => m.len() as i64,
        _ => panic!("json: `{path}` has no length"),
    }
}

pub fn json_has(src: &str, path: &str) -> bool {
    let mut v: serde_json::Value = match serde_json::from_str(src) {
        Ok(v) => v,
        Err(e) => panic!("json: invalid document: {e}"),
    };
    for seg in path.split('.') {
        let next = match (&v, seg.parse::<usize>()) {
            (serde_json::Value::Array(xs), Ok(i)) => xs.get(i).cloned(),
            (serde_json::Value::Object(m), _) => m.get(seg).cloned(),
            _ => None,
        };
        match next {
            Some(x) => v = x,
            None => return false,
        }
    }
    true
}

// ---- time -----------------------------------------------------------

/// Python's `time`, as a twin. There is nothing for a ground-truth
/// table to say about a clock — the answer is the machine's, and the
/// two runs read it at different moments — so what a twin owes here
/// is the unit and the reference point CPython documents, and the
/// gate holds an app that displays one to a fixed input instead.
fn epoch_nanos() -> i128 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_nanos() as i128,
        Err(e) => panic!("time: clock before epoch: {e}"),
    }
}

pub fn time_time() -> f64 {
    epoch_nanos() as f64 / 1e9
}

pub fn time_time_ns() -> i64 {
    epoch_nanos() as i64
}

/// `monotonic` and `perf_counter` are both "the highest-resolution
/// clock that cannot go backwards" here, as they are on most
/// platforms; CPython documents the reference point of either as
/// undefined, so only differences mean anything.
fn since_start() -> std::time::Duration {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed()
}

pub fn time_monotonic() -> f64 {
    since_start().as_secs_f64()
}

pub fn time_monotonic_ns() -> i64 {
    since_start().as_nanos() as i64
}

pub fn time_perf_counter() -> f64 {
    time_monotonic()
}

pub fn time_perf_counter_ns() -> i64 {
    time_monotonic_ns()
}

/// `time.sleep(secs)` — seconds as a float, and a negative one is an
/// error rather than a no-op, which is CPython's call.
pub fn time_sleep(secs: f64) {
    assert!(!(secs < 0.0), "sleep length must be non-negative");
    if secs > 0.0 && secs.is_finite() {
        std::thread::sleep(std::time::Duration::from_secs_f64(secs));
    }
}

/// UTC strftime of a millisecond timestamp — deterministic for a
/// fixed input, which is what a gate script feeds it.
pub fn clock_format_ms(ms: i64, fmt: &str) -> String {
    match chrono::DateTime::from_timestamp_millis(ms) {
        Some(dt) => dt.format(fmt).to_string(),
        None => panic!("clock: `{ms}` is out of range"),
    }
}

// ---- string, textwrap, bisect, heapq --------------------------------
// The small pure modules. Nothing here reads a clock, a file or a
// generator, so a view may call any of it; what each answers is
// pinned by a ground-truth table like the rest.

pub fn string_ascii_lowercase() -> String { "abcdefghijklmnopqrstuvwxyz".to_string() }
pub fn string_ascii_uppercase() -> String { "ABCDEFGHIJKLMNOPQRSTUVWXYZ".to_string() }
pub fn string_ascii_letters() -> String {
    format!("{}{}", string_ascii_lowercase(), string_ascii_uppercase())
}
pub fn string_digits() -> String { "0123456789".to_string() }
pub fn string_hexdigits() -> String { "0123456789abcdefABCDEF".to_string() }
pub fn string_octdigits() -> String { "01234567".to_string() }
pub fn string_punctuation() -> String { "!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~".to_string() }
pub fn string_whitespace() -> String { " \t\n\r\u{b}\u{c}".to_string() }
pub fn string_printable() -> String {
    format!(
        "{}{}{}{}",
        string_digits(),
        string_ascii_letters(),
        string_punctuation(),
        string_whitespace()
    )
}

/// `textwrap.dedent`, ported line for line from CPython's: the common
/// leading whitespace is found by comparing the smallest and largest
/// non-blank lines, and a line of nothing but whitespace becomes
/// empty.
pub fn textwrap_dedent(text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let non_blank: Vec<&&str> = lines
        .iter()
        .filter(|l| !l.is_empty() && !py_str_isspace(l))
        .collect();
    let l1 = non_blank.iter().min().map(|l| **l).unwrap_or("");
    let l2 = non_blank.iter().max().map(|l| **l).unwrap_or("");
    let l2c: Vec<char> = l2.chars().collect();
    let mut margin = 0usize;
    for (i, c) in l1.chars().enumerate() {
        margin = i;
        if l2c.get(i) != Some(&c) || !(c == ' ' || c == '\t') {
            break;
        }
        margin = i + 1;
    }
    // CPython's loop leaves `margin` at the index it broke on, which
    // is the first character that is not shared whitespace.
    let margin = l1.chars().take(margin).count();
    lines
        .iter()
        .map(|l| {
            if py_str_isspace(l) {
                String::new()
            } else {
                l.chars().skip(margin).collect()
            }
        })
        .collect::<Vec<String>>()
        .join("\n")
}

/// `textwrap.indent` with the default predicate: every line that is
/// not all whitespace.
pub fn textwrap_indent(text: &str, prefix: &str) -> String {
    let mut out = String::new();
    for line in py_str_splitlines_keepends(text) {
        if !py_str_isspace(&line) {
            out.push_str(prefix);
        }
        out.push_str(&line);
    }
    out
}

fn py_str_splitlines_keepends(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut cs = s.chars().peekable();
    while let Some(c) = cs.next() {
        cur.push(c);
        let ends = matches!(
            c,
            '\n' | '\u{b}' | '\u{c}' | '\r' | '\u{1c}' | '\u{1d}' | '\u{1e}'
                | '\u{85}' | '\u{2028}' | '\u{2029}'
        );
        if ends {
            if c == '\r' && cs.peek() == Some(&'\n') {
                cur.push(cs.next().expect("peeked"));
            }
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// The binary searches. `bisect_left` puts an equal value before the
/// ones already there and `bisect_right` after, which is the only
/// difference between them.
macro_rules! bisects {
    ($($left:ident, $right:ident, $t:ty;)*) => {$(
        pub fn $left(xs: Vec<$t>, x: $t) -> i64 {
            let (mut lo, mut hi) = (0usize, xs.len());
            while lo < hi {
                let mid = (lo + hi) / 2;
                if xs[mid] < x { lo = mid + 1 } else { hi = mid }
            }
            lo as i64
        }
        pub fn $right(xs: Vec<$t>, x: $t) -> i64 {
            let (mut lo, mut hi) = (0usize, xs.len());
            while lo < hi {
                let mid = (lo + hi) / 2;
                if x < xs[mid] { hi = mid } else { lo = mid + 1 }
            }
            lo as i64
        }
    )*};
}
bisects! {
    bisect_left_int, bisect_right_int, i64;
    bisect_left_str, bisect_right_str, String;
}
pub fn bisect_left_float(xs: Vec<f64>, x: f64) -> i64 {
    let (mut lo, mut hi) = (0usize, xs.len());
    while lo < hi {
        let mid = (lo + hi) / 2;
        if xs[mid] < x { lo = mid + 1 } else { hi = mid }
    }
    lo as i64
}
pub fn bisect_right_float(xs: Vec<f64>, x: f64) -> i64 {
    let (mut lo, mut hi) = (0usize, xs.len());
    while lo < hi {
        let mid = (lo + hi) / 2;
        if x < xs[mid] { hi = mid } else { lo = mid + 1 }
    }
    lo as i64
}

/// `heapq.nsmallest` / `nlargest`. What they answer is the sorted
/// prefix, which is how the documentation defines them; the heap is
/// an implementation detail of getting there.
macro_rules! nsmalls {
    ($($small:ident, $large:ident, $t:ty, $sort:ident;)*) => {$(
        pub fn $small(n: i64, xs: Vec<$t>) -> Vec<$t> {
            let mut out = $sort(xs);
            out.truncate(n.max(0) as usize);
            out
        }
        pub fn $large(n: i64, xs: Vec<$t>) -> Vec<$t> {
            let mut out = $sort(xs);
            out.reverse();
            out.truncate(n.max(0) as usize);
            out
        }
    )*};
}
nsmalls! {
    heapq_nsmallest_int, heapq_nlargest_int, i64, py_list_sorted_int;
    heapq_nsmallest_float, heapq_nlargest_float, f64, py_list_sorted_float;
    heapq_nsmallest_str, heapq_nlargest_str, String, py_list_sorted_str;
}

// ---- the rest of str ------------------------------------------------
// The methods the tour listed as "not yet". Case mapping is the full
// Unicode one on both sides, which is what `upper` and `lower`
// already relied on.

fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

/// Python's `str.title`: the first letter of each run of letters is
/// titlecased and the rest lowercased, so `"don't"` becomes
/// `"Don'T"` — the apostrophe ends the word.
pub fn py_str_title(s: &str) -> String {
    let mut out = String::new();
    let mut prev_cased = false;
    for c in s.chars() {
        if prev_cased {
            out.extend(c.to_lowercase());
        } else {
            out.push_str(&titlecase(c));
        }
        prev_cased = c.is_alphabetic();
    }
    out
}

pub fn py_str_capitalize(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i == 0 {
            out.push_str(&titlecase(c));
        } else {
            out.extend(c.to_lowercase());
        }
    }
    out
}

/// Unicode's titlecase mapping, which is not the uppercase one: `ß`
/// titlecases to "Ss" where it uppercases to "SS".
fn titlecase(c: char) -> String {
    unicode_case_mapping::to_titlecase(c)
        .into_iter()
        .take_while(|u| *u != 0)
        .filter_map(char::from_u32)
        .collect::<String>()
        .pipe_or(c)
}

trait PipeOr {
    fn pipe_or(self, c: char) -> String;
}
impl PipeOr for String {
    /// The table answers nothing for a character that titlecases to
    /// itself.
    fn pipe_or(self, c: char) -> String {
        if self.is_empty() { c.to_string() } else { self }
    }
}

pub fn py_str_swapcase(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_uppercase() {
            out.extend(c.to_lowercase());
        } else if c.is_lowercase() {
            out.extend(c.to_uppercase());
        } else {
            out.push(c);
        }
    }
    out
}

pub fn py_str_zfill(s: &str, width: i64) -> String {
    let cs = chars(s);
    let n = width.max(0) as usize;
    if cs.len() >= n {
        return s.to_string();
    }
    let pad = n - cs.len();
    // A leading sign stays in front of the zeros.
    match cs.first() {
        Some('+') | Some('-') => {
            let mut out = String::new();
            out.push(cs[0]);
            out.extend(std::iter::repeat_n('0', pad));
            out.extend(cs[1..].iter());
            out
        }
        _ => {
            let mut out: String = std::iter::repeat_n('0', pad).collect();
            out.push_str(s);
            out
        }
    }
}

fn pad_to(s: &str, width: i64, fill: char, mode: u8) -> String {
    let cs = chars(s);
    let n = width.max(0) as usize;
    if cs.len() >= n {
        return s.to_string();
    }
    let pad = n - cs.len();
    match mode {
        0 => format!("{s}{}", fill.to_string().repeat(pad)),
        1 => format!("{}{s}", fill.to_string().repeat(pad)),
        _ => {
            // `center` leans the odd character left when the string
            // itself has an even length, and right otherwise — which
            // is what CPython's `(width - len) / 2 + (width & 1 &
            // ~len)` works out to.
            let l = pad / 2 + usize::from(pad % 2 == 1 && cs.len() % 2 == 0);
            format!(
                "{}{s}{}",
                fill.to_string().repeat(l),
                fill.to_string().repeat(pad - l)
            )
        }
    }
}

pub fn py_str_ljust(s: &str, width: i64, fill: &str) -> String {
    pad_to(s, width, one_fill(fill), 0)
}
pub fn py_str_rjust(s: &str, width: i64, fill: &str) -> String {
    pad_to(s, width, one_fill(fill), 1)
}
pub fn py_str_center(s: &str, width: i64, fill: &str) -> String {
    pad_to(s, width, one_fill(fill), 2)
}
fn one_fill(fill: &str) -> char {
    let mut it = fill.chars();
    match (it.next(), it.next()) {
        (Some(c), None) => c,
        _ => panic!("The fill character must be exactly one character long"),
    }
}

/// Python asks two things of `isupper` and its siblings: that there is
/// at least one character of the kind, and that none contradicts it.
pub fn py_str_isupper(s: &str) -> bool {
    s.chars().any(char::is_uppercase) && !s.chars().any(char::is_lowercase)
}
pub fn py_str_islower(s: &str) -> bool {
    s.chars().any(char::is_lowercase) && !s.chars().any(char::is_uppercase)
}
pub fn py_str_isalpha(s: &str) -> bool {
    !s.is_empty() && s.chars().all(char::is_alphabetic)
}
pub fn py_str_isdigit(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_numeric())
}
pub fn py_str_isalnum(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_alphanumeric())
}
pub fn py_str_isspace(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c))
}
pub fn py_str_isascii(s: &str) -> bool {
    s.is_ascii()
}

pub fn py_str_removeprefix(s: &str, p: &str) -> String {
    s.strip_prefix(p).unwrap_or(s).to_string()
}
pub fn py_str_removesuffix(s: &str, p: &str) -> String {
    s.strip_suffix(p).unwrap_or(s).to_string()
}

/// `rfind`, and the two that raise where the finds answer -1.
pub fn py_str_rfind(s: &str, p: &str) -> i64 {
    match s.rfind(p) {
        Some(b) => s[..b].chars().count() as i64,
        None => -1,
    }
}
pub fn py_str_index_of(s: &str, p: &str) -> i64 {
    let i = py_str_find(s, p);
    assert!(i >= 0, "substring not found");
    i
}
pub fn py_str_rindex(s: &str, p: &str) -> i64 {
    let i = py_str_rfind(s, p);
    assert!(i >= 0, "substring not found");
    i
}

pub fn py_str_splitlines(s: &str) -> Vec<String> {
    py_str_splitlines_keepends(s)
        .into_iter()
        .map(|l| {
            let t = l.trim_end_matches(['\n', '\r', '\u{b}', '\u{c}', '\u{1c}', '\u{1d}',
                                        '\u{1e}', '\u{85}', '\u{2028}', '\u{2029}']);
            t.to_string()
        })
        .collect()
}

pub fn py_str_expandtabs(s: &str, size: i64) -> String {
    let size = size.max(0) as usize;
    let mut out = String::new();
    let mut col = 0usize;
    for c in s.chars() {
        match c {
            '\t' => {
                if size > 0 {
                    let n = size - col % size;
                    out.extend(std::iter::repeat_n(' ', n));
                    col += n;
                }
            }
            '\n' | '\r' => {
                out.push(c);
                col = 0;
            }
            _ => {
                out.push(c);
                col += 1;
            }
        }
    }
    out
}

/// `strip` and its two halves with a set of characters to remove —
/// a SET, not a prefix, which is the part people misread.
pub fn py_str_strip_chars(s: &str, set: &str) -> String {
    py_str_rstrip_chars(&py_str_lstrip_chars(s, set), set)
}
pub fn py_str_lstrip_chars(s: &str, set: &str) -> String {
    s.trim_start_matches(|c| set.contains(c)).to_string()
}
pub fn py_str_rstrip_chars(s: &str, set: &str) -> String {
    s.trim_end_matches(|c| set.contains(c)).to_string()
}

// ---- re -------------------------------------------------------------
// Python's `re`, as a twin — and the twin borrows CPython's own
// compiler. A pattern is a literal, so the TRANSLATOR compiles it,
// with `re._parser` and `re._compiler`, into the u32 array CPython
// runs; what is here is only the engine that executes that array
// (rustpython-sre_engine, the same one RustPython uses). The
// backtracking, the groups and the flags are therefore CPython's,
// not a second dialect of them, and no regular-expression compiler is
// written on this side.
//
// A `Match` has no shape a typed subset can hold, so what the dialect
// takes are the calls whose answer is already one of its types:
// `findall` (a list of str), `sub` (a str), `split` (a list of str)
// and the tests. The translator refuses the rest by name.

use rustpython_sre_engine::{Request, State};

fn sre_codes(codes: &[i64]) -> Vec<u32> {
    codes.iter().map(|c| *c as u32).collect()
}

/// The characters of `s`, so a match span (which CPython counts in
/// characters) can be sliced without counting the string twice.
fn chars_of(s: &str) -> Vec<char> {
    s.chars().collect()
}

fn slice(cs: &[char], a: usize, b: usize) -> String {
    cs[a.min(cs.len())..b.min(cs.len())].iter().collect()
}

/// One match: the whole span, then each group's span, in characters.
/// A group that did not participate has no span.
struct Found {
    start: usize,
    end: usize,
    groups: Vec<Option<(usize, usize)>>,
}

fn find_all(codes: &[i64], s: &str, ngroups: usize, limit: i64) -> Vec<Found> {
    let codes = sre_codes(codes);
    let mut out = Vec::new();
    let n = s.chars().count();
    let mut req = Request::new(s, 0, n, &codes, false);
    let mut state = State::default();
    loop {
        if limit > 0 && out.len() as i64 >= limit {
            break;
        }
        if req.start > req.end {
            break;
        }
        state.reset(&req, req.start);
        if !state.search(req) {
            break;
        }
        let (start, end) = (state.start, state.cursor.position);
        // `Marks::get` is indexed by the MARK pair, and group 1 writes
        // the first pair — so group n is `get(n - 1)`.
        let groups = (0..ngroups)
            .map(|i| {
                let (a, b) = state.marks.get(i);
                match (a.into_option(), b.into_option()) {
                    (Some(a), Some(b)) => Some((a, b)),
                    _ => None,
                }
            })
            .collect();
        out.push(Found { start, end, groups });
        // An empty match does not match again in the same place —
        // CPython's rule, and what keeps `sub("x*", "-", "abc")` from
        // looping.
        req.must_advance = end == start;
        req.start = end;
    }
    out
}

/// Is there a match at all? `re.search(p, s) is not None` in the app.
pub fn re_search(codes: Vec<i64>, s: &str) -> bool {
    !find_all(&codes, s, 0, 1).is_empty()
}

/// Anchored at the start (`re.match`) and over the whole string
/// (`re.fullmatch`).
fn anchored(codes: Vec<i64>, s: &str, whole: bool) -> bool {
    let codes = sre_codes(&codes);
    let n = s.chars().count();
    let req = Request::new(s, 0, n, &codes, whole);
    let mut state = State::default();
    state.py_match(&req)
}
pub fn re_match(codes: Vec<i64>, s: &str) -> bool {
    anchored(codes, s, false)
}
pub fn re_fullmatch(codes: Vec<i64>, s: &str) -> bool {
    anchored(codes, s, true)
}

/// `re.findall`. `group` is 0 for a pattern with no groups (the whole
/// match) and 1 for a pattern with one; the translator counts them
/// and refuses two or more, which answer tuples.
pub fn re_findall(codes: Vec<i64>, s: &str, group: i64) -> Vec<String> {
    let cs = chars_of(s);
    let ngroups = if group > 0 { group as usize } else { 0 };
    find_all(&codes, s, ngroups, 0)
        .into_iter()
        .map(|m| {
            if group == 0 {
                return slice(&cs, m.start, m.end);
            }
            // A group that did not participate is the empty string
            // here, which is what `findall` answers in Python too.
            match m.groups[group as usize - 1] {
                Some((a, b)) => slice(&cs, a, b),
                None => String::new(),
            }
        })
        .collect()
}

/// `re.sub`. The replacement template is parsed by CPython at
/// translate time into alternating literals and group numbers; what
/// arrives here is those two lists, with -1 marking a literal.
pub fn re_sub(
    codes: Vec<i64>,
    parts: Vec<i64>,
    lits: Vec<String>,
    s: &str,
    count: i64,
    ngroups: i64,
) -> String {
    let cs = chars_of(s);
    let mut out = String::new();
    let mut last = 0usize;
    for m in find_all(&codes, s, ngroups as usize, count) {
        out.push_str(&slice(&cs, last, m.start));
        for (i, g) in parts.iter().enumerate() {
            if *g < 0 {
                out.push_str(&lits[i]);
            } else if *g == 0 {
                out.push_str(&slice(&cs, m.start, m.end));
            } else {
                match m.groups[*g as usize - 1] {
                    Some((a, b)) => out.push_str(&slice(&cs, a, b)),
                    None => panic!("unmatched group {g}"),
                }
            }
        }
        last = m.end;
    }
    out.push_str(&slice(&cs, last, cs.len()));
    out
}

/// `re.split` over a pattern with no groups — a pattern WITH groups
/// interleaves their text, and a group that did not participate is
/// `None` there, which a `list[str]` cannot hold. The translator
/// refuses that case by name.
pub fn re_split(codes: Vec<i64>, s: &str, maxsplit: i64) -> Vec<String> {
    let cs = chars_of(s);
    let mut out = Vec::new();
    let mut last = 0usize;
    for m in find_all(&codes, s, 0, maxsplit) {
        out.push(slice(&cs, last, m.start));
        last = m.end;
    }
    out.push(slice(&cs, last, cs.len()));
    out
}

/// `re.escape` — the characters CPython escapes, and only those.
pub fn re_escape(s: &str) -> String {
    const SPECIAL: &str = "()[]{}?*+-|^$\\.&~# \t\n\r\u{b}\u{c}";
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if SPECIAL.contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

// ---- datetime -------------------------------------------------------
// Python's `datetime`, as a twin, carried as integers: a `date` is
// its ordinal (0001-01-01 is 1, which is what `toordinal` answers), a
// `datetime` is microseconds from that same origin, and a
// `timedelta` is microseconds. Comparison is then integer
// comparison, ordering included, and nothing new has to cross the
// binding boundary.
//
// Naive only. An aware datetime carries a zone, and a zone is what
// §2.1 of the plan keeps out until it can be read from the OS on
// both sides; `timezone` and `tzinfo` are refused by name meanwhile.

const DAY_US: i64 = 86_400_000_000;
/// Python's `date.max.toordinal()` — the calendar stops at 9999.
const ORD_MAX: i64 = 3_652_059;

fn ordinal_to_date(ord: i64) -> chrono::NaiveDate {
    assert!(
        (1..=ORD_MAX).contains(&ord),
        "date value out of range"
    );
    chrono::NaiveDate::from_num_days_from_ce_opt(ord as i32).expect("checked range")
}

fn date_to_ordinal(d: chrono::NaiveDate) -> i64 {
    i64::from(chrono::Datelike::num_days_from_ce(&d))
}

/// A datetime's day and its microseconds within that day. The
/// remainder is Euclidean so a value before the origin cannot happen
/// — the origin IS the minimum.
fn split_datetime(us: i64) -> (i64, i64) {
    assert!(us >= 0 && us < ORD_MAX * DAY_US + DAY_US, "date value out of range");
    (us.div_euclid(DAY_US) + 1, us.rem_euclid(DAY_US))
}

pub fn date_new(y: i64, m: i64, d: i64) -> i64 {
    let bad = |what: &str, lo: i64, hi: i64, got: i64| -> ! {
        panic!("{what} must be in {lo}..{hi}, not {got}")
    };
    if !(1..=9999).contains(&y) {
        bad("year", 1, 9999, y);
    }
    if !(1..=12).contains(&m) {
        bad("month", 1, 12, m);
    }
    match chrono::NaiveDate::from_ymd_opt(y as i32, m as u32, d as u32) {
        Some(x) => date_to_ordinal(x),
        // CPython names the month's own last day, which takes the
        // leap year into account for it.
        None => {
            let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
            let last = chrono::NaiveDate::from_ymd_opt(ny as i32, nm as u32, 1)
                .and_then(|f| f.pred_opt())
                .map(|f| i64::from(chrono::Datelike::day(&f)))
                .unwrap_or(31);
            panic!("day {d} must be in range 1..{last} for month {m} in year {y}")
        }
    }
}

pub fn date_today() -> i64 {
    date_to_ordinal(chrono::Local::now().date_naive())
}

pub fn date_from_iso(s: &str) -> i64 {
    match chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        Ok(d) => date_to_ordinal(d),
        Err(_) => panic!("Invalid isoformat string: '{s}'"),
    }
}

pub fn date_isoformat(ord: i64) -> String {
    let d = ordinal_to_date(ord);
    format!("{}", d.format("%Y-%m-%d"))
}

pub fn date_year(ord: i64) -> i64 {
    i64::from(chrono::Datelike::year(&ordinal_to_date(ord)))
}
pub fn date_month(ord: i64) -> i64 {
    i64::from(chrono::Datelike::month(&ordinal_to_date(ord)))
}
pub fn date_day(ord: i64) -> i64 {
    i64::from(chrono::Datelike::day(&ordinal_to_date(ord)))
}
/// Monday is 0, the way Python counts it.
pub fn date_weekday(ord: i64) -> i64 {
    i64::from(chrono::Datelike::weekday(&ordinal_to_date(ord)).num_days_from_monday())
}
pub fn date_isoweekday(ord: i64) -> i64 {
    date_weekday(ord) + 1
}
pub fn date_toordinal(ord: i64) -> i64 {
    ordinal_to_date(ord);
    ord
}
pub fn date_from_ordinal(ord: i64) -> i64 {
    ordinal_to_date(ord);
    ord
}
pub fn date_strftime(ord: i64, fmt: &str) -> String {
    py_strftime(ordinal_to_date(ord).and_hms_opt(0, 0, 0).expect("midnight"), fmt)
}
/// `str(date)` is its isoformat.
pub fn date_str(ord: i64) -> String {
    date_isoformat(ord)
}

/// `date + timedelta` adds whole DAYS: the delta normalizes first, so
/// twenty-five hours is one day, as it is in Python.
pub fn date_add_delta(ord: i64, us: i64) -> i64 {
    let out = ord + delta_days(us);
    assert!((1..=ORD_MAX).contains(&out), "date value out of range");
    out
}
pub fn date_sub_delta(ord: i64, us: i64) -> i64 {
    date_add_delta(ord, -us)
}
pub fn date_sub_date(a: i64, b: i64) -> i64 {
    (a - b) * DAY_US
}

pub fn datetime_new(y: i64, m: i64, d: i64, h: i64, mi: i64, s: i64, us: i64) -> i64 {
    let ord = date_new(y, m, d);
    for (what, v, hi) in [("hour", h, 23), ("minute", mi, 59), ("second", s, 59), ("microsecond", us, 999_999)] {
        assert!(
            (0..=hi).contains(&v),
            "{what} must be in 0..{hi}, not {v}"
        );
    }
    (ord - 1) * DAY_US + h * 3_600_000_000 + mi * 60_000_000 + s * 1_000_000 + us
}

pub fn datetime_now() -> i64 {
    let now = chrono::Local::now().naive_local();
    naive_to_micros(now)
}

fn naive_to_micros(t: chrono::NaiveDateTime) -> i64 {
    use chrono::Timelike;
    let ord = date_to_ordinal(t.date());
    (ord - 1) * DAY_US
        + i64::from(t.hour()) * 3_600_000_000
        + i64::from(t.minute()) * 60_000_000
        + i64::from(t.second()) * 1_000_000
        + i64::from(t.nanosecond() % 1_000_000_000) / 1_000
}

fn micros_to_naive(us: i64) -> chrono::NaiveDateTime {
    let (ord, rest) = split_datetime(us);
    ordinal_to_date(ord)
        .and_hms_micro_opt(
            (rest / 3_600_000_000) as u32,
            (rest / 60_000_000 % 60) as u32,
            (rest / 1_000_000 % 60) as u32,
            (rest % 1_000_000) as u32,
        )
        .expect("in range")
}

/// A naive datetime is local time, which is what CPython assumes when
/// it turns one into a timestamp — both runs read the same zone.
pub fn datetime_timestamp(us: i64) -> f64 {
    use chrono::TimeZone;
    let naive = micros_to_naive(us);
    match chrono::Local.from_local_datetime(&naive).earliest() {
        Some(t) => t.timestamp() as f64 + (us % 1_000_000) as f64 / 1e6,
        None => panic!("timestamp: {} has no local time", datetime_isoformat(us)),
    }
}

pub fn datetime_from_timestamp(secs: f64) -> i64 {
    use chrono::TimeZone;
    assert!(secs.is_finite(), "timestamp out of range");
    // Python rounds the fraction to the nearest microsecond.
    let whole = secs.floor();
    let frac = ((secs - whole) * 1e6).round() as i64;
    let (whole, frac) = if frac == 1_000_000 { (whole + 1.0, 0) } else { (whole, frac) };
    match chrono::Local.timestamp_opt(whole as i64, 0).earliest() {
        Some(t) => naive_to_micros(t.naive_local()) + frac,
        None => panic!("timestamp out of range"),
    }
}

pub fn datetime_from_iso(s: &str) -> i64 {
    let t = s.replacen('T', " ", 1);
    for fmt in ["%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%d %H:%M", "%Y-%m-%d"] {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&t, fmt) {
            return naive_to_micros(dt);
        }
        if fmt == "%Y-%m-%d" {
            if let Ok(d) = chrono::NaiveDate::parse_from_str(&t, fmt) {
                return (date_to_ordinal(d) - 1) * DAY_US;
            }
        }
    }
    panic!("Invalid isoformat string: '{s}'")
}

fn datetime_body(us: i64, sep: &str) -> String {
    let t = micros_to_naive(us);
    let head = t.format(&format!("%Y-%m-%d{sep}%H:%M:%S")).to_string();
    let frac = us % 1_000_000;
    if frac == 0 { head } else { format!("{head}.{frac:06}") }
}
pub fn datetime_isoformat(us: i64) -> String {
    datetime_body(us, "T")
}
/// `str(datetime)` is its isoformat with a space where the T is.
pub fn datetime_str(us: i64) -> String {
    datetime_body(us, " ")
}

pub fn datetime_date(us: i64) -> i64 {
    split_datetime(us).0
}
pub fn datetime_year(us: i64) -> i64 {
    date_year(datetime_date(us))
}
pub fn datetime_month(us: i64) -> i64 {
    date_month(datetime_date(us))
}
pub fn datetime_day(us: i64) -> i64 {
    date_day(datetime_date(us))
}
pub fn datetime_hour(us: i64) -> i64 {
    split_datetime(us).1 / 3_600_000_000
}
pub fn datetime_minute(us: i64) -> i64 {
    split_datetime(us).1 / 60_000_000 % 60
}
pub fn datetime_second(us: i64) -> i64 {
    split_datetime(us).1 / 1_000_000 % 60
}
pub fn datetime_microsecond(us: i64) -> i64 {
    split_datetime(us).1 % 1_000_000
}
pub fn datetime_weekday(us: i64) -> i64 {
    date_weekday(datetime_date(us))
}
pub fn datetime_isoweekday(us: i64) -> i64 {
    date_weekday(datetime_date(us)) + 1
}
pub fn datetime_toordinal(us: i64) -> i64 {
    datetime_date(us)
}
pub fn datetime_strftime(us: i64, fmt: &str) -> String {
    py_strftime(micros_to_naive(us), fmt)
}
pub fn datetime_add_delta(us: i64, d: i64) -> i64 {
    let out = us.checked_add(d).unwrap_or_else(|| panic!("result out of range"));
    split_datetime(out);
    out
}
pub fn datetime_sub_delta(us: i64, d: i64) -> i64 {
    datetime_add_delta(us, -d)
}
pub fn datetime_sub_datetime(a: i64, b: i64) -> i64 {
    a - b
}
/// `datetime.combine(date, midnight)` in the one shape the dialect
/// has: a date read as a datetime.
pub fn datetime_of_date(ord: i64) -> i64 {
    ordinal_to_date(ord);
    (ord - 1) * DAY_US
}

pub fn delta_new(days: i64, seconds: i64, micros: i64, millis: i64, minutes: i64, hours: i64, weeks: i64) -> i64 {
    let over = || -> ! { panic!("timedelta out of the 64-bit microsecond range") };
    let mul = |a: i64, b: i64| a.checked_mul(b).unwrap_or_else(|| over());
    let add = |a: i64, b: i64| a.checked_add(b).unwrap_or_else(|| over());
    let mut out = mul(days + weeks * 7, DAY_US);
    out = add(out, mul(hours, 3_600_000_000));
    out = add(out, mul(minutes, 60_000_000));
    out = add(out, mul(seconds, 1_000_000));
    out = add(out, mul(millis, 1_000));
    add(out, micros)
}

/// `timedelta` normalizes to days, seconds and microseconds with the
/// last two non-negative, so a negative delta is a negative day count
/// plus a positive remainder — which is why `str(timedelta(days=-1,
/// hours=2))` reads "-1 day, 2:00:00".
pub fn delta_days(us: i64) -> i64 {
    us.div_euclid(DAY_US)
}
pub fn delta_seconds(us: i64) -> i64 {
    us.rem_euclid(DAY_US) / 1_000_000
}
pub fn delta_microseconds(us: i64) -> i64 {
    us.rem_euclid(DAY_US) % 1_000_000
}
pub fn delta_total_seconds(us: i64) -> f64 {
    us as f64 / 1e6
}
pub fn delta_str(us: i64) -> String {
    let (days, rest) = (delta_days(us), us.rem_euclid(DAY_US));
    let (h, m, s, frac) = (
        rest / 3_600_000_000,
        rest / 60_000_000 % 60,
        rest / 1_000_000 % 60,
        rest % 1_000_000,
    );
    let mut out = String::new();
    if days != 0 {
        out.push_str(&format!("{days} day{}, ", if days.abs() == 1 { "" } else { "s" }));
    }
    out.push_str(&format!("{h}:{m:02}:{s:02}"));
    if frac != 0 {
        out.push_str(&format!(".{frac:06}"));
    }
    out
}
pub fn delta_add(a: i64, b: i64) -> i64 {
    a.checked_add(b).unwrap_or_else(|| panic!("result out of range"))
}
pub fn delta_sub(a: i64, b: i64) -> i64 {
    a.checked_sub(b).unwrap_or_else(|| panic!("result out of range"))
}
pub fn delta_mul(a: i64, n: i64) -> i64 {
    a.checked_mul(n).unwrap_or_else(|| panic!("result out of range"))
}
pub fn delta_neg(a: i64) -> i64 {
    -a
}
pub fn delta_abs(a: i64) -> i64 {
    a.abs()
}

const WDAY_SHORT: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const WDAY_LONG: [&str; 7] = [
    "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday",
];
const MON_SHORT: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const MON_LONG: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August",
    "September", "October", "November", "December",
];

/// `strftime`, written out rather than handed to a C library: the
/// directives CPython gives a meaning of its own, with the month and
/// day names of the C locale. `%c`, `%x`, `%X` and the platform
/// extensions (`%-d`) are refused, because what they answer is the
/// machine's business and not Python's.
fn py_strftime(t: chrono::NaiveDateTime, fmt: &str) -> String {
    use chrono::{Datelike, Timelike};
    let d = t.date();
    let wd = d.weekday().num_days_from_monday() as usize;
    let mut out = String::new();
    let mut cs = fmt.chars().peekable();
    while let Some(c) = cs.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let Some(k) = cs.next() else {
            panic!("stray %% in format string");
        };
        match k {
            'Y' => out.push_str(&format!("{:04}", d.year())),
            'y' => out.push_str(&format!("{:02}", d.year() % 100)),
            'm' => out.push_str(&format!("{:02}", d.month())),
            'd' => out.push_str(&format!("{:02}", d.day())),
            'H' => out.push_str(&format!("{:02}", t.hour())),
            'M' => out.push_str(&format!("{:02}", t.minute())),
            'S' => out.push_str(&format!("{:02}", t.second())),
            'f' => out.push_str(&format!("{:06}", t.nanosecond() / 1_000)),
            'j' => out.push_str(&format!("{:03}", d.ordinal())),
            'a' => out.push_str(WDAY_SHORT[wd]),
            'A' => out.push_str(WDAY_LONG[wd]),
            'b' => out.push_str(MON_SHORT[d.month0() as usize]),
            'B' => out.push_str(MON_LONG[d.month0() as usize]),
            'p' => out.push_str(if t.hour() < 12 { "AM" } else { "PM" }),
            'I' => {
                let h = t.hour() % 12;
                out.push_str(&format!("{:02}", if h == 0 { 12 } else { h }));
            }
            // Python counts the week from Sunday for %U and from
            // Monday for %W, and both give 00 to the days before the
            // first such weekday of the year.
            'w' => out.push_str(&((wd + 1) % 7).to_string()),
            'U' | 'W' => {
                let first = if k == 'U' {
                    (d.ordinal() as i32 - 1 - (wd as i32 + 1) % 7 + 7) / 7
                } else {
                    (d.ordinal() as i32 - 1 - wd as i32 + 7) / 7
                };
                out.push_str(&format!("{first:02}"));
            }
            // Naive here, so the zone is empty rather than absent.
            'z' | 'Z' => {}
            '%' => out.push('%'),
            other => panic!(
                "`%{other}` is not a format code the dialect takes — the platform decides \
                 what it means, and the two runs would not agree"
            ),
        }
    }
    out
}

// ---- strings --------------------------------------------------------

/// Total parse: trimmed integer or the default — the `.get(k, d)`
/// pattern applied to parsing, so both tiers agree on bad input
/// instead of raising vs trapping.
pub fn strings_to_int(s: &str, default: i64) -> i64 {
    s.trim().parse::<i64>().unwrap_or(default)
}

/// Queue an OS notification. The engine delivers it through the
/// platform when the app runs from an `.app` bundle; a bare binary
/// logs-and-drops at the platform layer and a headless run never
/// drains the queue — sending is always best-effort.
pub fn notify_send(title: &str, body: &str) {
    pixie_kernel::notify::send(title, body);
}

/// Sleep for `ms` milliseconds and answer 0. Both doors release
/// before they call it: the interpreted one detaches from Python, and
/// the compiled one is awaited, which puts it on the engine's pool.
/// Total float parse: the value or the default, never an error.
pub fn strings_to_float(s: &str, default: f64) -> f64 {
    s.trim().parse::<f64>().unwrap_or(default)
}

/// Scalar query: first column of the first row as an integer.
/// Wrap aggregates in COALESCE for the empty case.
pub fn sqlite_query_int_result(path: &str, sql: &str) -> std::io::Result<i64> {
    let conn = rusqlite::Connection::open(path)
        .map_err(|e| std::io::Error::other(format!("sqlite.query_int open {path}: {e}")))?;
    conn.query_row(sql, [], |row| row.get::<_, i64>(0))
        .map_err(|e| std::io::Error::other(format!("sqlite.query_int {path}: {e}")))
}

pub fn sqlite_query_int(path: &str, sql: &str) -> i64 {
    match sqlite_query_int_result(path, sql) {
        Ok(n) => n,
        Err(e) => panic!("{e}"),
    }
}

// ---- random ---------------------------------------------------------
// CPython's `random`, as a twin. The interpreted run calls CPython's
// own module, so this is the Mersenne Twister CPython uses and the
// algorithms `random.py` builds on it — the same seeding, the same
// rejection loop, the same `gauss` carry. Anything less and a seeded
// app would print two different sequences.

use std::sync::Mutex;

const MT_N: usize = 624;
const MT_M: usize = 397;

struct Mt {
    mt: [u32; MT_N],
    index: usize,
    /// `gauss` computes two normals at a time and keeps the spare,
    /// which `seed` throws away — CPython does both.
    gauss_next: Option<f64>,
}

impl Mt {
    fn init_genrand(&mut self, s: u32) {
        self.mt[0] = s;
        for i in 1..MT_N {
            let prev = self.mt[i - 1];
            self.mt[i] = 1812433253u32
                .wrapping_mul(prev ^ (prev >> 30))
                .wrapping_add(i as u32);
        }
        self.index = MT_N;
    }

    fn init_by_array(&mut self, key: &[u32]) {
        self.init_genrand(19650218);
        let (mut i, mut j) = (1usize, 0usize);
        for _ in 0..MT_N.max(key.len()) {
            let prev = self.mt[i - 1];
            self.mt[i] = (self.mt[i] ^ (prev ^ (prev >> 30)).wrapping_mul(1664525))
                .wrapping_add(key[j])
                .wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i >= MT_N {
                self.mt[0] = self.mt[MT_N - 1];
                i = 1;
            }
            if j >= key.len() {
                j = 0;
            }
        }
        for _ in 0..MT_N - 1 {
            let prev = self.mt[i - 1];
            self.mt[i] = (self.mt[i] ^ (prev ^ (prev >> 30)).wrapping_mul(1566083941))
                .wrapping_sub(i as u32);
            i += 1;
            if i >= MT_N {
                self.mt[0] = self.mt[MT_N - 1];
                i = 1;
            }
        }
        self.mt[0] = 0x8000_0000;
    }

    fn generate(&mut self) {
        const MAG01: [u32; 2] = [0, 0x9908_b0df];
        const UPPER: u32 = 0x8000_0000;
        const LOWER: u32 = 0x7fff_ffff;
        for kk in 0..MT_N - MT_M {
            let y = (self.mt[kk] & UPPER) | (self.mt[kk + 1] & LOWER);
            self.mt[kk] = self.mt[kk + MT_M] ^ (y >> 1) ^ MAG01[(y & 1) as usize];
        }
        for kk in MT_N - MT_M..MT_N - 1 {
            let y = (self.mt[kk] & UPPER) | (self.mt[kk + 1] & LOWER);
            self.mt[kk] = self.mt[kk + MT_M - MT_N] ^ (y >> 1) ^ MAG01[(y & 1) as usize];
        }
        let y = (self.mt[MT_N - 1] & UPPER) | (self.mt[0] & LOWER);
        self.mt[MT_N - 1] = self.mt[MT_M - 1] ^ (y >> 1) ^ MAG01[(y & 1) as usize];
        self.index = 0;
    }

    fn next_u32(&mut self) -> u32 {
        if self.index >= MT_N {
            self.generate();
        }
        let mut y = self.mt[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^ (y >> 18)
    }

    /// `genrand_res53`: 53 bits of a double, built from two draws.
    fn next_f64(&mut self) -> f64 {
        let a = (self.next_u32() >> 5) as f64;
        let b = (self.next_u32() >> 6) as f64;
        (a * 67108864.0 + b) * (1.0 / 9007199254740992.0)
    }

    fn getrandbits(&mut self, k: i64) -> i64 {
        assert!(k >= 0, "Cannot convert negative int");
        assert!(
            k < 64,
            "`random.getrandbits` past 63 bits does not fit the 64-bit int range"
        );
        if k == 0 {
            return 0;
        }
        if k <= 32 {
            return (self.next_u32() >> (32 - k)) as i64;
        }
        // Two words, the first one least significant — CPython builds
        // the integer from the words little-endian.
        let lo = self.next_u32() as u64;
        let hi = (self.next_u32() >> (64 - k)) as u64;
        ((hi << 32) | lo) as i64
    }

    /// `_randbelow_with_getrandbits`: draw the right number of bits
    /// and throw away anything past the range, which is what keeps
    /// the distribution flat.
    fn below(&mut self, n: i64) -> i64 {
        if n == 0 {
            return 0;
        }
        let k = 64 - (n as u64).leading_zeros() as i64;
        loop {
            let r = self.getrandbits(k);
            if r < n {
                return r;
            }
        }
    }
}

fn rng() -> &'static Mutex<Mt> {
    static RNG: std::sync::OnceLock<Mutex<Mt>> = std::sync::OnceLock::new();
    RNG.get_or_init(|| {
        let mut m = Mt { mt: [0; MT_N], index: MT_N, gauss_next: None };
        // Unseeded is unseeded, the way it is in Python: an app that
        // wants the two runs to print one sequence calls `seed`.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u32)
            .unwrap_or(0);
        m.init_by_array(&[now, std::process::id()]);
        Mutex::new(m)
    })
}

fn with_rng<R>(f: impl FnOnce(&mut Mt) -> R) -> R {
    let mut g = rng().lock().unwrap_or_else(|e| e.into_inner());
    f(&mut g)
}

/// `random.seed(n)` — CPython turns the absolute value into 32-bit
/// words and runs `init_by_array` over them.
pub fn random_seed(n: i64) {
    let mag = (n as i128).unsigned_abs() as u128;
    let mut key: Vec<u32> = Vec::new();
    let mut v = mag;
    while v > 0 {
        key.push((v & 0xffff_ffff) as u32);
        v >>= 32;
    }
    if key.is_empty() {
        key.push(0);
    }
    with_rng(|m| {
        m.init_by_array(&key);
        m.gauss_next = None;
    });
}

/// The binding wants a value back from every static.
pub fn random_seed_ret(n: i64) -> i64 {
    random_seed(n);
    n
}

pub fn random_random() -> f64 {
    with_rng(|m| m.next_f64())
}

pub fn random_getrandbits(k: i64) -> i64 {
    with_rng(|m| m.getrandbits(k))
}

pub fn random_randrange(stop: i64) -> i64 {
    assert!(stop > 0, "empty range for randrange()");
    with_rng(|m| m.below(stop))
}

pub fn random_randrange_from(start: i64, stop: i64) -> i64 {
    let width = stop - start;
    assert!(width > 0, "empty range in randrange({start}, {stop})");
    with_rng(|m| start + m.below(width))
}

pub fn random_randrange_step(start: i64, stop: i64, step: i64) -> i64 {
    assert!(step != 0, "zero step for randrange()");
    let width = stop - start;
    let n = if step > 0 {
        (width + step - 1) / step
    } else {
        (width + step + 1) / step
    };
    assert!(n > 0, "empty range in randrange({start}, {stop}, {step})");
    with_rng(|m| start + step * m.below(n))
}

pub fn random_randint(a: i64, b: i64) -> i64 {
    random_randrange_from(a, b + 1)
}

pub fn random_uniform(a: f64, b: f64) -> f64 {
    a + (b - a) * random_random()
}

/// `gauss` draws two normals from one pair of uniforms and keeps the
/// spare for the next call, so the sequence depends on how many times
/// it has been asked — CPython's carry, kept here too.
pub fn random_gauss(mu: f64, sigma: f64) -> f64 {
    let z = with_rng(|m| {
        if let Some(z) = m.gauss_next.take() {
            return z;
        }
        let x2pi = m.next_f64() * std::f64::consts::TAU;
        let g2rad = math_sqrt(-2.0 * math_log(1.0 - m.next_f64()));
        let z = math_cos(x2pi) * g2rad;
        m.gauss_next = Some(math_sin(x2pi) * g2rad);
        z
    });
    mu + z * sigma
}

/// `choice` and `sample` answer elements, so there is one of each per
/// element type — the translator picks by what the list holds.
macro_rules! random_pickers {
    ($($choice:ident, $sample:ident, $t:ty;)*) => {$(
        pub fn $choice(xs: Vec<$t>) -> $t {
            assert!(!xs.is_empty(), "Cannot choose from an empty sequence");
            let i = with_rng(|m| m.below(xs.len() as i64));
            xs[i as usize].clone()
        }
        pub fn $sample(xs: Vec<$t>, k: i64) -> Vec<$t> {
            sample_indices(xs.len(), k)
                .into_iter()
                .map(|i| xs[i].clone())
                .collect()
        }
    )*};
}

random_pickers! {
    random_choice_str, random_sample_str, String;
    random_choice_int, random_sample_int, i64;
    random_choice_float, random_sample_float, f64;
    random_choice_bool, random_sample_bool, bool;
}

/// The positions `random.sample` picks, in the order it picks them.
/// CPython chooses between two strategies by size, and which one runs
/// changes how many numbers are drawn, so the choice is part of the
/// answer rather than an optimization.
fn sample_indices(n: usize, k: i64) -> Vec<usize> {
    assert!(
        k >= 0 && k as usize <= n,
        "Sample larger than population or is negative"
    );
    let k = k as usize;
    let mut setsize = 21usize;
    if k > 5 {
        setsize += 4f64.powf(math_log_base((k * 3) as f64, 4.0).ceil()) as usize;
    }
    let mut out = vec![0usize; k];
    if n <= setsize {
        let mut pool: Vec<usize> = (0..n).collect();
        for (i, slot) in out.iter_mut().enumerate() {
            let j = with_rng(|m| m.below((n - i) as i64)) as usize;
            *slot = pool[j];
            pool[j] = pool[n - i - 1];
        }
    } else {
        let mut selected: std::collections::HashSet<usize> = std::collections::HashSet::new();
        for slot in out.iter_mut() {
            let mut j = with_rng(|m| m.below(n as i64)) as usize;
            while selected.contains(&j) {
                j = with_rng(|m| m.below(n as i64)) as usize;
            }
            selected.insert(j);
            *slot = j;
        }
    }
    out
}

// ---- statistics -----------------------------------------------------
// CPython's `statistics`, as a twin. Its `mean` and its `variance`
// add the data as exact rationals and round once at the end, which
// is a different number from the one a naive sum gives:
// `mean([0.1, 0.2, 0.3])` is 0.2, not 0.20000000000000004. Being
// close is not being the same, so the arithmetic here is exact too.
//
// The data is `list[float]`, and only that. CPython answers an int
// for `mean([1, 2, 3])` and a float for `mean([1, 2, 4])` — the type
// follows the values — which a statically typed subset cannot take,
// so the dialect refuses an int list by name rather than guessing.

use num_bigint::{BigInt, BigUint};
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};

/// A double as the exact fraction it is. Every double is one: a
/// mantissa over a power of two.
fn exact_ratio(x: f64) -> BigRational {
    assert!(
        x.is_finite(),
        "cannot convert {} to a rational",
        if x.is_nan() { "NaN".to_string() } else { py_float_repr(x) }
    );
    let bits = x.to_bits();
    let raw_exp = ((bits >> 52) & 0x7ff) as i64;
    let frac = bits & 0x000f_ffff_ffff_ffff;
    let (m, e) = if raw_exp == 0 {
        (frac, -1074i64)
    } else {
        (frac | (1u64 << 52), raw_exp - 1075)
    };
    let mut n = BigInt::from(m);
    if x.is_sign_negative() {
        n = -n;
    }
    let one = BigInt::from(1);
    if e >= 0 {
        BigRational::new(n << (e as usize), one)
    } else {
        BigRational::new(n, one << ((-e) as usize))
    }
}

/// CPython's `_sum` and `_ss` keep the non-finite values apart and
/// throw the finite ones away when there are any: the total is the
/// sum of the infinities and NaNs alone. That is why
/// `mean([inf, 1.0])` is `inf` rather than a NaN from `inf - inf`.
fn nonfinite_total(xs: &[f64]) -> Option<f64> {
    let mut total = 0.0f64;
    let mut any = false;
    for x in xs {
        if !x.is_finite() {
            total += *x;
            any = true;
        }
    }
    if any { Some(total) } else { None }
}

fn stat_len(xs: &[f64], what: &str, least: usize) -> usize {
    assert!(
        xs.len() >= least,
        "{what} requires at least {} data point{}",
        if least == 1 { "one".to_string() } else { "two".to_string() },
        if least == 1 { "" } else { "s" }
    );
    xs.len()
}

/// The sum of squared deviations, exactly — CPython's `_ss`. The
/// formula `(n*Σx² − (Σx)²)/n` is a poor one in floating point and an
/// exact one in rationals, which is why it is written this way there
/// and here.
fn sum_of_squares(xs: &[f64]) -> BigRational {
    let mut sx = BigRational::zero();
    let mut sxx = BigRational::zero();
    for x in xs {
        let r = exact_ratio(*x);
        sxx += &r * &r;
        sx += r;
    }
    let n = BigRational::from(BigInt::from(xs.len()));
    (&n * sxx - &sx * &sx) / n
}

/// `p/q` as the NEAREST double, ties to even.
///
/// Written out rather than borrowed, because rounding the numerator
/// and the denominator separately and dividing rounds twice, and the
/// second rounding is exactly what holding the sum exactly was for:
/// `stdev([1.0, 2.0])` comes out an ulp low that way.
fn ratio_to_f64(r: &BigRational) -> f64 {
    let neg = r.numer().is_negative();
    let p = r.numer().magnitude().clone();
    let q = r.denom().magnitude().clone();
    if p.is_zero() {
        return if neg { -0.0 } else { 0.0 };
    }
    // The quotient lies in [2**(e-1), 2**(e+1)); scale it so the
    // integer part carries 55 bits — 53 of mantissa, one to round on
    // and one to keep the comparison exact.
    let e = p.bits() as i64 - q.bits() as i64;
    let shift = 55 - e;
    let (np, nq) = if shift >= 0 {
        (p << shift as usize, q)
    } else {
        (p, q << ((-shift) as usize))
    };
    let mut quo = &np / &nq;
    let rem = np % nq;
    let mut exp = -shift;
    let mut sticky = !rem.is_zero();
    let mut round_bit = false;
    let extra = quo.bits() as i64 - 53;
    if extra > 0 {
        let mask = (BigUint::one() << extra as usize) - BigUint::one();
        let dropped = &quo & &mask;
        round_bit = dropped.bit((extra - 1) as u64);
        let below = (BigUint::one() << (extra - 1) as usize) - BigUint::one();
        if !(&dropped & &below).is_zero() {
            sticky = true;
        }
        quo >>= extra as usize;
        exp += extra;
    }
    let mut m = quo.to_u64().expect("53 bits fit");
    if round_bit && (sticky || m & 1 == 1) {
        m += 1;
        if m == 1u64 << 53 {
            m >>= 1;
            exp += 1;
        }
    }
    let out = math_ldexp_unchecked(m as f64, exp);
    assert!(
        out.is_finite(),
        "integer division result too large for a float"
    );
    if neg { -out } else { out }
}

/// The integer square root of `n/m`, rounded to ODD. Setting the low
/// bit when the root is inexact is what carries the "something was
/// dropped" fact into the division below, so the one rounding at the
/// end lands where a rounding of the true root would.
fn isqrt_rto(n: &BigUint, m: &BigUint) -> BigUint {
    let a = (n / m).sqrt();
    if &(&a * &a) * m != *n { a | BigUint::one() } else { a }
}

/// The square root of `n/m` as a correctly rounded double, by
/// CPython's method: take enough bits of the integer square root,
/// round those to odd, and let the final division do the only
/// rounding that reaches the answer.
fn sqrt_of_frac(n: &BigInt, m: &BigInt) -> f64 {
    assert!(!n.is_negative(), "math domain error");
    if n.is_zero() {
        return 0.0;
    }
    const SQRT_BIT_WIDTH: i64 = 2 * 53 + 3;
    let bits = |v: &BigInt| v.magnitude().bits() as i64;
    let (num, den) = (n.magnitude(), m.magnitude());
    let q = (bits(n) - bits(m) - SQRT_BIT_WIDTH).div_euclid(2);
    let (num, den) = if q >= 0 {
        (
            BigInt::from(isqrt_rto(num, &(den << (2 * q) as usize)) << q as usize),
            BigInt::from(1),
        )
    } else {
        (
            BigInt::from(isqrt_rto(&(num << ((-2 * q) as usize)), den)),
            BigInt::from(1) << ((-q) as usize),
        )
    };
    ratio_to_f64(&BigRational::new(num, den))
}

pub fn statistics_mean(xs: Vec<f64>) -> f64 {
    let n = stat_len(&xs, "mean", 1);
    if let Some(t) = nonfinite_total(&xs) {
        return t / n as f64;
    }
    let mut total = BigRational::zero();
    for x in &xs {
        total += exact_ratio(*x);
    }
    ratio_to_f64(&(total / BigRational::from(BigInt::from(n))))
}

/// `fmean` is the floating-point one on purpose: the correctly
/// rounded sum divided by the count, which rounds twice where `mean`
/// rounds once.
pub fn statistics_fmean(xs: Vec<f64>) -> f64 {
    let n = stat_len(&xs, "fmean", 1);
    math_fsum(xs) / n as f64
}

/// A NaN in the data has no place in an order, and CPython's own
/// answer here follows wherever its sort happened to leave it. Two
/// values is the shape a table can pin down.
pub fn statistics_median(xs: Vec<f64>) -> f64 {
    assert!(!xs.is_empty(), "no median for empty data");
    let sorted = py_list_sorted_float(xs);
    let n = sorted.len();
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    }
}

/// `mode` is the most common value, and a tie goes to whichever was
/// seen first — `Counter.most_common` keeps insertion order, so the
/// answer depends on it.
pub fn statistics_mode(xs: Vec<f64>) -> f64 {
    assert!(!xs.is_empty(), "no mode for empty data");
    let mut seen: Vec<(u64, f64, usize)> = Vec::new();
    for x in &xs {
        let key = x.to_bits();
        match seen.iter_mut().find(|(k, _, _)| *k == key) {
            Some(slot) => slot.2 += 1,
            None => seen.push((key, *x, 1)),
        }
    }
    let best = seen.iter().map(|(_, _, c)| *c).max().unwrap_or(0);
    seen.iter().find(|(_, _, c)| *c == best).expect("non-empty").1
}

pub fn statistics_variance(xs: Vec<f64>) -> f64 {
    let n = stat_len(&xs, "variance", 2);
    if let Some(t) = nonfinite_total(&xs) {
        return t / (n - 1) as f64;
    }
    ratio_to_f64(&(sum_of_squares(&xs) / BigRational::from(BigInt::from(n - 1))))
}

pub fn statistics_pvariance(xs: Vec<f64>) -> f64 {
    let n = stat_len(&xs, "pvariance", 1);
    if let Some(t) = nonfinite_total(&xs) {
        return t / n as f64;
    }
    ratio_to_f64(&(sum_of_squares(&xs) / BigRational::from(BigInt::from(n))))
}

pub fn statistics_stdev(xs: Vec<f64>) -> f64 {
    let n = stat_len(&xs, "stdev", 2);
    assert!(nonfinite_total(&xs).is_none(), "inf or nan encountered in data");
    let mss = sum_of_squares(&xs) / BigRational::from(BigInt::from(n - 1));
    sqrt_of_frac(mss.numer(), mss.denom())
}

pub fn statistics_pstdev(xs: Vec<f64>) -> f64 {
    let n = stat_len(&xs, "pstdev", 1);
    assert!(nonfinite_total(&xs).is_none(), "inf or nan encountered in data");
    let mss = sum_of_squares(&xs) / BigRational::from(BigInt::from(n));
    sqrt_of_frac(mss.numer(), mss.denom())
}

// ---- the *_or family ------------------------------------------------
// Return-value error handling as the ERGONOMIC DEFAULT for the
// native modules (owner course-correction): the native side has no
// exceptions — Results are values — so the 80% case reads better as
// a total function with a default. try/except stays for when the
// failure REASON matters.

pub fn fs_read_text_or(path: &str, default: &str) -> String {
    fs_read_text_result(path).unwrap_or_else(|_| default.to_string())
}

pub fn http_get_text_or(url: &str, default: &str) -> String {
    http_get_text_result(url).unwrap_or_else(|_| default.to_string())
}

pub fn sqlite_query_int_or(path: &str, sql: &str, default: i64) -> i64 {
    sqlite_query_int_result(path, sql).unwrap_or(default)
}

/// Rows or NOTHING: a failed query answers the empty list — absence
/// of rows and absence of a table read the same to a view.
pub fn sqlite_query_text_or(path: &str, sql: &str) -> Vec<String> {
    let conn = match rusqlite::Connection::open(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let rows = stmt.query_map([], |row| {
        Ok(match row.get::<_, rusqlite::types::Value>(0) {
            Ok(rusqlite::types::Value::Integer(i)) => i.to_string(),
            Ok(rusqlite::types::Value::Real(f)) => f.to_string(),
            Ok(rusqlite::types::Value::Text(s)) => s,
            Ok(rusqlite::types::Value::Null) => String::new(),
            Ok(rusqlite::types::Value::Blob(_)) => "<blob>".to_string(),
            Err(_) => String::new(),
        })
    });
    match rows {
        Ok(it) => it.filter_map(|r| r.ok()).collect(),
        Err(_) => Vec::new(),
    }
}

// ---- Python-semantics operations (the `Py` class) -----------------
//
// The translator rewrites Python's `/`, `//`, `%`, `**` and bare
// float/bool/enum text into calls to these. Tier A runs the real
// Python operator / str(); these functions reproduce CPython's
// results exactly, so the tiers agree by construction. Failure cases
// (zero division, complex results, overflow to infinity) panic —
// contained, matching Python's raised-exception abort of the same
// statement.

/// str(float) — CPython's shortest-round-trip rendering. The one
/// implementation lives in the kernel, because a `NumberField` has to
/// show its bound value with exactly the string an interpolation of
/// that number produces; this re-export is the name the generated
/// code calls (`Py.floatRepr`).
pub use pixie_kernel::py_float_repr;

/// str(bool) — "True"/"False".
pub fn py_bool_repr(v: bool) -> String {
    if v { "True" } else { "False" }.to_string()
}

/// int / int — always a float, correctly rounded the way CPython
/// rounds it (one rounding, not cast-then-divide's two).
pub fn py_truediv_int(a: i64, b: i64) -> f64 {
    if b == 0 {
        panic!("division by zero");
    }
    let neg = (a < 0) != (b < 0);
    let ua = a.unsigned_abs();
    let ub = b.unsigned_abs();
    // CPython's own fast path: both sides exactly representable.
    if ua <= 1 << 53 && ub <= 1 << 53 {
        let q = ua as f64 / ub as f64;
        return if neg { -q } else { q };
    }
    // Scale so the integer quotient carries >= 54 significant bits,
    // divide in 128 bits, round to nearest-even with the remainder
    // as the sticky bit.
    let la = 64 - ua.leading_zeros() as i32;
    let lb = 64 - ub.leading_zeros() as i32;
    let shift = 55 - (la - lb);
    let (q, r, exp) = if shift >= 0 {
        let num = (ua as u128) << shift;
        (num / ub as u128, num % ub as u128, -shift)
    } else {
        let den = (ub as u128) << -shift;
        (ua as u128 / den, ua as u128 % den, -shift)
    };
    let bits = 128 - q.leading_zeros() as i32;
    let k = bits - 53;
    debug_assert!(k >= 1);
    let low = q & ((1u128 << k) - 1);
    let half = 1u128 << (k - 1);
    let mut m = q >> k;
    let sticky = r != 0;
    if low > half || (low == half && (sticky || (m & 1) == 1)) {
        m += 1;
    }
    let val = (m as f64) * (2f64).powi(exp + k);
    if neg { -val } else { val }
}

/// float / float — IEEE division, except a zero divisor fails the
/// statement the way Python's ZeroDivisionError does.
pub fn py_truediv_float(a: f64, b: f64) -> f64 {
    if b == 0.0 {
        panic!("float division by zero");
    }
    a / b
}

/// int // int — rounds toward negative infinity.
pub fn py_floordiv_int(a: i64, b: i64) -> i64 {
    if b == 0 {
        panic!("integer division by zero");
    }
    let q = a / b;
    let r = a % b;
    if r != 0 && ((r < 0) != (b < 0)) { q - 1 } else { q }
}

/// int % int — the result carries the divisor's sign.
pub fn py_mod_int(a: i64, b: i64) -> i64 {
    if b == 0 {
        panic!("integer modulo by zero");
    }
    let r = a % b;
    if r != 0 && ((r < 0) != (b < 0)) { r + b } else { r }
}

/// float % float — fmod adjusted so the result carries the
/// divisor's sign (CPython's float modulo).
pub fn py_mod_float(a: f64, b: f64) -> f64 {
    if b == 0.0 {
        panic!("float modulo by zero");
    }
    let m = a % b;
    if m != 0.0 {
        if (b < 0.0) != (m < 0.0) { m + b } else { m }
    } else {
        0.0f64.copysign(b)
    }
}

/// float // float — CPython's float floor division (derived from
/// the adjusted modulo, then floored).
pub fn py_floordiv_float(a: f64, b: f64) -> f64 {
    if b == 0.0 {
        panic!("float floor division by zero");
    }
    let m = a % b;
    let mut d = (a - m) / b;
    if m != 0.0 && ((b < 0.0) != (m < 0.0)) {
        d -= 1.0;
    }
    if d != 0.0 {
        let fl = d.floor();
        if d - fl > 0.5 { fl + 1.0 } else { fl }
    } else {
        0.0f64.copysign(a / b)
    }
}

/// int ** int with a non-negative exponent — exact, overflow fails
/// the statement (Python grows past 64 bits there; the int range
/// check fails the same statement on its side).
pub fn py_pow_int(a: i64, b: i64) -> i64 {
    assert!(b >= 0, "negative exponents take the float form");
    let mut result: i64 = 1;
    let mut base = a;
    let mut e = b as u64;
    while e > 0 {
        if e & 1 == 1 {
            result = result.checked_mul(base).expect("`**` overflows the 64-bit int range");
        }
        e >>= 1;
        if e > 0 {
            base = base.checked_mul(base).expect("`**` overflows the 64-bit int range");
        }
    }
    result
}

/// float ** float — CPython's special cases, then libm pow.
pub fn py_pow_float(a: f64, b: f64) -> f64 {
    if b == 0.0 {
        return 1.0; // pow(x, ±0) == 1 even for nan/inf bases
    }
    if a.is_nan() {
        return a;
    }
    if b.is_nan() {
        return if a == 1.0 { 1.0 } else { b };
    }
    if b.is_infinite() {
        let av = a.abs();
        return if av == 1.0 {
            1.0
        } else if (av > 1.0) == (b > 0.0) {
            f64::INFINITY
        } else {
            0.0
        };
    }
    if a == 0.0 && b < 0.0 {
        panic!("0.0 cannot be raised to a negative power");
    }
    let mut base = a;
    let mut negate = false;
    if a < 0.0 {
        if b != b.floor() {
            panic!("negative number cannot be raised to a fractional power");
        }
        base = -a;
        negate = (b % 2.0).abs() == 1.0;
    }
    let r = base.powf(b);
    if r.is_infinite() && base.is_finite() {
        panic!("float power result too large");
    }
    if negate { -r } else { r }
}

// ---- Python's str, once, in Rust -----------------------------------
// The compiled run calls these; the development run calls CPython's
// own methods, so each one answers what CPython answers — including
// the failures, which trap the statement the way a raised exception
// ends it.

/// len(s) — code points, not bytes.
pub fn py_str_len(s: &str) -> i64 {
    s.chars().count() as i64
}

/// s[i] — one code point, negative counting from the back; past the
/// end raises IndexError in Python, and traps here.
pub fn py_str_index(s: &str, i: i64) -> String {
    let n = s.chars().count() as i64;
    let k = if i < 0 { i + n } else { i };
    if k < 0 || k >= n {
        panic!("string index out of range");
    }
    s.chars().nth(k as usize).expect("bounds checked").to_string()
}

/// s[a:b] — Python's slice: negative counts from the back, ends
/// clamp instead of failing, and a start past the stop answers "".
pub fn py_str_slice(s: &str, a: i64, b: i64) -> String {
    let cs: Vec<char> = s.chars().collect();
    let n = cs.len() as i64;
    let clamp = |v: i64| -> usize {
        let v = if v < 0 { v + n } else { v };
        v.clamp(0, n) as usize
    };
    let (lo, hi) = (clamp(a), clamp(b));
    if lo >= hi {
        return String::new();
    }
    cs[lo..hi].iter().collect()
}

pub fn py_str_upper(s: &str) -> String {
    s.to_uppercase()
}

pub fn py_str_lower(s: &str) -> String {
    s.to_lowercase()
}

/// .strip() / .lstrip() / .rstrip() with no argument: Python strips
/// whitespace, which is the set Rust's `char::is_whitespace` names.
pub fn py_str_strip(s: &str) -> String {
    s.trim().to_string()
}

pub fn py_str_lstrip(s: &str) -> String {
    s.trim_start().to_string()
}

pub fn py_str_rstrip(s: &str) -> String {
    s.trim_end().to_string()
}

/// s.split(sep) — the separator form: an empty separator raises in
/// Python, and "a,,b".split(",") keeps the empty field.
pub fn py_str_split(s: &str, sep: &str) -> Vec<String> {
    if sep.is_empty() {
        panic!("empty separator");
    }
    s.split(sep).map(|p| p.to_string()).collect()
}

/// s.split() with no argument: runs of whitespace, no empty fields.
pub fn py_str_split_ws(s: &str) -> Vec<String> {
    s.split_whitespace().map(|p| p.to_string()).collect()
}

pub fn py_str_join(sep: &str, parts: Vec<String>) -> String {
    parts.join(sep)
}

pub fn py_str_startswith(s: &str, p: &str) -> bool {
    s.starts_with(p)
}

pub fn py_str_endswith(s: &str, p: &str) -> bool {
    s.ends_with(p)
}

pub fn py_str_contains(s: &str, p: &str) -> bool {
    s.contains(p)
}

pub fn py_str_replace(s: &str, from: &str, to: &str) -> String {
    if from.is_empty() {
        // CPython inserts `to` between every character; the dialect
        // has no use for that, and guessing would be worse than a
        // named failure.
        panic!("replace() with an empty search string");
    }
    s.replace(from, to)
}

/// s.find(p) — the code-point index, or -1.
pub fn py_str_find(s: &str, p: &str) -> i64 {
    match s.find(p) {
        Some(byte) => s[..byte].chars().count() as i64,
        None => -1,
    }
}

/// s.count(p) — non-overlapping occurrences, Python's rule for the
/// empty pattern included.
pub fn py_str_count(s: &str, p: &str) -> i64 {
    if p.is_empty() {
        return s.chars().count() as i64 + 1;
    }
    s.matches(p).count() as i64
}

/// int(s) — Python's parse: surrounding whitespace and an optional
/// sign; anything else raises, and traps here.
pub fn py_int_of_str(s: &str) -> i64 {
    match s.trim().parse::<i64>() {
        Ok(v) => v,
        Err(_) => panic!("invalid literal for int(): {s:?}"),
    }
}

/// float(s) — the same shape, for floats.
pub fn py_float_of_str(s: &str) -> f64 {
    match s.trim().parse::<f64>() {
        Ok(v) => v,
        Err(_) => panic!("could not convert string to float: {s:?}"),
    }
}

/// float(i) — the widening Python does silently.
pub fn py_float_of_int(v: i64) -> f64 {
    v as f64
}

/// int(f) — Python truncates toward zero, and refuses nan/inf.
pub fn py_int_of_float(v: f64) -> i64 {
    float_to_int("int", v.trunc())
}

/// round(f) — Python rounds half to EVEN, which is not what Rust's
/// `f64::round` does.
pub fn py_round(v: f64) -> i64 {
    if v.is_nan() {
        panic!("cannot convert float NaN to integer");
    }
    if v.is_infinite() {
        panic!("cannot convert float infinity to integer");
    }
    let f = v.floor();
    let diff = v - f;
    let out = if diff > 0.5 {
        f + 1.0
    } else if diff < 0.5 {
        f
    } else if (f as i64) % 2 == 0 {
        f
    } else {
        f + 1.0
    };
    out as i64
}


// ---- Python's format mini-language ---------------------------------
// `f"{x:>10,.2f}"` in the development run is CPython's own formatter;
// the compiled run calls these, so the same spec has to produce the
// same text. The subset is the one the tour documents: fill and
// align, a sign, zero padding, a width, `,` grouping, a precision,
// and the types d / f / e / % / s.

struct Spec {
    fill: char,
    align: Option<char>,
    sign: char,
    zero: bool,
    width: usize,
    comma: bool,
    precision: Option<usize>,
    ty: Option<char>,
}

fn parse_spec(spec: &str) -> Spec {
    let cs: Vec<char> = spec.chars().collect();
    let mut i = 0;
    let mut out = Spec {
        fill: ' ',
        align: None,
        sign: '-',
        zero: false,
        width: 0,
        comma: false,
        precision: None,
        ty: None,
    };
    // [[fill]align]
    if cs.len() >= 2 && matches!(cs[1], '<' | '>' | '^' | '=') {
        out.fill = cs[0];
        out.align = Some(cs[1]);
        i = 2;
    } else if !cs.is_empty() && matches!(cs[0], '<' | '>' | '^' | '=') {
        out.align = Some(cs[0]);
        i = 1;
    }
    if i < cs.len() && matches!(cs[i], '+' | '-' | ' ') {
        out.sign = cs[i];
        i += 1;
    }
    if i < cs.len() && cs[i] == '0' {
        out.zero = true;
        out.fill = '0';
        if out.align.is_none() {
            out.align = Some('=');
        }
        i += 1;
    }
    let start = i;
    while i < cs.len() && cs[i].is_ascii_digit() {
        i += 1;
    }
    if i > start {
        out.width = cs[start..i].iter().collect::<String>().parse().unwrap_or(0);
    }
    if i < cs.len() && cs[i] == ',' {
        out.comma = true;
        i += 1;
    }
    if i < cs.len() && cs[i] == '.' {
        i += 1;
        let ps = i;
        while i < cs.len() && cs[i].is_ascii_digit() {
            i += 1;
        }
        out.precision = Some(cs[ps..i].iter().collect::<String>().parse().unwrap_or(0));
    }
    if i < cs.len() {
        out.ty = Some(cs[i]);
    }
    out
}

fn group3(digits: &str) -> String {
    let mut out = String::new();
    for (n, c) in digits.chars().rev().enumerate() {
        if n > 0 && n % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out.chars().rev().collect()
}

fn sign_of(neg: bool, sign: char) -> String {
    if neg {
        "-".into()
    } else if sign == '+' {
        "+".into()
    } else if sign == ' ' {
        " ".into()
    } else {
        String::new()
    }
}

fn pad(body: String, sign: String, sp: &Spec, numeric: bool) -> String {
    let text = format!("{sign}{body}");
    let len = text.chars().count();
    if len >= sp.width {
        return text;
    }
    let fill = sp.fill;
    let n = sp.width - len;
    match sp.align.unwrap_or(if numeric { '>' } else { '<' }) {
        '<' => format!("{text}{}", fill.to_string().repeat(n)),
        '^' => {
            let left = n / 2;
            format!(
                "{}{text}{}",
                fill.to_string().repeat(left),
                fill.to_string().repeat(n - left)
            )
        }
        '=' => format!("{sign}{}{body}", fill.to_string().repeat(n)),
        _ => format!("{}{text}", fill.to_string().repeat(n)),
    }
}

/// CPython's exponent form: at least two digits, always signed.
fn exp_form(v: f64, precision: usize) -> (String, bool) {
    let neg = v.is_sign_negative();
    let a = v.abs();
    let e = format!("{a:.*e}", precision);
    let (mant, exp) = e.split_once('e').expect("Rust {:e} has an exponent");
    let exp: i32 = exp.parse().unwrap_or(0);
    (
        format!(
            "{mant}e{}{:02}",
            if exp < 0 { '-' } else { '+' },
            exp.abs()
        ),
        neg,
    )
}

/// format(int, spec)
pub fn py_format_int(v: i64, spec: &str) -> String {
    let sp = parse_spec(spec);
    match sp.ty {
        Some('f') | Some('e') | Some('%') => return py_format_float(v as f64, spec),
        _ => {}
    }
    let neg = v < 0;
    let digits = v.unsigned_abs().to_string();
    let body = if sp.comma { group3(&digits) } else { digits };
    pad(body, sign_of(neg, sp.sign), &sp, true)
}

/// format(float, spec)
pub fn py_format_float(v: f64, spec: &str) -> String {
    let sp = parse_spec(spec);
    let prec = sp.precision.unwrap_or(6);
    let (body, neg) = match sp.ty {
        Some('e') => exp_form(v, prec),
        Some('%') => {
            let x = v * 100.0;
            (format!("{:.*}%", prec, x.abs()), x.is_sign_negative())
        }
        Some('f') => (format!("{:.*}", prec, v.abs()), v.is_sign_negative()),
        _ => {
            // No type: `str(v)`'s text, which is what a bare hole
            // renders — a width or a fill may still apply.
            let t = py_float_repr(v);
            match t.strip_prefix('-') {
                Some(rest) => (rest.to_string(), true),
                None => (t, false),
            }
        }
    };
    let body = if sp.comma {
        match body.split_once('.') {
            Some((int, rest)) => format!("{}.{rest}", group3(int)),
            None => group3(&body),
        }
    } else {
        body
    };
    pad(body, sign_of(neg, sp.sign), &sp, true)
}

/// format(str, spec) — width, fill and alignment, and a precision
/// that truncates.
pub fn py_format_str(s: &str, spec: &str) -> String {
    let sp = parse_spec(spec);
    let body = match sp.precision {
        Some(p) => s.chars().take(p).collect::<String>(),
        None => s.to_string(),
    };
    pad(body, String::new(), &sp, false)
}


// ---- Python's list, the operations the dialect leans on -----------
// The same arrangement the str twins use: written against CPython's
// semantics (empty `min` raises, a slice clamps, `sorted` is stable),
// and the gate holds the two runs together.

pub fn py_list_contains_str(xs: Vec<String>, v: &str) -> bool {
    xs.iter().any(|x| x == v)
}

/// xs[a:b] — Python's clamping, on a list.
pub fn py_list_slice_str(xs: Vec<String>, a: i64, b: i64) -> Vec<String> {
    let n = xs.len() as i64;
    let clamp = |v: i64| -> usize {
        let v = if v < 0 { v + n } else { v };
        v.clamp(0, n) as usize
    };
    let (lo, hi) = (clamp(a), clamp(b));
    if lo >= hi {
        return Vec::new();
    }
    xs[lo..hi].to_vec()
}

pub fn py_list_concat_str(a: Vec<String>, b: Vec<String>) -> Vec<String> {
    let mut out = a;
    out.extend(b);
    out
}

pub fn py_list_reversed_str(xs: Vec<String>) -> Vec<String> {
    let mut out = xs;
    out.reverse();
    out
}

pub fn py_list_contains_int(xs: Vec<i64>, v: i64) -> bool {
    xs.iter().any(|x| *x == v)
}

/// xs[a:b] — Python's clamping, on a list.
pub fn py_list_slice_int(xs: Vec<i64>, a: i64, b: i64) -> Vec<i64> {
    let n = xs.len() as i64;
    let clamp = |v: i64| -> usize {
        let v = if v < 0 { v + n } else { v };
        v.clamp(0, n) as usize
    };
    let (lo, hi) = (clamp(a), clamp(b));
    if lo >= hi {
        return Vec::new();
    }
    xs[lo..hi].to_vec()
}

pub fn py_list_concat_int(a: Vec<i64>, b: Vec<i64>) -> Vec<i64> {
    let mut out = a;
    out.extend(b);
    out
}

pub fn py_list_reversed_int(xs: Vec<i64>) -> Vec<i64> {
    let mut out = xs;
    out.reverse();
    out
}

pub fn py_list_contains_float(xs: Vec<f64>, v: f64) -> bool {
    xs.iter().any(|x| *x == v)
}

/// xs[a:b] — Python's clamping, on a list.
pub fn py_list_slice_float(xs: Vec<f64>, a: i64, b: i64) -> Vec<f64> {
    let n = xs.len() as i64;
    let clamp = |v: i64| -> usize {
        let v = if v < 0 { v + n } else { v };
        v.clamp(0, n) as usize
    };
    let (lo, hi) = (clamp(a), clamp(b));
    if lo >= hi {
        return Vec::new();
    }
    xs[lo..hi].to_vec()
}

pub fn py_list_concat_float(a: Vec<f64>, b: Vec<f64>) -> Vec<f64> {
    let mut out = a;
    out.extend(b);
    out
}

pub fn py_list_reversed_float(xs: Vec<f64>) -> Vec<f64> {
    let mut out = xs;
    out.reverse();
    out
}

pub fn py_list_contains_bool(xs: Vec<bool>, v: bool) -> bool {
    xs.iter().any(|x| *x == v)
}

/// xs[a:b] — Python's clamping, on a list.
pub fn py_list_slice_bool(xs: Vec<bool>, a: i64, b: i64) -> Vec<bool> {
    let n = xs.len() as i64;
    let clamp = |v: i64| -> usize {
        let v = if v < 0 { v + n } else { v };
        v.clamp(0, n) as usize
    };
    let (lo, hi) = (clamp(a), clamp(b));
    if lo >= hi {
        return Vec::new();
    }
    xs[lo..hi].to_vec()
}

pub fn py_list_concat_bool(a: Vec<bool>, b: Vec<bool>) -> Vec<bool> {
    let mut out = a;
    out.extend(b);
    out
}

pub fn py_list_reversed_bool(xs: Vec<bool>) -> Vec<bool> {
    let mut out = xs;
    out.reverse();
    out
}

pub fn py_list_sorted_str(xs: Vec<String>) -> Vec<String> {
    let mut out = xs;
    out.sort();
    out
}

pub fn py_list_sorted_int(xs: Vec<i64>) -> Vec<i64> {
    let mut out = xs;
    out.sort();
    out
}

pub fn py_list_sorted_float(xs: Vec<f64>) -> Vec<f64> {
    let mut out = xs;
    out.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    out
}

/// min(xs) / max(xs) — an empty list raises in Python, and traps here.
pub fn py_list_min_int(xs: Vec<i64>) -> i64 {
    if xs.is_empty() {
        panic!("min() arg is an empty sequence");
    }
    let mut m = xs[0];
    for v in xs.iter().skip(1) {
        if *v < m {
            m = *v;
        }
    }
    m
}

pub fn py_list_max_int(xs: Vec<i64>) -> i64 {
    if xs.is_empty() {
        panic!("max() arg is an empty sequence");
    }
    let mut m = xs[0];
    for v in xs.iter().skip(1) {
        if *v > m {
            m = *v;
        }
    }
    m
}

pub fn py_list_sum_int(xs: Vec<i64>) -> i64 {
    let mut t: i64 = 0;
    for v in xs {
        t += v;
    }
    t
}

pub fn py_min2_int(a: i64, b: i64) -> i64 {
    if b < a { b } else { a }
}

pub fn py_max2_int(a: i64, b: i64) -> i64 {
    if b > a { b } else { a }
}

/// min(xs) / max(xs) — an empty list raises in Python, and traps here.
pub fn py_list_min_float(xs: Vec<f64>) -> f64 {
    if xs.is_empty() {
        panic!("min() arg is an empty sequence");
    }
    let mut m = xs[0];
    for v in xs.iter().skip(1) {
        if *v < m {
            m = *v;
        }
    }
    m
}

pub fn py_list_max_float(xs: Vec<f64>) -> f64 {
    if xs.is_empty() {
        panic!("max() arg is an empty sequence");
    }
    let mut m = xs[0];
    for v in xs.iter().skip(1) {
        if *v > m {
            m = *v;
        }
    }
    m
}

pub fn py_list_sum_float(xs: Vec<f64>) -> f64 {
    let mut t: f64 = 0.0;
    for v in xs {
        t += v;
    }
    t
}

pub fn py_min2_float(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

pub fn py_max2_float(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

pub fn py_abs_int(v: i64) -> i64 {
    if v == i64::MIN {
        panic!("abs() of the smallest integer overflows");
    }
    v.abs()
}

pub fn py_abs_float(v: f64) -> f64 {
    v.abs()
}


/// `log(msg)` — a line on stderr, from either run. stdout is where
/// the headless dump lives, so a message that is not part of the
/// screen does not go there.
pub fn log_line(msg: &str) -> i64 {
    eprintln!("{msg}");
    0
}

/// `assert` and `raise`, once they have nothing left to say: end the
/// statement the way a raised exception ends it. The runtime contains
/// the abort, so the app keeps running.
pub fn py_abort(msg: &str) -> i64 {
    panic!("{msg}");
}


// ---- sqlite: bound parameters and whole rows ------------------------
// A value bound with `?` is never parsed as SQL, which is the point:
// the text a user types cannot become a statement. Values bind as
// TEXT and SQLite applies the column's affinity, so an INTEGER column
// stores the number — the same thing Python's sqlite3 does with a str
// parameter.

fn open_db(path: &str, who: &str) -> rusqlite::Connection {
    match rusqlite::Connection::open(path) {
        Ok(c) => c,
        Err(e) => panic!("sqlite.{who} open {path}: {e}"),
    }
}

fn cell_text(v: rusqlite::types::Value) -> String {
    match v {
        rusqlite::types::Value::Integer(i) => i.to_string(),
        rusqlite::types::Value::Real(f) => f.to_string(),
        rusqlite::types::Value::Text(s) => s,
        rusqlite::types::Value::Null => String::new(),
        rusqlite::types::Value::Blob(_) => "<blob>".to_string(),
    }
}

pub fn sqlite_exec_with(path: &str, sql: &str, params: Vec<String>) -> i64 {
    let conn = open_db(path, "exec");
    let bound: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p as &dyn rusqlite::ToSql).collect();
    match conn.execute(sql, bound.as_slice()) {
        Ok(n) => n as i64,
        Err(e) => panic!("sqlite.exec {path}: {e}"),
    }
}

pub fn sqlite_query_text_with(path: &str, sql: &str, params: Vec<String>) -> Vec<String> {
    let conn = open_db(path, "query_text");
    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(e) => panic!("sqlite.query_text {path}: {e}"),
    };
    let bound: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p as &dyn rusqlite::ToSql).collect();
    let rows = stmt.query_map(bound.as_slice(), |row| {
        Ok(cell_text(row.get::<_, rusqlite::types::Value>(0)?))
    });
    match rows {
        Ok(it) => it
            .map(|r| match r {
                Ok(s) => s,
                Err(e) => panic!("sqlite.query_text row: {e}"),
            })
            .collect(),
        Err(e) => panic!("sqlite.query_text {path}: {e}"),
    }
}

pub fn sqlite_query_int_with(path: &str, sql: &str, params: Vec<String>) -> i64 {
    let conn = open_db(path, "query_int");
    let bound: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p as &dyn rusqlite::ToSql).collect();
    match conn.query_row(sql, bound.as_slice(), |row| row.get::<_, i64>(0)) {
        Ok(v) => v,
        Err(e) => panic!("sqlite.query_int {path}: {e}"),
    }
}

/// The fallible twin of the bound read, for a `try` that catches it.
pub fn sqlite_query_int_with_result(
    path: &str,
    sql: &str,
    params: Vec<String>,
) -> std::io::Result<i64> {
    let conn = rusqlite::Connection::open(path)
        .map_err(|e| std::io::Error::other(format!("sqlite.query_int open {path}: {e}")))?;
    let bound: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p as &dyn rusqlite::ToSql).collect();
    conn.query_row(sql, bound.as_slice(), |row| row.get::<_, i64>(0))
        .map_err(|e| std::io::Error::other(format!("sqlite.query_int {path}: {e}")))
}

/// The total read: a missing table or a bad statement answers no
/// rows, the way the rest of the `_or` family answers a default.
pub fn sqlite_query_rows_or(path: &str, sql: &str, params: Vec<String>) -> Vec<Vec<String>> {
    let Ok(conn) = rusqlite::Connection::open(path) else {
        return Vec::new();
    };
    let Ok(mut stmt) = conn.prepare(sql) else {
        return Vec::new();
    };
    let n = stmt.column_count();
    let bound: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p as &dyn rusqlite::ToSql).collect();
    let Ok(rows) = stmt.query_map(bound.as_slice(), |row| {
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            out.push(cell_text(row.get::<_, rusqlite::types::Value>(i)?));
        }
        Ok(out)
    }) else {
        return Vec::new();
    };
    rows.filter_map(|r| r.ok()).collect()
}

pub fn sqlite_query_rows_or_all(path: &str, sql: &str) -> Vec<Vec<String>> {
    sqlite_query_rows_or(path, sql, Vec::new())
}

/// Whole rows, unbound — the two-argument spelling.
pub fn sqlite_query_rows_all(path: &str, sql: &str) -> Vec<Vec<String>> {
    sqlite_query_rows(path, sql, Vec::new())
}

pub fn sqlite_query_int_or_with(path: &str, sql: &str, default: i64, params: Vec<String>) -> i64 {
    let Ok(conn) = rusqlite::Connection::open(path) else {
        return default;
    };
    let bound: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p as &dyn rusqlite::ToSql).collect();
    conn.query_row(sql, bound.as_slice(), |row| row.get::<_, i64>(0))
        .unwrap_or(default)
}

pub fn sqlite_query_text_or_with(path: &str, sql: &str, params: Vec<String>) -> Vec<String> {
    let Ok(conn) = rusqlite::Connection::open(path) else {
        return Vec::new();
    };
    let Ok(mut stmt) = conn.prepare(sql) else {
        return Vec::new();
    };
    let bound: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p as &dyn rusqlite::ToSql).collect();
    let Ok(rows) = stmt.query_map(bound.as_slice(), |row| {
        Ok(cell_text(row.get::<_, rusqlite::types::Value>(0)?))
    }) else {
        return Vec::new();
    };
    rows.filter_map(|r| r.ok()).collect()
}

/// Every column of every row, as text — the multi-column read. A row
/// is a `list[str]`, so a result is a `list[list[str]]`.
pub fn sqlite_query_rows(path: &str, sql: &str, params: Vec<String>) -> Vec<Vec<String>> {
    let conn = open_db(path, "query_rows");
    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(e) => panic!("sqlite.query_rows {path}: {e}"),
    };
    let n = stmt.column_count();
    let bound: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p as &dyn rusqlite::ToSql).collect();
    let rows = stmt.query_map(bound.as_slice(), |row| {
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            out.push(cell_text(row.get::<_, rusqlite::types::Value>(i)?));
        }
        Ok(out)
    });
    match rows {
        Ok(it) => it
            .map(|r| match r {
                Ok(v) => v,
                Err(e) => panic!("sqlite.query_rows row: {e}"),
            })
            .collect(),
        Err(e) => panic!("sqlite.query_rows {path}: {e}"),
    }
}

// ---- http: POST, headers, timeouts, status --------------------------

fn agent_with(timeout_ms: i64) -> ureq::Agent {
    let mut b = ureq::AgentBuilder::new();
    if timeout_ms > 0 {
        b = b.timeout(std::time::Duration::from_millis(timeout_ms as u64));
    }
    b.build()
}

/// GET with a deadline. `0` keeps the client's own default.
pub fn http_get_text_timeout_result(url: &str, timeout_ms: i64) -> std::io::Result<String> {
    match agent_with(timeout_ms).get(url).call() {
        Ok(resp) => resp
            .into_string()
            .map_err(|e| std::io::Error::other(format!("http.get_text {url}: {e}"))),
        Err(e) => Err(std::io::Error::other(format!("http.get_text {url}: {e}"))),
    }
}

pub fn http_get_text_timeout(url: &str, timeout_ms: i64) -> String {
    match http_get_text_timeout_result(url, timeout_ms) {
        Ok(s) => s,
        Err(e) => panic!("{e}"),
    }
}

/// GET with headers. The map is sorted before it is applied, so the
/// request a script replays is the request the first run made.
pub fn http_get_text_with(url: &str, headers: std::collections::HashMap<String, String>) -> String {
    let mut req = ureq::get(url);
    let mut keys: Vec<&String> = headers.keys().collect();
    keys.sort();
    for k in keys {
        req = req.set(k, &headers[k]);
    }
    match req.call() {
        Ok(resp) => match resp.into_string() {
            Ok(s) => s,
            Err(e) => panic!("http.get_text_with {url}: {e}"),
        },
        Err(e) => panic!("http.get_text_with {url}: {e}"),
    }
}

pub fn http_post_text_as_result(
    url: &str,
    body: &str,
    content_type: &str,
) -> std::io::Result<String> {
    let ct = if content_type.is_empty() { "text/plain" } else { content_type };
    match ureq::post(url).set("Content-Type", ct).send_string(body) {
        Ok(resp) => resp
            .into_string()
            .map_err(|e| std::io::Error::other(format!("http.post_text {url}: {e}"))),
        Err(e) => Err(std::io::Error::other(format!("http.post_text {url}: {e}"))),
    }
}

/// POST a body as text/plain and read the answer.
pub fn http_post_text(url: &str, body: &str) -> String {
    http_post_text_as(url, body, "text/plain")
}

/// POST under a content type of the caller's choosing.
pub fn http_post_text_as(url: &str, body: &str, content_type: &str) -> String {
    match http_post_text_as_result(url, body, content_type) {
        Ok(s) => s,
        Err(e) => panic!("{e}"),
    }
}

pub fn http_post_text_result(url: &str, body: &str) -> std::io::Result<String> {
    http_post_text_as_result(url, body, "text/plain")
}

pub fn http_post_text_or(url: &str, body: &str, default: &str) -> String {
    http_post_text_as_result(url, body, "text/plain").unwrap_or_else(|_| default.to_string())
}

/// The status code, or 0 when the request never reached a server.
/// A 404 is an answer, not a failure, so it comes back as 404.
pub fn http_status(url: &str) -> i64 {
    match ureq::get(url).call() {
        Ok(resp) => resp.status() as i64,
        Err(ureq::Error::Status(code, _)) => code as i64,
        Err(_) => 0,
    }
}

// ---- fs: listing, appending, removing, the app's own directory ------

/// The names in a directory, sorted — a directory has no order of its
/// own, and a screen built from one has to be reproducible.
pub fn fs_list_dir(path: &str) -> Vec<String> {
    let rd = match std::fs::read_dir(path) {
        Ok(rd) => rd,
        Err(e) => panic!("fs.list_dir {path}: {e}"),
    };
    let mut out: Vec<String> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    out.sort();
    out
}

pub fn fs_append_text(path: &str, text: &str) -> i64 {
    use std::io::Write;
    if let Some(dir) = std::path::Path::new(path).parent() {
        if !dir.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(dir);
        }
    }
    let mut f = match std::fs::OpenOptions::new().create(true).append(true).open(path) {
        Ok(f) => f,
        Err(e) => panic!("fs.append_text {path}: {e}"),
    };
    match f.write_all(text.as_bytes()) {
        Ok(()) => text.len() as i64,
        Err(e) => panic!("fs.append_text {path}: {e}"),
    }
}

/// Remove a file. Missing is a failure, as it is in Python.
pub fn fs_remove(path: &str) -> i64 {
    match std::fs::remove_file(path) {
        Ok(()) => 0,
        Err(e) => panic!("fs.remove {path}: {e}"),
    }
}

pub fn fs_make_dir(path: &str) -> i64 {
    match std::fs::create_dir_all(path) {
        Ok(()) => 0,
        Err(e) => panic!("fs.make_dir {path}: {e}"),
    }
}

/// The directory an app may keep its own files in, created on the way
/// out: `~/Library/Application Support/<name>` on macOS.
pub fn fs_app_dir(name: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = std::path::Path::new(&home)
        .join("Library")
        .join("Application Support")
        .join(name);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        panic!("fs.app_dir {name}: {e}");
    }
    dir.to_string_lossy().into_owned()
}

// ---- json: writing ---------------------------------------------------
// Maps are written in key order. A Rust HashMap has no order and a
// Python dict has insertion order; writing by key is the one both can
// agree on, and it is the rule dict iteration already follows here.

// ---- json.dumps: the writer CPython is -------------------------------
// Python's `json.dumps`, as a twin. serde_json is close and not the
// same: it separates with `,` and `:` where CPython uses `", "` and
// `": "`, it leaves non-ASCII alone where CPython escapes it, it
// refuses NaN where CPython writes one, and it formats floats its own
// way. So the writer is written out.
//
// The pieces compose: a value is rendered to its text, and a
// container joins texts. That is what lets a document nest without a
// writer per shape — the twelve shape-specific ones this replaces
// could not nest at all.

/// A JSON string literal, with `ensure_ascii` — CPython's default.
/// Everything outside printable ASCII becomes `\uXXXX`, and a
/// character past the basic plane becomes the surrogate pair its
/// UTF-16 encoding is.
pub fn json_text(v: &str) -> String {
    let mut out = String::with_capacity(v.len() + 2);
    out.push('"');
    for c in v.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            ' '..='~' => out.push(c),
            _ => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{unit:04x}"));
                }
            }
        }
    }
    out.push('"');
    out
}

pub fn json_int(v: i64) -> String {
    v.to_string()
}

/// CPython writes a float as `repr` does, and writes the three values
/// JSON has no syntax for as `NaN`, `Infinity` and `-Infinity`.
pub fn json_float(v: f64) -> String {
    if v.is_nan() {
        return "NaN".to_string();
    }
    if v.is_infinite() {
        return if v > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    py_float_repr(v)
}

pub fn json_bool(v: bool) -> String {
    if v { "true" } else { "false" }.to_string()
}

pub fn json_null() -> String {
    "null".to_string()
}

/// Already-rendered elements as an array.
pub fn json_array(parts: Vec<String>) -> String {
    format!("[{}]", parts.join(", "))
}

/// Keys and already-rendered values as an object, in the order given
/// — which is the order the keys went into the dict, since that is
/// what a map answers here.
pub fn json_object(keys: Vec<String>, parts: Vec<String>) -> String {
    assert!(
        keys.len() == parts.len(),
        "json: {} keys against {} values",
        keys.len(),
        parts.len()
    );
    let body: Vec<String> = keys
        .iter()
        .zip(parts.iter())
        .map(|(k, v)| format!("{}: {v}", json_text(k)))
        .collect();
    format!("{{{}}}", body.join(", "))
}

/// Each element of a list of scalars, rendered. Four functions rather
/// than one because the crossing is typed; what they answer is the
/// same `List<String>` the containers above take, so they compose.
pub fn json_texts(xs: Vec<String>) -> Vec<String> {
    xs.iter().map(|x| json_text(x)).collect()
}
pub fn json_ints(xs: Vec<i64>) -> Vec<String> {
    xs.into_iter().map(json_int).collect()
}
pub fn json_floats(xs: Vec<f64>) -> Vec<String> {
    xs.into_iter().map(json_float).collect()
}
pub fn json_bools(xs: Vec<bool>) -> Vec<String> {
    xs.into_iter().map(json_bool).collect()
}

// ---- time: the machine's own zone -----------------------------------

/// strftime in the machine's timezone. One implementation means both
/// runs read the same zone database and print the same string; a
/// verification script that wants a fixed answer uses `format_ms`,
/// which is UTC.
pub fn clock_format_local_ms(ms: i64, fmt: &str) -> String {
    match chrono::DateTime::from_timestamp_millis(ms) {
        Some(dt) => dt.with_timezone(&chrono::Local).format(fmt).to_string(),
        None => panic!("clock: `{ms}` is out of range"),
    }
}

/// The machine's offset from UTC, in minutes, at that instant.
pub fn clock_local_offset_minutes(ms: i64) -> i64 {
    use chrono::Offset;
    match chrono::DateTime::from_timestamp_millis(ms) {
        Some(dt) => (dt.with_timezone(&chrono::Local).offset().fix().local_minus_utc() / 60) as i64,
        None => panic!("clock: `{ms}` is out of range"),
    }
}


// ---- clipboard ------------------------------------------------------
// One value, both runs: a window exchanges it with the platform every
// frame, a headless run keeps it to itself — so copying and pasting is
// something a script can check.

pub fn clipboard_set_text(text: &str) -> i64 {
    pixie_kernel::clipboard::set(text);
    text.len() as i64
}

pub fn clipboard_get_text() -> String {
    pixie_kernel::clipboard::get().as_str().to_string()
}


// ---- file dialogs ---------------------------------------------------
// A dialog waits for a person, so it belongs inside a task: the call
// blocks while the window keeps drawing. A headless run answers from
// the queue a script filled with `file:<path>` steps, so a flow that
// opens a file is replayed like any other.

pub fn fs_open_dialog(title: &str) -> String {
    pixie_kernel::dialog::ask(pixie_kernel::dialog::Kind::Open, title)
}

pub fn fs_save_dialog(name: &str) -> String {
    pixie_kernel::dialog::ask(pixie_kernel::dialog::Kind::Save, name)
}
