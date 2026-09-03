//! yokan: pixie's kernel + gpui engine, exposed to real CPython.
//!
//! The shape mirrors what pixie's own generated code does: a Component
//! whose `build` returns an Element tree — except `build` calls a
//! Python function, and listeners call Python closures. State lives in
//! ordinary Python objects; every event marks the view dirty and the
//! engine rebuilds (the Streamlit mental model, in-process and
//! state-preserving). ReloadWatch re-execs the source file on change
//! and swaps the view function while the Python state object survives.
//!
//! Design sheet: DESIGN.md beside this file. The standing constraint
//! (the standing discipline): every API here must stay
//! mechanically translatable to `.pix` — the app dialect is an honest
//! Python subset because CPython rides as the checked second tier.

use pixie_engine_gpui::{ReloadWatch, run_app};
use pixie_kernel::{
    AsyncCtx, Component, Element, ErasedHandle, LazyRows, List, Listener, Runtime, Str,
    BoolListener, FloatListener, IntListener, TextListener, World, mount,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule, PyTuple};
use std::cell::{Cell, RefCell};
use std::ffi::CString;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

static T0: OnceLock<Instant> = OnceLock::new();
static RUNNING: AtomicBool = AtomicBool::new(false);
static FIRST_BUILD_PRINTED: AtomicBool = AtomicBool::new(false);

thread_local! {
    static CURRENT_VIEW: Cell<Option<ErasedHandle>> = const { Cell::new(None) };
    static CURRENT_CTX: RefCell<Option<AsyncCtx>> = const { RefCell::new(None) };
    static PENDING_SPAWNS: RefCell<Vec<TaskSpec>> = const { RefCell::new(Vec::new()) };
}

use std::collections::HashMap as StdHashMap;

thread_local! {
    /// Positional identity for per-instance component state: the path
    /// of call indices from the view root (rows push their row index),
    /// the counter of components seen at the current level, and the
    /// slot cursor inside the current component. The same discipline
    /// as pixie's RowSeat/§8.30 keying — index-keyed, grow-only.
    static COMP_PATH: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
    static COMP_COUNTER: Cell<usize> = const { Cell::new(0) };
    static LOCAL_IX: Cell<usize> = const { Cell::new(0) };
    static LOCAL_SLOTS: RefCell<StdHashMap<Vec<usize>, Vec<Py<PyState>>>> =
        RefCell::new(StdHashMap::new());
}

fn reset_build_identity() {
    COMP_PATH.with(|p| p.borrow_mut().clear());
    COMP_COUNTER.with(|c| c.set(0));
    LOCAL_IX.with(|c| c.set(0));
}

fn reset_app_identity() {
    reset_build_identity();
    LOCAL_SLOTS.with(|m| m.borrow_mut().clear());
}

/// `@ui.component`: wraps a view helper so each CALL SITE gets a
/// stable positional identity, which is what `ui.local` keys its
/// per-instance state by. Reordering calls reassigns state (the
/// no-key rule); state survives rebuilds and live reloads.
#[pyclass(unsendable, name = "Component")]
struct PyComponent {
    f: Py<PyAny>,
    slots: bool,
}

/// A slotted component USE: `with card("x"):` — enter collects the
/// children, exit runs the body with them parked for `ui.slot()`.
/// The call-site identity (`ix`) was claimed at the call, so per-
/// instance `ui.local` state keys exactly as a plain call would.
#[pyclass(unsendable)]
struct PyComponentUse {
    f: Py<PyAny>,
    args: Py<PyTuple>,
    kwargs: Option<Py<PyDict>>,
    ix: usize,
}

#[pymethods]
impl PyComponentUse {
    fn __enter__(slf: &Bound<'_, PyComponentUse>) -> Py<PyComponentUse> {
        BUILD_FRAMES.with(|f| f.borrow_mut().push(Vec::new()));
        slf.clone().unbind()
    }

    #[pyo3(signature = (_t=None, _v=None, _tb=None))]
    fn __exit__(
        slf: &Bound<'_, PyComponentUse>,
        _t: Option<&Bound<'_, PyAny>>,
        _v: Option<&Bound<'_, PyAny>>,
        _tb: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        let py = slf.py();
        let frame = BUILD_FRAMES
            .with(|f| f.borrow_mut().pop())
            .unwrap_or_default();
        if _t.is_some() {
            return Ok(false);
        }
        let me = slf.borrow();
        SLOT_CHILDREN.with(|s| *s.borrow_mut() = Some(frame));
        COMP_PATH.with(|p| p.borrow_mut().push(me.ix));
        let saved_counter = COMP_COUNTER.with(|c| {
            let v = c.get();
            c.set(0);
            v
        });
        let saved_local = LOCAL_IX.with(|c| {
            let v = c.get();
            c.set(0);
            v
        });
        let r = me
            .f
            .call(py, me.args.bind(py), me.kwargs.as_ref().map(|k| k.bind(py)));
        COMP_PATH.with(|p| {
            p.borrow_mut().pop();
        });
        COMP_COUNTER.with(|c| c.set(saved_counter));
        LOCAL_IX.with(|c| c.set(saved_local));
        let leftover = SLOT_CHILDREN.with(|s| s.borrow_mut().take());
        r?;
        if leftover.is_some() {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(
                "this component was given children but its body never calls ui.slot()",
            ));
        }
        Ok(false)
    }
}

/// `ui.slot()`: place the children a `with component(...):` block
/// collected, at this point of the body.
#[pyfunction]
fn slot() -> PyResult<()> {
    let kids = SLOT_CHILDREN.with(|s| s.borrow_mut().take());
    let Some(kids) = kids else {
        return Err(pyo3::exceptions::PyRuntimeError::new_err(
            "ui.slot(): no children here — call this component as `with comp(...):` \
             (and declare it @ui.component(slots=True))",
        ));
    };
    BUILD_FRAMES.with(|f| {
        if let Some(frame) = f.borrow_mut().last_mut() {
            frame.extend(kids);
        }
    });
    Ok(())
}

#[pymethods]
impl PyComponent {
    #[pyo3(signature = (*args, **kwargs))]
    fn __call__(
        &self,
        py: Python<'_>,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        let my_ix = COMP_COUNTER.with(|c| c.get());
        COMP_COUNTER.with(|c| c.set(my_ix + 1));
        if self.slots {
            // Defer: the children arrive through the `with` block.
            let use_ = PyComponentUse {
                f: self.f.clone_ref(py),
                args: args.clone().unbind(),
                kwargs: kwargs.map(|k| k.clone().unbind()),
                ix: my_ix,
            };
            return Ok(Py::new(py, use_)?.into_any());
        }
        COMP_PATH.with(|p| p.borrow_mut().push(my_ix));
        COMP_COUNTER.with(|c| c.set(0));
        let saved_local = LOCAL_IX.with(|c| {
            let v = c.get();
            c.set(0);
            v
        });
        let r = self.f.call(py, args, kwargs);
        COMP_PATH.with(|p| {
            p.borrow_mut().pop();
        });
        COMP_COUNTER.with(|c| c.set(my_ix + 1));
        LOCAL_IX.with(|c| c.set(saved_local));
        r
    }
}

#[pyclass(unsendable)]
struct ComponentMaker {
    slots: bool,
}

#[pymethods]
impl ComponentMaker {
    fn __call__(&self, py: Python<'_>, f: Py<PyAny>) -> PyResult<Py<PyAny>> {
        Ok(Py::new(py, PyComponent { f, slots: self.slots })?.into_any())
    }
}

#[pyfunction]
#[pyo3(signature = (f=None, *, slots=false))]
fn component_deco(py: Python<'_>, f: Option<Py<PyAny>>, slots: bool) -> PyResult<Py<PyAny>> {
    match f {
        Some(f) => Ok(Py::new(py, PyComponent { f, slots })?.into_any()),
        None => Ok(Py::new(py, ComponentMaker { slots })?.into_any()),
    }
}



/// Per-instance state cell, positionally keyed. Only meaningful
/// inside an `@ui.component` body.
#[pyfunction]
fn local(py: Python<'_>, init: Py<PyAny>) -> PyResult<Py<PyState>> {
    check_native_int(py, &init)?;
    let path = COMP_PATH.with(|p| p.borrow().clone());
    if path.is_empty() {
        return Err(pyo3::exceptions::PyRuntimeError::new_err(
            "ui.local() only works inside an @ui.component body",
        ));
    }
    let ix = LOCAL_IX.with(|c| {
        let v = c.get();
        c.set(v + 1);
        v
    });
    LOCAL_SLOTS.with(|m| {
        let mut m = m.borrow_mut();
        let slots = m.entry(path).or_default();
        if ix < slots.len() {
            Ok(slots[ix].clone_ref(py))
        } else {
            let st = Py::new(py, PyState { v: RefCell::new(init) })?;
            slots.push(st.clone_ref(py));
            Ok(st)
        }
    })
}

struct TaskSpec {
    work: Py<PyAny>,
    on_done: Option<Py<PyAny>>,
    on_error: Option<Py<PyAny>>,
}

/// Adopt queued `ui.task` calls into the World's task queue. Called
/// wherever `&mut World` is naturally in hand right after Python ran
/// (listeners, timer ticks, task completions, run() startup) — so
/// `ui.task` itself never needs the World and can be called from any
/// callback without a double borrow.
fn drain_spawns(w: &mut World) {
    let specs = PENDING_SPAWNS.with(|t| t.take());
    for spec in specs {
        let (handle, completion) =
            pixie_kernel::completion::<Result<Py<PyAny>, Py<PyAny>>>();
        let work = spec.work;
        pixie_kernel::spawn_worker(move || {
            let r = Python::attach(|py| {
                work.call0(py)
                    .map_err(|e| e.into_value(py).into_any())
            });
            handle.complete(r);
        });
        let on_done = spec.on_done;
        let on_error = spec.on_error;
        w.spawn(async move {
            let r = completion.await;
            Python::attach(|py| match r {
                Ok(v) => {
                    if let Some(f) = &on_done {
                        if let Err(e) = f.call1(py, (v,)) {
                            e.print(py);
                        }
                    }
                }
                Err(exc) => {
                    if let Some(f) = &on_error {
                        if let Err(e) = f.call1(py, (exc,)) {
                            e.print(py);
                        }
                    } else {
                        let repr = exc
                            .bind(py)
                            .repr()
                            .map(|r| r.to_string())
                            .unwrap_or_else(|_| "<unprintable>".into());
                        eprintln!("[yokan] task failed: {repr}");
                    }
                }
            });
            // We are inside Runtime::turn — no World borrow is held,
            // so reach it through the stored ctx for the reactive tail.
            let ctx = CURRENT_CTX.with(|c| c.borrow().clone());
            if let Some(ctx) = ctx {
                ctx.with(after_py_callback);
            }
        });
    }
}

/// The reactive tail after any Python callback: adopt queued tasks,
/// then mark the mounted view dirty so the engine rebuilds.
fn after_py_callback(w: &mut World) {
    drain_spawns(w);
    if let Some(hv) = CURRENT_VIEW.with(|c| c.get()) {
        w.mark_view_dirty(hv);
    }
}

// ---------------------------------------------------------------------------
// Elements as opaque Python objects (consumed once when placed in a parent).

#[pyclass(unsendable)]
struct PyElement {
    el: RefCell<Option<Element>>,
}

impl PyElement {
    fn wrap(el: Element) -> Self {
        PyElement { el: RefCell::new(Some(el)) }
    }
}

#[pymethods]
impl PyElement {
    fn __enter__(slf: &Bound<'_, PyElement>) -> PyResult<Py<PyElement>> {
        {
            let pe = slf.borrow();
            let el = pe.el.borrow();
            match el.as_ref() {
                None => {
                    return Err(pyo3::exceptions::PyValueError::new_err(
                        "element already used",
                    ));
                }
                Some(e) => {
                    let mut probe = e.clone();
                    if set_children(&mut probe, Vec::new()).is_err() {
                        return Err(pyo3::exceptions::PyTypeError::new_err(
                            "only containers (column/row/grid/stack/scroll_view/h_scroll_view/data_table/modal) work as `with` blocks",
                        ));
                    }
                }
            }
        }
        BUILD_FRAMES.with(|f| f.borrow_mut().push(Vec::new()));
        Ok(slf.clone().unbind())
    }

