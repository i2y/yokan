//! The twins of Perl's own answers, for the run that has no perl in it.
//!
//! Rakugan's compiled run is a translation to `.pix`, so anything the
//! app writes that perl would answer for itself has to be answered the
//! same way here — where the name is Perl's, perl's output is the
//! specification. The translator writes a call to one of these
//! functions wherever the two would otherwise part: a number or a bool
//! put into a string, `%`, `/` between two whole numbers, `int`, and
//! `sprintf`.
//!
//! The interpreted run calls none of this. It is perl, and perl already
//! answers; this is the other half of the pair the gate compares.

/// `$a % $b`. Perl's remainder takes the sign of the right side, where
/// Rust's takes the left: `-7 % 3` is 2 in perl and -1 in Rust.
pub fn mod_int(a: i64, b: i64) -> i64 {
    if b == 0 {
        panic!("Illegal modulus zero");
    }
    let r = a.wrapping_rem(b);
    if r != 0 && (r < 0) != (b < 0) { r + b } else { r }
}

/// `$a / $b` between two whole numbers. Perl answers a number with a
/// fraction even when the division comes out even.
pub fn div_int(a: i64, b: i64) -> f64 {
    if b == 0 {
        panic!("Illegal division by zero");
    }
    a as f64 / b as f64
}

/// `$a / $b` with a fraction on either side. Perl dies on a zero
/// divisor whatever the numbers are; a raw division would answer Inf.
pub fn div_num(a: f64, b: f64) -> f64 {
    if b == 0.0 {
        panic!("Illegal division by zero");
    }
    a / b
}

/// What perl says when a message does not end its own line: the
/// statement's place is appended. `die "boom"` is `boom at FILE line
/// N.\n`, `die "boom\n"` is `boom\n`. The translator writes the place.
pub fn die_text(text: &str, at: &str) -> String {
    // A `die` with nothing to say says "Died".
    let text = if text.is_empty() { "Died" } else { text };
    if text.ends_with('\n') {
        text.to_string()
    } else {
        format!("{text}{at}")
    }
}

/// `warn`: the text to standard error, the way perl writes it. Answers
/// 0 so the statement has something to hold.
pub fn warn_at(text: &str, at: &str) -> i64 {
    eprint!("{}", die_text(text, at));
    0
}

/// `die`: the handler stops here. The engine contains the panic the
/// way perl's door contains the die, and each prints the message.
pub fn die_at(text: &str, at: &str) -> i64 {
    panic!("{}", die_text(text, at))
}

/// The failures above as `!T`, for a `try` to catch. Each answers the
/// text perl's `$e` holds, place included; the translator writes the
/// place, so the twin only has to know perl's words.
pub fn try_div_int(a: i64, b: i64, at: &str) -> Result<f64, String> {
    if b == 0 {
        return Err(format!("Illegal division by zero{at}"));
    }
    Ok(a as f64 / b as f64)
}

pub fn try_div_num(a: f64, b: f64, at: &str) -> Result<f64, String> {
    if b == 0.0 {
        return Err(format!("Illegal division by zero{at}"));
    }
    Ok(a / b)
}

pub fn try_mod_int(a: i64, b: i64, at: &str) -> Result<i64, String> {
    if b == 0 {
        return Err(format!("Illegal modulus zero{at}"));
    }
    Ok(mod_int(a, b))
}

pub fn try_sqrt(v: f64, at: &str) -> Result<f64, String> {
    if v < 0.0 {
        return Err(format!("Can't take sqrt of {}{at}", num_text(v)));
    }
    Ok(v.sqrt())
}

/// `0 + $s`. Perl reads as much of a number off the front of a string
/// as it can and answers 0 when there is none: `"3abc" + 1` is 4 and
/// `"abc" + 1` is 1. Leading space is skipped; what follows the number
/// is dropped.
pub fn num_of(s: &str) -> f64 {
    let b = s.trim_start();
    let mut end = 0;
    let mut seen_digit = false;
    let mut seen_dot = false;
    let mut seen_exp = false;
    for (i, c) in b.char_indices() {
        match c {
            '+' | '-' if i == 0 => {}
            '+' | '-' if seen_exp && matches!(b[..i].chars().last(), Some('e') | Some('E')) => {}
            '0'..='9' => seen_digit = true,
            '.' if !seen_dot && !seen_exp => seen_dot = true,
            'e' | 'E' if seen_digit && !seen_exp => seen_exp = true,
            _ => break,
        }
        end = i + c.len_utf8();
    }
    if !seen_digit {
        return 0.0;
    }
    // A trailing `e` or sign belongs to an exponent that never came.
    let mut t = &b[..end];
    while !t.is_empty() && !t.ends_with(|c: char| c.is_ascii_digit()) {
        t = &t[..t.len() - 1];
    }
    t.parse::<f64>().unwrap_or(0.0)
}

/// `int($x)`. Perl throws the fraction away, towards zero.
pub fn int_of(v: f64) -> i64 {
    v.trunc() as i64
}

/// A number in a string. Perl prints one with `%.15g`, which is why
/// `0.1 + 0.2` reads as `0.3` there and as `0.30000000000000004` in a
/// language that prints every digit it holds.
pub fn num_text(v: f64) -> String {
    g_format(v, 15)
}

/// A bool in a string. Perl's true prints as `1` and its false as
/// nothing at all.
pub fn bool_text(v: bool) -> String {
    if v { "1" } else { "" }.to_string()
}

/// `sprintf(FORMAT, $x)` with one number.
pub fn fmt_num(fmt: &str, v: f64) -> String {
    format_one(fmt, Arg::Num(v))
}

/// `sprintf(FORMAT, $n)` with one whole number.
pub fn fmt_int(fmt: &str, v: i64) -> String {
    format_one(fmt, Arg::Int(v))
}

/// `sprintf(FORMAT, $s)` with one string.
pub fn fmt_str(fmt: &str, v: &str) -> String {
    format_one(fmt, Arg::Str(v.to_string()))
}

// ---------------------------------------------------------------------------
// %g, the way C prints it, which is what perl prints a number with.

fn g_format(v: f64, sig: usize) -> String {
    if v.is_nan() {
        return "NaN".to_string();
    }
    if v.is_infinite() {
        return if v < 0.0 { "-Inf" } else { "Inf" }.to_string();
    }
    if v == 0.0 {
        return "0".to_string();
    }
    let sig = sig.max(1);
    // %g picks the exponent form when the exponent is below -4 or not
    // below the precision, and the decimal form otherwise; either way
    // it drops the zeros a fixed rendering would leave behind.
    let exp = {
        let e = format!("{:.*e}", sig - 1, v);
        e.split('e').nth(1).unwrap_or("0").parse::<i32>().unwrap_or(0)
    };
    if exp < -4 || exp >= sig as i32 {
        let s = format!("{:.*e}", sig - 1, v);
        let (mantissa, e) = s.split_once('e').unwrap_or((s.as_str(), "0"));
        let mantissa = trim_zeros(mantissa);
        let e: i32 = e.parse().unwrap_or(0);
        format!("{}e{}{:02}", mantissa, if e < 0 { '-' } else { '+' }, e.abs())
    } else {
        let decimals = (sig as i32 - 1 - exp).max(0) as usize;
        trim_zeros(&format!("{:.*}", decimals, v)).to_string()
    }
}

