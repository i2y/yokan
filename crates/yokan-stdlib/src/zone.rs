//! The machine's own zone data, read the way CPython reads it.
//!
//! `zoneinfo` is Python's name, so CPython decides the answers — and
//! CPython's answers come from the TZif files on the machine rather
//! than from a database of its own. The compiled run reads the same
//! files, in the same order, with the same rules, which is what makes
//! the two runs agree about a zone nobody can see inside.
//!
//! The rules are a port of CPython's own `zoneinfo/_zoneinfo.py` and
//! `zoneinfo/_common.py`: the transition table in UTC and in local
//! time (one list per `fold`), the POSIX rule in the file's footer for
//! times past the last transition, and the DST offset inferred from
//! the `isdst` flags.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// One local time type: the offset from UTC, how much of it is DST,
/// and the abbreviation (`JST`, `EDT`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tti {
    pub utcoff: i64,
    pub dstoff: i64,
    pub name: String,
}

/// What is in effect past the last transition: a single type, or the
/// POSIX rule the file's footer carries.
#[derive(Clone, Debug)]
enum After {
    Fixed(Tti),
    Rule(Box<TzStr>),
}

#[derive(Clone, Debug)]
pub struct Zone {
    trans_utc: Vec<i64>,
    /// The same transitions in local seconds, one list per `fold`.
    trans_local: [Vec<i64>; 2],
    /// The type in effect from each transition on.
    ttinfos: Vec<Tti>,
    tti_before: Option<Tti>,
    after: After,
}

// ---- the file ------------------------------------------------------

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.at + n > self.b.len() {
            return Err("Invalid TZif file: unexpected end of file".into());
        }
        let out = &self.b[self.at..self.at + n];
        self.at += n;
        Ok(out)
    }
    fn i32be(&mut self) -> Result<i32, String> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into().expect("4 bytes")))
    }
    fn i64be(&mut self) -> Result<i64, String> {
        Ok(i64::from_be_bytes(self.take(8)?.try_into().expect("8 bytes")))
    }
}

struct Header {
    version: u8,
    isutcnt: usize,
    isstdcnt: usize,
    leapcnt: usize,
    timecnt: usize,
    typecnt: usize,
    charcnt: usize,
}

fn read_header(r: &mut Reader) -> Result<Header, String> {
    if r.take(4)? != b"TZif" {
        return Err("Invalid TZif file: magic not found".into());
    }
    let version = r.take(1)?[0];
    let version = if version == 0 { 1 } else { version - b'0' };
    r.take(15)?;
    let mut n = [0usize; 6];
    for slot in &mut n {
        *slot = r.i32be()?.max(0) as usize;
    }
    Ok(Header {
        version,
        isutcnt: n[0],
        isstdcnt: n[1],
        leapcnt: n[2],
        timecnt: n[3],
        typecnt: n[4],
        charcnt: n[5],
    })
}

