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
// Not "Python's math reimplemented": yokan's math, ONE Rust
// implementation both tiers call — so fidelity-to-CPython never needs
// proving, only self-consistency, which the two doors give for free.

pub fn math_sqrt(v: f64) -> f64 { v.sqrt() }
pub fn math_sin(v: f64) -> f64 { v.sin() }
pub fn math_cos(v: f64) -> f64 { v.cos() }
pub fn math_pow(a: f64, b: f64) -> f64 { a.powf(b) }
pub fn math_fabs(v: f64) -> f64 { v.abs() }
pub fn math_floor(v: f64) -> i64 { v.floor() as i64 }
pub fn math_ceil(v: f64) -> i64 { v.ceil() as i64 }
pub fn math_pi() -> f64 { std::f64::consts::PI }

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

pub fn time_now_ms() -> i64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_millis() as i64,
        Err(e) => panic!("time: clock before epoch: {e}"),
    }
}

/// UTC strftime of a millisecond timestamp — deterministic for a
/// fixed input, which is what a gate script feeds it.
pub fn time_format_ms(ms: i64, fmt: &str) -> String {
    match chrono::DateTime::from_timestamp_millis(ms) {
        Some(dt) => dt.format(fmt).to_string(),
        None => panic!("time: `{ms}` is out of range"),
    }
}

// ---- strings --------------------------------------------------------

/// Total parse: trimmed integer or the default — the `.get(k, d)`
/// pattern applied to parsing, so both tiers agree on bad input
/// instead of raising vs trapping.
pub fn strings_to_int(s: &str, default: i64) -> i64 {
    s.trim().parse::<i64>().unwrap_or(default)
}

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
// A seeded SplitMix64: yokan's random, ONE deterministic sequence in
// both tiers (the doors and the binding hit the SAME process-local
// state, and a gate script seeds explicitly).

use std::sync::atomic::{AtomicU64, Ordering};

static RNG_STATE: AtomicU64 = AtomicU64::new(0x9E3779B97F4A7C15);

pub fn random_seed(n: i64) {
    RNG_STATE.store(n as u64 ^ 0x9E3779B97F4A7C15, Ordering::Relaxed);
}

fn next_u64() -> u64 {
    let mut z = RNG_STATE
        .fetch_add(0x9E3779B97F4A7C15, Ordering::Relaxed)
        .wrapping_add(0x9E3779B97F4A7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

/// Uniform in [lo, hi] (inclusive); panics if lo > hi.
pub fn random_int(lo: i64, hi: i64) -> i64 {
    if lo > hi {
        panic!("random.int: lo {lo} > hi {hi}");
    }
    let span = (hi - lo) as u64 + 1;
    lo + (next_u64() % span) as i64
}

/// Uniform in [0, 1).
pub fn random_float() -> f64 {
    (next_u64() >> 11) as f64 / (1u64 << 53) as f64
}

/// Binding-friendly seed (the .rpi layer wants a return value).
pub fn random_seed_ret(n: i64) -> i64 {
    random_seed(n);
    n
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

/// str(float) — CPython's shortest-round-trip rendering: fixed
/// notation while the decimal point sits in [-3, 16], scientific
/// with a two-digit-minimum signed exponent outside, integral
/// values keep a trailing `.0`.
pub fn py_float_repr(v: f64) -> String {
    if v.is_nan() {
        return "nan".to_string();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf" } else { "-inf" }.to_string();
    }
    let neg = v.is_sign_negative();
    let a = v.abs();
    let e = format!("{a:e}"); // shortest digits, e.g. "1.2345e-7"
    let (mant, exp) = e.split_once('e').expect("Rust {:e} always has an exponent");
    let exp: i32 = exp.parse().expect("integer exponent");
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    let decpt = exp + 1; // digits before the decimal point
    let n = digits.len() as i32;
    let body = if (-3..=16).contains(&decpt) {
        if decpt <= 0 {
            format!("0.{}{}", "0".repeat((-decpt) as usize), digits)
        } else if decpt >= n {
            format!("{}{}.0", digits, "0".repeat((decpt - n) as usize))
        } else {
            format!("{}.{}", &digits[..decpt as usize], &digits[decpt as usize..])
        }
    } else {
        let rest = &digits[1..];
        let m = if rest.is_empty() {
            digits[..1].to_string()
        } else {
            format!("{}.{}", &digits[..1], rest)
        };
        format!("{}e{}{:02}", m, if exp < 0 { '-' } else { '+' }, exp.abs())
    };
    if neg { format!("-{body}") } else { body }
}

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