    #[pyo3(signature = (_t=None, _v=None, _tb=None))]
    fn __exit__(
        slf: &Bound<'_, PyElement>,
        _t: Option<&Bound<'_, PyAny>>,
        _v: Option<&Bound<'_, PyAny>>,
        _tb: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        let py = slf.py();
        let frame = BUILD_FRAMES
            .with(|f| f.borrow_mut().pop())
            .unwrap_or_default();
        if _t.is_some() {
            return Ok(false); // exception: unwind, keep frames balanced
        }
        let kids: Vec<Element> = frame
            .into_iter()
            .filter_map(|pe| pe.bind(py).borrow().el.borrow_mut().take())
            .collect();
        {
            let pe = slf.borrow();
            let mut el = pe.el.borrow_mut();
            let Some(e) = el.as_mut() else {
                return Err(pyo3::exceptions::PyValueError::new_err("element already used"));
            };
            set_children(e, kids)
                .map_err(pyo3::exceptions::PyTypeError::new_err)?;
        }
        // The completed container joins the ENCLOSING frame, exactly
        // as if it had just been constructed there.
        BUILD_FRAMES.with(|f| {
            if let Some(outer) = f.borrow_mut().last_mut() {
                outer.push(slf.clone().unbind());
            }
        });
        Ok(false)
    }
}

/// A typed State cell, annotation-first: `count:
/// ui.State[int] = ui.State(0)` — read `count()`, write
/// `count.set(v)`, `+=` sugar. No observer machinery: yokan
/// rebuilds the whole view after every callback, so `set` is just a
/// store. Reload copies values old→new by module-level name.
#[pyclass(unsendable, name = "State")]
struct PyState {
    v: RefCell<Py<PyAny>>,
}

/// The compiled tier's Int is i64: a Python int (or list item)
/// beyond that range is refused BEFORE the write, so both tiers fail
/// the same statement at the same point instead of diverging.
fn check_native_int(py: Python<'_>, v: &Py<PyAny>) -> PyResult<()> {
    let b = v.bind(py);
    if b.is_instance_of::<pyo3::types::PyBool>() {
        return Ok(());
    }
    if b.is_instance_of::<pyo3::types::PyInt>() {
        if b.extract::<i64>().is_err() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "int value exceeds the native 64-bit range (Int is i64 in the compiled tier)",
            ));
        }
    } else if let Ok(list) = b.downcast::<pyo3::types::PyList>() {
        for item in list.iter() {
            if item.is_instance_of::<pyo3::types::PyInt>()
                && !item.is_instance_of::<pyo3::types::PyBool>()
                && item.extract::<i64>().is_err()
            {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "list item exceeds the native 64-bit int range",
                ));
            }
        }
    } else if b.hasattr("__dataclass_fields__")? {
        let fields = b.getattr("__dataclass_fields__")?;
        for (k, _) in fields.downcast::<PyDict>()?.iter() {
            let item = b.getattr(k.downcast::<pyo3::types::PyString>()?)?;
            if item.is_instance_of::<pyo3::types::PyInt>()
                && !item.is_instance_of::<pyo3::types::PyBool>()
                && item.extract::<i64>().is_err()
            {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "dataclass int field exceeds the native 64-bit range",
                ));
            }
        }
    } else if let Ok(d) = b.downcast::<PyDict>() {
        for (_, item) in d.iter() {
            if item.is_instance_of::<pyo3::types::PyInt>()
                && !item.is_instance_of::<pyo3::types::PyBool>()
                && item.extract::<i64>().is_err()
            {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "dict value exceeds the native 64-bit int range",
                ));
            }
        }
    }
    Ok(())
}

#[pymethods]
impl PyState {
    #[new]
    fn new(py: Python<'_>, v: Py<PyAny>) -> PyResult<Self> {
        check_native_int(py, &v)?;
        Ok(PyState { v: RefCell::new(v) })
    }
    fn __call__(&self, py: Python<'_>) -> Py<PyAny> {
        self.v.borrow().clone_ref(py)
    }
    fn value(&self, py: Python<'_>) -> Py<PyAny> {
        self.v.borrow().clone_ref(py)
    }
    fn set(&self, py: Python<'_>, v: Py<PyAny>) -> PyResult<()> {
        check_native_int(py, &v)?;
        *self.v.borrow_mut() = v;
        Ok(())
    }
    fn __str__(&self, py: Python<'_>) -> PyResult<String> {
        self.v.borrow().bind(py).str().map(|s| s.to_string())
    }
    /// `cell[k] = v` — the per-entry write. On a dict cell the key
    /// is a str, on a list cell an index; the write lands IN PLACE on
    /// the held Python object (no copy), the value is range-checked,
    /// and the compiled twin is pixie's `m[k] = v` / `xs[i] = v`.
    fn __setitem__(
        &self,
        py: Python<'_>,
        key: &Bound<'_, PyAny>,
        value: Py<PyAny>,
    ) -> PyResult<()> {
        check_native_int(py, &value)?;
        self.v.borrow().bind(py).set_item(key, value)?;
        Ok(())
    }

    fn __iadd__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<()> {
        let cur = self.v.borrow().clone_ref(py);
        let new = cur.bind(py).add(other)?.unbind();
        check_native_int(py, &new)?;
        *self.v.borrow_mut() = new;
        Ok(())
    }
    fn __isub__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<()> {
        let cur = self.v.borrow().clone_ref(py);
        let new = cur.bind(py).sub(other)?.unbind();
        check_native_int(py, &new)?;
        *self.v.borrow_mut() = new;
        Ok(())
    }
    #[classmethod]
    fn __class_getitem__(
        cls: &Bound<'_, pyo3::types::PyType>,
        _item: &Bound<'_, PyAny>,
    ) -> Py<pyo3::types::PyType> {
        cls.clone().unbind()
    }
}

thread_local! {
    /// Open `with` container frames: elements auto-append to the top
    /// frame at creation; explicit placement as a child argument
    /// consumes them first, so the frame sweep skips them (the
    /// "created appends, placement steals" rule — mixing works).
    static BUILD_FRAMES: RefCell<Vec<Vec<Py<PyElement>>>> = const { RefCell::new(Vec::new()) };
    /// Children collected by a `with component(...):` block, waiting
    /// for the body's `ui.slot()` to place them.
    static SLOT_CHILDREN: RefCell<Option<Vec<Py<PyElement>>>> = const { RefCell::new(None) };
}

/// Constructor return type: converts into a Python `PyElement` and,
/// when a `with` frame is open, registers the object in it.
struct Reg(PyElement);

impl Reg {
    fn wrap(el: Element) -> Self {
        Reg(PyElement::wrap(el))
    }

    /// The same, with the universal `tooltip=` rider applied — an
    /// empty string is no tooltip, so an element that carries none
    /// registers exactly what it did before the rider existed.
    fn tip(tooltip: &str, el: Element) -> Self {
        Reg::wrap(if tooltip.is_empty() {
            el
        } else {
            Element::Tooltip {
                text: Str::from(tooltip),
                children: vec![el],
            }
        })
    }
}

impl<'py> IntoPyObject<'py> for Reg {
    type Target = PyElement;
    type Output = Bound<'py, PyElement>;
    type Error = PyErr;
    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        let b = Bound::new(py, self.0)?;
        BUILD_FRAMES.with(|f| {
            if let Some(frame) = f.borrow_mut().last_mut() {
                frame.push(b.clone().unbind());
            }
        });
        Ok(b)
    }
}

/// The §8.35 riders, tier-A side: wrap the element the way both
/// lowerers do natively — interpolation runs in the shared kernel,
/// so scripted `advance:` sees identical frames in both tiers.
fn wrap_anim(el: Element, animate: f64, easing: &str, enter: bool, exit: bool) -> Element {
    if animate == 0.0 && easing.is_empty() && !enter && !exit {
        return el;
    }
    let e = pixie_kernel::anim::Easing::parse(easing).unwrap_or(pixie_kernel::anim::Easing::Linear);
    Element::Anim {
        duration: animate,
        easing: e,
        enter,
        exit,
        opacity: 1.0,
        children: vec![el],
    }
}

/// The grid-span riders, tier-A side: the same `Element::GridCell`
/// both lowerers strip `colSpan:`/`rowSpan:` into. Outermost, so a
/// Grid parent sees the cell (an animation rider stays inside).
fn wrap_span(el: Element, col_span: i64, row_span: i64) -> Element {
    if col_span <= 1 && row_span <= 1 {
        return el;
    }
    Element::GridCell { col_span: col_span.max(1), row_span: row_span.max(1), children: vec![el] }
}

fn set_children(el: &mut Element, kids: Vec<Element>) -> Result<(), &'static str> {
    match el {
        // A theme scope wraps exactly one container — the `with`
        // block's children belong to the container inside it.
        Element::Themed { children, .. } if children.len() == 1 => {
            set_children(&mut children[0], kids)
        }
        // Same for an animation rider around a container.
        Element::Anim { children, .. } if children.len() == 1 => {
            set_children(&mut children[0], kids)
        }
        // ...and for the tooltip rider: `with column(tooltip="…")`
        // opens the column, not the wrapper.
        Element::Tooltip { children, .. } if children.len() == 1 => {
            set_children(&mut children[0], kids)
        }
        Element::Column { children, .. }
        | Element::Row { children, .. }
        | Element::Grid { children, .. }
        | Element::ScrollView { children, .. }
        | Element::Modal { children, .. } => {
            *children = kids;
            Ok(())
        }
        Element::Stack(children) | Element::HScrollView(children) | Element::DataTable(children) => {
            *children = kids;
            Ok(())
        }
        _ => Err("only containers can be used as `with` blocks"),
    }
}

/// Call a Python view-ish function (the view, a row builder) and
/// resolve its element: either the return value, or — with-style —
/// the single element left in the frame the call opened.
fn invoke_view(py: Python<'_>, f: &Bound<'_, PyAny>, args: Bound<'_, PyTuple>) -> Element {
    BUILD_FRAMES.with(|fr| fr.borrow_mut().push(Vec::new()));
    let res = f.call1(args);
    let frame = BUILD_FRAMES.with(|fr| fr.borrow_mut().pop()).unwrap_or_default();
    match res {
        Ok(v) if !v.is_none() => match v.downcast::<PyElement>() {
            Ok(pe) => {
                let pe = pe.borrow();
                pe.el
                    .borrow_mut()
                    .take()
                    .unwrap_or_else(|| err_text("view returned a used element"))
            }
            Err(_) => err_text("view must return a yokan element (or build one via `with`)"),
        },
        Ok(_) => {
            let mut roots: Vec<Element> = frame
                .into_iter()
                .filter_map(|pe| pe.bind(py).borrow().el.borrow_mut().take())
                .collect();
            match roots.len() {
                1 => roots.remove(0),
                0 => err_text("view built nothing (return an element or use `with ui.column(): ...`)"),
                _ => err_text("with-style view must build exactly one root"),
            }
        }
        Err(e) => {
            e.print(py);
            err_text("Python error in view — see terminal")
        }
    }
}

fn take_el(child: &Bound<'_, PyAny>) -> PyResult<Element> {
    let pe: PyRef<PyElement> = child.extract().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err("children must be yokan elements")
    })?;
    pe.el.borrow_mut().take().ok_or_else(|| {
        pyo3::exceptions::PyValueError::new_err("element already used (each element can appear once)")
    })
}

fn take_children(children: &Bound<'_, PyTuple>) -> PyResult<Vec<Element>> {
    let mut out = Vec::with_capacity(children.len());
    for c in children.iter() {
        out.push(take_el(&c)?);
    }
    Ok(out)
}