fn trim_zeros(s: &str) -> &str {
    if !s.contains('.') {
        return s;
    }
    let s = s.trim_end_matches('0');
    s.strip_suffix('.').unwrap_or(s)
}

enum Arg {
    Int(i64),
    Num(f64),
    Str(String),
}

/// The subset of perl's `sprintf` the dialect takes: literal text, `%%`
/// for a per cent sign, and ONE conversion with its flags, width and
/// precision.
fn format_one(fmt: &str, arg: Arg) -> String {
    let mut out = String::new();
    let mut chars = fmt.chars().peekable();
    let mut used = false;
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        if chars.peek() == Some(&'%') {
            chars.next();
            out.push('%');
            continue;
        }
        let mut flags = String::new();
        while matches!(chars.peek(), Some('-') | Some('+') | Some(' ') | Some('0') | Some('#')) {
            flags.push(chars.next().unwrap());
        }
        let mut width = String::new();
        while matches!(chars.peek(), Some(c) if c.is_ascii_digit()) {
            width.push(chars.next().unwrap());
        }
        let mut prec: Option<usize> = None;
        if chars.peek() == Some(&'.') {
            chars.next();
            let mut p = String::new();
            while matches!(chars.peek(), Some(c) if c.is_ascii_digit()) {
                p.push(chars.next().unwrap());
            }
            prec = Some(p.parse().unwrap_or(0));
        }
        let conv = chars.next().unwrap_or('s');
        if used {
            panic!("this format takes more than one value");
        }
        used = true;
        let body = match (&arg, conv) {
            (Arg::Num(v), 'f' | 'F') => format!("{:.*}", prec.unwrap_or(6), v),
            (Arg::Num(v), 'e' | 'E') => {
                let s = format!("{:.*e}", prec.unwrap_or(6), v);
                let s = exp_two_digits(&s);
                if conv == 'E' { s.to_uppercase() } else { s }
            }
            (Arg::Num(v), 'g' | 'G') => g_format(*v, prec.unwrap_or(6).max(1)),
            (Arg::Num(v), 'd' | 'i') => format!("{}", *v as i64),
            (Arg::Num(v), 's') => num_text(*v),
            (Arg::Int(v), 'd' | 'i') => format!("{}", v),
            (Arg::Int(v), 'f' | 'F') => format!("{:.*}", prec.unwrap_or(6), *v as f64),
            (Arg::Int(v), 'x') => format!("{:x}", v),
            (Arg::Int(v), 'X') => format!("{:X}", v),
            (Arg::Int(v), 'o') => format!("{:o}", v),
            (Arg::Int(v), 'b') => format!("{:b}", v),
            (Arg::Int(v), 's') => format!("{}", v),
            (Arg::Str(s), 's') => match prec {
                Some(p) => s.chars().take(p).collect(),
                None => s.clone(),
            },
            (_, c) => panic!("the conversion %{c} is not in the dialect"),
        };
        let sign = matches!(&arg, Arg::Int(v) if *v < 0) || matches!(&arg, Arg::Num(v) if *v < 0.0);
        let body = if flags.contains('+') && !sign && conv != 's' {
            format!("+{body}")
        } else {
            body
        };
        out.push_str(&pad(&body, &flags, &width));
    }
    out
}

fn pad(body: &str, flags: &str, width: &str) -> String {
    let want: usize = width.parse().unwrap_or(0);
    let have = body.chars().count();
    if have >= want {
        return body.to_string();
    }
    let fill = want - have;
    if flags.contains('-') {
        return format!("{body}{}", " ".repeat(fill));
    }
    if flags.contains('0') {
        // A sign stays in front of the zeros it is padded with.
        if let Some(rest) = body.strip_prefix('-') {
            return format!("-{}{rest}", "0".repeat(fill));
        }
        if let Some(rest) = body.strip_prefix('+') {
            return format!("+{}{rest}", "0".repeat(fill));
        }
        return format!("{}{body}", "0".repeat(fill));
    }
    format!("{}{body}", " ".repeat(fill))
}