impl Zone {
    /// Parse one TZif file. Version 2 and 3 carry a 64-bit block after
    /// the 32-bit one, and the 64-bit block is the one to read.
    pub fn parse(bytes: &[u8]) -> Result<Zone, String> {
        let mut r = Reader { b: bytes, at: 0 };
        let mut h = read_header(&mut r)?;
        let wide = h.version >= 2;
        if wide {
            let skip = h.timecnt * 5
                + h.typecnt * 6
                + h.charcnt
                + h.leapcnt * 8
                + h.isstdcnt
                + h.isutcnt;
            r.take(skip)?;
            h = read_header(&mut r)?;
        }

        let mut trans_utc = Vec::with_capacity(h.timecnt);
        for _ in 0..h.timecnt {
            trans_utc.push(if wide { r.i64be()? } else { i64::from(r.i32be()?) });
        }
        let trans_idx: Vec<usize> = r.take(h.timecnt)?.iter().map(|b| *b as usize).collect();
        if trans_idx.iter().any(|i| *i >= h.typecnt) {
            return Err("Invalid transition index found while reading TZif".into());
        }

        let mut utcoff = Vec::with_capacity(h.typecnt);
        let mut isdst = Vec::with_capacity(h.typecnt);
        let mut abbrind = Vec::with_capacity(h.typecnt);
        for _ in 0..h.typecnt {
            utcoff.push(i64::from(r.i32be()?));
            let rest = r.take(2)?;
            isdst.push(rest[0] != 0);
            abbrind.push(rest[1] as usize);
        }
        let chars = r.take(h.charcnt)?.to_vec();
        let abbr: Vec<String> = abbrind.iter().map(|i| abbr_at(&chars, *i)).collect();

        // Past the transitions, version 2 and 3 carry a POSIX rule.
        let mut tz_str = None;
        if h.version >= 2 {
            r.take(h.isutcnt + h.isstdcnt + h.leapcnt * 12)?;
            if r.at < r.b.len() && r.b[r.at] == b'\n' {
                r.at += 1;
                let end = r.b[r.at..]
                    .iter()
                    .position(|c| *c == b'\n')
                    .ok_or("Invalid TZif file: unexpected end of file")?;
                let line = String::from_utf8_lossy(&r.b[r.at..r.at + end]).into_owned();
                if !line.is_empty() {
                    tz_str = Some(line);
                }
            }
        }

        let dstoff = utcoff_to_dstoff(&trans_idx, &utcoff, &isdst);
        let types: Vec<Tti> = (0..h.typecnt)
            .map(|i| Tti {
                utcoff: utcoff[i],
                dstoff: dstoff[i],
                name: abbr[i].clone(),
            })
            .collect();
        let trans_local = ts_to_local(&trans_idx, &trans_utc, &utcoff);
        let ttinfos: Vec<Tti> = trans_idx.iter().map(|i| types[*i].clone()).collect();

        // The first type that is not DST is what stood before the
        // first transition.
        let tti_before = isdst
            .iter()
            .position(|d| !*d)
            .map(|i| types[i].clone())
            .or_else(|| ttinfos.first().cloned());

        let after = match tz_str.as_deref() {
            Some(s) => match parse_tz_str(s)? {
                ParsedTz::Rule(t) => After::Rule(Box::new(t)),
                ParsedTz::Fixed(t) => After::Fixed(t),
            },
            None => match ttinfos.last().or_else(|| types.last()) {
                Some(t) => After::Fixed(t.clone()),
                None => return Err("No time zone information found.".into()),
            },
        };
        Ok(Zone {
            trans_utc,
            trans_local,
            ttinfos,
            tti_before,
            after,
        })
    }

    /// The type in effect at a WALL clock time, in local seconds since
    /// 1970 — what `utcoffset()` and `tzname()` answer. `fold` is 0
    /// here, as a dialect with no `fold` keyword always means the
    /// first of an ambiguous pair.
    pub fn at_local(&self, local: i64, year: i64, fold: usize) -> Tti {
        let lt = &self.trans_local[fold];
        if !lt.is_empty() && local < lt[0] {
            return self.tti_before.clone().unwrap_or_else(|| self.after_tti());
        }
        if lt.is_empty() || local > lt[lt.len() - 1] {
            return match &self.after {
                After::Fixed(t) => t.clone(),
                After::Rule(r) => r.at_local(local, year, fold),
            };
        }
        let idx = bisect_right(lt, local) - 1;
        self.ttinfos[idx].clone()
    }