fn err_text(msg: &str) -> Element {
    Element::Text {
        text: Str::from(msg),
        font_size: 14.0,
        color: Str::from("#ff6666"),
        align: Str::from(""),
        grow: 0.0,
    }
}

fn to_list_f64(v: Vec<f64>) -> List<f64> {
    let mut l = List::default();
    for x in v {
        l.push(x);
    }
    l
}

fn to_list_str(v: Vec<String>) -> List<Str> {
    let mut l = List::default();
    for x in v {
        l.push(Str::from(x));
    }
    l
}


// ---------------------------------------------------------------------------
// Element constructors.

#[pyfunction(signature = (text, size=0.0, color=String::new(), align=String::new(), grow=0.0, animate=0.0, easing=String::new(), enter=false, exit=false, tooltip=String::new()))]
#[allow(clippy::too_many_arguments)]
fn text(text: String, size: f64, color: String, align: String, grow: f64, animate: f64, easing: String, enter: bool, exit: bool, tooltip: String) -> Reg {
    Reg::tip(&tooltip, wrap_anim(
        Element::Text {
            text: Str::from(text),
            font_size: size,
            color: Str::from(color),
            align: Str::from(align),
            grow,
        },
        animate,
        &easing,
        enter,
        exit,
    ))
}

#[pyfunction(signature = (label, on_click=None, width=0.0, height=0.0, size=0.0, background=String::new(), grow=0.0, color=String::new(), hover_background=String::new(), active_background=String::new(), border_radius=0.0, border_width=0.0, border_color=String::new(), basis=0.0, animate=0.0, easing=String::new(), enter=false, exit=false, col_span=1, row_span=1, tooltip=String::new()))]
#[allow(clippy::too_many_arguments)]
fn button(
    label: String,
    on_click: Option<Py<PyAny>>,
    width: f64,
    height: f64,
    size: f64,
    background: String,
    grow: f64,
    color: String,
    hover_background: String,
    active_background: String,
    border_radius: f64,
    border_width: f64,
    border_color: String,
    basis: f64,
    animate: f64,
    easing: String,
    enter: bool,
    exit: bool,
    col_span: i64,
    row_span: i64,
    tooltip: String,
) -> Reg {
    let listener: Listener = match on_click {
        Some(cb) => Rc::new(move |w: &mut World| {
            Python::attach(|py| {
                if let Err(e) = cb.call0(py) {
                    e.print(py);
                }
            });
            after_py_callback(w);
        }),
        None => Rc::new(|_| {}),
    };
    Reg::tip(&tooltip, wrap_span(wrap_anim(Element::Button {
        label: Str::from(label),
        background: Str::from(background),
        hover_background: Str::from(hover_background),
        active_background: Str::from(active_background),
        width,
        height,
        font_size: size,
        color: Str::from(color),
        grow,
        basis,
        border_radius,
        border_width,
        border_color: Str::from(border_color),
        on_click: listener,
    }, animate, &easing, enter, exit), col_span, row_span))
}

fn text_listener(cb: Py<PyAny>) -> TextListener {
    Rc::new(move |w: &mut World, s: Str| {
        Python::attach(|py| {
            if let Err(e) = cb.call1(py, (s.as_str(),)) {
                e.print(py);
            }
        });
        after_py_callback(w);
    })
}

fn bool_listener(cb: Py<PyAny>) -> BoolListener {
    Rc::new(move |w: &mut World, v: bool| {
        Python::attach(|py| {
            if let Err(e) = cb.call1(py, (v,)) {
                e.print(py);
            }
        });
        after_py_callback(w);
    })
}

fn float_listener(cb: Py<PyAny>) -> FloatListener {
    Rc::new(move |w: &mut World, v: f64| {
        Python::attach(|py| {
            if let Err(e) = cb.call1(py, (v,)) {
                e.print(py);
            }
        });
        after_py_callback(w);
    })
}

fn int_listener(cb: Py<PyAny>) -> IntListener {
    Rc::new(move |w: &mut World, v: i64| {
        Python::attach(|py| {
            if let Err(e) = cb.call1(py, (v,)) {
                e.print(py);
            }
        });
        after_py_callback(w);
    })
}

/// The form controls: value in from state, the handler receives the
/// NEW value as its one argument (bool / float / int).
#[pyfunction(signature = (label, checked=false, on_change=None, tooltip=String::new()))]
fn checkbox(label: String, checked: bool, on_change: Option<Py<PyAny>>, tooltip: String) -> Reg {
    Reg::tip(&tooltip, Element::Checkbox {
        label: Str::from(label),
        checked,
        on_toggle: on_change.map(bool_listener),
    })
}

#[pyfunction(signature = (label, checked=false, on_change=None, tooltip=String::new()))]
fn switch(label: String, checked: bool, on_change: Option<Py<PyAny>>, tooltip: String) -> Reg {
    Reg::tip(&tooltip, Element::Switch {
        label: Str::from(label),
        checked,
        on_toggle: on_change.map(bool_listener),
    })
}

#[pyfunction(signature = (value=0.0, min=0.0, max=1.0, step=0.0, on_change=None, tooltip=String::new()))]
fn slider(value: f64, min: f64, max: f64, step: f64, on_change: Option<Py<PyAny>>, tooltip: String) -> Reg {
    Reg::tip(&tooltip, Element::Slider {
        value,
        min,
        max,
        step,
        on_change: on_change.map(float_listener),
    })
}

fn str_list(v: Vec<String>) -> List<Str> {
    v.into_iter().map(Str::from).collect()
}

#[pyfunction(signature = (options=vec![], selected=0, on_change=None, tooltip=String::new()))]
fn select(options: Vec<String>, selected: i64, on_change: Option<Py<PyAny>>, tooltip: String) -> Reg {
    Reg::tip(&tooltip, Element::Select {
        options: str_list(options),
        selected,
        on_select: on_change.map(int_listener),
    })
}

#[pyfunction(signature = (options=vec![], selected=0, on_change=None, tooltip=String::new()))]
fn radio_group(options: Vec<String>, selected: i64, on_change: Option<Py<PyAny>>, tooltip: String) -> Reg {
    Reg::tip(&tooltip, Element::RadioGroup {
        options: str_list(options),
        selected,
        on_select: on_change.map(int_listener),
    })
}

#[pyfunction(signature = (labels=vec![], active=0, on_change=None, tooltip=String::new()))]
fn tab_bar(labels: Vec<String>, active: i64, on_change: Option<Py<PyAny>>, tooltip: String) -> Reg {
    Reg::tip(&tooltip, Element::TabBar {
        labels: str_list(labels),
        active,
        on_select: on_change.map(int_listener),
    })
}

/// The fourth chooser: same `options`/`selected` contract as
/// `select`/`radio_group`, painted as one joined pill group.
#[pyfunction(signature = (options=vec![], selected=0, on_change=None, tooltip=String::new()))]
fn segmented(options: Vec<String>, selected: i64, on_change: Option<Py<PyAny>>, tooltip: String) -> Reg {
    Reg::tip(&tooltip, Element::Segmented {
        options: str_list(options),
        selected,
        on_select: on_change.map(int_listener),
    })
}

#[pyfunction(signature = (value, placeholder=String::new(), on_change=None, on_submit=None, multiline=false, rows=0.0, tooltip=String::new()))]
fn text_field(
    value: String,
    placeholder: String,
    on_change: Option<Py<PyAny>>,
    on_submit: Option<Py<PyAny>>,
    multiline: bool,
    rows: f64,
    tooltip: String,
) -> Reg {
    Reg::tip(&tooltip, Element::TextField {
        value: Str::from(value),
        placeholder: Str::from(placeholder),
        on_change: on_change.map(text_listener),
        on_submit: on_submit.map(text_listener),
        multiline,
        rows,
    })
}

#[pyfunction(signature = (*children, spacing=-1.0, padding=0.0, background=String::new(), grow=0.0, border_radius=0.0, border_width=0.0, border_color=String::new(), theme=String::new(), animate=0.0, easing=String::new(), enter=false, exit=false, tooltip=String::new()))]
#[allow(clippy::too_many_arguments)]
fn column(
    children: &Bound<'_, PyTuple>,
    spacing: f64,
    padding: f64,
    background: String,
    grow: f64,
    border_radius: f64,
    border_width: f64,
    border_color: String,
    theme: String,
    animate: f64,
    easing: String,
    enter: bool,
    exit: bool,
    tooltip: String,
) -> PyResult<Reg> {
    let el = Element::Column {
        spacing,
        padding,
        background: Str::from(background),
        grow,
        border_radius,
        border_width,
        border_color: Str::from(border_color),
        children: take_children(children)?,
    };
    let el = if theme.is_empty() {
        el
    } else {
        // The tree SCOPE rider (§8.37): tokens under this node
        // resolve in the named palette, in both tiers, because the
        // resolution runs in the shared kernel.
        Element::Themed { theme: Str::from(theme), children: vec![el] }
    };
    Ok(Reg::tip(&tooltip, wrap_anim(el, animate, &easing, enter, exit)))
}

#[pyfunction(signature = (*children, spacing=-1.0, padding=0.0, background=String::new(), grow=0.0, border_radius=0.0, border_width=0.0, border_color=String::new(), tooltip=String::new()))]
#[allow(clippy::too_many_arguments)]
fn row(
    children: &Bound<'_, PyTuple>,
    spacing: f64,
    padding: f64,
    background: String,
    grow: f64,
    border_radius: f64,
    border_width: f64,
    border_color: String,
    tooltip: String,
) -> PyResult<Reg> {
    Ok(Reg::tip(&tooltip, Element::Row {
        spacing,
        padding,
        background: Str::from(background),
        grow,
        border_radius,
        border_width,
        border_color: Str::from(border_color),
        children: take_children(children)?,
    }))
}

#[pyfunction(signature = (data, labels=None, width=0.0, height=0.0, tooltip=String::new()))]
fn bar_chart(data: Vec<f64>, labels: Option<Vec<String>>, width: f64, height: f64, tooltip: String) -> Reg {
    Reg::tip(&tooltip, Element::BarChart {
        data: to_list_f64(data),
        labels: to_list_str(labels.unwrap_or_default()),
        width,
        height,
    })
}

#[pyfunction(signature = (data, labels=None, width=0.0, height=0.0, tooltip=String::new()))]
fn line_chart(data: Vec<f64>, labels: Option<Vec<String>>, width: f64, height: f64, tooltip: String) -> Reg {
    Reg::tip(&tooltip, Element::LineChart {
        data: to_list_f64(data),
        labels: to_list_str(labels.unwrap_or_default()),
        width,
        height,
    })
}

#[pyfunction]
fn progress(value: f64) -> Reg {
    Reg::wrap(Element::ProgressBar { value })
}

#[pyfunction(signature = (size=0.0, tooltip=String::new()))]
fn spinner(size: f64, tooltip: String) -> Reg {
    Reg::tip(&tooltip, Element::Spinner { size })
}

#[pyfunction(signature = (source, width=0.0, height=0.0, tooltip=String::new()))]
fn image(source: String, width: f64, height: f64, tooltip: String) -> Reg {
    Reg::tip(&tooltip, Element::Image { source: Str::from(source), width, height })
}

#[pyfunction(signature = (source, width=0.0, height=0.0, tooltip=String::new()))]
fn svg(source: String, width: f64, height: f64, tooltip: String) -> Reg {
    Reg::tip(&tooltip, Element::Svg { source: Str::from(source), width, height })
}

#[pyfunction(signature = (*children, height=0.0, tooltip=String::new()))]
fn scroll_view(children: &Bound<'_, PyTuple>, height: f64, tooltip: String) -> PyResult<Reg> {
    Ok(Reg::tip(&tooltip, Element::ScrollView { height, children: take_children(children)? }))
}

#[pyfunction(signature = (*children, tooltip=String::new()))]
fn h_scroll_view(children: &Bound<'_, PyTuple>, tooltip: String) -> PyResult<Reg> {
    Ok(Reg::tip(&tooltip, Element::HScrollView(take_children(children)?)))
}