/// C prints an exponent with at least two digits; Rust prints the
/// fewest it can.
fn exp_two_digits(s: &str) -> String {
    match s.split_once('e') {
        Some((m, e)) => {
            let (sign, digits) = match e.strip_prefix('-') {
                Some(d) => ('-', d),
                None => ('+', e.strip_prefix('+').unwrap_or(e)),
            };
            format!("{m}e{sign}{:0>2}", digits)
        }
        None => s.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Perl's own functions. Where the name is Perl's, perl's output is the
// specification: every one of these is held to a table `tools/gen_expected.pl`
// printed by running the same case through perl itself.

/// `length($s)`. Under `use utf8` a string is characters, not bytes,
/// and the dialect turns utf8 on for every app.
pub fn length_of(s: &str) -> i64 {
    s.chars().count() as i64
}

/// `substr($s, $off)`. A negative offset counts from the end; an
/// offset past the end answers nothing.
pub fn substr_from(s: &str, off: i64) -> String {
    let cs: Vec<char> = s.chars().collect();
    // Perl's own offset rules (`substr_span`), with the length running
    // to the end: an offset that counts past the start clamps to it,
    // and one past the end answers undef, written here as "".
    // "Through the end of the string": a length no string reaches.
    match substr_span(cs.len(), off, i64::MAX / 4) {
        Some((start, n)) => cs[start..start + n].iter().collect(),
        None => String::new(),
    }
}

/// `substr($s, $off, $len)`. A negative length leaves that many
/// characters off the end.
pub fn substr_len(s: &str, off: i64, len: i64) -> String {
    let cs: Vec<char> = s.chars().collect();
    match substr_span(cs.len(), off, len) {
        Some((start, n)) => cs[start..start + n].iter().collect(),
        None => String::new(),
    }
}

/// `index($s, $sub)` — where it starts, or -1.
pub fn index_of(s: &str, sub: &str) -> i64 {
    index_from(s, sub, 0)
}

/// `index($s, $sub, $pos)` — the same, looking from `pos` on.
pub fn index_from(s: &str, sub: &str, pos: i64) -> i64 {
    let cs: Vec<char> = s.chars().collect();
    let ss: Vec<char> = sub.chars().collect();
    let from = pos.max(0) as usize;
    if ss.len() > cs.len() {
        return -1;
    }
    for i in from..=(cs.len() - ss.len()) {
        if cs[i..i + ss.len()] == ss[..] {
            return i as i64;
        }
    }
    -1
}

/// `rindex($s, $sub)` — the last place it starts, or -1.
pub fn rindex_of(s: &str, sub: &str) -> i64 {
    let cs: Vec<char> = s.chars().collect();
    rindex_from(s, sub, cs.len() as i64)
}

/// `rindex($s, $sub, $pos)` — the last place at or before `pos`.
pub fn rindex_from(s: &str, sub: &str, pos: i64) -> i64 {
    let cs: Vec<char> = s.chars().collect();
    let ss: Vec<char> = sub.chars().collect();
    if ss.len() > cs.len() || pos < 0 {
        return -1;
    }
    let last = (pos as usize).min(cs.len() - ss.len());
    for i in (0..=last).rev() {
        if cs[i..i + ss.len()] == ss[..] {
            return i as i64;
        }
    }
    -1
}

/// `uc($s)`. Character by character, which is what perl does — the
/// whole-string mapping a language may offer instead knows about the
/// end of a word, and perl does not.
pub fn uc(s: &str) -> String {
    s.chars().flat_map(char::to_uppercase).collect()
}

/// `lc($s)`. `lc("ΣΑΣ")` is `σασ` in perl: no final-sigma rule.
pub fn lc(s: &str) -> String {
    s.chars().flat_map(char::to_lowercase).collect()
}

/// `ucfirst($s)`. The first character takes Unicode's TITLECASE
/// mapping, which is not the uppercase one: `ucfirst("ß")` is "Ss"
/// where `uc("ß")` is "SS".
pub fn ucfirst(s: &str) -> String {
    let mut cs = s.chars();
    match cs.next() {
        Some(c) => titlecase(c) + cs.as_str(),
        None => String::new(),
    }
}

fn titlecase(c: char) -> String {
    let mapped: String = unicode_case_mapping::to_titlecase(c)
        .into_iter()
        .take_while(|u| *u != 0)
        .filter_map(char::from_u32)
        .collect();
    // The table answers nothing for a character that titlecases to
    // itself.
    if mapped.is_empty() { c.to_string() } else { mapped }
}

/// `lcfirst($s)`.
pub fn lcfirst(s: &str) -> String {
    let mut cs = s.chars();
    match cs.next() {
        Some(c) => c.to_lowercase().chain(cs).collect(),
        None => String::new(),
    }
}

/// `reverse $s` where one string is wanted.
pub fn reverse_str(s: &str) -> String {
    s.chars().rev().collect()
}

/// `join($sep, @parts)`.
pub fn join(sep: &str, parts: Vec<String>) -> String {
    parts.join(sep)
}

/// `split($sep, $s)` where the separator is written out. perl drops
/// the empty fields at the end, and only those.
pub fn split_on(sep: &str, s: &str) -> Vec<String> {
    if sep.is_empty() {
        return s.chars().map(|c| c.to_string()).collect();
    }
    let mut out: Vec<String> = s.split(sep).map(str::to_string).collect();
    // A leading empty field stays; the trailing ones go.
    while out.last().is_some_and(|f| f.is_empty()) {
        out.pop();
    }
    out
}

/// `split ' ', $s` — the one separator perl gives a meaning of its
/// own: leading whitespace goes, and any run of it separates.
pub fn split_words(s: &str) -> Vec<String> {
    s.split_whitespace().map(str::to_string).collect()
}

/// `abs($x)`.
pub fn abs_num(v: f64) -> f64 {
    v.abs()
}

/// `abs($n)` on a whole number.
pub fn abs_int(v: i64) -> i64 {
    v.abs()
}

/// `sqrt($x)`. perl stops on a negative, and says so.
pub fn sqrt_of(v: f64) -> f64 {
    if v < 0.0 {
        panic!("Can't take sqrt of {}", num_text(v));
    }
    v.sqrt()
}

/// `POSIX::floor`.
pub fn floor_of(v: f64) -> f64 {
    v.floor()
}

/// `POSIX::ceil`.
pub fn ceil_of(v: f64) -> f64 {
    v.ceil()
}

/// `POSIX::fmod`. The remainder takes the sign of the left side, which
/// is C's rule and not `%`'s.
pub fn fmod_of(a: f64, b: f64) -> f64 {
    a % b
}

/// `List::Util::sum`. perl answers undef for an empty list, and the
/// dialect has no undef, so this says so rather than inventing a zero.
pub fn sum_int(xs: Vec<i64>) -> i64 {
    if xs.is_empty() {
        panic!("sum of an empty list is undef in perl, and this dialect has no undef");
    }
    xs.iter().sum()
}

/// `List::Util::sum` over numbers with a fraction.
pub fn sum_num(xs: Vec<f64>) -> f64 {
    if xs.is_empty() {
        panic!("sum of an empty list is undef in perl, and this dialect has no undef");
    }
    xs.iter().sum()
}

/// `List::Util::max`.
pub fn max_int(xs: Vec<i64>) -> i64 {
    *xs.iter()
        .max()
        .expect("max of an empty list is undef in perl, and this dialect has no undef")
}

/// `List::Util::min`.
pub fn min_int(xs: Vec<i64>) -> i64 {
    *xs.iter()
        .min()
        .expect("min of an empty list is undef in perl, and this dialect has no undef")
}

/// `List::Util::max` over numbers with a fraction.
pub fn max_num(xs: Vec<f64>) -> f64 {
    fold_num(xs, true)
}

/// `List::Util::min` over numbers with a fraction.
pub fn min_num(xs: Vec<f64>) -> f64 {
    fold_num(xs, false)
}

fn fold_num(xs: Vec<f64>, want_max: bool) -> f64 {
    let mut it = xs.into_iter();
    let mut best = it
        .next()
        .expect("max of an empty list is undef in perl, and this dialect has no undef");
    for v in it {
        if (want_max && v > best) || (!want_max && v < best) {
            best = v;
        }
    }
    best
}

/// `List::Util::uniq` over strings: the first of each, in the order
/// they came.
pub fn uniq_str(xs: Vec<String>) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for x in xs {
        if !seen.contains(&x) {
            seen.push(x);
        }
    }
    seen
}

/// `List::Util::uniq` over whole numbers.
pub fn uniq_int(xs: Vec<i64>) -> Vec<i64> {
    let mut seen: Vec<i64> = Vec::new();
    for x in xs {
        if !seen.contains(&x) {
            seen.push(x);
        }
    }
    seen
}

/// `POSIX::strftime($fmt, gmtime($epoch))`.
pub fn strftime_utc(fmt: &str, epoch: i64) -> String {
    strftime_at(fmt, epoch, 0)
}

/// `POSIX::strftime($fmt, localtime($epoch))`, with the machine's own
/// offset from UTC at that moment.
pub fn strftime_local(fmt: &str, epoch: i64) -> String {
    strftime_at(fmt, epoch, local_offset(epoch))
}

/// Seconds east of UTC where this machine stands, at `epoch`. Read
/// from the platform, because that is where perl reads it.
fn local_offset(epoch: i64) -> i64 {
    unsafe extern "C" {
        fn localtime_r(t: *const i64, tm: *mut CTm) -> *mut CTm;
    }
    let mut tm = CTm {
        zone: std::ptr::null(),
        ..Default::default()
    };
    let t = epoch;
    unsafe { localtime_r(&t, &mut tm) };
    tm.gmtoff
}

/// `strftime`'s `tm`, as the platform lays it out.
#[repr(C)]
#[derive(Default)]
struct CTm {
    sec: i32,
    min: i32,
    hour: i32,
    mday: i32,
    mon: i32,
    year: i32,
    wday: i32,
    yday: i32,
    isdst: i32,
    gmtoff: i64,
    zone: *const std::ffi::c_char,
}

/// LC_TIME, which is not the same number everywhere.
#[cfg(target_os = "macos")]
const LC_TIME: i32 = 5;
#[cfg(not(target_os = "macos"))]
const LC_TIME: i32 = 2;

/// A day or month name, asked of the platform rather than written out
/// here. perl's `POSIX::strftime` answers in the machine's LC_TIME
/// locale, and this is a twin of perl: on a Japanese machine both runs
/// have to say 木曜日, or the gate is comparing two different programs
/// rather than two runs of one.
///
/// A Rust process never calls `setlocale`, so it begins in "C" and
/// would answer English for ever; the call below is what reaches the
/// platform's own table at all. LC_TIME alone, because nothing else
/// here wants the locale's opinion — a decimal comma inside a number
/// would be a different bug. Held to a table printed under LC_ALL=C,
/// so what the table claims is true wherever it is read.
fn locale_name(fmt: &str, wday: i64, mon: i64) -> String {
    unsafe extern "C" {
        fn setlocale(category: i32, locale: *const std::ffi::c_char) -> *mut std::ffi::c_char;
        fn strftime(
            s: *mut std::ffi::c_char,
            max: usize,
            format: *const std::ffi::c_char,
            tm: *const CTm,
        ) -> usize;
    }
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| unsafe {
        setlocale(LC_TIME, c"".as_ptr());
    });
    // Only the two fields these four directives read are filled: the
    // rest of `tm` never reaches the answer.
    let tm = CTm {
        wday: wday as i32,
        mon: (mon - 1) as i32,
        zone: std::ptr::null(),
        ..Default::default()
    };
    let cfmt = std::ffi::CString::new(fmt).expect("a literal with no NUL");
    let mut buf = [0i8; 256];
    let n = unsafe {
        strftime(
            buf.as_mut_ptr() as *mut std::ffi::c_char,
            buf.len(),
            cfmt.as_ptr(),
            &tm,
        )
    };
    let bytes: Vec<u8> = buf[..n].iter().map(|&c| c as u8).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// The directives an app may write, and no others: an unknown one is
/// a mistake worth stopping for rather than a per cent sign printed
/// back.
fn strftime_at(fmt: &str, epoch: i64, offset: i64) -> String {
    let t = epoch + offset;
    let days = t.div_euclid(86_400);
    let secs = t.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (secs / 3600, (secs / 60) % 60, secs % 60);
    // 1970-01-01 was a Thursday.
    let wday = (days + 4).rem_euclid(7);
    let yday = days - days_from_civil(y, 1, 1) + 1;
    let mut out = String::new();
    let mut cs = fmt.chars().peekable();
    while let Some(c) = cs.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match cs.next() {
            Some('Y') => out.push_str(&y.to_string()),
            Some('y') => out.push_str(&format!("{:02}", y.rem_euclid(100))),
            Some('m') => out.push_str(&format!("{m:02}")),
            Some('d') => out.push_str(&format!("{d:02}")),
            Some('e') => out.push_str(&format!("{d:2}")),
            Some('H') => out.push_str(&format!("{hh:02}")),
            Some('M') => out.push_str(&format!("{mm:02}")),
            Some('S') => out.push_str(&format!("{ss:02}")),
            Some('j') => out.push_str(&format!("{yday:03}")),
            Some('F') => out.push_str(&format!("{y}-{m:02}-{d:02}")),
            Some('T') => out.push_str(&format!("{hh:02}:{mm:02}:{ss:02}")),
            Some('A') => out.push_str(&locale_name("%A", wday, m)),
            Some('a') => out.push_str(&locale_name("%a", wday, m)),
            Some('B') => out.push_str(&locale_name("%B", wday, m)),
            Some('b') => out.push_str(&locale_name("%b", wday, m)),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('%') => out.push('%'),
            Some(other) => panic!("the strftime directive %{other} is not in the dialect"),
            None => panic!("a strftime format ends with a bare %"),
        }
    }
    out
}

