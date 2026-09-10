// Generated from crates/yokan-stdlib/stdlib.toml by rakugan/tools/gen_capi.pl.
// Do not edit by hand; edit the manifest and run the generator.
//! The framework's own standard library, reachable from a door written
//! in another language.
//!
//! `yokan-stdlib` is one implementation that a translated language's
//! COMPILED run links through pixie's binding door. Its interpreted run
//! has to land on the same code or the gate compares two libraries, so
//! this is the same functions through the C face.
//!
//! The convention is generic, the way the element builder's is: the
//! caller pushes the arguments, names the row by number, and reads the
//! answer back. Adding a function to the manifest therefore adds an arm
//! here and nothing to the ABI. The one thing the ABI does say about a
//! row is how its failure comes back: see the two `pixie_std_call`s.

use std::cell::{Cell, RefCell};
use std::ffi::{CStr, c_char};

enum Arg {
    S(String),
    I(i64),
    N(f64),
    L(Vec<String>),
}

thread_local! {
    static ARGS: RefCell<Vec<Arg>> = const { RefCell::new(Vec::new()) };
    static BUILDING: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
    /// What the last call answered, when it answered text: one row of
    /// one cell for a string, one row per element for a list, and the
    /// rows themselves for a query.
    static ROWS: RefCell<Vec<Vec<String>>> = const { RefCell::new(Vec::new()) };
    static NUM: Cell<f64> = const { Cell::new(0.0) };
    /// Whether the last checked call failed. Its message is the
    /// answer text, so the door reads it the way it reads any string.
    static FAILED: Cell<bool> = const { Cell::new(false) };
}

/// # Safety
/// `v` is NULL or NUL-terminated.
unsafe fn text(v: *const c_char) -> String {
    if v.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(v) }.to_string_lossy().into_owned()
}

fn push(a: Arg) {
    BUILDING.with(|b| {
        let mut b = b.borrow_mut();
        match (&mut *b, a) {
            (Some(list), Arg::S(s)) => list.push(s),
            (_, a) => ARGS.with(|args| args.borrow_mut().push(a)),
        }
    });
}

/// Start a call: the arguments pushed after this one are its own.
#[unsafe(no_mangle)]
pub extern "C" fn pixie_std_reset() {
    ARGS.with(|a| a.borrow_mut().clear());
    BUILDING.with(|b| *b.borrow_mut() = None);
    ROWS.with(|r| r.borrow_mut().clear());
}

/// # Safety
/// `v` is NULL or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pixie_std_arg_str(v: *const c_char) {
    push(Arg::S(unsafe { text(v) }));
}

#[unsafe(no_mangle)]
pub extern "C" fn pixie_std_arg_int(v: i64) {
    push(Arg::I(v));
}

#[unsafe(no_mangle)]
pub extern "C" fn pixie_std_arg_num(v: f64) {
    push(Arg::N(v));
}

/// The strings pushed until the matching end are one list argument.
#[unsafe(no_mangle)]
pub extern "C" fn pixie_std_arg_list_begin() {
    BUILDING.with(|b| *b.borrow_mut() = Some(Vec::new()));
}

#[unsafe(no_mangle)]
pub extern "C" fn pixie_std_arg_list_end() {
    let list = BUILDING.with(|b| b.borrow_mut().take()).unwrap_or_default();
    ARGS.with(|a| a.borrow_mut().push(Arg::L(list)));
}

/// How many rows the last call's answer holds.
#[unsafe(no_mangle)]
pub extern "C" fn pixie_std_rows() -> i64 {
    ROWS.with(|r| r.borrow().len() as i64)
}

/// How many cells one of those rows holds.
#[unsafe(no_mangle)]
pub extern "C" fn pixie_std_cells(row: i64) -> i64 {
    ROWS.with(|r| r.borrow().get(row as usize).map_or(0, |c| c.len() as i64))
}

/// Put one cell where the character reader can pick it up.
#[unsafe(no_mangle)]
pub extern "C" fn pixie_std_pick(row: i64, col: i64) {
    let cell = ROWS.with(|r| {
        r.borrow()
            .get(row as usize)
            .and_then(|c| c.get(col as usize))
            .cloned()
            .unwrap_or_default()
    });
    crate::answer_with(&cell);
}

/// What the last call answered when it answered a number with a
/// fraction.
#[unsafe(no_mangle)]
pub extern "C" fn pixie_std_answer_num() -> f64 {
    NUM.with(|n| n.get())
}

fn s(args: &[Arg], i: usize) -> &str {
    match args.get(i) {
        Some(Arg::S(v)) => v,
        _ => panic!("the standard library was given the wrong shape of argument {i}"),
    }
}

