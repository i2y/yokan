//! The embedded-Python headless test: build a small app in real
//! CPython, drive it through the shared kernel script harness, and
//! assert on the dumped element tree. This is yokan's seat in the
//! workspace gate — no window, no display, CI-safe.

use yokan::yokan;
use pyo3::prelude::*;

#[test]
fn scripted_counter_and_input() {
    pyo3::append_to_inittab!(yokan);
    Python::initialize();
    Python::attach(|py| {
        let code = cr#"
import yokan as ui

s = {"n": 0, "q": ""}

def view(st):
    return ui.column(
        ui.text(f"count: {st['n']}"),
        ui.button("+1", on_click=lambda: st.update(n=st["n"] + 1)),
        ui.text_field(st["q"], on_change=lambda t: st.update(q=t)),
        ui.text(f"typed: {st['q']}"),
    )

out = ui._headless(view, s, "click:+1,click:+1,input:hello")
"#;
        let m = pyo3::types::PyModule::from_code(py, code, c"headless_t.py", c"headless_t")
            .expect("python module ran");
        let out: String = m
            .getattr("out")
            .expect("out binding")
            .extract()
            .expect("string dump");
        assert!(out.contains("count: 2"), "dump was:\n{out}");
        assert!(out.contains("typed: hello"), "dump was:\n{out}");
        // The state object was mutated through real listeners.
        let n: i64 = m
            .getattr("s")
            .unwrap()
            .get_item("n")
            .unwrap()
            .extract()
            .unwrap();
        assert_eq!(n, 2);

        // ui.task: work() runs on a worker thread (GIL released by the
        // harness), on_done lands on the UI thread, and the settle
        // loop waits for the completion deterministically.
        let code2 = cr#"
import yokan as ui

s2 = {"r": "pending"}

def view2(st):
    return ui.column(
        ui.text(f"result: {st['r']}"),
        ui.button(
            "go",
            on_click=lambda: ui.task(lambda: 40 + 2, on_done=lambda v: s2.update(r=v)),
        ),
    )

out2 = ui._headless(view2, s2, "click:go")
"#;
        let m2 = pyo3::types::PyModule::from_code(py, code2, c"headless_t2.py", c"headless_t2")
            .expect("task module ran");
        let out2: String = m2.getattr("out2").unwrap().extract().unwrap();
        assert!(out2.contains("result: 42"), "dump was:\n{out2}");

        // with-style declarative building, mixing allowed both ways:
        // elements created in a `with` auto-append; explicit placement
        // as a child argument steals them out of the frame.
        let code3 = cr##"
import yokan as ui

s3 = {"n": 5}

def view3(st):
    with ui.column(spacing=4):
        ui.text(f"n={st['n']}")
        with ui.row():
            ui.button("+1", on_click=lambda: s3.update(n=st["n"] + 1))
            ui.text("inner", color="#888")
        ui.row(ui.text("functional child"))

out3 = ui._headless(view3, s3, "click:+1")
"##;
        let m3 = pyo3::types::PyModule::from_code(py, code3, c"headless_t3.py", c"headless_t3")
            .expect("with module ran");
        let out3: String = m3.getattr("out3").unwrap().extract().unwrap();
        assert!(out3.contains("n=6"), "dump was:\n{out3}");
        assert!(out3.contains("functional child"), "dump was:\n{out3}");
        assert!(
            out3.contains("Row[Button(+1), Text(inner, color=#888)]"),
            "dump was:\n{out3}"
        );

        // Typed State cells: zero-arg view, `count()` reads,
        // `.set` writes — including a bound method as a handler.
        let code4 = cr#"
import yokan as ui

count = ui.State(3)
label = ui.State("")

def view4():
    with ui.column():
        ui.text(f"c={count()}")
        ui.button("+1", on_click=lambda: count.set(count() + 1))
        ui.text_field(label(), on_change=label.set)
        ui.text(f"L={label()}")

out4 = ui._headless(view4, None, "click:+1,input:xyz")
"#;
        let m4 = pyo3::types::PyModule::from_code(py, code4, c"headless_t4.py", c"headless_t4")
            .expect("state module ran");
        let out4: String = m4.getattr("out4").unwrap().extract().unwrap();
        assert!(out4.contains("c=4"), "dump was:\n{out4}");
        assert!(out4.contains("L=xyz"), "dump was:\n{out4}");

        // @ui.component + ui.local: positional per-instance state —
        // two call sites, independent counters.
        let code5 = cr#"
import yokan as ui

@ui.component
def bump(step: int):
    n: ui.State[int] = ui.local(0)
    with ui.row():
        ui.text(f"v{step}={n()}")
        ui.button(f"+{step}", on_click=lambda: n.set(n() + step))

def view5():
    with ui.column():
        bump(1)
        bump(5)

out5 = ui._headless(view5, None, "click:+1,click:+5,click:+1")
"#;
        let m5 = pyo3::types::PyModule::from_code(py, code5, c"headless_t5.py", c"headless_t5")
            .expect("component module ran");
        let out5: String = m5.getattr("out5").unwrap().extract().unwrap();
        assert!(out5.contains("v1=2"), "dump was:\n{out5}");
        assert!(out5.contains("v5=5"), "dump was:\n{out5}");
    });
}