/// Days since 1970-01-01 to a civil date, and back. Hinnant's
/// algorithm, which is exact for every year a 64-bit epoch reaches.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

// ---------------------------------------------------------------------------
// More of perl's own: repetition, `**`, `ord` and `chr`, `hex` and `oct`,
// `chomp` and `chop`, the four-argument `substr`, `splice`, the libm
// functions, `builtin::trim`, `tr///` and List::Util's `maxstr`/`minstr`.
// The same rule: each is held to rows perl printed.

/// `$s x $n`. A count of zero or less answers nothing (perl warns on a
/// negative one, and answers the same).
pub fn repeat_str(s: &str, n: i64) -> String {
    if n <= 0 {
        return String::new();
    }
    s.repeat(n as usize)
}

/// `(LIST) x $n` over whole numbers.
pub fn repeat_int(xs: Vec<i64>, n: i64) -> Vec<i64> {
    repeat_list(xs, n)
}

/// `(LIST) x $n` over numbers with a fraction.
pub fn repeat_num(xs: Vec<f64>, n: i64) -> Vec<f64> {
    repeat_list(xs, n)
}

/// `(LIST) x $n` over strings.
pub fn repeat_strs(xs: Vec<String>, n: i64) -> Vec<String> {
    repeat_list(xs, n)
}

fn repeat_list<T: Clone>(xs: Vec<T>, n: i64) -> Vec<T> {
    if n <= 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(xs.len() * n as usize);
    for _ in 0..n {
        out.extend(xs.iter().cloned());
    }
    out
}

/// `$a ** $b` on two whole numbers, `b` not negative (the translator
/// refuses a negative one). Exact while the answer fits 64 bits, and a
/// stop when it does not.
///
/// perl is looser than that in both directions, and this is where the
/// two part. Past 64 bits it grows into a fraction where this stops.
/// And it answers a fraction well before that: whenever the base is a
/// power of two, or the bits of the base times the exponent pass 64,
/// perl computes in floating point and keeps the result as one, so
/// `2 ** 50` prints `1.12589990684262e+15` there and `1125899906842624`
/// here, and `3 ** 39` is exact here and rounded there. Below 1e15 the
/// two print the same digits, and the table holds only rows perl
/// prints as digits.
pub fn pow_int(a: i64, b: i64) -> i64 {
    if b < 0 {
        panic!("a negative exponent on two whole numbers is not in the dialect");
    }
    match a {
        0 => return if b == 0 { 1 } else { 0 },
        1 => return 1,
        -1 => return if b % 2 == 0 { 1 } else { -1 },
        _ => {}
    }
    // Anything else to the 64th or more is past 64 bits already.
    u32::try_from(b)
        .ok()
        .filter(|e| *e < 64)
        .and_then(|e| a.checked_pow(e))
        .unwrap_or_else(|| panic!("Integer overflow in **"))
}