    /// The type in effect at an INSTANT, in UTC seconds since 1970,
    /// and whether that instant is the second of an ambiguous pair —
    /// CPython's `fromutc`.
    pub fn at_utc(&self, utc: i64, year: i64) -> (Tti, bool) {
        let n = self.trans_utc.len();
        if n >= 1 && utc < self.trans_utc[0] {
            return (self.tti_before.clone().unwrap_or_else(|| self.after_tti()), false);
        }
        if n == 0 || utc > self.trans_utc[n - 1] {
            return match &self.after {
                After::Fixed(t) => (t.clone(), false),
                After::Rule(r) => r.at_utc(utc, year),
            };
        }
        let idx = bisect_right(&self.trans_utc, utc);
        let (prev, cur) = if n > 1 && utc >= self.trans_utc[1] {
            (self.ttinfos[idx - 2].clone(), self.ttinfos[idx - 1].clone())
        } else {
            (
                self.tti_before.clone().unwrap_or_else(|| self.ttinfos[0].clone()),
                self.ttinfos[0].clone(),
            )
        };
        let shift = prev.utcoff - cur.utcoff;
        let fold = shift > utc - self.trans_utc[idx - 1];
        (cur, fold)
    }

    fn after_tti(&self) -> Tti {
        match &self.after {
            After::Fixed(t) => t.clone(),
            After::Rule(r) => r.std.clone(),
        }
    }
}

fn abbr_at(chars: &[u8], idx: usize) -> String {
    let end = chars[idx..]
        .iter()
        .position(|c| *c == 0)
        .map_or(chars.len(), |p| idx + p);
    String::from_utf8_lossy(&chars[idx..end]).into_owned()
}

