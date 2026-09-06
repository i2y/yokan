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
    let start = start_of(off, cs.len());
    match start {
        Some(i) => cs[i..].iter().collect(),
        None => String::new(),
    }
}

/// `substr($s, $off, $len)`. A negative length leaves that many
/// characters off the end.
pub fn substr_len(s: &str, off: i64, len: i64) -> String {
    let cs: Vec<char> = s.chars().collect();
    let Some(start) = start_of(off, cs.len()) else {
        return String::new();
    };
    let end = if len < 0 {
        let e = cs.len() as i64 + len;
        if e < start as i64 { start } else { e as usize }
    } else {
        (start + len as usize).min(cs.len())
    };
    cs[start..end].iter().collect()
}

fn start_of(off: i64, n: usize) -> Option<usize> {
    let i = if off < 0 { n as i64 + off } else { off };
    if i < 0 || i > n as i64 { None } else { Some(i as usize) }
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
        zone: *const i8,
    }
    let mut tm = CTm {
        zone: std::ptr::null(),
        ..Default::default()
    };
    let t = epoch;
    unsafe { localtime_r(&t, &mut tm) };
    tm.gmtoff
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
    const WDAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
    const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June",
                                "July", "August", "September", "October", "November", "December"];
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
            Some('A') => out.push_str(WDAYS[wday as usize]),
            Some('a') => out.push_str(&WDAYS[wday as usize][..3]),
            Some('B') => out.push_str(MONTHS[(m - 1) as usize]),
            Some('b') => out.push_str(&MONTHS[(m - 1) as usize][..3]),
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
