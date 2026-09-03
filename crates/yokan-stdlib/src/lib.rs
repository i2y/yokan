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
pub fn time_sleep_ms(ms: i64) -> i64 {
    if ms > 0 {
        std::thread::sleep(std::time::Duration::from_millis(ms as u64));
    }
    0
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
    if v.is_nan() || v.is_infinite() {
        panic!("cannot convert float NaN or infinity to integer");
    }
    v.trunc() as i64
}

/// round(f) — Python rounds half to EVEN, which is not what Rust's
/// `f64::round` does.
pub fn py_round(v: f64) -> i64 {
    if v.is_nan() || v.is_infinite() {
        panic!("cannot convert float NaN or infinity to integer");
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

pub fn json_dumps_str(v: &str) -> String {
    serde_json::Value::String(v.to_string()).to_string()
}

pub fn json_dumps_int(v: i64) -> String {
    v.to_string()
}

pub fn json_dumps_float(v: f64) -> String {
    serde_json::Value::from(v).to_string()
}

pub fn json_dumps_bool(v: bool) -> String {
    if v { "true" } else { "false" }.to_string()
}

pub fn json_dumps_list_str(xs: Vec<String>) -> String {
    serde_json::Value::Array(xs.into_iter().map(serde_json::Value::String).collect()).to_string()
}

pub fn json_dumps_list_int(xs: Vec<i64>) -> String {
    serde_json::Value::Array(xs.into_iter().map(serde_json::Value::from).collect()).to_string()
}

pub fn json_dumps_list_float(xs: Vec<f64>) -> String {
    serde_json::Value::Array(xs.into_iter().map(serde_json::Value::from).collect()).to_string()
}

pub fn json_dumps_list_bool(xs: Vec<bool>) -> String {
    serde_json::Value::Array(xs.into_iter().map(serde_json::Value::Bool).collect()).to_string()
}

fn dumps_map(mut pairs: Vec<(String, serde_json::Value)>) -> String {
    // Sorted here rather than left to the map: serde_json's `Map` is a
    // BTreeMap only while nothing in the crate graph asks it to
    // preserve insertion order, and key order is the answer either way.
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    serde_json::Value::Object(pairs.into_iter().collect()).to_string()
}

pub fn json_dumps_map_str(m: std::collections::HashMap<String, String>) -> String {
    dumps_map(m.into_iter().map(|(k, v)| (k, serde_json::Value::String(v))).collect())
}

pub fn json_dumps_map_int(m: std::collections::HashMap<String, i64>) -> String {
    dumps_map(m.into_iter().map(|(k, v)| (k, serde_json::Value::from(v))).collect())
}

pub fn json_dumps_map_float(m: std::collections::HashMap<String, f64>) -> String {
    dumps_map(m.into_iter().map(|(k, v)| (k, serde_json::Value::from(v))).collect())
}

pub fn json_dumps_map_bool(m: std::collections::HashMap<String, bool>) -> String {
    dumps_map(m.into_iter().map(|(k, v)| (k, serde_json::Value::Bool(v))).collect())
}

// ---- time: the machine's own zone -----------------------------------

/// strftime in the machine's timezone. One implementation means both
/// runs read the same zone database and print the same string; a
/// verification script that wants a fixed answer uses `format_ms`,
/// which is UTC.
pub fn time_format_local_ms(ms: i64, fmt: &str) -> String {
    match chrono::DateTime::from_timestamp_millis(ms) {
        Some(dt) => dt.with_timezone(&chrono::Local).format(fmt).to_string(),
        None => panic!("time: `{ms}` is out of range"),
    }
}

/// The machine's offset from UTC, in minutes, at that instant.
pub fn time_local_offset_minutes(ms: i64) -> i64 {
    use chrono::Offset;
    match chrono::DateTime::from_timestamp_millis(ms) {
        Some(dt) => (dt.with_timezone(&chrono::Local).offset().fix().local_minus_utc() / 60) as i64,
        None => panic!("time: `{ms}` is out of range"),
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