fn bisect_right(xs: &[i64], v: i64) -> usize {
    let (mut lo, mut hi) = (0usize, xs.len());
    while lo < hi {
        let mid = (lo + hi) / 2;
        if v < xs[mid] {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    lo
}

/// `.dst()` is the difference between this offset and the standard
/// one, and the file does not carry it — CPython infers it from the
/// neighbouring transitions, and so does this.
fn utcoff_to_dstoff(trans_idx: &[usize], utcoff: &[i64], isdst: &[bool]) -> Vec<i64> {
    let typecnt = isdst.len();
    let mut dstoffs = vec![0i64; typecnt];
    let dst_cnt = isdst.iter().filter(|d| **d).count();
    let mut dst_found = 0usize;
    for i in 1..trans_idx.len() {
        if dst_cnt == dst_found {
            break;
        }
        let idx = trans_idx[i];
        if !isdst[idx] || dstoffs[idx] != 0 {
            continue;
        }
        let mut dstoff = 0i64;
        let utc = utcoff[idx];
        let comp = trans_idx[i - 1];
        if !isdst[comp] {
            dstoff = utc - utcoff[comp];
        }
        if dstoff == 0 && idx < typecnt - 1 && i + 1 < trans_idx.len() {
            let comp = trans_idx[i + 1];
            if isdst[comp] {
                continue;
            }
            dstoff = utc - utcoff[comp];
        }
        if dstoff != 0 {
            dst_found += 1;
            dstoffs[idx] = dstoff;
        }
    }
    if dst_found != dst_cnt {
        // A type flagged DST whose offset nothing pinned: an hour is a
        // better guess than nothing, which is what CPython says too.
        for i in 0..typecnt {
            if dstoffs[i] == 0 && isdst[i] {
                dstoffs[i] = 3600;
            }
        }
    }
    dstoffs
}

/// The transitions in LOCAL seconds, one list per `fold`: the same
/// instant read with the offset on either side of the change.
fn ts_to_local(trans_idx: &[usize], trans_utc: &[i64], utcoff: &[i64]) -> [Vec<i64>; 2] {
    if trans_utc.is_empty() {
        return [Vec::new(), Vec::new()];
    }
    let mut out = [trans_utc.to_vec(), trans_utc.to_vec()];
    let (mut o0, mut o1) = if utcoff.len() > 1 {
        (utcoff[0], utcoff[trans_idx[0]])
    } else {
        (utcoff[0], utcoff[0])
    };
    if o1 > o0 {
        std::mem::swap(&mut o0, &mut o1);
    }
    out[0][0] += o0;
    out[1][0] += o1;
    for i in 1..trans_idx.len() {
        let (mut a, mut b) = (utcoff[trans_idx[i - 1]], utcoff[trans_idx[i]]);
        if b > a {
            std::mem::swap(&mut a, &mut b);
        }
        out[0][i] += a;
        out[1][i] += b;
    }
    out
}

// ---- the POSIX rule in the footer ----------------------------------

#[derive(Clone, Debug)]
struct TzStr {
    std: Tti,
    dst: Tti,
    dst_diff: i64,
    start: DayRule,
    end: DayRule,
}

#[derive(Clone, Debug)]
enum DayRule {
    /// `Jn` (no leap day counted) or `n` (leap day counted).
    Day { d: i64, julian: bool, time: i64 },
    /// `Mm.w.d` — the w-th day `d` of month `m`.
    Cal { m: i64, w: i64, d: i64, time: i64 },
}

enum ParsedTz {
    Fixed(Tti),
    Rule(TzStr),
}

impl TzStr {
    fn transitions(&self, year: i64) -> (i64, i64) {
        (self.start.year_to_epoch(year), self.end.year_to_epoch(year))
    }

    fn at_local(&self, ts: i64, year: i64, fold: usize) -> Tti {
        let (mut start, mut end) = self.transitions(year);
        if (fold == 0) == (self.dst_diff >= 0) {
            end -= self.dst_diff;
        } else {
            start += self.dst_diff;
        }
        let isdst = if start < end {
            start <= ts && ts < end
        } else {
            !(end <= ts && ts < start)
        };
        if isdst { self.dst.clone() } else { self.std.clone() }
    }

    fn at_utc(&self, ts: i64, year: i64) -> (Tti, bool) {
        let (mut start, mut end) = self.transitions(year);
        start -= self.std.utcoff;
        end -= self.dst.utcoff;
        let isdst = if start < end {
            start <= ts && ts < end
        } else {
            !(end <= ts && ts < start)
        };
        let (ambig_start, ambig_end) = if self.dst_diff > 0 {
            (end, end + self.dst_diff)
        } else {
            (start, start - self.dst_diff)
        };
        let fold = ambig_start <= ts && ts < ambig_end;
        (if isdst { self.dst.clone() } else { self.std.clone() }, fold)
    }
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Days between 1970-01-01 and YEAR-01-01.
fn days_before_year(year: i64) -> i64 {
    let y = year - 1;
    y * 365 + y / 4 - y / 100 + y / 400 - 719_162
}

const DAYS_BEFORE_MONTH: [i64; 13] = [-1, 0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];

impl DayRule {
    fn year_to_epoch(&self, year: i64) -> i64 {
        match self {
            DayRule::Day { d, julian, time } => {
                let mut d = *d;
                if *julian && d >= 59 && is_leap(year) {
                    d += 1;
                }
                (days_before_year(year) + d) * 86400 + time
            }
            DayRule::Cal { m, w, d, time } => {
                // The first weekday of the month, 0 = Monday as the
                // calendar counts it, and how long the month is.
                let first_ord = days_before_year(year) + DAYS_BEFORE_MONTH[*m as usize]
                    + i64::from(*m > 2 && is_leap(year))
                    + 1;
                // 1970-01-01 was a Thursday: ordinal 0 there, and
                // Monday is 0, so the weekday is (ord + 3) mod 7.
                let first_day = (first_ord + 3).rem_euclid(7);
                let days_in_month = month_len(year, *m);
                let mut month_day = (d - (first_day + 1)).rem_euclid(7) + 1;
                month_day += (w - 1) * 7;
                if month_day > days_in_month {
                    month_day -= 7;
                }
                let ordinal = days_before_year(year)
                    + DAYS_BEFORE_MONTH[*m as usize]
                    + i64::from(*m > 2 && is_leap(year))
                    + month_day;
                ordinal * 86400 + time
            }
        }
    }
}

fn month_len(year: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if is_leap(year) {
                29
            } else {
                28
            }
        }
    }
}

/// `std offset[dst[offset][,start[/time],end[/time]]]`, the POSIX TZ
/// string. Parsed by hand rather than by a regular expression, because
/// this crate's regular expressions are CPython's own bytecode and a
/// zone file is read before any of that exists.
fn parse_tz_str(s: &str) -> Result<ParsedTz, String> {
    let (offsets, rules) = match s.find(',') {
        Some(i) => (&s[..i], Some(&s[i + 1..])),
        None => (s, None),
    };
    let bad = || format!("{s} is not a valid TZ string");
    let mut at = 0usize;
    let std_abbr = take_abbr(offsets, &mut at).ok_or_else(bad)?;
    let std_offset = match take_delta(offsets, &mut at) {
        Some(v) => v,
        None => 0,
    };
    let dst_abbr = take_abbr(offsets, &mut at);
    let mut out_std = Tti {
        utcoff: std_offset,
        dstoff: 0,
        name: std_abbr,
    };
    let Some(dst_abbr) = dst_abbr else {
        if rules.is_some() {
            return Err(format!("Transition rule present without DST: {s}"));
        }
        if at != offsets.len() {
            return Err(bad());
        }
        out_std.dstoff = 0;
        return Ok(ParsedTz::Fixed(out_std));
    };
    let dst_offset = take_delta(offsets, &mut at).unwrap_or(std_offset + 3600);
    if at != offsets.len() {
        return Err(bad());
    }
    let Some(rules) = rules else {
        return Err(format!("Missing transition rules: {s}"));
    };
    let (a, b) = rules.split_once(',').ok_or_else(bad)?;
    let start = parse_day_rule(a).ok_or_else(bad)?;
    let end = parse_day_rule(b).ok_or_else(bad)?;
    let dst_diff = dst_offset - std_offset;
    Ok(ParsedTz::Rule(TzStr {
        std: out_std,
        dst: Tti {
            utcoff: dst_offset,
            dstoff: dst_diff,
            name: dst_abbr,
        },
        dst_diff,
        start,
        end,
    }))
}

/// `JST`, or `<+09>` — the bracketed form is what a zone with a
/// numeric abbreviation uses.
fn take_abbr(s: &str, at: &mut usize) -> Option<String> {
    let b = s.as_bytes();
    if *at >= b.len() {
        return None;
    }
    if b[*at] == b'<' {
        let end = s[*at..].find('>')? + *at;
        let out = s[*at + 1..end].to_string();
        *at = end + 1;
        return Some(out);
    }
    let start = *at;
    while *at < b.len() && !b[*at].is_ascii_digit() && !matches!(b[*at], b'+' | b'-' | b',' | b':' | b'.') {
        *at += 1;
    }
    if *at == start {
        return None;
    }
    Some(s[start..*at].to_string())
}

/// `[+|-]hh[:mm[:ss]]`, in seconds, with POSIX's sign convention
/// already flipped to the one an offset from UTC uses.
fn take_delta(s: &str, at: &mut usize) -> Option<i64> {
    let (v, used) = read_hms(&s[*at..])?;
    *at += used;
    // POSIX writes the offset to ADD to local time to get UTC.
    Some(-v)
}

fn read_hms(s: &str) -> Option<(i64, usize)> {
    let b = s.as_bytes();
    let mut i = 0usize;
    let mut sign = 1i64;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        if b[i] == b'-' {
            sign = -1;
        }
        i += 1;
    }
    let start = i;
    while i < b.len() && b[i].is_ascii_digit() && i - start < 3 {
        i += 1;
    }
    if i == start {
        return None;
    }
    let h: i64 = s[start..i].parse().ok()?;
    let mut m = 0i64;
    let mut sec = 0i64;
    if i + 2 < b.len() && b[i] == b':' && b[i + 1].is_ascii_digit() && b[i + 2].is_ascii_digit() {
        m = s[i + 1..i + 3].parse().ok()?;
        i += 3;
        if i + 2 < b.len() && b[i] == b':' && b[i + 1].is_ascii_digit() && b[i + 2].is_ascii_digit() {
            sec = s[i + 1..i + 3].parse().ok()?;
            i += 3;
        }
    }
    Some((sign * (h * 3600 + m * 60 + sec), i))
}