#[pyfunction(signature = (*children, tooltip=String::new()))]
fn data_table(children: &Bound<'_, PyTuple>, tooltip: String) -> PyResult<Reg> {
    Ok(Reg::tip(&tooltip, Element::DataTable(take_children(children)?)))
}

#[pyfunction(signature = (*children, open=true, tooltip=String::new()))]
fn modal(children: &Bound<'_, PyTuple>, open: bool, tooltip: String) -> PyResult<Reg> {
    Ok(Reg::tip(&tooltip, Element::Modal { open, children: take_children(children)? }))
}

#[pyfunction(signature = (*children, tooltip=String::new()))]
fn stack(children: &Bound<'_, PyTuple>, tooltip: String) -> PyResult<Reg> {
    Ok(Reg::tip(&tooltip, Element::Stack(take_children(children)?)))
}

#[pyfunction(signature = (*children, columns=2, rows=0, spacing=-1.0, padding=0.0, background=String::new(), grow=0.0, border_radius=0.0, border_width=0.0, border_color=String::new(), tooltip=String::new()))]
#[allow(clippy::too_many_arguments)]
fn grid(
    children: &Bound<'_, PyTuple>,
    columns: i64,
    rows: i64,
    spacing: f64,
    padding: f64,
    background: String,
    grow: f64,
    border_radius: f64,
    border_width: f64,
    border_color: String,
    tooltip: String,
) -> PyResult<Reg> {
    Ok(Reg::tip(&tooltip, Element::Grid {
        columns,
        rows,
        spacing,
        padding,
        background: Str::from(background),
        grow,
        border_radius,
        border_width,
        border_color: Str::from(border_color),
        children: take_children(children)?,
    }))
}

#[pyfunction(signature = (child, col_span=1, row_span=1, tooltip=String::new()))]
fn grid_cell(child: &Bound<'_, PyAny>, col_span: i64, row_span: i64, tooltip: String) -> PyResult<Reg> {
    Ok(Reg::tip(&tooltip, Element::GridCell {
        col_span,
        row_span,
        children: vec![take_el(child)?],
    }))
}

/// Virtualized rows: `row(i)` is called only for the visible range
/// (pixie's LazyRows + gpui uniform_list — ~14 calls for 100k rows).
#[pyfunction(signature = (count, row, item_height=24.0, height=0.0, virtualized=true, grow=0.0, tooltip=String::new()))]
fn list_view(count: usize, row: Py<PyAny>, item_height: f64, height: f64, virtualized: bool, grow: f64, tooltip: String) -> Reg {
    let build: Rc<dyn Fn(&World, std::ops::Range<usize>) -> Vec<Element>> =
        Rc::new(move |_w, range| {
            Python::attach(|py| {
                range
                    .map(|i| {
                        let args = match PyTuple::new(py, [i]) {
                            Ok(a) => a,
                            Err(e) => {
                                e.print(py);
                                return err_text("internal: could not build row args");
                            }
                        };
                        COMP_PATH.with(|p| p.borrow_mut().push(i));
                        let saved_c = COMP_COUNTER.with(|c| {
                            let v = c.get();
                            c.set(0);
                            v
                        });
                        let saved_l = LOCAL_IX.with(|c| {
                            let v = c.get();
                            c.set(0);
                            v
                        });
                        let el = invoke_view(py, row.bind(py), args);
                        COMP_PATH.with(|p| {
                            p.borrow_mut().pop();
                        });
                        COMP_COUNTER.with(|c| c.set(saved_c));
                        LOCAL_IX.with(|c| c.set(saved_l));
                        el
                    })
                    .collect()
            })
        });
    Reg::tip(&tooltip, Element::ListView {
        virtualized,
        item_height,
        height,
        grow,
        children: Vec::new(),
        lazy: Some(LazyRows { len: count, build }),
    })
}

/// Run `work()` on a background worker thread, then `on_done(result)`
/// (or `on_error(exception)`) on the UI thread, followed by a rebuild.
/// Callable from anywhere: handlers, timer ticks, other task
/// callbacks, or before `run()`.
#[pyfunction(signature = (work, on_done=None, on_error=None))]
fn task(work: Py<PyAny>, on_done: Option<Py<PyAny>>, on_error: Option<Py<PyAny>>) -> PyResult<()> {
    PENDING_SPAWNS.with(|t| t.borrow_mut().push(TaskSpec { work, on_done, on_error }));
    Ok(())
}

// ---------------------------------------------------------------------------
// Timers: declared before `run()`, fired by the kernel off the
// animation clock — a frame in a window, an `advance:<ms>` in a
// script, so both runs tick the same number of times.

thread_local! {
    static PENDING_TIMERS: RefCell<Vec<(f64, Py<PyAny>)>> = const { RefCell::new(Vec::new()) };
}

/// Move the queued `every` registrations into the kernel's timer
/// store. The kernel fires them off the animation clock — a frame in
/// a window, an `advance:<ms>` in a script — which is what lets the
/// compiled run and this one tick the same number of times.
fn install_timers(rt: &Runtime) {
    for (secs, cb) in PENDING_TIMERS.with(|t| t.take()) {
        let ms = secs * 1000.0;
        rt.with(move |w: &mut World| {
            pixie_kernel::timer::every(
                w,
                ms,
                Rc::new(move |w: &mut World| {
                    Python::attach(|py| {
                        if let Err(e) = cb.call0(py) {
                            e.print(py);
                        }
                    });
                    after_py_callback(w);
                }),
            );
        });
    }
}

// ---------------------------------------------------------------------------
// Keys: declared before `run()`, delivered by the kernel — a
// keystroke in a window, a `key:<chord>` step in a script.

thread_local! {
    static PENDING_KEYS: RefCell<Vec<(String, Py<PyAny>)>> = const { RefCell::new(Vec::new()) };
    static PENDING_ANY_KEY: RefCell<Vec<Py<PyAny>>> = const { RefCell::new(Vec::new()) };
}

fn install_keys(rt: &Runtime) {
    for (chord, cb) in PENDING_KEYS.with(|k| k.take()) {
        rt.with(move |w: &mut World| {
            pixie_kernel::keys::bind(
                w,
                &chord,
                Rc::new(move |w: &mut World| {
                    Python::attach(|py| {
                        if let Err(e) = cb.call0(py) {
                            e.print(py);
                        }
                    });
                    after_py_callback(w);
                }),
            );
        });
    }
    for cb in PENDING_ANY_KEY.with(|k| k.take()) {
        rt.with(move |w: &mut World| {
            pixie_kernel::keys::on_key(
                w,
                Rc::new(move |w: &mut World, key: Str| {
                    Python::attach(|py| {
                        if let Err(e) = cb.call1(py, (key.as_str(),)) {
                            e.print(py);
                        }
                    });
                    after_py_callback(w);
                }),
            );
        });
    }
}

/// Declare a shortcut: the chord, spelled the way the platform spells
/// it (`cmd+s`, `shift-tab`), and the handler it runs. Call before
/// `run()`.
#[pyfunction]
fn shortcut(chord: &str, on_press: Py<PyAny>) -> PyResult<()> {
    if RUNNING.load(Ordering::SeqCst) {
        return Ok(());
    }
    if chord.trim().is_empty() {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "shortcut() needs a chord, like \"cmd+s\"",
        ));
    }
    PENDING_KEYS.with(|k| k.borrow_mut().push((chord.to_string(), on_press)));
    Ok(())
}

/// Declare a handler that sees every key, as the chord it was.
#[pyfunction]
fn on_key(handler: Py<PyAny>) -> PyResult<()> {
    if RUNNING.load(Ordering::SeqCst) {
        return Ok(());
    }
    PENDING_ANY_KEY.with(|k| k.borrow_mut().push(handler));
    Ok(())
}

// ---------------------------------------------------------------------------
// The menu bar: declared before `run()`, handed to the platform when
// the window opens — and pickable by name in a headless script.

thread_local! {
    static PENDING_MENUS: RefCell<Vec<(String, String, Py<PyAny>)>> =
        const { RefCell::new(Vec::new()) };
}

fn install_menu_items(rt: &Runtime) {
    for (menu, item, cb) in PENDING_MENUS.with(|m| m.take()) {
        rt.with(move |w: &mut World| {
            pixie_kernel::menu::item(
                w,
                &menu,
                &item,
                Rc::new(move |w: &mut World| {
                    Python::attach(|py| {
                        if let Err(e) = cb.call0(py) {
                            e.print(py);
                        }
                    });
                    after_py_callback(w);
                }),
            );
        });
    }
}

/// Declare one item in the application's menu bar: the menu it sits
/// in, the name it shows, and the handler it runs. Call before
/// `run()`; declaration order is menu order.
#[pyfunction]
fn menu_item(menu: &str, item: &str, on_pick: Py<PyAny>) -> PyResult<()> {
    if RUNNING.load(Ordering::SeqCst) {
        return Ok(());
    }
    if menu.trim().is_empty() || item.trim().is_empty() {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "menu_item() needs a menu and an item name",
        ));
    }
    PENDING_MENUS.with(|m| m.borrow_mut().push((menu.to_string(), item.to_string(), on_pick)));
    Ok(())
}

// ---------------------------------------------------------------------------
// Files dropped on the window: declared before `run()`, delivered by
// the platform's drag — or by a script's `drop:<path>` step.

thread_local! {
    static PENDING_DROPS: RefCell<Vec<Py<PyAny>>> = const { RefCell::new(Vec::new()) };
}

fn install_drops(rt: &Runtime) {
    for cb in PENDING_DROPS.with(|d| d.take()) {
        rt.with(move |w: &mut World| {
            pixie_kernel::drop::on_file(
                w,
                Rc::new(move |w: &mut World, path: Str| {
                    Python::attach(|py| {
                        if let Err(e) = cb.call1(py, (path.as_str(),)) {
                            e.print(py);
                        }
                    });
                    after_py_callback(w);
                }),
            );
        });
    }
}

/// Declare what happens to a file dragged onto the window: the
/// handler receives its path. Call before `run()`.
#[pyfunction]
fn on_file_drop(handler: Py<PyAny>) -> PyResult<()> {
    if RUNNING.load(Ordering::SeqCst) {
        return Ok(());
    }
    PENDING_DROPS.with(|d| d.borrow_mut().push(handler));
    Ok(())
}

/// Register a periodic callback. Call before `run()`; the callback
/// runs on the UI thread and a rebuild follows. During a live reload
/// the module re-executes with the app already running, and `every`
/// registrations are ignored then — changing timers needs a restart.
#[pyfunction]
fn every(seconds: f64, on_tick: Py<PyAny>) -> PyResult<()> {
    if RUNNING.load(Ordering::SeqCst) {
        return Ok(());
    }
    if !(seconds > 0.0) {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "every() needs a positive interval in seconds",
        ));
    }
    PENDING_TIMERS.with(|t| t.borrow_mut().push((seconds, on_tick)));
    Ok(())
}

// ---------------------------------------------------------------------------
// The component: build() calls the (swappable) Python view function.

struct Shared {
    view_fn: RefCell<Py<PyAny>>,
    state: Py<PyAny>,
}

#[derive(Clone)]
struct PyView(Rc<Shared>);

impl Component for PyView {
    fn build(&self, _w: &World) -> Element {
        let el = Python::attach(|py| {
            reset_build_identity();
            let f = self.0.view_fn.borrow();
            let fb = f.bind(py);
            let argc: usize = fb
                .getattr("__code__")
                .and_then(|c| c.getattr("co_argcount"))
                .and_then(|n| n.extract())
                .unwrap_or(1);
            let args = if argc == 0 {
                PyTuple::empty(py)
            } else {
                match PyTuple::new(py, [self.0.state.bind(py)]) {
                    Ok(a) => a,
                    Err(e) => {
                        e.print(py);
                        return err_text("internal: could not build view args");
                    }
                }
            };
            invoke_view(py, fb, args)
        });
        if !FIRST_BUILD_PRINTED.swap(true, Ordering::SeqCst) {
            if let Some(t0) = T0.get() {
                eprintln!(
                    "[yokan] first build done at {:.1} ms",
                    t0.elapsed().as_secs_f64() * 1000.0
                );
            }
        }
        el
    }
}