fn i(args: &[Arg], i: usize) -> i64 {
    match args.get(i) {
        Some(Arg::I(v)) => *v,
        _ => panic!("the standard library was given the wrong shape of argument {i}"),
    }
}

fn n(args: &[Arg], i: usize) -> f64 {
    match args.get(i) {
        Some(Arg::N(v)) => *v,
        Some(Arg::I(v)) => *v as f64,
        _ => panic!("the standard library was given the wrong shape of argument {i}"),
    }
}

fn l(args: &[Arg], i: usize) -> Vec<String> {
    match args.get(i) {
        Some(Arg::L(v)) => v.clone(),
        _ => panic!("the standard library was given the wrong shape of argument {i}"),
    }
}

fn text_answer(v: String) -> i64 {
    ROWS.with(|r| *r.borrow_mut() = vec![vec![v]]);
    0
}

fn list_answer(v: Vec<String>) -> i64 {
    ROWS.with(|r| *r.borrow_mut() = v.into_iter().map(|s| vec![s]).collect());
    0
}

fn rows_answer(v: Vec<Vec<String>>) -> i64 {
    ROWS.with(|r| *r.borrow_mut() = v);
    0
}

fn num_answer(v: f64) -> i64 {
    NUM.with(|n| n.set(v));
    0
}

/// One row of the manifest, by the number the generator gave it.
///
/// A row that fails panics, and a panic cannot leave a C function: the
/// process ends. That is the library's own rule — the plain form stops
/// the app, the `_or` twin answers a default — and a door whose
/// language has no way to catch it keeps this entry.
#[unsafe(no_mangle)]
pub extern "C" fn pixie_std_call(id: i32) -> i64 {
    let args = ARGS.with(|a| a.take());
    call(id, &args)
}

/// The same row, for a door whose language has a `die` of its own. A
/// failure comes back instead of ending the process: `pixie_std_failed`
/// says so, the message is the answer text, and the door raises it in
/// its own words at the line the app wrote — which is what lets a
/// `try` in that language catch what the compiled run's `try` catches.
#[unsafe(no_mangle)]
pub extern "C" fn pixie_std_call_checked(id: i32) -> i64 {
    let args = ARGS.with(|a| a.take());
    FAILED.with(|f| f.set(false));
    match pixie_kernel::contain_quiet(|| call(id, &args)) {
        Ok(v) => v,
        Err(msg) => {
            ROWS.with(|r| *r.borrow_mut() = vec![vec![msg]]);
            FAILED.with(|f| f.set(true));
            0
        }
    }
}

/// Whether the last `pixie_std_call_checked` failed.
#[unsafe(no_mangle)]
pub extern "C" fn pixie_std_failed() -> i32 {
    FAILED.with(|f| f.get()) as i32
}