fn parse_day_rule(s: &str) -> Option<DayRule> {
    let (date, time) = match s.split_once('/') {
        Some((d, t)) => (d, read_hms(t).map(|(v, _)| v)?),
        None => (s, 2 * 3600),
    };
    if let Some(rest) = date.strip_prefix('M') {
        let mut parts = rest.split('.');
        let m: i64 = parts.next()?.parse().ok()?;
        let w: i64 = parts.next()?.parse().ok()?;
        let d: i64 = parts.next()?.parse().ok()?;
        if parts.next().is_some() || !(1..=12).contains(&m) || !(1..=5).contains(&w) || !(0..=6).contains(&d) {
            return None;
        }
        return Some(DayRule::Cal { m, w, d, time });
    }
    let (digits, julian) = match date.strip_prefix('J') {
        Some(rest) => (rest, true),
        None => (date, false),
    };
    let d: i64 = digits.parse().ok()?;
    Some(DayRule::Day { d, julian, time })
}

// ---- finding the file ----------------------------------------------

/// Where CPython looks: `PYTHONTZPATH` when it is set, and otherwise
/// the directories its own default names. A `tzdata` package is
/// reached through `PYTHONPATH`, which is where an environment that
/// installed one puts it.
fn search_path() -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    match std::env::var("PYTHONTZPATH") {
        Ok(v) if !v.is_empty() => {
            for part in v.split(':') {
                if !part.is_empty() {
                    out.push(std::path::PathBuf::from(part));
                }
            }
        }
        _ => {
            for d in [
                "/usr/share/zoneinfo",
                "/usr/lib/zoneinfo",
                "/usr/share/lib/zoneinfo",
                "/etc/zoneinfo",
            ] {
                out.push(std::path::PathBuf::from(d));
            }
        }
    }
    if let Ok(v) = std::env::var("PYTHONPATH") {
        for part in v.split(':') {
            if !part.is_empty() {
                out.push(std::path::Path::new(part).join("tzdata").join("zoneinfo"));
            }
        }
    }
    out
}

