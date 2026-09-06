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

#[cfg(test)]
mod tests;