fn call(id: i32, args: &[Arg]) -> i64 {
    match id {
        1 => text_answer(yokan_stdlib::fs_read_text(s(args, 0))),  // fs.read_text
        2 => yokan_stdlib::fs_write_text(s(args, 0), s(args, 1)),  // fs.write_text
        3 => (yokan_stdlib::fs_exists(s(args, 0))) as i64,  // fs.exists
        4 => text_answer(yokan_stdlib::fs_read_text_or(s(args, 0), s(args, 1))),  // fs.read_text_or
        5 => list_answer(yokan_stdlib::fs_list_dir(s(args, 0))),  // fs.list_dir
        6 => yokan_stdlib::fs_append_text(s(args, 0), s(args, 1)),  // fs.append_text
        7 => yokan_stdlib::fs_remove(s(args, 0)),  // fs.remove
        8 => yokan_stdlib::fs_make_dir(s(args, 0)),  // fs.make_dir
        9 => text_answer(yokan_stdlib::fs_app_dir(s(args, 0))),  // fs.app_dir
        10 => text_answer(yokan_stdlib::fs_open_dialog(s(args, 0))),  // fs.open_dialog
        11 => text_answer(yokan_stdlib::fs_save_dialog(s(args, 0))),  // fs.save_dialog
        12 => yokan_stdlib::sqlite_exec(s(args, 0), s(args, 1)),  // sqlite.exec
        13 => list_answer(yokan_stdlib::sqlite_query_text(s(args, 0), s(args, 1))),  // sqlite.query_text
        14 => yokan_stdlib::sqlite_query_int(s(args, 0), s(args, 1)),  // sqlite.query_int
        15 => yokan_stdlib::sqlite_query_int_or(s(args, 0), s(args, 1), i(args, 2)),  // sqlite.query_int_or
        16 => list_answer(yokan_stdlib::sqlite_query_text_or(s(args, 0), s(args, 1))),  // sqlite.query_text_or
        17 => yokan_stdlib::sqlite_exec_with(s(args, 0), s(args, 1), l(args, 2)),  // sqlite.exec
        18 => list_answer(yokan_stdlib::sqlite_query_text_with(s(args, 0), s(args, 1), l(args, 2))),  // sqlite.query_text
        19 => yokan_stdlib::sqlite_query_int_with(s(args, 0), s(args, 1), l(args, 2)),  // sqlite.query_int
        20 => yokan_stdlib::sqlite_query_int_or_with(s(args, 0), s(args, 1), i(args, 2), l(args, 3)),  // sqlite.query_int_or
        21 => list_answer(yokan_stdlib::sqlite_query_text_or_with(s(args, 0), s(args, 1), l(args, 2))),  // sqlite.query_text_or
        22 => rows_answer(yokan_stdlib::sqlite_query_rows_all(s(args, 0), s(args, 1))),  // sqlite.query_rows
        23 => rows_answer(yokan_stdlib::sqlite_query_rows(s(args, 0), s(args, 1), l(args, 2))),  // sqlite.query_rows
        24 => rows_answer(yokan_stdlib::sqlite_query_rows_or_all(s(args, 0), s(args, 1))),  // sqlite.query_rows_or
        25 => rows_answer(yokan_stdlib::sqlite_query_rows_or(s(args, 0), s(args, 1), l(args, 2))),  // sqlite.query_rows_or
        26 => yokan_stdlib::clipboard_set_text(s(args, 0)),  // clipboard.set_text
        27 => text_answer(yokan_stdlib::clipboard_get_text()),  // clipboard.get_text
        28 => (yokan_stdlib::keys_down(s(args, 0))) as i64,  // keys.down
        29 => (yokan_stdlib::keys_pressed(s(args, 0))) as i64,  // keys.pressed
        30 => (yokan_stdlib::keys_released(s(args, 0))) as i64,  // keys.released
        31 => yokan_stdlib::audio_play(s(args, 0)),  // audio.play
        32 => yokan_stdlib::audio_play_at(s(args, 0), n(args, 1)),  // audio.play
        33 => yokan_stdlib::audio_stop(),  // audio.stop
        34 => { yokan_stdlib::notify_send(s(args, 0), s(args, 1)); 0 },  // notify.send
        35 => text_answer(yokan_stdlib::http_get_text(s(args, 0))),  // http.get_text
        36 => text_answer(yokan_stdlib::http_get_text_or(s(args, 0), s(args, 1))),  // http.get_text_or
        37 => text_answer(yokan_stdlib::http_get_text_timeout(s(args, 0), i(args, 1))),  // http.get_text
        38 => text_answer(yokan_stdlib::http_post_text(s(args, 0), s(args, 1))),  // http.post_text
        39 => text_answer(yokan_stdlib::http_post_text_as(s(args, 0), s(args, 1), s(args, 2))),  // http.post_text
        40 => text_answer(yokan_stdlib::http_post_text_or(s(args, 0), s(args, 1), s(args, 2))),  // http.post_text_or
        41 => yokan_stdlib::http_status(s(args, 0)),  // http.status
        42 => text_answer(yokan_stdlib::json_get_text(s(args, 0), s(args, 1))),  // jsondoc.get_text
        43 => yokan_stdlib::json_get_int(s(args, 0), s(args, 1)),  // jsondoc.get_int
        44 => num_answer(yokan_stdlib::json_get_float(s(args, 0), s(args, 1))),  // jsondoc.get_float
        45 => (yokan_stdlib::json_get_bool(s(args, 0), s(args, 1))) as i64,  // jsondoc.get_bool
        46 => yokan_stdlib::json_length(s(args, 0), s(args, 1)),  // jsondoc.length
        47 => (yokan_stdlib::json_has(s(args, 0), s(args, 1))) as i64,  // jsondoc.has
        48 => yokan_stdlib::strings_to_int(s(args, 0), i(args, 1)),  // strings.to_int
        49 => num_answer(yokan_stdlib::strings_to_float(s(args, 0), n(args, 1))),  // strings.to_float
        50 => text_answer(yokan_stdlib::clock_format_ms(i(args, 0), s(args, 1))),  // clock.format_ms
        51 => text_answer(yokan_stdlib::clock_format_local_ms(i(args, 0), s(args, 1))),  // clock.format_local_ms
        52 => yokan_stdlib::clock_local_offset_minutes(i(args, 0)),  // clock.local_offset_minutes
        other => panic!("the standard library has no row {other}"),
    }
}