/// `$a ** $b` with a fraction on either side: C's `pow`, which is what
/// perl calls. `0 ** -1` is Inf and `(-8) ** (1/3)` is NaN in both.
pub fn pow_num(a: f64, b: f64) -> f64 {
    a.powf(b)
}

/// `ord($s)`: the first character's code point, and 0 for nothing.
pub fn ord_of(s: &str) -> i64 {
    s.chars().next().map_or(0, |c| c as i64)
}

/// `chr($n)`. perl answers U+FFFD for a negative number and warns. A
/// surrogate, or a number past U+10FFFF, it puts into a string that is
/// not valid text — nothing a `String` can hold — so those answer U+FFFD
/// here too; the table has no row there.
pub fn chr_of(n: i64) -> String {
    u32::try_from(n)
        .ok()
        .and_then(char::from_u32)
        .unwrap_or('\u{FFFD}')
        .to_string()
}

/// `hex($s)`: an optional `x` or `0x` (either case), then hex digits,
/// stopping at the first character that is not one — perl warns there
/// and answers what it has, so `hex("ffg")` is 255 and `hex(" ff")` is
/// 0, since `hex` skips no space. An underscore is skipped when a digit
/// follows it, even a leading one: `hex("1_000")` is 4096, `hex("1__0")`
/// is 1. Anything perl cannot hold in one byte, anywhere in the string,
/// is fatal in perl 5.44 ("Wide character in hex") and here. Past 63
/// bits perl answers a UV, then a fraction; this stops.
pub fn hex_of(s: &str) -> i64 {
    let b = latin1_bytes(s, "hex");
    let digits = match b.first().map(u8::to_ascii_lowercase) {
        Some(b'x') => &b[1..],
        Some(b'0') if b.get(1).is_some_and(|c| c.eq_ignore_ascii_case(&b'x')) => &b[2..],
        _ => &b[..],
    };
    grok(digits, 16, "hex")
}

/// `oct($s)`: leading space skipped (unlike `hex`), one leading `0`
/// dropped, then `x`, `b` or `o` (either case) names the base and
/// anything else is octal. So `oct("0x1f")`, `oct("x1f")`, `oct("0b101")`,
/// `oct("0o17")` and `oct("0755")` all read as written, and `oct("789")`
/// is 7 with a warning in perl. The same underscore and overflow rules
/// as `hex`.
pub fn oct_of(s: &str) -> i64 {
    let b = latin1_bytes(s, "oct");
    let mut i = 0;
    while i < b.len() && matches!(b[i], b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r') {
        i += 1;
    }
    let mut t = &b[i..];
    if t.first() == Some(&b'0') {
        t = &t[1..];
    }
    match t.first().map(u8::to_ascii_lowercase) {
        Some(b'x') => grok(&t[1..], 16, "oct"),
        Some(b'b') => grok(&t[1..], 2, "oct"),
        Some(b'o') => grok(&t[1..], 8, "oct"),
        _ => grok(t, 8, "oct"),
    }
}

/// perl reads `hex` and `oct` off bytes, and a character it cannot
/// hold in one is fatal: "Wide character in hex".
fn latin1_bytes(s: &str, what: &str) -> Vec<u8> {
    s.chars()
        .map(|c| u8::try_from(c as u32).unwrap_or_else(|_| panic!("Wide character in {what}")))
        .collect()
}

/// perl's `grok_hex`, `grok_oct` and `grok_bin` after the prefix: the
/// digits of `base`, an underscore skipped when a digit follows it, and
/// a stop at anything else.
fn grok(b: &[u8], base: u32, what: &str) -> i64 {
    let digit = |c: u8| (c as char).to_digit(base);
    let mut acc: i64 = 0;
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'_' {
            if b.get(i + 1).is_some_and(|c| digit(*c).is_some()) {
                i += 1;
                continue;
            }
            break;
        }
        let Some(d) = digit(b[i]) else { break };
        acc = acc
            .checked_mul(base as i64)
            .and_then(|a| a.checked_add(d as i64))
            .unwrap_or_else(|| panic!("Integer overflow in {what}"));
        i += 1;
    }
    acc
}

/// `chomp`: one trailing newline off, and only a newline — perl's `$/`
/// is "\n" and the dialect does not change it, so "\r\n" keeps its "\r".
pub fn chomp_str(s: &str) -> String {
    s.strip_suffix('\n').unwrap_or(s).to_string()
}

/// `chop`: the last character off, whatever it is.
pub fn chop_str(s: &str) -> String {
    let mut out = s.to_string();
    out.pop();
    out
}

/// `join($sep, @ns)` over whole numbers.
pub fn join_int(sep: &str, xs: Vec<i64>) -> String {
    xs.iter().map(i64::to_string).collect::<Vec<_>>().join(sep)
}

/// `join($sep, @xs)` over numbers with a fraction, each written as perl
/// writes a number — `%.15g`, so `1e15` joins as `1e+15`.
pub fn join_num(sep: &str, xs: Vec<f64>) -> String {
    xs.iter().map(|v| num_text(*v)).collect::<Vec<_>>().join(sep)
}

/// `substr($s, $off, $len, $repl)`, the four-argument form, answering
/// the string after; the translator writes it back. Offsets count
/// characters. A negative offset counts from the end and a negative
/// length leaves that many at the end, as in the other forms; where
/// those answer undef for an offset outside the string, this one dies,
/// as perl does.
pub fn substr_replace(s: &str, off: i64, len: i64, repl: &str) -> String {
    let cs: Vec<char> = s.chars().collect();
    let Some((start, n)) = substr_span(cs.len(), off, len) else {
        panic!("substr outside of string");
    };
    let mut out: String = cs[..start].iter().collect();
    out.push_str(repl);
    out.extend(&cs[start + n..]);
    out
}

/// perl's `translate_substr_offsets`: where a substr starts and how far
/// it runs, or nothing when it is outside the string. An offset past the
/// end is outside. A negative offset counts from the end; one that counts
/// past the start is clamped to it — unless the length, counted from
/// that offset, also ends before the start, which is outside too. A
/// length of zero or more runs from the offset and is capped at the end;
/// a negative one stops that many before the end, and never before the
/// offset.
fn substr_span(curlen: usize, off: i64, len: i64) -> Option<(usize, usize)> {
    let n = curlen as i64;
    let mut pos1 = off;
    if off < 0 && curlen > 0 {
        pos1 += n;
    }
    if pos1 > n {
        return None;
    }
    let pos2 = if len < 0 {
        n + len
    } else if pos1 < 0 {
        pos1 + len
    } else if len > n - pos1 {
        n
    } else {
        pos1 + len
    };
    let (pos1, pos2) = if pos2 < 0 {
        if pos1 < 0 {
            return None;
        }
        (pos1, 0)
    } else {
        (pos1.max(0), pos2)
    };
    let pos2 = pos2.max(pos1).min(n);
    Some((pos1 as usize, (pos2 - pos1) as usize))
}