fn make_watch(path: PathBuf, shared: Rc<Shared>, hv: ErasedHandle) -> ReloadWatch {
    let p2 = path.clone();
    ReloadWatch {
        path,
        reload: Box::new(move |w: &mut World| {
            let ok = Python::attach(|py| -> bool {
                let Ok(src) = std::fs::read_to_string(&p2) else {
                    return false;
                };
                let (Ok(src_c), Ok(file_c), Ok(name_c)) = (
                    CString::new(src),
                    CString::new(p2.to_string_lossy().as_bytes()),
                    CString::new("pixie_reload"),
                ) else {
                    return false;
                };
                match PyModule::from_code(py, &src_c, &file_c, &name_c) {
                    Ok(m) => match m.getattr("view") {
                        Ok(v) => {
                            // State cells: the re-exec made fresh ones;
                            // carry the old values over by name so a
                            // reload never resets what the user built up.
                            if let Ok(old_g) = shared
                                .view_fn
                                .borrow()
                                .bind(py)
                                .getattr("__globals__")
                            {
                                if let (Ok(old_d), Ok(new_d)) = (
                                    old_g.downcast::<PyDict>(),
                                    m.getattr("__dict__")
                                        .and_then(|d| Ok(d.downcast::<PyDict>()?.clone())),
                                ) {
                                    for (k, nv) in new_d.iter() {
                                        if let Ok(ns) = nv.downcast::<PyState>() {
                                            if let Ok(Some(ov)) = old_d.get_item(&k) {
                                                if let Ok(os) = ov.downcast::<PyState>() {
                                                    let _ = ns.borrow().set(py, os.borrow().value(py));
                                                }
                                            }
                                            continue;
                                        }
                                        // @ui.state bundle instances: copy fields by name.
                                        let tagged = nv
                                            .get_type()
                                            .hasattr("__pixie_state__")
                                            .unwrap_or(false);
                                        if tagged {
                                            if let Ok(Some(ov)) = old_d.get_item(&k) {
                                                let old_tagged = ov
                                                    .get_type()
                                                    .hasattr("__pixie_state__")
                                                    .unwrap_or(false);
                                                if old_tagged {
                                                    if let Ok(od) = ov.getattr("__dict__") {
                                                        if let Ok(odict) = od.downcast::<PyDict>() {
                                                            for (fk, fv) in odict.iter() {
                                                                if let Ok(name) = fk.extract::<String>() {
                                                                    let _ = nv.setattr(name.as_str(), fv);
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            *shared.view_fn.borrow_mut() = v.unbind();
                            eprintln!("[yokan] reloaded view from {}", p2.display());
                            true
                        }
                        Err(e) => {
                            e.print(py);
                            false
                        }
                    },
                    Err(e) => {
                        e.print(py);
                        false
                    }
                }
            });
            if ok {
                w.mark_view_dirty(hv);
            }
            ok
        }),
    }
}

#[pyfunction(signature = (view, state=None, title="yokan".to_string(), watch=true, theme=None, on_start=None, width=0.0, height=0.0))]
fn run(
    py: Python<'_>,
    view: Py<PyAny>,
    state: Option<Py<PyAny>>,
    title: String,
    watch: bool,
    theme: Option<String>,
    on_start: Option<Py<PyAny>>,
    width: f64,
    height: f64,
) -> PyResult<()> {
    // A reload re-exec of the app module must not start a second app.
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    // Register the app's twins (Enums, dataclasses) for the crates
    // loader — declared plainly, no decorator required.
    if let Ok(g) = view.bind(py).getattr("__globals__") {
        if let Ok(m) = py.import("yokan") {
            let _ = m.getattr("_register_twins").and_then(|f| f.call1((g,)));
        }
    }
    let state = match state {
        Some(s) => s,
        None => PyDict::new(py).into_any().unbind(),
    };
    let src_path: Option<PathBuf> = view
        .bind(py)
        .getattr("__code__")
        .ok()
        .and_then(|c| c.getattr("co_filename").ok())
        .and_then(|f| f.extract::<String>().ok())
        .map(PathBuf::from)
        .filter(|p| p.is_file());
    // Everything below runs with the GIL RELEASED (pyo3-tracked
    // `detach`), so `ui.task` workers can attach while the engine
    // loop owns this thread. The closure captures only Send things
    // (Py handles, strings); the non-Send Runtime/World are built
    // inside it. Every callback re-enters via `Python::attach`.
    py.detach(move || {
        if let Some(t) = &theme {
            // The engine and the headless branch both read
            // PIXIE_THEME from the environment.
            unsafe { std::env::set_var("PIXIE_THEME", t) };
        }
        reset_app_identity();
        let shared = Rc::new(Shared { view_fn: RefCell::new(view), state });
        let mut w = World::new();
        let h = mount(&mut w, PyView(shared.clone()), &[]);
        CURRENT_VIEW.with(|c| c.set(Some(h.erase())));
        let rt = Runtime::new(w);
        CURRENT_CTX.with(|c| *c.borrow_mut() = Some(rt.ctx()));
        rt.with(drain_spawns); // ui.task calls made before run()
        // Headless: PIXIE_SCRIPT runs the shared kernel harness (the
        // same one generated apps and the tier gate use) and never
        // opens a window. Timers are skipped — a never-completing
        // task would spin the settle loop; scripted time is
        // `advance:`'s business.
        if let Ok(script) = std::env::var("PIXIE_SCRIPT") {
            install_timers(&rt);
            install_keys(&rt);
            install_menu_items(&rt);
        install_drops(&rt);
            install_drops(&rt);
            rt.with(drain_spawns);
            if let Some(f) = &on_start {
                Python::attach(|py| {
                    if let Err(e) = f.bind(py).call0() {
                        e.print(py);
                    }
                });
                rt.with(|w: &mut World| after_py_callback(w));
            }
            let _ = rt.with(|w| w.take_dirty_views());
            rt.with(|w: &mut World| {
                pixie_kernel::theme::set_light(
                    w,
                    std::env::var("PIXIE_THEME").is_ok_and(|v| v == "light"),
                )
            });
            let mut tree = rt.with(|w| pixie_kernel::build_prepared(w, h));
            pixie_kernel::script::anim_settle(&rt, h, &mut tree);
            rt.with(|w| println!("{}", tree.dump(w)));
            println!("{}", pixie_kernel::script::run(&rt, h, &mut tree, &script));
            return;
        }
        if let Some(f) = &on_start {
            Python::attach(|py| {
                if let Err(e) = f.bind(py).call0() {
                    e.print(py);
                }
            });
            rt.with(|w: &mut World| after_py_callback(w));
        }
        install_timers(&rt);
        install_keys(&rt);
        install_menu_items(&rt);
        install_drops(&rt);
        let watch_opt = if watch {
            src_path.map(|p| make_watch(p, shared.clone(), h.erase()))
        } else {
            None
        };
        // width/height come as a pair (the translator enforces it);
        // 0.0 = the engine default.
        let win = (width > 0.0 && height > 0.0).then_some((width, height));
        run_app(rt, h, &title, watch_opt, win);
    });
    Ok(())
}

/// `@ui.py`: marks a function as a CPython ESCAPE — it stays Python
/// in both tiers. On CPython it is the identity; the translator turns
/// calls into bindings against a generated pyo3-embedding crate, so
/// the native binary runs exactly this function on an embedded
/// interpreter. Annotate every parameter and the return.
#[pyfunction(name = "py")]
fn py_escape(f: Py<PyAny>) -> Py<PyAny> {
    f
}

/// `@ui.state`: the typed-class bundle. Applies `dataclass` when the
/// class is not one already, injects `update(**kw)` (the
/// lambda-friendly writer), and tags the class so reload can carry
/// instance fields old→new by module-level name.
#[pyfunction]
fn state(py: Python<'_>, cls: Py<PyAny>) -> PyResult<Py<PyAny>> {
    let dc = py.import("dataclasses")?;
    // Mutable defaults (list/dict) lift to default_factory BEFORE
    // dataclass sees them — models get the same treatment stores
    // always had (idempotent: a second pass sees field objects).
    let prep_ns = PyDict::new(py);
    py.run(
        c"def _pixie_prep(cls):\n    import copy\n    import dataclasses\n    for k, v in list(vars(cls).items()):\n        if isinstance(v, (list, dict)):\n            setattr(cls, k, dataclasses.field(default_factory=(lambda v=v: copy.deepcopy(v))))\n    return cls\n",
        None,
        Some(&prep_ns),
    )?;
    let prep = prep_ns
        .get_item("_pixie_prep")?
        .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("prep helper missing"))?;
    let cls = prep.call1((cls,))?.unbind();
    let b = cls.bind(py).clone();
    let is_dc: bool = dc.call_method1("is_dataclass", (&b,))?.extract()?;
    let b = if is_dc {
        b
    } else {
        dc.call_method1("dataclass", (&b,))?
    };
    // Weak[...] fields become weakref-backed properties: reads deref
    // (target or None — pixie's `weak prop` read), writes wrap.
    let weak_ns = PyDict::new(py);
    py.run(
        c"def _pixie_weakify(cls):\n    import weakref\n    import annotationlib\n    anns = annotationlib.get_annotations(cls, format=annotationlib.Format.FORWARDREF)\n    for name, ann in list(anns.items()):\n        if getattr(ann, '__pixie_weak__', False):\n            def _get(self, _n=name):\n                r = self.__dict__.get(_n)\n                return None if r is None else r()\n            def _set(self, v, _n=name):\n                import weakref as _w\n                self.__dict__[_n] = None if v is None else _w.ref(v)\n            setattr(cls, name, property(_get, _set))\n    return cls\n",
        None,
        Some(&weak_ns),
    )?;
    let weakify = weak_ns
        .get_item("_pixie_weakify")?
        .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("weakify helper missing"))?;
    let b = weakify.call1((&b,))?;
    let ns = PyDict::new(py);
    py.run(
        c"def _pixie_setattr(self, k, v):\n    if type(v) is int and not -9223372036854775808 <= v <= 9223372036854775807:\n        raise ValueError('int value exceeds the native 64-bit range (Int is i64 in the compiled tier)')\n    if type(v) is list:\n        for x in v:\n            if type(x) is int and not -9223372036854775808 <= x <= 9223372036854775807:\n                raise ValueError('list item exceeds the native 64-bit int range')\n    object.__setattr__(self, k, v)\ndef _pixie_update(self, **kw):\n    for k, v in kw.items():\n        setattr(self, k, v)\n",
        None,
        Some(&ns),
    )?;
    let f = ns
        .get_item("_pixie_update")?
        .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("update helper missing"))?;
    b.setattr("update", f)?;
    let sa = ns
        .get_item("_pixie_setattr")?
        .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("setattr helper missing"))?;
    b.setattr("__setattr__", sa)?;
    b.setattr("__pixie_state__", true)?;
    Ok(b.unbind())
}

/// `ui.style(**kwargs)` — a named bag of element kwargs. Tier A
/// uses it by SPLAT (`ui.text("hi", **chip)`), so it needs no
/// element changes at all; the translator turns the splat into the
/// native `style:` rider and the bag into a `style` block.
#[pyfunction(signature = (**kwargs))]
fn style_bag(py: Python<'_>, kwargs: Option<Bound<'_, PyDict>>) -> Py<PyDict> {
    match kwargs {
        Some(d) => d.unbind(),
        None => PyDict::new(py).unbind(),
    }
}

/// `@ui.store` — a process-lifetime singleton with fields AND
/// methods: the decorator returns the INSTANCE, so the class name
/// itself is the store (`Cart.add(...)`, `Cart.total`). Mutable
/// defaults (list/dict) become per-instance copies first, then the
/// `state` machinery applies (validating writes, reload carry).
#[pyfunction]
fn store(py: Python<'_>, cls: Py<PyAny>) -> PyResult<Py<PyAny>> {
    let ns = PyDict::new(py);
    py.run(
        c"def _pixie_storeify(cls):\n    import copy\n    import dataclasses\n    for k, v in list(vars(cls).items()):\n        if isinstance(v, (list, dict)):\n            setattr(cls, k, dataclasses.field(default_factory=(lambda v=v: copy.deepcopy(v))))\n    return cls\n",
        None,
        Some(&ns),
    )?;
    let prep = ns
        .get_item("_pixie_storeify")?
        .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("storeify helper missing"))?;
    let cls = prep.call1((cls,))?.unbind();
    let cls = state(py, cls)?;
    let inst = cls.bind(py).call0()?;
    inst.setattr("__pixie_store__", true)?;
    Ok(inst.unbind())
}

/// `@ui.model` — an observed, instantiable class. The runtime is
/// the `state` decorator's (dataclass-ify, validating __setattr__,
/// update()): the one-instance rule was always the translator's.
#[pyfunction]
fn model(py: Python<'_>, cls: Py<PyAny>) -> PyResult<Py<PyAny>> {
    // `state` is no longer a public name (@store and @model cover its
    // ground); the machinery lives on as model's implementation.
    state(py, cls)
}

/// `@ui.value` — shorthand for `@dataclass(frozen=True)`: a native
/// value type without the ceremony. Same class either way; the
/// translator accepts both spellings.
#[pyfunction]
fn value(py: Python<'_>, cls: Py<PyAny>) -> PyResult<Py<PyAny>> {
    // An Enum twin: no dataclass to apply — register and hand back.
    let enum_cls = py.import("enum")?.getattr("Enum")?;
    let is_enum = cls
        .bind(py)
        .cast::<pyo3::types::PyType>()
        .ok()
        .map(|t| t.is_subclass(&enum_cls).unwrap_or(false))
        .unwrap_or(false);
    if is_enum {
        if let Ok(m) = py.import("yokan") {
            if let Ok(reg) = m.getattr("_values") {
                let _ = reg.set_item(cls.bind(py).getattr("__name__")?, cls.bind(py));
            }
        }
        return Ok(cls);
    }
    let dc = py.import("dataclasses")?.getattr("dataclass")?;
    let kwargs = pyo3::types::PyDict::new(py);
    kwargs.set_item("frozen", true)?;
    let deco = dc.call((), Some(&kwargs))?;
    let out = deco.call1((cls,))?;
    // Register by name: the crates loader rebuilds a crate struct
    // return as this exact class (the app's declared twin).
    if let Ok(m) = py.import("yokan") {
        if let Ok(reg) = m.getattr("_values") {
            let _ = reg.set_item(out.getattr("__name__")?, &out);
        }
    }
    Ok(out.unbind())
}

/// Test/CI entry: run the app headless against `script` and return
/// "initial dump\nfinal dump" instead of printing. No window, no
/// timers. Mirrors `run()`'s PIXIE_SCRIPT branch.
#[pyfunction(signature = (view, state=None, script=String::new(), on_start=None))]
fn _headless(
    py: Python<'_>,
    view: Py<PyAny>,
    state: Option<Py<PyAny>>,
    script: String,
    on_start: Option<Py<PyAny>>,
) -> PyResult<String> {
    // Determinism: with the cycle collector off, a strong cycle
    // leaks in tier A exactly as it does natively, and weak reads
    // agree by construction (Weak breaks cycles; then refcount
    // frees at the same statement release does).
    py.run(c"import gc; gc.disable()", None, None)?;
    if let Ok(g) = view.bind(py).getattr("__globals__") {
        if let Ok(m) = py.import("yokan") {
            let _ = m.getattr("_register_twins").and_then(|f| f.call1((g,)));
        }
    }
    let state = match state {
        Some(s) => s,
        None => PyDict::new(py).into_any().unbind(),
    };
    let out = py.detach(move || {
        reset_app_identity();
        let shared = Rc::new(Shared { view_fn: RefCell::new(view), state });
        let mut w = World::new();
        let h = mount(&mut w, PyView(shared), &[]);
        CURRENT_VIEW.with(|c| c.set(Some(h.erase())));
        let rt = Runtime::new(w);
        CURRENT_CTX.with(|c| *c.borrow_mut() = Some(rt.ctx()));
        install_timers(&rt);
        install_keys(&rt);
        install_menu_items(&rt);
        install_drops(&rt);
        rt.with(drain_spawns);
        if let Some(f) = &on_start {
            // The startup hook: contained like any handler — a
            // failing start prints and the app still opens.
            Python::attach(|py| {
                if let Err(e) = f.bind(py).call0() {
                    e.print(py);
                }
            });
            rt.with(|w: &mut World| after_py_callback(w));
        }
        let _ = rt.with(|w| w.take_dirty_views());
        rt.with(|w: &mut World| pixie_kernel::theme::set_light(w, false));
        let mut tree = rt.with(|w| pixie_kernel::build_prepared(w, h));
        pixie_kernel::script::anim_settle(&rt, h, &mut tree);
        let first = rt.with(|w| tree.dump(w));
        let last = pixie_kernel::script::run(&rt, h, &mut tree, &script);
        format!("{first}\n{last}")
    });
    Ok(out)
}

/// The module lane's tier-A doors: `yokan.fs.*` etc. call the SAME
/// yokan-stdlib functions the compiled tier binds through
/// `.rpi` — one implementation, two doors, the gate arbitrates.
/// First law of a door: it RELEASES the GIL for the Rust work —
/// http proved it by deadlocking (the ureq call blocked while the
/// Python fixture server thread waited for the GIL the door held).
#[pyfunction]
#[pyo3(name = "read_text")]
fn py_fs_read_text(py: Python<'_>, path: &str) -> PyResult<String> {
    py.detach(|| yokan_stdlib::fs_read_text_result(path))
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
}

#[pyfunction]
#[pyo3(name = "write_text")]
fn py_fs_write_text(py: Python<'_>, path: &str, text: &str) -> i64 {
    py.detach(|| yokan_stdlib::fs_write_text(path, text))
}

#[pyfunction]
#[pyo3(name = "exists")]
fn py_fs_exists(py: Python<'_>, path: &str) -> bool {
    py.detach(|| yokan_stdlib::fs_exists(path))
}

/// `params` is the bound form: the values ride beside the statement
/// instead of inside it, so text a user typed can never become SQL.
#[pyfunction]
#[pyo3(name = "exec", signature = (path, sql, params = None))]
fn py_sqlite_exec(py: Python<'_>, path: &str, sql: &str, params: Option<Vec<String>>) -> i64 {
    py.detach(|| match params {
        Some(p) => yokan_stdlib::sqlite_exec_with(path, sql, p),
        None => yokan_stdlib::sqlite_exec(path, sql),
    })
}

#[pyfunction]
#[pyo3(name = "query_text", signature = (path, sql, params = None))]
fn py_sqlite_query_text(
    py: Python<'_>,
    path: &str,
    sql: &str,
    params: Option<Vec<String>>,
) -> Vec<String> {
    py.detach(|| match params {
        Some(p) => yokan_stdlib::sqlite_query_text_with(path, sql, p),
        None => yokan_stdlib::sqlite_query_text(path, sql),
    })
}

/// The total form: no table, no rows — no raise.
#[pyfunction]
#[pyo3(name = "query_rows_or", signature = (path, sql, params = None))]
fn py_sqlite_query_rows_or(
    py: Python<'_>,
    path: &str,
    sql: &str,
    params: Option<Vec<String>>,
) -> Vec<Vec<String>> {
    py.detach(|| yokan_stdlib::sqlite_query_rows_or(path, sql, params.unwrap_or_default()))
}

/// Every column of every row, as text — the multi-column read.
#[pyfunction]
#[pyo3(name = "query_rows", signature = (path, sql, params = None))]
fn py_sqlite_query_rows(
    py: Python<'_>,
    path: &str,
    sql: &str,
    params: Option<Vec<String>>,
) -> Vec<Vec<String>> {
    py.detach(|| yokan_stdlib::sqlite_query_rows(path, sql, params.unwrap_or_default()))
}

#[pyfunction]
#[pyo3(name = "get_text", signature = (url, timeout_ms = 0))]
fn py_http_get_text(py: Python<'_>, url: &str, timeout_ms: i64) -> PyResult<String> {
    py.detach(|| match timeout_ms {
        0 => yokan_stdlib::http_get_text_result(url),
        ms => yokan_stdlib::http_get_text_timeout_result(url, ms),
    })
    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
}

#[pyfunction]
#[pyo3(name = "get_text_with")]
fn py_http_get_text_with(
    py: Python<'_>,
    url: &str,
    headers: std::collections::HashMap<String, String>,
) -> String {
    py.detach(|| yokan_stdlib::http_get_text_with(url, headers))
}

#[pyfunction]
#[pyo3(name = "post_text", signature = (url, body, content_type = None))]
fn py_http_post_text(
    py: Python<'_>,
    url: &str,
    body: &str,
    content_type: Option<&str>,
) -> String {
    py.detach(|| match content_type {
        Some(ct) => yokan_stdlib::http_post_text_as(url, body, ct),
        None => yokan_stdlib::http_post_text(url, body),
    })
}

#[pyfunction]
#[pyo3(name = "post_text_or")]
fn py_http_post_text_or(py: Python<'_>, url: &str, body: &str, default: &str) -> String {
    py.detach(|| yokan_stdlib::http_post_text_or(url, body, default))
}

/// The status code, or 0 when the request never reached a server.
#[pyfunction]
#[pyo3(name = "status")]
fn py_http_status(py: Python<'_>, url: &str) -> i64 {
    py.detach(|| yokan_stdlib::http_status(url))
}

#[pyfunction] #[pyo3(name = "sqrt")]
fn py_math_sqrt(v: f64) -> f64 { yokan_stdlib::math_sqrt(v) }
#[pyfunction] #[pyo3(name = "sin")]
fn py_math_sin(v: f64) -> f64 { yokan_stdlib::math_sin(v) }
#[pyfunction] #[pyo3(name = "cos")]
fn py_math_cos(v: f64) -> f64 { yokan_stdlib::math_cos(v) }
#[pyfunction] #[pyo3(name = "pow")]
fn py_math_pow(a: f64, b: f64) -> f64 { yokan_stdlib::math_pow(a, b) }
#[pyfunction] #[pyo3(name = "fabs")]
fn py_math_fabs(v: f64) -> f64 { yokan_stdlib::math_fabs(v) }
#[pyfunction] #[pyo3(name = "floor")]
fn py_math_floor(v: f64) -> i64 { yokan_stdlib::math_floor(v) }
#[pyfunction] #[pyo3(name = "ceil")]
fn py_math_ceil(v: f64) -> i64 { yokan_stdlib::math_ceil(v) }
#[pyfunction] #[pyo3(name = "pi")]
fn py_math_pi() -> f64 { yokan_stdlib::math_pi() }

#[pyfunction] #[pyo3(name = "get_text")]
fn py_json_get_text(py: Python<'_>, src: &str, path: &str) -> String {
    py.detach(|| yokan_stdlib::json_get_text(src, path))
}
#[pyfunction] #[pyo3(name = "get_int")]
fn py_json_get_int(py: Python<'_>, src: &str, path: &str) -> i64 {
    py.detach(|| yokan_stdlib::json_get_int(src, path))
}
#[pyfunction] #[pyo3(name = "get_float")]
fn py_json_get_float(py: Python<'_>, src: &str, path: &str) -> f64 {
    py.detach(|| yokan_stdlib::json_get_float(src, path))
}
#[pyfunction] #[pyo3(name = "get_bool")]
fn py_json_get_bool(py: Python<'_>, src: &str, path: &str) -> bool {
    py.detach(|| yokan_stdlib::json_get_bool(src, path))
}
#[pyfunction] #[pyo3(name = "length")]
fn py_json_length(py: Python<'_>, src: &str, path: &str) -> i64 {
    py.detach(|| yokan_stdlib::json_length(src, path))
}
#[pyfunction] #[pyo3(name = "has")]
fn py_json_has(py: Python<'_>, src: &str, path: &str) -> bool {
    py.detach(|| yokan_stdlib::json_has(src, path))
}

#[pyfunction] #[pyo3(name = "to_int")]
fn py_strings_to_int(s: &str, default: i64) -> i64 {
    yokan_stdlib::strings_to_int(s, default)
}

#[pyfunction] #[pyo3(name = "to_float")]
fn py_strings_to_float(s: &str, default: f64) -> f64 {
    yokan_stdlib::strings_to_float(s, default)
}

#[pyfunction] #[pyo3(name = "send")]
fn py_notify_send(title: &str, body: &str) {
    yokan_stdlib::notify_send(title, body)
}

#[pyfunction] #[pyo3(name = "query_int", signature = (path, sql, params = None))]
fn py_sqlite_query_int(
    py: Python<'_>,
    path: &str,
    sql: &str,
    params: Option<Vec<String>>,
) -> PyResult<i64> {
    py.detach(|| match params {
        Some(p) => Ok(yokan_stdlib::sqlite_query_int_with(path, sql, p)),
        None => yokan_stdlib::sqlite_query_int_result(path, sql),
    })
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
}

#[pyfunction] #[pyo3(name = "read_text_or")]
fn py_fs_read_text_or(py: Python<'_>, path: &str, default: &str) -> String {
    py.detach(|| yokan_stdlib::fs_read_text_or(path, default))
}

#[pyfunction] #[pyo3(name = "get_text_or")]
fn py_http_get_text_or(py: Python<'_>, url: &str, default: &str) -> String {
    py.detach(|| yokan_stdlib::http_get_text_or(url, default))
}

#[pyfunction] #[pyo3(name = "query_int_or", signature = (path, sql, default, params = None))]
fn py_sqlite_query_int_or(
    py: Python<'_>,
    path: &str,
    sql: &str,
    default: i64,
    params: Option<Vec<String>>,
) -> i64 {
    py.detach(|| match params {
        Some(p) => yokan_stdlib::sqlite_query_int_or_with(path, sql, default, p),
        None => yokan_stdlib::sqlite_query_int_or(path, sql, default),
    })
}

#[pyfunction] #[pyo3(name = "query_text_or", signature = (path, sql, params = None))]
fn py_sqlite_query_text_or(
    py: Python<'_>,
    path: &str,
    sql: &str,
    params: Option<Vec<String>>,
) -> Vec<String> {
    py.detach(|| match params {
        Some(p) => yokan_stdlib::sqlite_query_text_or_with(path, sql, p),
        None => yokan_stdlib::sqlite_query_text_or(path, sql),
    })
}

#[pyfunction] #[pyo3(name = "seed")]
fn py_random_seed(n: i64) { yokan_stdlib::random_seed(n) }
#[pyfunction] #[pyo3(name = "int")]
fn py_random_int(lo: i64, hi: i64) -> i64 { yokan_stdlib::random_int(lo, hi) }
#[pyfunction] #[pyo3(name = "float")]
fn py_random_float() -> f64 { yokan_stdlib::random_float() }

#[pyfunction] #[pyo3(name = "now_ms")]
fn py_time_now_ms() -> i64 { yokan_stdlib::time_now_ms() }
#[pyfunction] #[pyo3(name = "format_ms")]
fn py_time_format_ms(py: Python<'_>, ms: i64, fmt: &str) -> String {
    py.detach(|| yokan_stdlib::time_format_ms(ms, fmt))
}
#[pyfunction] #[pyo3(name = "log")]
fn py_log(py: Python<'_>, msg: &str) -> i64 {
    py.detach(|| yokan_stdlib::log_line(msg))
}

#[pyfunction] #[pyo3(name = "sleep_ms")]
fn py_time_sleep_ms(py: Python<'_>, ms: i64) -> i64 {
    py.detach(|| yokan_stdlib::time_sleep_ms(ms))
}

#[pyfunction] #[pyo3(name = "format_local_ms")]
fn py_time_format_local_ms(py: Python<'_>, ms: i64, fmt: &str) -> String {
    py.detach(|| yokan_stdlib::time_format_local_ms(ms, fmt))
}

#[pyfunction] #[pyo3(name = "local_offset_minutes")]
fn py_time_local_offset_minutes(py: Python<'_>, ms: i64) -> i64 {
    py.detach(|| yokan_stdlib::time_local_offset_minutes(ms))
}

#[pyfunction] #[pyo3(name = "set_text")]
fn py_clipboard_set_text(py: Python<'_>, text: &str) -> i64 {
    py.detach(|| yokan_stdlib::clipboard_set_text(text))
}

#[pyfunction] #[pyo3(name = "get_text")]
fn py_clipboard_get_text(py: Python<'_>) -> String {
    py.detach(yokan_stdlib::clipboard_get_text)
}

#[pyfunction] #[pyo3(name = "list_dir")]
fn py_fs_list_dir(py: Python<'_>, path: &str) -> Vec<String> {
    py.detach(|| yokan_stdlib::fs_list_dir(path))
}

#[pyfunction] #[pyo3(name = "append_text")]
fn py_fs_append_text(py: Python<'_>, path: &str, text: &str) -> i64 {
    py.detach(|| yokan_stdlib::fs_append_text(path, text))
}

#[pyfunction] #[pyo3(name = "remove")]
fn py_fs_remove(py: Python<'_>, path: &str) -> i64 {
    py.detach(|| yokan_stdlib::fs_remove(path))
}

#[pyfunction] #[pyo3(name = "make_dir")]
fn py_fs_make_dir(py: Python<'_>, path: &str) -> i64 {
    py.detach(|| yokan_stdlib::fs_make_dir(path))
}

/// Both doors release Python while they wait: a dialog is a person's
/// decision, and the interpreted run's UI thread has to keep going.
#[pyfunction] #[pyo3(name = "open_dialog", signature = (title = ""))]
fn py_fs_open_dialog(py: Python<'_>, title: &str) -> String {
    py.detach(|| yokan_stdlib::fs_open_dialog(title))
}

#[pyfunction] #[pyo3(name = "save_dialog", signature = (name = ""))]
fn py_fs_save_dialog(py: Python<'_>, name: &str) -> String {
    py.detach(|| yokan_stdlib::fs_save_dialog(name))
}

#[pyfunction] #[pyo3(name = "app_dir")]
fn py_fs_app_dir(py: Python<'_>, name: &str) -> String {
    py.detach(|| yokan_stdlib::fs_app_dir(name))
}

/// `json.dumps(v)` — the door reads the value's type at run time, the
/// translator reads it from the annotation; both land on the same
/// stdlib writer, which is what makes the two runs print one string.
/// Bools are looked at before ints because a Python bool IS an int.
#[pyfunction] #[pyo3(name = "dumps")]
fn py_json_dumps(v: &Bound<'_, PyAny>) -> PyResult<String> {
    use pyo3::types::{PyBool, PyDict as PyDictT, PyList};
    if v.is_instance_of::<PyBool>() {
        return Ok(yokan_stdlib::json_dumps_bool(v.extract()?));
    }
    if let Ok(n) = v.extract::<i64>() {
        return Ok(yokan_stdlib::json_dumps_int(n));
    }
    if let Ok(f) = v.extract::<f64>() {
        return Ok(yokan_stdlib::json_dumps_float(f));
    }
    if let Ok(t) = v.extract::<String>() {
        return Ok(yokan_stdlib::json_dumps_str(&t));
    }
    if v.is_instance_of::<PyList>() {
        if let Ok(xs) = v.extract::<Vec<bool>>() {
            return Ok(yokan_stdlib::json_dumps_list_bool(xs));
        }
        if let Ok(xs) = v.extract::<Vec<i64>>() {
            return Ok(yokan_stdlib::json_dumps_list_int(xs));
        }
        if let Ok(xs) = v.extract::<Vec<f64>>() {
            return Ok(yokan_stdlib::json_dumps_list_float(xs));
        }
        if let Ok(xs) = v.extract::<Vec<String>>() {
            return Ok(yokan_stdlib::json_dumps_list_str(xs));
        }
    }
    if v.is_instance_of::<PyDictT>() {
        use std::collections::HashMap;
        if let Ok(m) = v.extract::<HashMap<String, bool>>() {
            return Ok(yokan_stdlib::json_dumps_map_bool(m));
        }
        if let Ok(m) = v.extract::<HashMap<String, i64>>() {
            return Ok(yokan_stdlib::json_dumps_map_int(m));
        }
        if let Ok(m) = v.extract::<HashMap<String, f64>>() {
            return Ok(yokan_stdlib::json_dumps_map_float(m));
        }
        if let Ok(m) = v.extract::<HashMap<String, String>>() {
            return Ok(yokan_stdlib::json_dumps_map_str(m));
        }
    }
    Err(pyo3::exceptions::PyTypeError::new_err(
        "json.dumps writes a str, int, float, bool, a list of one of those, \
         or a dict with str keys and one of those as values",
    ))
}

#[pymodule]
pub fn yokan(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let _ = T0.set(Instant::now());
    m.add_class::<PyState>()?;
    m.add_function(wrap_pyfunction!(text, m)?)?;
    m.add_function(wrap_pyfunction!(button, m)?)?;
    m.add_function(wrap_pyfunction!(text_field, m)?)?;
    m.add_function(wrap_pyfunction!(column, m)?)?;
    m.add_function(wrap_pyfunction!(row, m)?)?;
    m.add_function(wrap_pyfunction!(grid, m)?)?;
    m.add_function(wrap_pyfunction!(grid_cell, m)?)?;
    m.add_function(wrap_pyfunction!(stack, m)?)?;
    m.add_function(wrap_pyfunction!(list_view, m)?)?;
    m.add_function(wrap_pyfunction!(scroll_view, m)?)?;
    m.add_function(wrap_pyfunction!(h_scroll_view, m)?)?;
    m.add_function(wrap_pyfunction!(data_table, m)?)?;
    m.add_function(wrap_pyfunction!(modal, m)?)?;
    m.add_function(wrap_pyfunction!(image, m)?)?;
    m.add_function(wrap_pyfunction!(svg, m)?)?;
    m.add_function(wrap_pyfunction!(bar_chart, m)?)?;
    m.add_function(wrap_pyfunction!(line_chart, m)?)?;
    m.add_function(wrap_pyfunction!(progress, m)?)?;
    m.add_function(wrap_pyfunction!(spinner, m)?)?;
    m.add_function(wrap_pyfunction!(checkbox, m)?)?;
    m.add_function(wrap_pyfunction!(switch, m)?)?;
    m.add_function(wrap_pyfunction!(slider, m)?)?;
    m.add_function(wrap_pyfunction!(select, m)?)?;
    m.add_function(wrap_pyfunction!(radio_group, m)?)?;
    m.add_function(wrap_pyfunction!(tab_bar, m)?)?;
    m.add_function(wrap_pyfunction!(segmented, m)?)?;
    m.add_function(wrap_pyfunction!(py_escape, m)?)?;
    m.add_function(wrap_pyfunction!(model, m)?)?;
    m.add_function(wrap_pyfunction!(value, m)?)?;
    {
        // `Weak[T]` — the not-owning reference annotation. Reads on
        // a Weak field answer the target or None; the compiled twin
        // is pixie's `weak prop` (breaks ownership cycles).
        let py = m.py();
        let ns = PyDict::new(py);
        py.run(
            c"class Weak:\n    def __class_getitem__(cls, item):\n        return type('WeakOf', (), {'__pixie_weak__': True})\n",
            None,
            Some(&ns),
        )?;
        let w = ns
            .get_item("Weak")?
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("Weak class missing"))?;
        m.add("Weak", w)?;
    }
    m.add_function(wrap_pyfunction!(store, m)?)?;
    // Every @value class registers here by name; the crates loader
    // uses it to rebuild struct and enum returns as the app's own
    // types. `_register_twins` sweeps an app module's globals at
    // startup so plain `class X(Enum)` and `@dataclass(frozen=True)`
    // twins register with no decorator at all.
    m.add("_values", pyo3::types::PyDict::new(m.py()))?;
    {
        let ns = pyo3::types::PyDict::new(m.py());
        m.py().run(
            c"def _register_twins(g):\n    import dataclasses, enum, yokan\n    for v in list(g.values()):\n        if isinstance(v, type) and (\n            (issubclass(v, enum.Enum) and v is not enum.Enum)\n            or dataclasses.is_dataclass(v)\n        ):\n            yokan._values[v.__name__] = v\n",
            None,
            Some(&ns),
        )?;
        let f = ns
            .get_item("_register_twins")?
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("twin sweeper missing"))?;
        m.add("_register_twins", f)?;
    }
    {
        // `yokan.crates` — the interpreted run's door to declared
        // Rust crates: each attribute lazily loads the pyo3 shim the
        // gate builds into <app>/.yokan/ext/<name>.so (searched next
        // to the running script, then under the cwd). The compiled
        // run reaches the same crate through its .rpi binding, so
        // both runs call one implementation.
        let ns = pyo3::types::PyDict::new(m.py());
        m.py().run(
            c"class _Crates:\n    def __getattr__(self, name):\n        import importlib.util, os, sys\n        cands = []\n        envd = os.environ.get('YOKAN_EXT_DIR')\n        if envd:\n            cands.append(os.path.join(envd, name + '.so'))\n        argv0 = sys.argv[0] if sys.argv and sys.argv[0] else ''\n        if argv0:\n            cands.append(os.path.join(os.path.dirname(os.path.abspath(argv0)), '.yokan', 'ext', name + '.so'))\n        cands.append(os.path.join(os.getcwd(), '.yokan', 'ext', name + '.so'))\n        for p in cands:\n            if os.path.exists(p):\n                spec = importlib.util.spec_from_file_location('yokan_ext_' + name.replace('-', '_'), p)\n                mod = importlib.util.module_from_spec(spec)\n                spec.loader.exec_module(mod)\n                mp = p + '.meta.json'\n                if os.path.exists(mp):\n                    import json as _json\n                    import yokan as _y\n                    meta = _json.load(open(mp))\n                    def _rb(cls, tup):\n                        import dataclasses\n                        vals = []\n                        for fld, v in zip(dataclasses.fields(cls), tup):\n                            t = fld.type\n                            nm = t if isinstance(t, str) else getattr(t, '__name__', None)\n                            sub = _y._values.get(nm) if nm else None\n                            if isinstance(v, tuple):\n                                if sub is None:\n                                    raise TypeError(f'field `{fld.name}` of `{cls.__name__}` is a nested crate struct - declare its same-shaped twin class in the app')\n                                vals.append(_rb(sub, v))\n                            else:\n                                vals.append(v)\n                        return cls(*vals)\n                    for fn, sname in meta.get('ret_structs', {}).items():\n                        raw = getattr(mod, fn)\n                        def _wrap(raw=raw, sname=sname, fn=fn, cname=name):\n                            def call(*a):\n                                cls = _y._values.get(sname)\n                                if cls is None:\n                                    raise TypeError(f'crates.{cname}.{fn} returns `{sname}` - declare its same-shaped twin class in the app')\n                                return _rb(cls, raw(*a))\n                            return call\n                        setattr(mod, fn, _wrap())\n                    for fn, ename in meta.get('ret_enums', {}).items():\n                        raw = getattr(mod, fn)\n                        def _wrap(raw=raw, ename=ename, fn=fn, cname=name):\n                            def call(*a):\n                                cls = _y._values.get(ename)\n                                if cls is None:\n                                    raise TypeError(f'crates.{cname}.{fn} returns `{ename}` - declare its same-shaped Enum twin in the app')\n                                return cls[raw(*a)]\n                            return call\n                        setattr(mod, fn, _wrap())\n                object.__setattr__(self, name, mod)\n                return mod\n        raise ImportError(\n            f'yokan.crates.{name}: built door not found - run `yokan sync <app.py>` (or `yokan gate`) once; it needs the repository checkout and Rust')\n",
            None,
            Some(&ns),
        )?;
        let cls = ns
            .get_item("_Crates")?
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("crates loader missing"))?;
        m.add("crates", cls.call0()?)?;
    }
    let sty = wrap_pyfunction!(style_bag, m)?;
    m.add("style", sty)?;
    {
        let comp = wrap_pyfunction!(component_deco, m)?;
        m.add("component", comp)?;
    }
    m.add_function(wrap_pyfunction!(slot, m)?)?;
    m.add_function(wrap_pyfunction!(local, m)?)?;
    m.add_function(wrap_pyfunction!(task, m)?)?;
    m.add_function(wrap_pyfunction!(every, m)?)?;
    m.add_function(wrap_pyfunction!(shortcut, m)?)?;
    m.add_function(wrap_pyfunction!(on_key, m)?)?;
    m.add_function(wrap_pyfunction!(menu_item, m)?)?;
    m.add_function(wrap_pyfunction!(on_file_drop, m)?)?;
    m.add_function(wrap_pyfunction!(run, m)?)?;
    m.add_function(wrap_pyfunction!(_headless, m)?)?;
    let fs = PyModule::new(m.py(), "fs")?;
    fs.add_function(wrap_pyfunction!(py_fs_read_text, &fs)?)?;
    fs.add_function(wrap_pyfunction!(py_fs_write_text, &fs)?)?;
    fs.add_function(wrap_pyfunction!(py_fs_exists, &fs)?)?;
    fs.add_function(wrap_pyfunction!(py_fs_read_text_or, &fs)?)?;
    fs.add_function(wrap_pyfunction!(py_fs_list_dir, &fs)?)?;
    fs.add_function(wrap_pyfunction!(py_fs_append_text, &fs)?)?;
    fs.add_function(wrap_pyfunction!(py_fs_remove, &fs)?)?;
    fs.add_function(wrap_pyfunction!(py_fs_make_dir, &fs)?)?;
    fs.add_function(wrap_pyfunction!(py_fs_app_dir, &fs)?)?;
    fs.add_function(wrap_pyfunction!(py_fs_open_dialog, &fs)?)?;
    fs.add_function(wrap_pyfunction!(py_fs_save_dialog, &fs)?)?;
    m.add_submodule(&fs)?;
    let sqlite = PyModule::new(m.py(), "sqlite")?;
    sqlite.add_function(wrap_pyfunction!(py_sqlite_exec, &sqlite)?)?;
    sqlite.add_function(wrap_pyfunction!(py_sqlite_query_text, &sqlite)?)?;
    sqlite.add_function(wrap_pyfunction!(py_sqlite_query_int, &sqlite)?)?;
    sqlite.add_function(wrap_pyfunction!(py_sqlite_query_int_or, &sqlite)?)?;
    sqlite.add_function(wrap_pyfunction!(py_sqlite_query_text_or, &sqlite)?)?;
    sqlite.add_function(wrap_pyfunction!(py_sqlite_query_rows, &sqlite)?)?;
    sqlite.add_function(wrap_pyfunction!(py_sqlite_query_rows_or, &sqlite)?)?;
    m.add_submodule(&sqlite)?;
    let http = PyModule::new(m.py(), "http")?;
    http.add_function(wrap_pyfunction!(py_http_get_text, &http)?)?;
    http.add_function(wrap_pyfunction!(py_http_get_text_or, &http)?)?;
    http.add_function(wrap_pyfunction!(py_http_get_text_with, &http)?)?;
    http.add_function(wrap_pyfunction!(py_http_post_text, &http)?)?;
    http.add_function(wrap_pyfunction!(py_http_post_text_or, &http)?)?;
    http.add_function(wrap_pyfunction!(py_http_status, &http)?)?;
    m.add_submodule(&http)?;
    // `from yokan import fs` works by attribute; this makes the
    // dotted forms (`import yokan.fs` etc.) resolve too.
    let mathm = PyModule::new(m.py(), "math")?;
    mathm.add_function(wrap_pyfunction!(py_math_sqrt, &mathm)?)?;
    mathm.add_function(wrap_pyfunction!(py_math_sin, &mathm)?)?;
    mathm.add_function(wrap_pyfunction!(py_math_cos, &mathm)?)?;
    mathm.add_function(wrap_pyfunction!(py_math_pow, &mathm)?)?;
    mathm.add_function(wrap_pyfunction!(py_math_fabs, &mathm)?)?;
    mathm.add_function(wrap_pyfunction!(py_math_floor, &mathm)?)?;
    mathm.add_function(wrap_pyfunction!(py_math_ceil, &mathm)?)?;
    mathm.add_function(wrap_pyfunction!(py_math_pi, &mathm)?)?;
    m.add_submodule(&mathm)?;
    let jsonm = PyModule::new(m.py(), "json")?;
    jsonm.add_function(wrap_pyfunction!(py_json_get_text, &jsonm)?)?;
    jsonm.add_function(wrap_pyfunction!(py_json_get_int, &jsonm)?)?;
    jsonm.add_function(wrap_pyfunction!(py_json_get_float, &jsonm)?)?;
    jsonm.add_function(wrap_pyfunction!(py_json_get_bool, &jsonm)?)?;
    jsonm.add_function(wrap_pyfunction!(py_json_length, &jsonm)?)?;
    jsonm.add_function(wrap_pyfunction!(py_json_has, &jsonm)?)?;
    jsonm.add_function(wrap_pyfunction!(py_json_dumps, &jsonm)?)?;
    m.add_submodule(&jsonm)?;
    let notifym = PyModule::new(m.py(), "notify")?;
    notifym.add_function(wrap_pyfunction!(py_notify_send, &notifym)?)?;
    m.add_submodule(&notifym)?;
    let stringsm = PyModule::new(m.py(), "strings")?;
    stringsm.add_function(wrap_pyfunction!(py_strings_to_int, &stringsm)?)?;
    stringsm.add_function(wrap_pyfunction!(py_strings_to_float, &stringsm)?)?;
    m.add_submodule(&stringsm)?;
    let randomm = PyModule::new(m.py(), "random")?;
    randomm.add_function(wrap_pyfunction!(py_random_seed, &randomm)?)?;
    randomm.add_function(wrap_pyfunction!(py_random_int, &randomm)?)?;
    randomm.add_function(wrap_pyfunction!(py_random_float, &randomm)?)?;
    m.add_submodule(&randomm)?;
    m.add_function(wrap_pyfunction!(py_log, m)?)?;
    let clipm = PyModule::new(m.py(), "clipboard")?;
    clipm.add_function(wrap_pyfunction!(py_clipboard_set_text, &clipm)?)?;
    clipm.add_function(wrap_pyfunction!(py_clipboard_get_text, &clipm)?)?;
    m.add_submodule(&clipm)?;
    let timem = PyModule::new(m.py(), "time")?;
    timem.add_function(wrap_pyfunction!(py_time_now_ms, &timem)?)?;
    timem.add_function(wrap_pyfunction!(py_time_format_ms, &timem)?)?;
    timem.add_function(wrap_pyfunction!(py_time_sleep_ms, &timem)?)?;
    timem.add_function(wrap_pyfunction!(py_time_format_local_ms, &timem)?)?;
    timem.add_function(wrap_pyfunction!(py_time_local_offset_minutes, &timem)?)?;
    m.add_submodule(&timem)?;
    let sysmod = m.py().import("sys")?.getattr("modules")?;
    sysmod.set_item("yokan.fs", &fs)?;
    sysmod.set_item("yokan.sqlite", &sqlite)?;
    sysmod.set_item("yokan.http", &http)?;
    sysmod.set_item("yokan.math", &mathm)?;
    sysmod.set_item("yokan.json", &jsonm)?;
    sysmod.set_item("yokan.time", &timem)?;
    sysmod.set_item("yokan.strings", &stringsm)?;
    sysmod.set_item("yokan.random", &randomm)?;
    Ok(())
}