/// A key names a file under a zone directory, and nothing above one:
/// CPython refuses an absolute key and a `..` on the way.
fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && !key.starts_with('/')
        && !key.starts_with('\\')
        && key.split(['/', '\\']).all(|p| !p.is_empty() && p != "." && p != "..")
}

fn read_zone(key: &str) -> Result<Zone, String> {
    // CPython's two messages: a key that could reach outside a zone
    // directory is a ValueError, and one that simply is not there is
    // a KeyError — whose text carries the quotes a KeyError prints.
    let missing = || format!("'No time zone found with key {key}'");
    if !valid_key(key) {
        return Err(format!(
            "ZoneInfo keys must refer to subdirectories of TZPATH, got: {key}"
        ));
    }
    for dir in search_path() {
        let path = dir.join(key);
        if let Ok(bytes) = std::fs::read(&path) {
            return Zone::parse(&bytes);
        }
    }
    Err(missing())
}

/// One read per key per process. A zone file is a few kilobytes and an
/// app asks for the same zone on every frame.
pub fn zone(key: &str) -> Result<std::sync::Arc<Zone>, String> {
    static CACHE: OnceLock<Mutex<HashMap<String, Result<std::sync::Arc<Zone>, String>>>> =
        OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = cache.lock().unwrap_or_else(|e| e.into_inner());
    map.entry(key.to_string())
        .or_insert_with(|| read_zone(key).map(std::sync::Arc::new))
        .clone()
}