/// `splice(@xs, $off, $len, LIST)` over whole numbers, answering the
/// list after; the translator writes it back. A negative offset counts
/// from the end, and one that counts past the start dies with perl's
/// words; a negative length leaves that many at the end; an offset past
/// the end appends (perl warns there).
pub fn splice_int(xs: Vec<i64>, off: i64, len: i64, repl: Vec<i64>) -> Vec<i64> {
    splice_list(xs, off, len, repl)
}

/// `splice` over numbers with a fraction.
pub fn splice_num(xs: Vec<f64>, off: i64, len: i64, repl: Vec<f64>) -> Vec<f64> {
    splice_list(xs, off, len, repl)
}

/// `splice` over strings.
pub fn splice_strs(xs: Vec<String>, off: i64, len: i64, repl: Vec<String>) -> Vec<String> {
    splice_list(xs, off, len, repl)
}

/// `pp_splice`'s arithmetic, in its order: the negative length is
/// resolved against the offset as given, and the offset is clamped to
/// the end after that.
fn splice_list<T>(mut xs: Vec<T>, off: i64, len: i64, repl: Vec<T>) -> Vec<T> {
    let n = xs.len() as i64;
    let mut offset = off;
    if offset < 0 {
        offset += n;
    }
    if offset < 0 {
        panic!("Modification of non-creatable array value attempted, subscript {off}");
    }
    let mut length = len;
    if length < 0 {
        length = (length + n - offset).max(0);
    }
    offset = offset.min(n);
    length = length.min(n - offset);
    let start = offset as usize;
    xs.splice(start..start + length as usize, repl);
    xs
}

/// `time`: seconds since the epoch.
pub fn time_now() -> i64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        Err(e) => -(e.duration().as_secs() as i64),
    }
}

/// `sin`, `cos`, `exp` and `atan2`: the platform's libm, which is what
/// perl calls too. Their rows allow one ulp, since two libms may round
/// the last bit differently.
pub fn sin_of(v: f64) -> f64 {
    v.sin()
}

pub fn cos_of(v: f64) -> f64 {
    v.cos()
}

pub fn exp_of(v: f64) -> f64 {
    v.exp()
}

pub fn atan2_of(y: f64, x: f64) -> f64 {
    y.atan2(x)
}

/// `log($x)`. perl stops at zero and below, and says so — writing the
/// number with `%g` rather than as a string, which is why a negative
/// zero reads `-0` in the message where `"$x"` would read `0`.
pub fn log_of(v: f64) -> f64 {
    if v <= 0.0 {
        let text = if v == 0.0 && v.is_sign_negative() { "-0".to_string() } else { num_text(v) };
        panic!("Can't take log of {text}");
    }
    v.ln()
}

/// `builtin::trim`: whitespace off both ends, by Unicode's White_Space
/// — which is perl's `\s` and Rust's `char::is_whitespace`, the same
/// twenty-five characters: U+3000 and U+0085 go, U+200B and U+FEFF stay.
pub fn trim_str(s: &str) -> String {
    s.trim().to_string()
}

/// `tr/FROM/TO/FLAGS` as a value (`/r`): each character of FROM becomes
/// its partner in TO. Ranges (`a-z`) and the escapes `\n`, `\t`, `\\`
/// and `\-` are read in both, and a character's first place in FROM is
/// the one that counts. A TO shorter than FROM repeats its last
/// character — or, with the `d` flag, deletes what has no partner — and
/// an empty TO is FROM itself. The `s` flag squeezes a run of characters
/// that came out the same into one; a deleted character does not break
/// the run and an untouched one does, as perl's own loop has it. The `c`
/// flag is not taken.
pub fn tr_str(s: &str, from: &str, to: &str, flags: &str) -> String {
    let delete = flags.contains('d');
    let squeeze = flags.contains('s');
    let from = tr_chars(from);
    let mut to = tr_chars(to);
    if to.is_empty() && !delete {
        to = from.clone();
    }
    // What each character of FROM becomes: its partner, the last
    // partner repeated, or nothing at all.
    let mut map: std::collections::HashMap<char, Option<char>> = std::collections::HashMap::new();
    for (i, &c) in from.iter().enumerate() {
        map.entry(c).or_insert_with(|| {
            to.get(i).copied().or_else(|| if delete { None } else { to.last().copied() })
        });
    }
    let mut out = String::new();
    let mut previous: Option<char> = None;
    for c in s.chars() {
        match map.get(&c) {
            None => {
                out.push(c);
                previous = None;
            }
            Some(None) => {}
            Some(Some(t)) => {
                if !(squeeze && previous == Some(*t)) {
                    out.push(*t);
                    previous = Some(*t);
                }
            }
        }
    }
    out
}

/// `$s =~ tr/FROM//` where the count is wanted: how many characters of
/// `s` are in FROM.
pub fn tr_count(s: &str, from: &str) -> i64 {
    let set = tr_chars(from);
    s.chars().filter(|c| set.contains(c)).count() as i64
}

/// One side of a `tr///`, expanded: escapes first, then ranges. A `-`
/// written with a backslash, or standing first or last, is a character
/// rather than a range.
fn tr_chars(spec: &str) -> Vec<char> {
    let mut items: Vec<(char, bool)> = Vec::new();
    let mut cs = spec.chars();
    while let Some(c) = cs.next() {
        if c != '\\' {
            items.push((c, false));
            continue;
        }
        match cs.next() {
            Some('n') => items.push(('\n', true)),
            Some('t') => items.push(('\t', true)),
            Some(other) => items.push((other, true)),
            None => items.push(('\\', true)),
        }
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let (c, _) = items[i];
        if i + 2 < items.len() && items[i + 1] == ('-', false) {
            let (end, _) = items[i + 2];
            if end < c {
                panic!("Invalid range \"{c}-{end}\" in transliteration operator");
            }
            out.extend(c..=end);
            i += 3;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// `List::Util::maxstr`: the greatest by string comparison, which is
/// code point order. perl answers undef for an empty list, and this
/// dialect has no undef; nothing is answered instead, and the table has
/// no row there.
pub fn max_str(xs: Vec<String>) -> String {
    xs.into_iter().max().unwrap_or_default()
}

/// `List::Util::minstr`.
pub fn min_str(xs: Vec<String>) -> String {
    xs.into_iter().min().unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Regular expressions.
//
// perl's own engine cannot be lifted out of the interpreter: there is
// no way to hand a compiled program to anything else, and `use re
// 'debug'` prints for a person to read. So the compiled run runs
// PCRE2, whose stated design goal is Perl's syntax and semantics, and
// the table `tools/gen_expected.pl` prints says where the two agree
// for the syntax an app may write. What pcre2compat lists as
// different, the translator refuses by name before it gets here.

fn compile(pat: &str, flags: &str) -> pcre2::bytes::Regex {
    let mut b = pcre2::bytes::RegexBuilder::new();
    b.utf(true).ucp(true);
    if flags.contains('i') {
        b.caseless(true);
    }
    if flags.contains('x') {
        b.extended(true);
    }
    if flags.contains('m') {
        b.multi_line(true);
    }
    if flags.contains('s') {
        b.dotall(true);
    }
    b.build(pat)
        .unwrap_or_else(|e| panic!("this pattern does not compile: {e}"))
}

/// `$s =~ /pat/`.
pub fn re_matches(pat: &str, flags: &str, s: &str) -> bool {
    compile(pat, flags)
        .is_match(s.as_bytes())
        .unwrap_or_else(|e| panic!("matching failed: {e}"))
}

/// `$1`, `$2`, … after a match, and `$&` as group 0. Nothing matched
/// answers nothing, which is what perl leaves in an unset group.
pub fn re_capture(pat: &str, flags: &str, s: &str, n: i64) -> String {
    let re = compile(pat, flags);
    match re.captures(s.as_bytes()) {
        Ok(Some(c)) => c
            .get(n as usize)
            .map(|m| String::from_utf8_lossy(m.as_bytes()).into_owned())
            .unwrap_or_default(),
        Ok(None) => String::new(),
        Err(e) => panic!("matching failed: {e}"),
    }
}

/// `$+{name}` after a match.
pub fn re_capture_named(pat: &str, flags: &str, s: &str, name: &str) -> String {
    let re = compile(pat, flags);
    match re.captures(s.as_bytes()) {
        Ok(Some(c)) => c
            .name(name)
            .map(|m| String::from_utf8_lossy(m.as_bytes()).into_owned())
            .unwrap_or_default(),
        Ok(None) => String::new(),
        Err(e) => panic!("matching failed: {e}"),
    }
}

/// `s/pat/repl/` and, with `g` among the flags, `s/pat/repl/g`. The
/// replacement is text with `$1` … `$9` in it, which is what perl
/// puts there.
pub fn re_subst(pat: &str, flags: &str, s: &str, repl: &str) -> String {
    let re = compile(pat, flags);
    let all = flags.contains('g');
    let bytes = s.as_bytes();
    let mut out = String::new();
    let mut at = 0usize;
    loop {
        let caps = match re.captures(&bytes[at..]) {
            Ok(Some(c)) => c,
            Ok(None) => break,
            Err(e) => panic!("matching failed: {e}"),
        };
        let whole = caps.get(0).expect("a match has a whole");
        out.push_str(&String::from_utf8_lossy(&bytes[at..at + whole.start()]));
        out.push_str(&expand(repl, &caps));
        let end = whole.end();
        // A pattern that matched nothing moves on by one, the way
        // perl's own `s///g` does rather than standing still.
        if end == whole.start() {
            if at + end >= bytes.len() {
                at += end;
                break;
            }
            let step = next_boundary(bytes, at + end) - (at + end);
            out.push_str(&String::from_utf8_lossy(&bytes[at + end..at + end + step]));
            at += end + step;
        } else {
            at += end;
        }
        if !all {
            break;
        }
    }
    out.push_str(&String::from_utf8_lossy(&bytes[at..]));
    out
}

fn next_boundary(bytes: &[u8], i: usize) -> usize {
    let mut j = i + 1;
    while j < bytes.len() && (bytes[j] & 0xC0) == 0x80 {
        j += 1;
    }
    j
}

fn expand(repl: &str, caps: &pcre2::bytes::Captures<'_>) -> String {
    let mut out = String::new();
    let mut cs = repl.chars().peekable();
    while let Some(c) = cs.next() {
        if c != '$' {
            out.push(c);
            continue;
        }
        match cs.peek() {
            // `$0` is the program's name in perl, not a group.
            Some(d) if d.is_ascii_digit() && *d != '0' => {
                let n = cs.next().unwrap().to_digit(10).unwrap() as usize;
                if let Some(m) = caps.get(n) {
                    out.push_str(&String::from_utf8_lossy(m.as_bytes()));
                }
            }
            _ => out.push('$'),
        }
    }
    out
}

/// `split /pat/, $s`. perl drops the empty fields at the end, a
/// separator that captures puts what it captured into the list, and a
/// separator that matches nothing splits between characters — but
/// never before the first one and never twice in the same place.
pub fn re_split(pat: &str, flags: &str, s: &str) -> Vec<String> {
    let re = compile(pat, flags);
    let groups = re.captures_len() - 1;
    let bytes = s.as_bytes();
    let mut out: Vec<String> = Vec::new();
    let mut field = 0usize;
    let mut search = 0usize;
    while search <= bytes.len() {
        let caps = match re.captures(&bytes[search..]) {
            Ok(Some(c)) => c,
            Ok(None) => break,
            Err(e) => panic!("matching failed: {e}"),
        };
        let m = caps.get(0).expect("a match has a whole");
        let (ms, me) = (search + m.start(), search + m.end());
        if ms == me {
            if ms >= bytes.len() {
                break;
            }
            // A separator of no width right where the last one ended
            // is not a second split point.
            if ms > field {
                out.push(String::from_utf8_lossy(&bytes[field..ms]).into_owned());
                push_groups(&mut out, &caps, groups);
                field = ms;
            }
            search = next_boundary(bytes, ms);
            continue;
        }
        out.push(String::from_utf8_lossy(&bytes[field..ms]).into_owned());
        push_groups(&mut out, &caps, groups);
        field = me;
        search = me;
    }
    out.push(String::from_utf8_lossy(&bytes[field..]).into_owned());
    while out.last().is_some_and(|f| f.is_empty()) {
        out.pop();
    }
    out
}

fn push_groups(out: &mut Vec<String>, caps: &pcre2::bytes::Captures<'_>, groups: usize) {
    for i in 1..=groups {
        out.push(
            caps.get(i)
                .map(|g| String::from_utf8_lossy(g.as_bytes()).into_owned())
                .unwrap_or_default(),
        );
    }
}

/// `$s =~ /pat/g` where a list is wanted: every match, or every first
/// group when the pattern has one.
pub fn re_all(pat: &str, flags: &str, s: &str) -> Vec<String> {
    let re = compile(pat, flags);
    let groups = re.captures_len() - 1;
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut at = 0usize;
    while at <= bytes.len() {
        let caps = match re.captures(&bytes[at..]) {
            Ok(Some(c)) => c,
            Ok(None) => break,
            Err(e) => panic!("matching failed: {e}"),
        };
        let whole = caps.get(0).expect("a match has a whole");
        if groups == 0 {
            out.push(String::from_utf8_lossy(whole.as_bytes()).into_owned());
        } else {
            for i in 1..=groups {
                out.push(
                    caps.get(i)
                        .map(|m| String::from_utf8_lossy(m.as_bytes()).into_owned())
                        .unwrap_or_default(),
                );
            }
        }
        let end = whole.end();
        at += if end == whole.start() {
            if at + end >= bytes.len() {
                break;
            }
            next_boundary(bytes, at + end) - at
        } else {
            end
        };
    }
    out
}

/// `scalar(() = $s =~ /pat/g)` — how many VALUES the match hands
/// back, which for a pattern with two groups is two per match, the
/// way perl counts them.
pub fn re_count(pat: &str, flags: &str, s: &str) -> i64 {
    re_all(pat, flags, s).len() as i64
}

// --- perl's own random numbers -----------------------------------------------
//
// Since 5.20 perl carries its own drand48 on every platform: a 48-bit
// linear congruence, seeded from the low 32 bits of what `srand` is
// given, and `rand(N)` is the next value times N. So the sequence a
// seeded app draws is the same everywhere, and this is that sequence.
// `List::Util::shuffle` draws from the same state.
// One state for the whole program, as perl has one per interpreter:
// the app seeds at startup and draws from a handler, and the compiled
// run does those on different threads.
static RAND48: std::sync::Mutex<u64> = std::sync::Mutex::new(0);

const DRAND48_MASK: u64 = (1u64 << 48) - 1;

/// `srand($seed)`: perl reads the number's digits, so a minus sign is
/// dropped (`srand(-1)` is `srand(1)`), keeps the low 32 bits of what is
/// left, and answers the seed as given.
pub fn srand(seed: i64) -> i64 {
    let low = seed.unsigned_abs() as u32;
    *RAND48.lock().unwrap_or_else(|e| e.into_inner()) = 0x330E + ((low as u64) << 16);
    seed
}

fn drand48() -> f64 {
    let mut s = RAND48.lock().unwrap_or_else(|e| e.into_inner());
    let x = (s.wrapping_mul(0x5DEECE66D).wrapping_add(0xB)) & DRAND48_MASK;
    *s = x;
    x as f64 / (1u64 << 48) as f64
}

/// `rand($n)`: the next value in `[0, n)`; `rand(0)` and `rand()` are
/// `rand(1)`, as perl has them.
pub fn rand(n: f64) -> f64 {
    let n = if n == 0.0 { 1.0 } else { n };
    drand48() * n
}

fn shuffle_in_place<T>(xs: &mut [T]) {
    let mut index = xs.len();
    while index > 1 {
        let swap = (drand48() * index as f64) as usize;
        index -= 1;
        xs.swap(swap, index);
    }
}

/// `List::Util::shuffle`, drawing from perl's own state: the swaps
/// walk down from the end, as the XS does.
pub fn shuffle_int(xs: Vec<i64>) -> Vec<i64> {
    let mut xs = xs;
    shuffle_in_place(&mut xs);
    xs
}

pub fn shuffle_str(xs: Vec<String>) -> Vec<String> {
    let mut xs = xs;
    shuffle_in_place(&mut xs);
    xs
}

pub fn shuffle_num(xs: Vec<f64>) -> Vec<f64> {
    let mut xs = xs;
    shuffle_in_place(&mut xs);
    xs
}

#[cfg(test)]
mod failing {
    //! What stops a handler, and what a `try` is handed instead. The
    //! tables cannot hold a die, so these hold the words.
    use super::*;

    #[test]
    #[should_panic(expected = "Illegal division by zero")]
    fn a_zero_divisor_dies_with_perls_words() {
        div_num(1.5, 0.0);
    }

    #[test]
    #[should_panic(expected = "boom at x.pl line 3.")]
    fn die_names_the_place_unless_the_text_ends_its_line() {
        die_at("boom", " at x.pl line 3.\n");
    }

    #[test]
    fn the_try_forms_carry_the_place() {
        let at = " at x.pl line 3.\n";
        assert_eq!(try_div_int(1, 0, at), Err("Illegal division by zero at x.pl line 3.\n".to_string()));
        assert_eq!(try_div_num(1.0, 0.0, at), Err("Illegal division by zero at x.pl line 3.\n".to_string()));
        assert_eq!(try_mod_int(7, 0, at), Err("Illegal modulus zero at x.pl line 3.\n".to_string()));
        assert_eq!(try_sqrt(-2.5, at), Err("Can't take sqrt of -2.5 at x.pl line 3.\n".to_string()));
        assert_eq!(try_div_int(7, 2, at), Ok(3.5));
        assert_eq!(try_mod_int(-7, 3, at), Ok(2));
        assert_eq!(die_text("boom\n", at), "boom\n");
    }

    #[test]
    #[should_panic(expected = "Can't take log of 0")]
    fn log_of_zero_dies_with_perls_words() {
        log_of(0.0);
    }

    #[test]
    #[should_panic(expected = "Can't take log of -0")]
    fn log_of_negative_zero_writes_the_sign_as_perl_does() {
        log_of(-0.0);
    }

    #[test]
    #[should_panic(expected = "substr outside of string")]
    fn a_replacement_past_the_end_dies() {
        substr_replace("hello", 6, 0, "X");
    }

    #[test]
    #[should_panic(expected = "substr outside of string")]
    fn a_replacement_that_ends_before_the_start_dies() {
        substr_replace("hello", -10, 2, "X");
    }

    #[test]
    #[should_panic(expected = "Integer overflow in **")]
    fn a_power_past_64_bits_stops() {
        pow_int(2, 63);
    }

    #[test]
    #[should_panic(expected = "Integer overflow in hex")]
    fn a_hex_number_past_63_bits_stops() {
        hex_of("ffffffffffffffff");
    }

    #[test]
    #[should_panic(expected = "Integer overflow in oct")]
    fn an_octal_number_past_63_bits_stops() {
        oct_of("2000000000000000000000");
    }

    #[test]
    #[should_panic(expected = "Wide character in hex")]
    fn hex_of_a_wide_character_dies_as_perl_does() {
        hex_of("ff日");
    }

    #[test]
    #[should_panic(expected = "Modification of non-creatable array value attempted, subscript -4")]
    fn a_splice_before_the_start_dies_with_perls_words() {
        splice_int(vec![1, 2, 3], -4, 1, vec![]);
    }

    #[test]
    #[should_panic(expected = "Invalid range \"z-a\" in transliteration operator")]
    fn a_backwards_range_in_tr_is_refused() {
        tr_str("abc", "z-a", "x", "");
    }

    #[test]
    fn rand_is_perls_drand48() {
        // `perl -e 'srand(42); printf "%.17g %.17g", rand(), rand()'`
        srand(42);
        assert_eq!(rand(1.0), 0.74452500006100664);
        assert_eq!(rand(0.0), 0.34270147871890799);
        // the low 32 bits of the seed are what perl keeps
        srand(4294967296);
        let a = rand(1.0);
        srand(0);
        assert_eq!(a, rand(1.0));
    }
}
