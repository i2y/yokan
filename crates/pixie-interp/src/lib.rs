//! The rung-2 view-slice interpreter (S6, productized).
//!
//! A running app holds its view body as data: on save, the source is
//! re-parsed with the real pixie parser, and — when everything outside
//! the view body is unchanged (fingerprint match) — the new body is
//! walked against the live World to produce the next Element tree. No
//! rustc, no restart, World state preserved. Interpreted views reach
//! compiled classes only through emitter-registered reflection tables,
//! and interpreted closures capture only table fn pointers, Copy
//! handles, and cloned AST — the §3.2 capture rule, dynamically.
//!
//! The interpreter's vocabulary deliberately mirrors the emitter's
//! view subset; a construct the emitter rejects is undefined here too
//! (it errors, and the reload keeps the last good tree). Reloaded
//! sources skip the checker — a type error in an edited expression
//! surfaces as a reload-time error, and rung 1 catches it on the next
//! real build.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use pixie_kernel::{
    BoolListener, Bytes, Element, ErasedHandle, FloatListener, IntListener, LazyRows, List,
    Listener, Str, TextListener, World,
};
use pixie_syntax::ast::{
    self, AssignOp, BinOp, ElementMember, Expr, ExprKind, StateFieldKind, Stmt, StrPart, UnaryOp,
};
use pixie_syntax::span::FileId;
use pixie_syntax::view::{items_of_block, items_of_members, ViewItem};

// ---------------------------------------------------------------------------
// Dynamic values.

#[derive(Clone, Debug)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(Str),
    List(Vec<Value>),
    /// A World OBJECT: the erased handle plus the class key its
    /// reflection entries are registered under (§8.41). Classes refer
    /// to classes now (§8.40), so a value read out of a prop can be
    /// another object — and a `List<Handle<C>>` is a list of these.
    /// Carrying the class is what lets a further `.prop` read
    /// dispatch through the tables without any type information.
    Object(ErasedHandle, String),
    /// A `Map<K, V>` read out of a property (§8.68). Keys are the
    /// scalar values a pixie map key can be, so the pair list is
    /// enough — and it stays SORTED, because the compiled tier reads
    /// a `BTreeMap` and the two tiers have to agree on order.
    Map(Vec<(Value, Value)>),
    /// A byte string (§8.68). Its length is the only thing pixie can
    /// ask of one today, which is what the compiled tier offers too.
    Bytes(Bytes),
    /// A STRUCT value (§8.68): its name plus its fields by surface
    /// name, so a member read dispatches without any type
    /// information — the same trick `Value::Object` plays for a
    /// class. A struct is a value, so this is a copy, which is
    /// exactly what a struct assignment does.
    Struct(String, Vec<(String, Value)>),
    /// `nil` — the empty half of a `T?` (§8.68). A present optional
    /// is just the value, which is what makes `case` over one work
    /// with no unwrapping in the interpreter.
    Nil,
    Unit,
}

/// Map keys compare by value. Only the scalar shapes a pixie map key
/// can take are compared; anything else is not a key (§8.68).
/// The ordering the compiled tier's `BTreeMap` gives the same keys —
/// the interp pair list stays sorted by it.
fn value_key_cmp(a: &Value, b: &Value) -> std::cmp::Ordering {
    use std::cmp::Ordering::Equal;
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x.cmp(y),
        (Value::Str(x), Value::Str(y)) => x.cmp(y),
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
        (Value::Float(x), Value::Float(y)) => x.partial_cmp(y).unwrap_or(Equal),
        _ => Equal,
    }
}

/// The bindings a `when Variant(a, b)` arm introduces, zipped with
/// the crossed payload fields (positional, the tuple-variant rule).
fn enum_arm_binds(
    pat: &ast::Pattern,
    fields: Option<&[(String, Value)]>,
) -> Option<Vec<(String, Value)>> {
    let ast::Pattern::Ctor { args, .. } = pat else {
        return None;
    };
    let fields = fields?;
    let mut out = Vec::new();
    for (a, (_, fv)) in args.iter().zip(fields.iter()) {
        if let ast::Pattern::Bind { name, .. } = a {
            out.push((name.name.clone(), fv.clone()));
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

fn value_key_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Str(x), Value::Str(y)) => x.as_str() == y.as_str(),
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Float(x), Value::Float(y)) => x == y,
        _ => false,
    }
}

impl Value {
    pub fn render(&self) -> String {
        match self {
            Value::Int(v) => v.to_string(),
            Value::Float(v) => v.to_string(),
            Value::Bool(v) => v.to_string(),
            Value::Str(s) => s.as_str().to_string(),
            Value::List(v) => format!("[{} items]", v.len()),
            // An object has no text form of its own: rendering one is
            // an authoring mistake (interpolate one of its props), and
            // the class name is the useful thing to see.
            Value::Object(_, c) => format!("<{c}>"),
            Value::Map(kv) => format!("{{{} entries}}", kv.len()),
            // Like an object: a struct has no text form of its own,
            // and the name is the useful thing to see.
            Value::Struct(n, _) => format!("<{n}>"),
            Value::Bytes(b) => format!("{} bytes", b.len()),
            // The same text `format!("{}", None::<T>)` cannot print:
            // pixie's compiled tier renders an absent optional as
            // nothing, and so does this.
            Value::Nil => String::new(),
            Value::Unit => String::new(),
        }
    }
    /// The type name for a diagnostic. A `String` rather than a
    /// `&'static str` because an object's name is its CLASS, which is
    /// only known at run time — and leaking a `str` to fake a static
    /// lifetime is not a thing a runtime should do.
    fn type_name(&self) -> String {
        match self {
            Value::Int(_) => "Int".to_string(),
            Value::Float(_) => "Float".to_string(),
            Value::Bool(_) => "Bool".to_string(),
            Value::Str(_) => "String".to_string(),
            Value::List(_) => "List".to_string(),
            Value::Object(_, c) => c.clone(),
            Value::Map(_) => "Map".to_string(),
            Value::Struct(n, _) => n.clone(),
            Value::Bytes(_) => "Bytes".to_string(),
            Value::Nil => "nil".to_string(),
            Value::Unit => "Void".to_string(),
        }
    }
    pub fn as_int(&self) -> Result<i64, String> {
        match self {
            Value::Int(v) => Ok(*v),
            other => Err(format!("expected Int, got {}", other.type_name())),
        }
    }
    /// §8.55: Int widens to Float, so a Float slot takes either.
    /// The checker makes the same call statically; this is the tier
    /// where the value carries its own type.
    pub fn as_float(&self) -> Result<f64, String> {
        match self {
            Value::Float(v) => Ok(*v),
            Value::Int(v) => Ok(*v as f64),
            other => Err(format!("expected Float, got {}", other.type_name())),
        }
    }
    pub fn as_bool(&self) -> Result<bool, String> {
        match self {
            Value::Bool(v) => Ok(*v),
            other => Err(format!("expected Bool, got {}", other.type_name())),
        }
    }
    /// The pairs a map value carries (§8.68), in key order.
    pub fn as_map_value(&self) -> Result<Vec<(Value, Value)>, String> {
        match self {
            Value::Map(kv) => Ok(kv.clone()),
            other => Err(format!("expected Map, got {}", other.type_name())),
        }
    }
    /// The bytes a byte-string value carries (§8.68).
    pub fn as_bytes_value(&self) -> Result<Bytes, String> {
        match self {
            Value::Bytes(b) => Ok(b.clone()),
            other => Err(format!("expected Bytes, got {}", other.type_name())),
        }
    }
    /// The handle an object value carries (§8.53). Reflection-table
    /// entries take this to accept an object as a method argument.
    pub fn as_object(&self) -> Result<ErasedHandle, String> {
        match self {
            Value::Object(h, _) => Ok(*h),
            other => Err(format!("expected an object, got {}", other.type_name())),
        }
    }
    pub fn as_str_value(&self) -> Result<Str, String> {
        match self {
            Value::Str(s) => Ok(s.clone()),
            other => Err(format!("expected String, got {}", other.type_name())),
        }
    }
}

// ---------------------------------------------------------------------------
// Reflection tables — the emitter registers every class here.

pub type GetterFn = fn(&World, ErasedHandle) -> Value;
pub type RowFn = fn(&World, ErasedHandle, &[usize]) -> Option<ErasedHandle>;
pub type SetterFn = fn(&mut World, ErasedHandle, Value) -> Result<(), String>;
pub type MethodFn = fn(&mut World, ErasedHandle, Vec<Value>) -> Result<(), String>;
pub type GlobalFn = fn(&World) -> ErasedHandle;
/// Build one object from constructor arguments (§8.53). The
/// interpreted tier needs this to run a handler that says
/// `let c = C()`, which the compiled tier lowers to `w.insert`.
pub type CtorFn = fn(&mut World, Vec<Value>) -> Result<ErasedHandle, String>;

#[derive(Default)]
pub struct Tables {
    getters: HashMap<(String, String), GetterFn>,
    setters: HashMap<(String, String), SetterFn>,
    methods: HashMap<(String, String), MethodFn>,
    globals: HashMap<String, (String, GlobalFn)>,
    /// Row seats (§8.30): seat field name → (row class, indexed
    /// erased-handle getter). Rows are ensured compiled-side in
    /// `prepare`; the interpreter only reads.
    rows: HashMap<String, (String, RowFn)>,
    /// Class name → constructor.
    ctors: HashMap<String, CtorFn>,
    /// `static fn`s (§8.54): World-free by definition, which is what
    /// makes them the one callable a VIEW may evaluate.
    statics: HashMap<(String, String), StaticFn>,
}

pub type StaticFn = fn(Vec<Value>) -> Result<Value, String>;

impl Tables {
    pub fn new() -> Self {
        Tables::default()
    }
    pub fn row(&mut self, seat: &str, class: &str, f: RowFn) {
        self.rows.insert(seat.into(), (class.into(), f));
    }
    pub fn getter(&mut self, class: &str, prop: &str, f: GetterFn) {
        self.getters.insert((class.into(), prop.into()), f);
    }
    pub fn setter(&mut self, class: &str, prop: &str, f: SetterFn) {
        self.setters.insert((class.into(), prop.into()), f);
    }
    pub fn ctor(&mut self, class: &str, f: CtorFn) {
        self.ctors.insert(class.into(), f);
    }
    pub fn static_fn(&mut self, class: &str, name: &str, f: StaticFn) {
        self.statics.insert((class.into(), name.into()), f);
    }
    pub fn method(&mut self, class: &str, name: &str, f: MethodFn) {
        self.methods.insert((class.into(), name.into()), f);
    }
    pub fn global(&mut self, name: &str, class: &str, f: GlobalFn) {
        self.globals.insert(name.into(), (class.into(), f));
    }
}

// ---------------------------------------------------------------------------
// Source handling: parse, fingerprint, view extraction.

pub fn parse_module(text: &str) -> Result<ast::Module, String> {
    pixie_syntax::parse(FileId(0), text).map_err(|e| e.message)
}

/// Remove position-dependent debris (`Span { .. }`, `BlockId(..)`)
/// from a Debug rendering, so whitespace / body-length edits inside
/// the view don't shift the fingerprint of unrelated items.
fn strip_positions(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    loop {
        let span_at = rest.find("Span {");
        let block_at = rest.find("BlockId(");
        match (span_at, block_at) {
            (None, None) => {
                out.push_str(rest);
                return out;
            }
            (a, b) => {
                let (at, close, skip_open) = if a.is_some() && (b.is_none() || a < b) {
                    (a.unwrap(), '}', "Span {".len())
                } else {
                    (b.unwrap(), ')', "BlockId(".len())
                };
                out.push_str(&rest[..at]);
                let tail = &rest[at + skip_open..];
                match tail.find(close) {
                    Some(end) => rest = &tail[end + 1..],
                    None => {
                        return out;
                    }
                }
            }
        }
    }
}

/// Fingerprint everything the compiled binary depends on: every item,
/// except that a view contributes only its head (name, params, state
/// fields) — its body is the interpreter's slice.
pub fn module_fingerprint(m: &ast::Module) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for item in &m.items {
        match item {
            ast::Item::View(v) => {
                "view".hash(&mut h);
                v.name.name.hash(&mut h);
                strip_positions(&format!("{:?}", v.params)).hash(&mut h);
                strip_positions(&format!("{:?}", v.state_fields)).hash(&mut h);
            }
            // Styles splice into view bodies before either tier reads
            // them, so an edit to one is a view-slice edit: excluded
            // from the AOT fingerprint, style edits hot-reload (rung
            // 2) exactly like view-body edits.
            ast::Item::Style(_) => {}
            other => strip_positions(&format!("{other:?}")).hash(&mut h),
        }
    }
    h.finish()
}

/// The live slice: the view's body plus the names the interpreter
/// resolves dynamically.
pub struct LiveView {
    pub root: ast::Element,
    pub state_cells: Vec<String>,
}

pub fn extract_view(m: &ast::Module) -> Option<LiveView> {
    m.items.iter().find_map(|i| match i {
        ast::Item::View(v) => Some(LiveView {
            root: v.root.clone(),
            state_cells: v
                .state_fields
                .iter()
                .filter(|sf| matches!(sf.kind, StateFieldKind::Property { .. }))
                .map(|sf| sf.name.name.clone())
                .collect(),
        }),
        _ => None,
    })
}

/// What a reload needs from the entry's imports (§8.72): their `pub`
/// styles as source, and their views already resolved.
#[derive(Default)]
pub struct ForeignReload {
    pub styles: String,
    pub views: Vec<(String, Vec<ast::ViewDecl>)>,
}

/// The foreign inputs a reload needs, rebuilt from the imported
/// modules' CURRENT source (§8.72).
///
/// They used to be baked into the binary as text at build time, which
/// froze them: editing another module's `pub style` — or a component
/// body it exports — meant a rebuild, even though both are
/// view-slice material. Reading the files at reload time makes them
/// live, and it is the same filter the driver applies, so visibility
/// is unchanged: only `pub` crosses.
pub fn foreign_reload(sources: &[(String, String)]) -> ForeignReload {
    let mut styles = String::new();
    let mut views: Vec<(String, Vec<ast::ViewDecl>)> = Vec::new();
    for (name, text) in sources {
        let Ok(m) = parse_module(text) else { continue };
        for item in &m.items {
            if let ast::Item::Style(sd) = item {
                if sd.is_pub {
                    let span = sd.span;
                    styles.push_str(&text[span.start as usize..span.end as usize]);
                    styles.push('\n');
                }
            }
        }
        // The module's OWN styles resolve HERE, in the module that
        // wrote them (§8.75), so an exported component travels with
        // its private styles already spliced in. That is also why the
        // views are handed over parsed: after resolving, no source
        // text corresponds to them any more.
        let m = pixie_syntax::iflet::desugar_module(&m);
        let m = pixie_syntax::style::desugar_module_with(&m, &[]).unwrap_or(m);
        // EVERY view, not just the exported ones: a public component's
        // body may use a private sibling (§8.30 — a foreign body
        // resolves in its HOME module). `is_pub` still decides who
        // may NAME one.
        let vs: Vec<ast::ViewDecl> = m
            .items
            .iter()
            .filter_map(|i| match i {
                ast::Item::View(v) => Some(v.clone()),
                _ => None,
            })
            .collect();
        if !vs.is_empty() {
            views.push((name.clone(), vs));
        }
    }
    ForeignReload { styles, views }
}

/// Read the imported modules named by `paths` and prepare what a
/// reload needs from them. A file that has gone missing contributes
/// nothing, which is what a half-saved edit looks like — the caller
/// keeps its last good view rather than tearing the window down.
pub fn foreign_reload_from_paths(paths: &[(String, String)]) -> ForeignReload {
    let sources: Vec<(String, String)> = paths
        .iter()
        .filter_map(|(name, path)| {
            std::fs::read_to_string(path)
                .ok()
                .map(|t| (name.clone(), t))
        })
        .collect();
    foreign_reload(&sources)
}

/// The whole program's shape: the entry module plus every import
/// (§8.72). `module_fingerprint` omits styles and view bodies, so
/// this changes exactly when something the COMPILED half owns
/// changes — a class, a store, a view's state fields — and not when
/// a style or a view body does, in the entry or in an import.
///
/// One function, called by all three places that ask: the build
/// bakes it, `pixie watch` compares it, and the running binary
/// re-derives it. The first time these were three separate
/// expressions they disagreed, which is how a hot reload silently
/// shows the wrong thing.
pub fn program_fingerprint(
    entry_spliced: &ast::Module,
    foreign_paths: &[(String, String)],
) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    module_fingerprint(entry_spliced).hash(&mut h);
    for (name, path) in foreign_paths {
        name.hash(&mut h);
        match std::fs::read_to_string(path) {
            Ok(t) => match parse_module(&t) {
                Ok(fm) => module_fingerprint(&fm).hash(&mut h),
                // A module caught mid-save hashes as its own text, so
                // the caller waits for the next write instead of
                // acting on a half-written file.
                Err(_) => t.hash(&mut h),
            },
            Err(_) => "missing".hash(&mut h),
        }
    }
    h.finish()
}

/// `program_fingerprint` from the entry's SOURCE — the shape the
/// running binary and the watch loop both have.
pub fn program_fingerprint_of(
    entry_text: &str,
    foreign_paths: &[(String, String)],
) -> Result<u64, String> {
    let m = parse_module(entry_text)?;
    let m = pixie_syntax::iflet::desugar_module(&m);
    let foreign = foreign_reload_from_paths(foreign_paths);
    let m = pixie_syntax::component::splice_module_with(&m, &foreign.views, true)
        .map_err(|e| e.message)?;
    Ok(program_fingerprint(&m, foreign_paths))
}

/// Parse + fingerprint + extract in one step (the reload path).
pub fn reload_from_source(text: &str) -> Result<(u64, LiveView), String> {
    reload_from_source_with(text, &ForeignReload::default())
}

/// Parse the baked foreign-components snippet: segments introduced by
/// `//pixie module: <name>` lines, each segment a module's views.
pub fn parse_foreign_components(
    snippet: &str,
) -> Result<Vec<(String, Vec<pixie_syntax::ast::ViewDecl>)>, String> {
    let mut out: Vec<(String, Vec<pixie_syntax::ast::ViewDecl>)> = Vec::new();
    if snippet.trim().is_empty() {
        return Ok(out);
    }
    let mut current: Option<(String, String)> = None;
    let finish = |cur: Option<(String, String)>,
                      out: &mut Vec<(String, Vec<pixie_syntax::ast::ViewDecl>)>|
     -> Result<(), String> {
        if let Some((name, buf)) = cur {
            let m = parse_module(&buf)
                .map_err(|e| format!("foreign components of `{name}`: {e}"))?;
            let vs: Vec<pixie_syntax::ast::ViewDecl> = m
                .items
                .into_iter()
                .filter_map(|i| match i {
                    ast::Item::View(v) => Some(v),
                    _ => None,
                })
                .collect();
            out.push((name, vs));
        }
        Ok(())
    };
    for line in snippet.lines() {
        if let Some(name) = line.strip_prefix("//pixie module: ") {
            finish(current.take(), &mut out)?;
            current = Some((name.trim().to_string(), String::new()));
        } else if let Some((_, buf)) = &mut current {
            buf.push_str(line);
            buf.push('\n');
        }
    }
    finish(current, &mut out)?;
    Ok(out)
}

/// `reload_from_source` with the cross-module leg: `foreign_styles`
/// is the `pub style` source snippet of the entry's imports, baked
/// into the binary at build time. Editing a FOREIGN style therefore
/// needs a rebuild — only entry-file edits hot-reload.
pub fn reload_from_source_with(
    text: &str,
    foreign: &ForeignReload,
) -> Result<(u64, LiveView), String> {
    let m = parse_module(text)?;
    // Component splice BEFORE the fingerprint: hoisted per-instance
    // state lands in the root view's field list, which the
    // fingerprint hashes — adding/removing a stateful component use
    // is a rung-1 edit, body edits stay rung 2. `reload_info` at
    // build time runs the same order. Imported components resolve
    // from the baked snippet (§8.29).
    let foreign_views = foreign.views.clone();
    // Same desugar the driver runs (§8.69), in the same place.
    let m = pixie_syntax::iflet::desugar_module(&m);
    let m = pixie_syntax::component::splice_module_with(&m, &foreign_views, true)
        .map_err(|e| e.message)?;
    // The style splice runs after, view-slice only.
    let fp = module_fingerprint(&m);
    let foreign_mod = if foreign.styles.is_empty() {
        None
    } else {
        Some(parse_module(&foreign.styles)?)
    };
    let foreign: Vec<&pixie_syntax::ast::StyleDecl> = foreign_mod
        .iter()
        .flat_map(|fm| {
            fm.items.iter().filter_map(|i| match i {
                ast::Item::Style(s) => Some(s),
                _ => None,
            })
        })
        .collect();
    let m = pixie_syntax::style::desugar_module_with(&m, &foreign).map_err(|e| e.message)?;
    let lv = extract_view(&m).ok_or_else(|| "no `view` in source".to_string())?;
    Ok((fp, lv))
}

// ---------------------------------------------------------------------------
// The interpreter.

/// Object fields of the live view: (surface name, class name, handle).
/// Includes the synthesized `__pixie_state` holder when state cells
/// exist.
pub struct FieldEnv {
    pub fields: Vec<(String, String, ErasedHandle)>,
}

/// What action closures capture: tables + the field map + state-cell
/// names, all behind Rc.
#[derive(Clone)]
struct ClosEnv {
    tables: Rc<Tables>,
    fields: Rc<Vec<(String, String, ErasedHandle)>>,
    state_cells: Rc<Vec<String>>,
}

impl ClosEnv {
    /// Resolve a base object name: view field or global.
    fn base(&self, name: &str, w: &World) -> Option<(String, ErasedHandle)> {
        if let Some((_, class, h)) = self.fields.iter().find(|(n, _, _)| n == name) {
            return Some((class.clone(), *h));
        }
        if let Some((class, f)) = self.tables.globals.get(name) {
            return Some((class.clone(), f(w)));
        }
        None
    }
    fn holder(&self) -> Result<(String, ErasedHandle), String> {
        self.fields
            .iter()
            .find(|(n, _, _)| n == "__pixie_state")
            .map(|(_, c, h)| (c.clone(), *h))
            .ok_or_else(|| "state cell used but no state holder mounted".to_string())
    }
    fn get_prop(&self, class: &str, prop: &str, h: ErasedHandle, w: &World) -> Result<Value, String> {
        match self.tables.getters.get(&(class.to_string(), prop.to_string())) {
            Some(f) => Ok(f(w, h)),
            None => Err(format!("no readable property `{prop}` on `{class}`")),
        }
    }
    fn set_prop(
        &self,
        class: &str,
        prop: &str,
        h: ErasedHandle,
        v: Value,
        w: &mut World,
    ) -> Result<(), String> {
        match self.tables.setters.get(&(class.to_string(), prop.to_string())) {
            Some(f) => f(w, h, v),
            None => Err(format!("no writable property `{prop}` on `{class}`")),
        }
    }
}

/// Per-evaluation scope: loop variables (view side) or action locals.
#[derive(Default)]
struct Scope {
    vars: Vec<(String, Value)>,
    /// Index path of the enclosing `for` repeaters, outermost first,
    /// for `__PixieRowScope` (per-row component state, §8.30/§8.34).
    /// Empty outside repeaters.
    row_path: Vec<usize>,
}

fn eval_expr(e: &Expr, env: &ClosEnv, scope: &Scope, w: &World) -> Result<Value, String> {
    match &e.kind {
        ExprKind::Int(v) => Ok(Value::Int(*v)),
        ExprKind::Float(v) => Ok(Value::Float(*v)),
        ExprKind::Bool(v) => Ok(Value::Bool(*v)),
        ExprKind::Str(parts) => {
            let mut out = String::new();
            for p in parts {
                match p {
                    StrPart::Text(t) => out.push_str(t),
                    StrPart::Interp(inner) => {
                        let v = eval_expr(inner, env, scope, w)?;
                        let _ = write!(out, "{}", v.render());
                    }
                    // §8.54, the interpreted half. The compiled tier
                    // hands the spec to `format!`; there is no
                    // `format!` at run time, so the same grammar is
                    // applied by hand — and it has to produce the
                    // SAME bytes, which the tier gate checks.
                    StrPart::InterpFmt { expr, format_spec } => {
                        let v = eval_expr(expr, env, scope, w)?;
                        out.push_str(&render_formatted(&v, format_spec)?);
                    }
                }
            }
            Ok(Value::Str(Str::from(out)))
        }
        ExprKind::Ident(n) => {
            if let Some((_, v)) = scope.vars.iter().rev().find(|(s, _)| s == n) {
                return Ok(v.clone());
            }
            if env.state_cells.iter().any(|c| c == n) {
                let (class, h) = env.holder()?;
                return env.get_prop(&class, n, h, w);
            }
            Err(format!("`{n}` is not a loop variable, local, or state cell"))
        }
        ExprKind::Member { receiver, name } => {
            // §8.68: the two map built-ins a view can iterate. Checked
            // against the receiver's VALUE rather than the name alone,
            // because a class may perfectly well have a property
            // called `values` — the charts demo does. The pair list is
            // already in key order, so both come back in the order the
            // compiled tier's `BTreeMap` produces.
            if name.name == "keys" || name.name == "values" {
                if let Ok(Value::Map(kv)) = eval_expr(receiver, env, scope, w) {
                    let want_keys = name.name == "keys";
                    return Ok(Value::List(
                        kv.into_iter()
                            .map(|(k, val)| if want_keys { k } else { val })
                            .collect(),
                    ));
                }
            }
            // A field of a STRUCT value (§8.68).
            if let Ok(Value::Struct(_, fields)) = eval_expr(receiver, env, scope, w) {
                if let Some((_, v)) = fields.iter().find(|(f, _)| f == &name.name) {
                    return Ok(v.clone());
                }
            }
            if name.name == "length" {
                let v = eval_expr(receiver, env, scope, w)?;
                return match v {
                    Value::List(xs) => Ok(Value::Int(xs.len() as i64)),
                    Value::Str(s) => Ok(Value::Int(s.len() as i64)),
                    // §8.68: a map and a byte string both have one.
                    Value::Map(kv) => Ok(Value::Int(kv.len() as i64)),
                    Value::Bytes(b) => Ok(Value::Int(b.len() as i64)),
                    other => Err(format!("`length` on {}", other.type_name())),
                };
            }
            if let ExprKind::Ident(r) = &receiver.kind {
                if let Some((class, h)) = env.base(r, w) {
                    return env.get_prop(&class, &name.name, h, w);
                }
            }
            // Reaching THROUGH an object (§8.41): the receiver
            // evaluates to a handle carrying its class, so the next
            // property read dispatches through the same tables. This
            // is what makes `note.tag.label` and `kept[0].tag.label`
            // readable from a view, matching the compiled tier.
            let base = eval_expr(receiver, env, scope, w)?;
            match base {
                Value::Object(h, class) => env.get_prop(&class, &name.name, h, w),
                other => Err(format!(
                    "`{}` needs an object or a field/global base, got {}",
                    name.name,
                    other.type_name()
                )),
            }
        }
        ExprKind::Binary { op, lhs, rhs } => {
            let l = eval_expr(lhs, env, scope, w)?;
            let r = eval_expr(rhs, env, scope, w)?;
            eval_binop(op, l, r)
        }
        ExprKind::Unary { op, expr } => {
            let v = eval_expr(expr, env, scope, w)?;
            match op {
                UnaryOp::Neg => match v {
                    Value::Int(x) => Ok(Value::Int(-x)),
                    Value::Float(x) => Ok(Value::Float(-x)),
                    other => Err(format!("cannot negate {}", other.type_name())),
                },
                UnaryOp::Not => Ok(Value::Bool(!v.as_bool()?)),
            }
        }
        ExprKind::Await(_) => Err("`await` is not available in interpreted views".into()),
        // `xs[i]` / `m[k]` in a view (§8.38's spec, interpreted side).
        ExprKind::Index { receiver, index } => {
            let base = eval_expr(receiver, env, scope, w)?;
            let i = eval_expr(index, env, scope, w)?;
            match base {
                Value::List(xs) => {
                    let n = xs.len() as i64;
                    let k = i.as_int()?;
                    if k < 0 || k >= n {
                        return Err(format!("list index {k} out of range (length {n})"));
                    }
                    Ok(xs[k as usize].clone())
                }
                // §8.68: a map subscript answers `T?`, so a missing
                // key is `nil` rather than a trap — the read side of
                // `m[k]`'s contract, matching `Map::at`.
                Value::Map(kv) => Ok(kv
                    .into_iter()
                    .find(|(k, _)| value_key_eq(k, &i))
                    .map(|(_, v)| v)
                    .unwrap_or(Value::Nil)),
                other => Err(format!("cannot index {}", other.type_name())),
            }
        }
        ExprKind::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(eval_expr(item, env, scope, w)?);
            }
            Ok(Value::List(out))
        }
        // `C.staticFn(args)` (§8.54): the one call a view may make —
        // World-free by definition, dispatched through the statics
        // table the compiled tier registers.
        ExprKind::MethodCall {
            receiver,
            method,
            args,
            block: None,
            ..
        } => {
            if let ExprKind::Ident(recv) = &receiver.kind {
                if let Some(f) = env
                    .tables
                    .statics
                    .get(&(recv.clone(), method.name.clone()))
                {
                    let mut vals = Vec::with_capacity(args.len());
                    for a in args {
                        vals.push(eval_expr(a, env, scope, w)?);
                    }
                    return f(vals);
                }
            }
            Err("this expression is not interpretable in views yet".into())
        }
        _ => Err("this expression is not interpretable in views yet".into()),
    }
}

/// One formatted interpolation. Mirrors what `format!` does for the
/// spec subset `pixie_codegen::rust_format_spec` admits: an optional
/// fill+align, an optional zero pad, a width, a `.precision`, and a
/// printf-style type letter.
fn render_formatted(v: &Value, spec: &str) -> Result<String, String> {
    let mut rest = spec;
    // A trailing type letter. `x`/`X`/`o`/`b` change the RENDERING;
    // the rest only say what the value is, which is already known.
    let mut radix = None;
    let mut float_spec = false;
    if let Some(c) = rest.chars().last() {
        match c {
            'x' | 'X' | 'o' | 'b' => {
                radix = Some(c);
                rest = &rest[..rest.len() - c.len_utf8()];
            }
            'f' | 'd' | 'e' | 's' => {
                if c == 'f' {
                    float_spec = true;
                }
                rest = &rest[..rest.len() - c.len_utf8()];
            }
            _ => {}
        }
    }
    // fill + align
    let chars: Vec<char> = rest.chars().collect();
    let (mut fill, mut align, mut i) = (' ', None, 0usize);
    if chars.len() >= 2 && matches!(chars[1], '<' | '^' | '>') {
        fill = chars[0];
        align = Some(chars[1]);
        i = 2;
    } else if !chars.is_empty() && matches!(chars[0], '<' | '^' | '>') {
        align = Some(chars[0]);
        i = 1;
    }
    if i < chars.len() && chars[i] == '0' {
        fill = '0';
        if align.is_none() {
            align = Some('>');
        }
        i += 1;
    }
    let mut width = 0usize;
    while i < chars.len() && chars[i].is_ascii_digit() {
        width = width * 10 + chars[i].to_digit(10).unwrap() as usize;
        i += 1;
    }
    let mut precision = None;
    if i < chars.len() && chars[i] == '.' {
        i += 1;
        let mut n = 0usize;
        while i < chars.len() && chars[i].is_ascii_digit() {
            n = n * 10 + chars[i].to_digit(10).unwrap() as usize;
            i += 1;
        }
        precision = Some(n);
    }
    if i != chars.len() {
        return Err(format!("`{spec}` is not a format spec"));
    }

    let mut body = match (radix, v) {
        (Some('x'), Value::Int(n)) => format!("{n:x}"),
        (Some('X'), Value::Int(n)) => format!("{n:X}"),
        (Some('o'), Value::Int(n)) => format!("{n:o}"),
        (Some('b'), Value::Int(n)) => format!("{n:b}"),
        (Some(_), other) => {
            return Err(format!(
                "a radix format needs an Int, got {}",
                other.type_name()
            ));
        }
        (None, Value::Float(f)) => {
            if float_spec && f.is_nan() {
                // The f-spec spells NaN the way Python does — the
                // compiled tier does the same, by the same condition.
                "nan".to_string()
            } else {
                match precision {
                    Some(p) => format!("{f:.*}", p),
                    None => f.to_string(),
                }
            }
        }
        (None, other) => match (precision, other) {
            // Rust truncates a string to its precision.
            (Some(p), Value::Str(s)) => s.as_str().chars().take(p).collect(),
            _ => other.render(),
        },
    };
    if body.chars().count() < width {
        let pad = width - body.chars().count();
        // Zero padding goes AFTER the sign: `-7` at width 5 is
        // `-0007`, never `000-7`. Only zero-fill does this — a space
        // or a custom fill pads outside the sign like any other
        // character.
        if fill == '0' && matches!(align, Some('>')) {
            if let Some(rest) = body.strip_prefix('-') {
                return Ok(format!("-{}{rest}", "0".repeat(pad)));
            }
            if let Some(rest) = body.strip_prefix('+') {
                return Ok(format!("+{}{rest}", "0".repeat(pad)));
            }
        }
        body = match align.unwrap_or(if matches!(v, Value::Str(_)) { '<' } else { '>' }) {
            '<' => format!("{body}{}", fill.to_string().repeat(pad)),
            '^' => {
                let l = pad / 2;
                format!(
                    "{}{body}{}",
                    fill.to_string().repeat(l),
                    fill.to_string().repeat(pad - l)
                )
            }
            _ => format!("{}{body}", fill.to_string().repeat(pad)),
        };
    }
    Ok(body)
}

fn eval_binop(op: &BinOp, l: Value, r: Value) -> Result<Value, String> {
    use Value::*;
    Ok(match (op, l, r) {
        (BinOp::Add, Int(a), Int(b)) => Int(a + b),
        (BinOp::Sub, Int(a), Int(b)) => Int(a - b),
        (BinOp::Mul, Int(a), Int(b)) => Int(a * b),
        (BinOp::Div, Int(a), Int(b)) => {
            if b == 0 {
                return Err("division by zero".into());
            }
            Int(a / b)
        }
        (BinOp::Mod, Int(a), Int(b)) => {
            if b == 0 {
                return Err("modulo by zero".into());
            }
            Int(a % b)
        }
        // String concatenation — mirrors the kernel's `Add for Str`.
        (BinOp::Add, Str(a), Str(b)) => Str(a + b),
        (BinOp::Add, Float(a), Float(b)) => Float(a + b),
        (BinOp::Sub, Float(a), Float(b)) => Float(a - b),
        (BinOp::Mul, Float(a), Float(b)) => Float(a * b),
        (BinOp::Div, Float(a), Float(b)) => Float(a / b),
        (BinOp::Lt, Int(a), Int(b)) => Bool(a < b),
        (BinOp::LtEq, Int(a), Int(b)) => Bool(a <= b),
        (BinOp::Gt, Int(a), Int(b)) => Bool(a > b),
        (BinOp::GtEq, Int(a), Int(b)) => Bool(a >= b),
        (BinOp::Lt, Float(a), Float(b)) => Bool(a < b),
        (BinOp::LtEq, Float(a), Float(b)) => Bool(a <= b),
        (BinOp::Gt, Float(a), Float(b)) => Bool(a > b),
        (BinOp::GtEq, Float(a), Float(b)) => Bool(a >= b),
        (BinOp::Eq, a, b) => Bool(value_eq(&a, &b)?),
        (BinOp::NotEq, a, b) => Bool(!value_eq(&a, &b)?),
        (BinOp::And, Bool(a), Bool(b)) => Bool(a && b),
        (BinOp::Or, Bool(a), Bool(b)) => Bool(a || b),
        (op, l, r) => {
            return Err(format!(
                "operator {op:?} not interpretable on {} / {}",
                l.type_name(),
                r.type_name()
            ));
        }
    })
}

fn value_eq(a: &Value, b: &Value) -> Result<bool, String> {
    use Value::*;
    Ok(match (a, b) {
        (Int(x), Int(y)) => x == y,
        (Float(x), Float(y)) => x == y,
        (Bool(x), Bool(y)) => x == y,
        (Str(x), Str(y)) => x == y,
        _ => {
            return Err(format!(
                "cannot compare {} with {}",
                a.type_name(),
                b.type_name()
            ));
        }
    })
}

// ---------------------------------------------------------------------------
// Actions.

/// Evaluate an expression in a handler, where the World is MUTABLE.
/// Almost everything delegates to the read-only `eval_expr`; the one
/// thing that cannot is constructing an object, which needs to insert
/// (§8.53). Splitting it this way keeps `eval_expr` — used by the
/// pure view build — unable to mutate by construction.
fn eval_action_expr(
    e: &Expr,
    env: &ClosEnv,
    scope: &mut Scope,
    w: &mut World,
) -> Result<Value, String> {
    if let ExprKind::Call { callee, args, block: None, .. } = &e.kind {
        if let ExprKind::Ident(class) = &callee.kind {
            if let Some(f) = env.tables.ctors.get(class).copied() {
                let mut vals = Vec::with_capacity(args.len());
                for a in args {
                    vals.push(eval_action_expr(a, env, scope, w)?);
                }
                let h = f(w, vals)?;
                return Ok(Value::Object(h, class.clone()));
            }
        }
    }
    // A method call may itself construct, so it cannot go through the
    // read-only path either.
    if let ExprKind::MethodCall { receiver, method, args, block: None, .. } = &e.kind {
        // On a handler local holding an object.
        if let ExprKind::Ident(rn) = &receiver.kind {
            if let Some((_, Value::Object(oh, oc))) =
                scope.vars.iter().rev().find(|(s2, _)| s2 == rn)
            {
                let (oh, oc) = (*oh, oc.clone());
                if let Some(f) = env.tables.methods.get(&(oc.clone(), method.name.clone())).copied()
                {
                    let mut vals = Vec::with_capacity(args.len());
                    for a in args {
                        vals.push(eval_action_expr(a, env, scope, w)?);
                    }
                    f(w, oh, vals)?;
                    return Ok(Value::Unit);
                }
            }
        }
        if let ExprKind::Ident(r) = &receiver.kind {
            if let Some((class, h)) = env.base(r, w) {
                if env
                    .tables
                    .methods
                    .contains_key(&(class.clone(), method.name.clone()))
                {
                    let mut vals = Vec::with_capacity(args.len());
                    for a in args {
                        vals.push(eval_action_expr(a, env, scope, w)?);
                    }
                    let f = env.tables.methods[&(class.clone(), method.name.clone())];
                    f(w, h, vals)?;
                    // A `Void` method used as a value is a checker
                    // error, so nothing observes this.
                    return Ok(Value::Unit);
                }
            }
        }
    }
    eval_expr(e, env, scope, w)
}

fn eval_action(e: &Expr, env: &ClosEnv, scope: &mut Scope, w: &mut World) -> Result<(), String> {
    match &e.kind {
        // Direct method call: `field.method(args)` / `Global.method(args)`.
        ExprKind::MethodCall {
            receiver,
            method,
            args,
            block: None,
            ..
        } => {
            let ExprKind::Ident(r) = &receiver.kind else {
                return Err("handlers call a field/global method or run a block".into());
            };
            let Some((class, h)) = env.base(r, w) else {
                return Err(format!("`{r}` is not a view field or global"));
            };
            let mut vals = Vec::with_capacity(args.len());
            for a in args {
                vals.push(eval_action_expr(a, env, scope, w)?);
            }
            let Some(f) = env
                .tables
                .methods
                .get(&(class.clone(), method.name.clone()))
            else {
                return Err(format!(
                    "no invokable method `{}` on `{class}`",
                    method.name
                ));
            };
            f(w, h, vals)
        }
        ExprKind::Block(b) => {
            run_action_block(b, env, scope, w)?;
            Ok(())
        }
        _ => Err("handlers call a field/global method or run a block".into()),
    }
}

/// Returns the control flow the statement produced — `Flow::Return`
/// has to travel out through an enclosing loop, the way the compiled
/// tier's `return` leaves the whole closure (§8.62).
fn eval_action_stmt(
    s: &Stmt,
    env: &ClosEnv,
    scope: &mut Scope,
    w: &mut World,
) -> Result<Flow, String> {
    match s {
        Stmt::Let { name, value, .. } | Stmt::Var { name, value, .. } => {
            let v = eval_action_expr(value, env, scope, w)?;
            scope.vars.push((name.name.clone(), v));
            Ok(Flow::Normal)
        }
        Stmt::Assign {
            target, op, value, ..
        } => {
            let v = eval_expr(value, env, scope, w)?;
            // Resolve the assign target: `field.prop` / `Global.prop`,
            // a bare state cell, or an action local.
            let (class, prop, h) = match &target.kind {
                ExprKind::Member { receiver, name } => {
                    // Through a handler local holding an object
                    // (§8.53) — the compiled tier's mirror.
                    if let ExprKind::Ident(rn) = &receiver.kind {
                        if let Some((_, Value::Object(oh, oc))) =
                            scope.vars.iter().rev().find(|(s2, _)| s2 == rn)
                        {
                            let (oh, oc) = (*oh, oc.clone());
                            let next = apply_assign_op(
                                op,
                                || env.get_prop(&oc, &name.name, oh, w).unwrap_or(Value::Unit),
                                v,
                            )?;
                            env.set_prop(&oc, &name.name, oh, next, w)?;
                            return Ok(Flow::Normal);
                        }
                    }
                    let ExprKind::Ident(r) = &receiver.kind else {
                        return Err("assignment target must be `field.prop` or a state cell".into());
                    };
                    let Some((class, h)) = env.base(r, w) else {
                        return Err(format!("`{r}` is not a view field or global"));
                    };
                    (class, name.name.clone(), h)
                }
                ExprKind::Ident(n) => {
                    if let Some(slot) = scope.vars.iter_mut().rev().find(|(s2, _)| s2 == n) {
                        let next = apply_assign_op(op, || slot.1.clone(), v)?;
                        slot.1 = next;
                        return Ok(Flow::Normal);
                    }
                    if env.state_cells.iter().any(|c| c == n) {
                        let (class, h) = env.holder()?;
                        (class, n.clone(), h)
                    } else {
                        return Err(format!("`{n}` is not assignable here"));
                    }
                }
                ExprKind::Index { receiver, index } => {
                    // `xs[i] = v` / `m[k] = v` — both tiers now.
                    if !matches!(op, AssignOp::Eq) {
                        return Err("compound assignment through an index is not supported".into());
                    }
                    let ix = eval_expr(index, env, scope, w)?;
                    let (class, prop, h) = match &receiver.kind {
                        ExprKind::Ident(n) if env.state_cells.iter().any(|c| c == n) => {
                            let (class, h) = env.holder()?;
                            (class, n.clone(), h)
                        }
                        ExprKind::Member { receiver: r2, name } => {
                            let ExprKind::Ident(r) = &r2.kind else {
                                return Err("index-assignment target must be a property".into());
                            };
                            let Some((class, h)) = env.base(r, w) else {
                                return Err(format!("`{r}` is not a view field or global"));
                            };
                            (class, name.name.clone(), h)
                        }
                        _ => return Err("index-assignment target must be a property".into()),
                    };
                    let cur = env.get_prop(&class, &prop, h, w)?;
                    let next = match (cur, ix) {
                        (Value::List(mut xs), Value::Int(i)) => {
                            let ln = xs.len() as i64;
                            if i < 0 || i >= ln {
                                return Err(format!("index {i} out of range (len {ln})"));
                            }
                            xs[i as usize] = v;
                            Value::List(xs)
                        }
                        (Value::Map(mut kv), key) => {
                            match kv.iter_mut().find(|(k2, _)| value_key_eq(k2, &key)) {
                                Some(slot) => slot.1 = v,
                                None => {
                                    kv.push((key, v));
                                    kv.sort_by(|a, b| value_key_cmp(&a.0, &b.0));
                                }
                            }
                            Value::Map(kv)
                        }
                        (other, _) => {
                            return Err(format!(
                                "`{prop}` ({}) is not indexable for assignment",
                                other.type_name()
                            ))
                        }
                    };
                    env.set_prop(&class, &prop, h, next, w)?;
                    return Ok(Flow::Normal);
                }
                _ => return Err("assignment target must be `field.prop` or a state cell".into()),
            };
            let next = apply_assign_op(op, || env.get_prop(&class, &prop, h, w).unwrap_or(Value::Unit), v)?;
            env.set_prop(&class, &prop, h, next, w)?;
            Ok(Flow::Normal)
        }
        Stmt::Expr(e) => {
            // `x.list.push(v)` — the COW read-modify-writeback.
            if let ExprKind::MethodCall {
                receiver,
                method,
                args,
                block: None,
                ..
            } = &e.kind
            {
                if method.name == "push" {
                    if let ExprKind::Member { receiver: r2, name: pname } = &receiver.kind {
                        if let ExprKind::Ident(r) = &r2.kind {
                            if let Some((class, h)) = env.base(r, w) {
                                if args.len() != 1 {
                                    return Err("`push` takes one argument".into());
                                }
                                let v = eval_expr(&args[0], env, scope, w)?;
                                let cur = env.get_prop(&class, &pname.name, h, w)?;
                                let Value::List(mut xs) = cur else {
                                    return Err(format!("`{}` is not a List", pname.name));
                                };
                                xs.push(v);
                                env.set_prop(&class, &pname.name, h, Value::List(xs), w)?;
                                return Ok(Flow::Normal);
                            }
                        }
                    }
                }
            }
            // A method call in statement position may be on an
            // object local, or may construct — both need the mutable
            // path, and `eval_action` only knows field/global
            // receivers (§8.53).
            if matches!(
                e.kind,
                ExprKind::MethodCall { block: None, .. } | ExprKind::Call { block: None, .. }
            ) {
                eval_action_expr(e, env, scope, w)?;
                return Ok(Flow::Normal);
            }
            eval_action(e, env, scope, w)?;
            Ok(Flow::Normal)
        }
        // Control flow in a handler (§8.53) — the compiled tier's
        // mirror, statement for statement, because a divergence here
        // is exactly what the tier gate exists to catch.
        Stmt::For { binding, index, iter, body, .. } => {
            let depth = scope.vars.len();
            match &iter.kind {
                ExprKind::Range { start, end, inclusive } => {
                    let a = eval_expr(start, env, scope, w)?.as_int()?;
                    let b = eval_expr(end, env, scope, w)?.as_int()?;
                    let last = if *inclusive { b } else { b - 1 };
                    let mut i = a;
                    let mut turn = 0i64;
                    while i <= last {
                        scope.vars.truncate(depth);
                        scope.vars.push((binding.name.clone(), Value::Int(i)));
                        if let Some(ix) = index {
                            scope.vars.push((ix.name.clone(), Value::Int(turn)));
                        }
                        turn += 1;
                        match run_action_block(body, env, scope, w)? {
                            Flow::Break => break,
                            Flow::Return => {
                                scope.vars.truncate(depth);
                                return Ok(Flow::Return);
                            }
                            Flow::Normal | Flow::Continue => {}
                        }
                        i += 1;
                    }
                }
                _ => {
                    let Value::List(xs) = eval_expr(iter, env, scope, w)? else {
                        return Err("`for` needs a List or a range".into());
                    };
                    for (n, item) in xs.iter().enumerate() {
                        scope.vars.truncate(depth);
                        scope.vars.push((binding.name.clone(), item.clone()));
                        if let Some(ix) = index {
                            scope.vars.push((ix.name.clone(), Value::Int(n as i64)));
                        }
                        match run_action_block(body, env, scope, w)? {
                            Flow::Break => break,
                            Flow::Return => {
                                scope.vars.truncate(depth);
                                return Ok(Flow::Return);
                            }
                            Flow::Normal | Flow::Continue => {}
                        }
                    }
                }
            }
            scope.vars.truncate(depth);
            Ok(Flow::Normal)
        }
        Stmt::While { cond, body, .. } => {
            let depth = scope.vars.len();
            // The compiled tier trusts rustc to stop a runaway loop;
            // the interpreter is running inside the editor's process,
            // so it caps instead of hanging the window.
            let mut spins = 0u32;
            while eval_expr(cond, env, scope, w)?.as_bool()? {
                match run_action_block(body, env, scope, w)? {
                    Flow::Break => break,
                    Flow::Return => {
                        scope.vars.truncate(depth);
                        return Ok(Flow::Return);
                    }
                    Flow::Normal | Flow::Continue => {}
                }
                scope.vars.truncate(depth);
                spins += 1;
                if spins > 10_000_000 {
                    return Err("`while` in a handler ran 10M times — stopping".into());
                }
            }
            scope.vars.truncate(depth);
            Ok(Flow::Normal)
        }
        _ => Err("this statement is not interpretable in handlers yet".into()),
    }
}

/// How a handler block finished. `break` and `continue` have to reach
/// the enclosing loop without unwinding through the error channel,
/// which the interpreter uses for real failures.
enum Flow {
    Normal,
    Break,
    Continue,
    /// A bare `return`: the handler is done (§8.62).
    Return,
}

fn run_action_block(
    b: &ast::Block,
    env: &ClosEnv,
    scope: &mut Scope,
    w: &mut World,
) -> Result<Flow, String> {
    for st in &b.stmts {
        match st {
            Stmt::Break { .. } => return Ok(Flow::Break),
            Stmt::Continue { .. } => return Ok(Flow::Continue),
            // Mirrors codegen exactly (§8.62): a bare `return` is an
            // early exit, a value has nowhere to go.
            Stmt::Return { value: None, .. } => return Ok(Flow::Return),
            Stmt::Return { value: Some(_), .. } => {
                return Err(
                    "a handler runs for effect and returns nothing — write a bare \
                     `return` to stop early, or store the value in a property"
                        .into(),
                );
            }
            Stmt::Emit { signal, .. } => {
                return Err(format!(
                    "`emit` sends a signal from the object that owns it, and a handler \
                     is not inside one. Call a method that emits `{}`",
                    signal.name
                ));
            }
            Stmt::Batch { .. } => {
                return Err(
                    "writes are already batched: no view rebuilds until the handler \
                     returns, and writing one property twice notifies once. Drop the \
                     `batch` block"
                        .into(),
                );
            }
            // `case` in a handler (§8.69), and therefore `if let`.
            // Mirrors codegen: a `T?` binds what it holds, an enum
            // matches by variant name, and an unlisted variant does
            // nothing.
            Stmt::Expr(e) if matches!(e.kind, ExprKind::Case { .. }) => {
                let ExprKind::Case { scrutinee, arms } = &e.kind else {
                    unreachable!("guarded")
                };
                let v = eval_action_expr(scrutinee, env, scope, w)?;
                let taken: Option<(&ast::Block, Option<Vec<(String, Value)>>)> = match &v {
                    Value::Nil => arms
                        .iter()
                        .find(|a| !is_some_pattern(&a.pattern))
                        .map(|a| (&a.body, None)),
                    _ => match arms.iter().find(|a| is_some_pattern(&a.pattern)) {
                        Some(arm) => {
                            let bind = match &arm.pattern {
                                ast::Pattern::Ctor { args, .. } => match args.as_slice() {
                                    [ast::Pattern::Bind { name, .. }] => {
                                        Some(vec![(name.name.clone(), v.clone())])
                                    }
                                    _ => None,
                                },
                                _ => None,
                            };
                            Some((&arm.body, bind))
                        }
                        None => {
                            // Name-only enums arrive as Str(name);
                            // payload ones as Struct(variant, fields)
                            // — match the name, bind the fields.
                            let (name, fields) = match &v {
                                Value::Struct(n2, fs) => (n2.clone(), Some(fs.clone())),
                                other => (other.render(), None),
                            };
                            arms.iter()
                                .find(|a| match &a.pattern {
                                    ast::Pattern::Ctor { name: n, .. } => n.name == name,
                                    ast::Pattern::Wild { .. } => true,
                                    _ => false,
                                })
                                .map(|a| (&a.body, enum_arm_binds(&a.pattern, fields.as_deref())))
                        }
                    },
                };
                if let Some((body, bind)) = taken {
                    let depth = scope.vars.len();
                    if let Some(bs) = bind {
                        for b in bs {
                            scope.vars.push(b);
                        }
                    }
                    let flow = run_action_block(body, env, scope, w)?;
                    scope.vars.truncate(depth);
                    if !matches!(flow, Flow::Normal) {
                        return Ok(flow);
                    }
                }
            }
            Stmt::Expr(e) if matches!(e.kind, ExprKind::If { .. }) => {
                let ExprKind::If { cond, then_b, else_b, let_binding } = &e.kind else {
                    unreachable!("guarded")
                };
                if let_binding.is_some() {
                    return Err("`if let` survived the desugar (§8.69) — this is a pixie bug".into());
                }
                let taken = if eval_expr(cond, env, scope, w)?.as_bool()? {
                    Some(then_b)
                } else {
                    else_b.as_ref()
                };
                if let Some(blk) = taken {
                    match run_action_block(blk, env, scope, w)? {
                        Flow::Normal => {}
                        other => return Ok(other),
                    }
                }
            }
            other => match eval_action_stmt(other, env, scope, w)? {
                Flow::Normal => {}
                f => return Ok(f),
            },
        }
    }
    if let Some(t) = &b.trailing {
        let stmt = Stmt::Expr((**t).clone());
        match &stmt {
            // A block's last item is its trailing expression, and the
            // control-flow shapes are statements — route them back
            // through the statement walk rather than the value one
            // (§8.69 added `Case` to what that covers).
            Stmt::Expr(e)
                if matches!(e.kind, ExprKind::If { .. } | ExprKind::Case { .. }) =>
            {
                let mut one = b.clone();
                one.stmts = vec![stmt];
                one.trailing = None;
                return run_action_block(&one, env, scope, w);
            }
            _ => match eval_action_stmt(&stmt, env, scope, w)? {
                Flow::Normal => {}
                f => return Ok(f),
            },
        }
    }
    Ok(Flow::Normal)
}

fn apply_assign_op(
    op: &AssignOp,
    current: impl FnOnce() -> Value,
    v: Value,
) -> Result<Value, String> {
    let bin = match op {
        AssignOp::Eq => return Ok(v),
        AssignOp::PlusEq => BinOp::Add,
        AssignOp::MinusEq => BinOp::Sub,
        AssignOp::StarEq => BinOp::Mul,
        AssignOp::SlashEq => BinOp::Div,
    };
    eval_binop(&bin, current(), v)
}

// ---------------------------------------------------------------------------
// Element building.

pub fn build_view(
    lv: &LiveView,
    env: &FieldEnv,
    tables: &Rc<Tables>,
    w: &World,
) -> Result<Element, String> {
    let cenv = ClosEnv {
        tables: tables.clone(),
        fields: Rc::new(env.fields.clone()),
        state_cells: Rc::new(lv.state_cells.clone()),
    };
    let scope = Scope::default();
    build_element(&lv.root, &cenv, &scope, w)
}

/// Is the element body exactly one `for` repeater and nothing else
/// (properties aside)? This predicate MUST match codegen's — it
/// decides whether rows go lazy, and the tiers diverge if the two
/// sides disagree.
///
/// Ordinary `for` bodies hold as many elements as they like (§8.56).
/// A VIRTUALIZED one does not, and cannot: a lazy row is built on
/// demand as one `Element` for one index, so "one row is one element"
/// is what virtualization means rather than a lowering limit.
fn single_repeater_of(
    el: &ast::Element,
) -> Result<Option<(&str, Option<&str>, &Expr, &ast::Element)>, String> {
    let mut non_props = el
        .members
        .iter()
        .filter(|m| !matches!(m, ElementMember::Property { .. }));
    match (non_props.next(), non_props.next()) {
        (
            Some(ElementMember::Stmt(Stmt::For {
                binding,
                index,
                iter,
                body,
                ..
            })),
            None,
        ) => {
            let one = body.stmts.is_empty()
                && matches!(
                    body.trailing.as_deref().map(|t| &t.kind),
                    Some(ExprKind::Element(_))
                );
            if !one {
                return Err(VIRTUAL_ROW_RULE.into());
            }
            let ExprKind::Element(child) = &body.trailing.as_deref().unwrap().kind else {
                unreachable!("checked above");
            };
            Ok(Some((
                binding.name.as_str(),
                index.as_ref().map(|i| i.name.as_str()),
                iter,
                child,
            )))
        }
        _ => Ok(None),
    }
}

/// Both tiers say this, word for word.
pub(crate) const VIRTUAL_ROW_RULE: &str =
    "a virtualized ListView builds one element per row, so its `for` body \
     holds exactly one element — wrap several in a Column";

/// A string literal's text, when the expr is a plain literal.
fn str_lit(e: &Expr) -> Option<String> {
    match &e.kind {
        ExprKind::Str(parts) => {
            let mut out = String::new();
            for p in parts {
                match p {
                    ast::StrPart::Text(t) => out.push_str(t),
                    _ => return None,
                }
            }
            Some(out)
        }
        _ => None,
    }
}

fn prop_of<'e>(el: &'e ast::Element, key: &str) -> Option<&'e Expr> {
    el.members.iter().find_map(|m| match m {
        ElementMember::Property { key: k, value, .. } if k == key => Some(value),
        _ => None,
    })
}

fn eval_text(e: &Expr, env: &ClosEnv, scope: &Scope, w: &World) -> Result<Str, String> {
    let v = eval_expr(e, env, scope, w)?;
    match v {
        Value::Str(s) => Ok(s),
        other => Ok(Str::from(other.render())),
    }
}

/// Evaluate a chart's `data:` to a kernel `List<f64>`. Mirrors
/// codegen's `lower_view_float_list`: Float elements, with Int
/// widening element by element (§8.55) via `as_float`.
fn eval_float_list(
    e: &Expr,
    env: &ClosEnv,
    scope: &Scope,
    w: &World,
) -> Result<List<f64>, String> {
    let v = eval_expr(e, env, scope, w)?;
    let Value::List(xs) = v else {
        return Err(format!("expected List<Float>, got {}", v.type_name()));
    };
    let mut out: List<f64> = List::new();
    for x in &xs {
        out.push(x.as_float()?);
    }
    Ok(out)
}

/// The `List<String>` twin of `eval_float_list`, for `labels:`.
fn eval_str_list(
    e: &Expr,
    env: &ClosEnv,
    scope: &Scope,
    w: &World,
) -> Result<List<Str>, String> {
    let v = eval_expr(e, env, scope, w)?;
    let Value::List(xs) = v else {
        return Err(format!("expected List<String>, got {}", v.type_name()));
    };
    let mut out: List<Str> = List::new();
    for x in &xs {
        out.push(x.as_str_value()?);
    }
    Ok(out)
}

/// The optional `width:`/`height:` pair every sized leaf shares
/// (Image, Svg, the charts). Mirrors codegen's `lower_view_size`
/// exactly, widening included (§8.55); default 0.0 (unset).
fn eval_size(
    el: &ast::Element,
    env: &ClosEnv,
    scope: &Scope,
    w: &World,
) -> Result<(f64, f64), String> {
    let width = match prop_of(el, "width") {
        Some(v) => eval_expr(v, env, scope, w)?.as_float()?,
        None => 0.0,
    };
    let height = match prop_of(el, "height") {
        Some(v) => eval_expr(v, env, scope, w)?.as_float()?,
        None => 0.0,
    };
    Ok((width, height))
}

/// Build a click listener from a handler expression. The closure
/// captures the cloned AST and the Rc'd env; failures at click time
/// report and no-op instead of tearing the app down.
fn make_listener(e: &Expr, env: &ClosEnv) -> Listener {
    let ast = e.clone();
    let env = env.clone();
    Rc::new(move |w: &mut World| {
        let mut scope = Scope::default();
        if let Err(err) = eval_action(&ast, &env, &mut scope, w) {
            eprintln!("pixie reload: handler error: {err}");
        }
    })
}

fn make_text_listener(e: &Expr, env: &ClosEnv) -> TextListener {
    let ast = e.clone();
    let env = env.clone();
    Rc::new(move |w: &mut World, text: Str| {
        let mut scope = Scope::default();
        scope.vars.push(("text".into(), Value::Str(text)));
        if let Err(err) = eval_action(&ast, &env, &mut scope, w) {
            eprintln!("pixie reload: handler error: {err}");
        }
    })
}

/// `make_text_listener`'s Bool twin, for the toggles' `onToggle:` —
/// the implicit `checked` carries the NEW value.
fn make_bool_listener(e: &Expr, env: &ClosEnv) -> BoolListener {
    let ast = e.clone();
    let env = env.clone();
    Rc::new(move |w: &mut World, checked: bool| {
        let mut scope = Scope::default();
        scope.vars.push(("checked".into(), Value::Bool(checked)));
        if let Err(err) = eval_action(&ast, &env, &mut scope, w) {
            eprintln!("pixie reload: handler error: {err}");
        }
    })
}

/// The Float twin of `make_text_listener`, for the Slider's
/// `onChange`: the implicit `value` argument carries the new value.
fn make_float_listener(e: &Expr, env: &ClosEnv) -> FloatListener {
    let ast = e.clone();
    let env = env.clone();
    Rc::new(move |w: &mut World, value: f64| {
        let mut scope = Scope::default();
        scope.vars.push(("value".into(), Value::Float(value)));
        if let Err(err) = eval_action(&ast, &env, &mut scope, w) {
            eprintln!("pixie reload: handler error: {err}");
        }
    })
}

/// `make_text_listener`, one primitive over: the choosers' `onSelect`
/// binds the chosen 0-based index as the implicit `index` argument.
fn make_int_listener(e: &Expr, env: &ClosEnv) -> IntListener {
    let ast = e.clone();
    let env = env.clone();
    Rc::new(move |w: &mut World, index: i64| {
        let mut scope = Scope::default();
        scope.vars.push(("index".into(), Value::Int(index)));
        if let Err(err) = eval_action(&ast, &env, &mut scope, w) {
            eprintln!("pixie reload: handler error: {err}");
        }
    })
}

/// Build one element, wrapping it in a `GridCell` when it carries the
/// universal grid-item spans — `pixie_codegen::lower_element`'s mirror,
/// down to the "no span props, no wrapper" rule that keeps every
/// pre-Grid demo dumping byte-identically.
fn build_element(
    el: &ast::Element,
    env: &ClosEnv,
    scope: &Scope,
    w: &World,
) -> Result<Element, String> {
    let col = prop_of(el, "colSpan");
    let row = prop_of(el, "rowSpan");
    let inner = build_element_inner(el, env, scope, w)?;
    // Same nesting as codegen: semantics innermost (they describe the
    // element), then the animation wrapper, then the grid cell.
    let inner = build_semantics(el, inner, env, scope, w)?;
    let inner = build_tooltip(el, inner, env, scope, w)?;
    let inner = build_themed(el, inner, env, scope, w)?;
    let inner = build_anim(el, inner, env, scope, w)?;
    if col.is_none() && row.is_none() {
        return Ok(inner);
    }
    let col_span = match col {
        Some(v) => eval_expr(v, env, scope, w)?.as_int()?,
        None => 1,
    };
    let row_span = match row {
        Some(v) => eval_expr(v, env, scope, w)?.as_int()?,
        None => 1,
    };
    Ok(Element::GridCell {
        col_span,
        row_span,
        children: vec![inner],
    })
}

/// `pixie_codegen::lower_themed`'s mirror (§8.37).
fn build_themed(
    el: &ast::Element,
    inner: Element,
    env: &ClosEnv,
    scope: &Scope,
    w: &World,
) -> Result<Element, String> {
    let Some(t) = prop_of(el, "theme") else {
        return Ok(inner);
    };
    let name = match str_lit(t) {
        Some(name) => {
            if pixie_kernel::theme::by_name(&name).is_none() {
                return Err(format!(
                    "unknown theme `{name}` — one of {}",
                    pixie_kernel::theme::NAMES.join(", ")
                ));
            }
            Str::from(name)
        }
        None => eval_text(t, env, scope, w)?,
    };
    Ok(Element::Themed {
        theme: name,
        children: vec![inner],
    })
}

/// `pixie_codegen::lower_tooltip`'s mirror.
fn build_tooltip(
    el: &ast::Element,
    inner: Element,
    env: &ClosEnv,
    scope: &Scope,
    w: &World,
) -> Result<Element, String> {
    let Some(t) = prop_of(el, "tooltip") else {
        return Ok(inner);
    };
    Ok(Element::Tooltip {
        text: eval_text(t, env, scope, w)?,
        children: vec![inner],
    })
}

/// `pixie_codegen::lower_semantics`'s mirror: `role:` from the closed
/// vocabulary `pixie_kernel::a11y::Role` owns, `label:` any string.
fn build_semantics(
    el: &ast::Element,
    inner: Element,
    env: &ClosEnv,
    scope: &Scope,
    w: &World,
) -> Result<Element, String> {
    let role = prop_of(el, "role");
    // The toggles OWN `label:` (mirrors codegen): only `role:` rides
    // on a Checkbox / Switch — their accessible name derives from
    // the label they already carry.
    let label = if matches!(el.name.name.as_str(), "Checkbox" | "Switch") {
        None
    } else {
        prop_of(el, "label")
    };
    if role.is_none() && label.is_none() {
        return Ok(inner);
    }
    let role = match role {
        Some(e) => {
            // A literal is checked here; anything else is an
            // ordinary String expression (§8.57, mirroring codegen).
            match str_lit(e) {
                Some(name) => {
                    if pixie_kernel::a11y::Role::parse(&name).is_none() {
                        return Err(format!(
                            "unknown role `{name}` — one of {}",
                            pixie_kernel::a11y::Role::ALL
                                .iter()
                                .map(|r| r.name())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    }
                    Str::from(name)
                }
                None => eval_text(e, env, scope, w)?,
            }
        }
        None => Str::new(),
    };
    let label = match label {
        Some(v) => eval_text(v, env, scope, w)?,
        None => Str::new(),
    };
    Ok(Element::Semantics {
        role,
        label,
        children: vec![inner],
    })
}

/// `pixie_codegen::lower_anim`'s mirror, down to the errors: the
/// riders only mean something with `animate:` present, and `easing:`
/// is checked against a closed vocabulary when it is a literal
/// (§8.57).
fn build_anim(
    el: &ast::Element,
    inner: Element,
    env: &ClosEnv,
    scope: &Scope,
    w: &World,
) -> Result<Element, String> {
    let duration = prop_of(el, "animate");
    let easing = prop_of(el, "easing");
    let enter = prop_of(el, "enter");
    let exit = prop_of(el, "exit");
    if duration.is_none() {
        if easing.or(enter).or(exit).is_some() {
            return Err(
                "`easing:` / `enter:` / `exit:` describe a tween, and `animate:` is what \
                 starts one — without it there is nothing for them to shape"
                    .into(),
            );
        }
        return Ok(inner);
    }
    let dur = eval_expr(duration.expect("guarded"), env, scope, w)?.as_float()?;
    let ease = match easing {
        Some(e) => match str_lit(e) {
            Some(name) => pixie_kernel::Easing::parse(&name)
                .ok_or(format!("unknown easing `{name}` — one of linear, in, out, inOut"))?,
            // An unknown name at run time falls back rather than
            // aborting a frame, exactly as the compiled tier does.
            None => pixie_kernel::Easing::parse(eval_text(e, env, scope, w)?.as_str())
                .unwrap_or(pixie_kernel::Easing::Out),
        },
        None => pixie_kernel::Easing::Out,
    };
    let flag = |e: Option<&Expr>| -> Result<bool, String> {
        match e {
            None => Ok(false),
            Some(x) => eval_expr(x, env, scope, w)?.as_bool(),
        }
    };
    Ok(Element::Anim {
        duration: dur,
        easing: ease,
        enter: flag(enter)?,
        exit: flag(exit)?,
        opacity: 1.0,
        children: vec![inner],
    })
}

fn build_element_inner(
    el: &ast::Element,
    env: &ClosEnv,
    scope: &Scope,
    w: &World,
) -> Result<Element, String> {
    match el.name.name.as_str() {
        // Per-row component state (§8.30): resolve this repeater
        // row's state handle from the seat (rows were ensured by the
        // compiled `prepare` phase) and expose it to the subtree —
        // bindings AND action closures — as an ordinary object field.
        "__PixieRowScope" => {
            let seat = prop_of(el, "__seat")
                .and_then(|e| str_lit(e))
                .ok_or("__PixieRowScope needs `__seat:`")?;
            let row_name = prop_of(el, "__row")
                .and_then(|e| str_lit(e))
                .ok_or("__PixieRowScope needs `__row:`")?;
            let depth: usize = prop_of(el, "__depth")
                .and_then(|e| str_lit(e))
                .and_then(|d| d.parse().ok())
                .ok_or("__PixieRowScope needs `__depth:`")?;
            if depth == 0 || depth > scope.row_path.len() {
                return Err(format!(
                    "per-row state at repeater depth {depth} built under {} enclosing \
                     `for`s — the component splice and the interpreter disagree",
                    scope.row_path.len()
                ));
            }
            let path = &scope.row_path[..depth];
            let (_, _, seat_h) = env
                .fields
                .iter()
                .find(|(n, _, _)| *n == seat)
                .ok_or_else(|| format!("row seat `{seat}` is not a registered field"))?;
            let (class, getter) = env
                .tables
                .rows
                .get(&seat)
                .ok_or_else(|| format!("row seat `{seat}` has no reflection entry"))?;
            let row_h = getter(w, *seat_h, path).ok_or_else(|| {
                format!(
                    "row {path:?} of `{seat}` is not prepared (list changed shape? rung-1 rebuild)"
                )
            })?;
            let child = el
                .members
                .iter()
                .find_map(|m| match m {
                    ElementMember::Child(c) => Some(c),
                    _ => None,
                })
                .ok_or("__PixieRowScope holds one element")?;
            let mut fields2 = (*env.fields).clone();
            fields2.push((row_name, class.clone(), row_h));
            let env2 = ClosEnv {
                tables: env.tables.clone(),
                fields: Rc::new(fields2),
                state_cells: env.state_cells.clone(),
            };
            build_element(child, &env2, scope, w)
        }
        "Text" => {
            let t = prop_of(el, "text").ok_or("Text needs `text:`")?;
            let font_size = match prop_of(el, "fontSize") {
                Some(v) => eval_expr(v, env, scope, w)?.as_float()?,
                None => 0.0,
            };
            let color = match prop_of(el, "color") {
                Some(v) => eval_text(v, env, scope, w)?,
                None => Str::new(),
            };
            let align = match prop_of(el, "align") {
                Some(v) => eval_text(v, env, scope, w)?,
                None => Str::new(),
            };
            let grow = match prop_of(el, "grow") {
                Some(v) => eval_expr(v, env, scope, w)?.as_float()?,
                None => 0.0,
            };
            Ok(Element::Text {
                text: eval_text(t, env, scope, w)?,
                font_size,
                color,
                align,
                grow,
            })
        }
        "Button" => {
            let label = prop_of(el, "text")
                .or_else(|| prop_of(el, "label"))
                .ok_or("Button needs `text:`")?;
            let action = prop_of(el, "onClick").ok_or("Button needs `onClick:`")?;
            let background = match prop_of(el, "background") {
                Some(v) => eval_text(v, env, scope, w)?,
                None => Str::new(),
            };
            let hover_background = match prop_of(el, "hover.background") {
                Some(v) => eval_text(v, env, scope, w)?,
                None => Str::new(),
            };
            let active_background = match prop_of(el, "active.background") {
                Some(v) => eval_text(v, env, scope, w)?,
                None => Str::new(),
            };
            let width = match prop_of(el, "width") {
                Some(v) => eval_expr(v, env, scope, w)?.as_float()?,
                None => 0.0,
            };
            let height = match prop_of(el, "height") {
                Some(v) => eval_expr(v, env, scope, w)?.as_float()?,
                None => 0.0,
            };
            let font_size = match prop_of(el, "fontSize") {
                Some(v) => eval_expr(v, env, scope, w)?.as_float()?,
                None => 0.0,
            };
            let color = match prop_of(el, "color") {
                Some(v) => eval_text(v, env, scope, w)?,
                None => Str::new(),
            };
            let grow = match prop_of(el, "grow") {
                Some(v) => eval_expr(v, env, scope, w)?.as_float()?,
                None => 0.0,
            };
            let basis = match prop_of(el, "basis") {
                Some(v) => eval_expr(v, env, scope, w)?.as_float()?,
                None => 0.0,
            };
            let (border_radius, border_width, border_color) = box_props_of(el, env, scope, w)?;
            Ok(Element::Button {
                label: eval_text(label, env, scope, w)?,
                background,
                hover_background,
                active_background,
                width,
                height,
                font_size,
                color,
                grow,
                basis,
                border_radius,
                border_width,
                border_color,
                on_click: make_listener(action, env),
            })
        }
        "TextField" => {
            let value = match prop_of(el, "text") {
                Some(t) => eval_text(t, env, scope, w)?,
                None => Str::new(),
            };
            let placeholder = match prop_of(el, "placeholder") {
                Some(t) => eval_text(t, env, scope, w)?,
                None => Str::new(),
            };
            let multiline = match prop_of(el, "multiline") {
                Some(v) => eval_expr(v, env, scope, w)?.as_bool()?,
                None => false,
            };
            let rows = match prop_of(el, "rows") {
                Some(v) => eval_expr(v, env, scope, w)?.as_float()?,
                None => 0.0,
            };
            Ok(Element::TextField {
                value,
                placeholder,
                on_change: prop_of(el, "onTextChanged").map(|a| make_text_listener(a, env)),
                on_submit: prop_of(el, "onSubmitted").map(|a| make_text_listener(a, env)),
                multiline,
                rows,
            })
        }
        "Column" | "Row" => {
            // Style props, spliced or written directly. `spacing`
            // keeps `-1.0` = unset (0 is honest zero — a style can
            // remove the default gap); mirrors codegen exactly.
            let spacing = match prop_of(el, "spacing") {
                Some(v) => eval_expr(v, env, scope, w)?.as_float()?,
                None => -1.0,
            };
            let padding = match prop_of(el, "padding") {
                Some(v) => eval_expr(v, env, scope, w)?.as_float()?,
                None => 0.0,
            };
            let background = match prop_of(el, "background") {
                Some(v) => eval_text(v, env, scope, w)?,
                None => Str::new(),
            };
            let grow = match prop_of(el, "grow") {
                Some(v) => eval_expr(v, env, scope, w)?.as_float()?,
                None => 0.0,
            };
            let (border_radius, border_width, border_color) = box_props_of(el, env, scope, w)?;
            let children = build_children(el, env, scope, w)?;
            Ok(if el.name.name == "Column" {
                Element::Column {
                    spacing,
                    padding,
                    background,
                    grow,
                    border_radius,
                    border_width,
                    border_color,
                    children,
                }
            } else {
                Element::Row {
                    spacing,
                    padding,
                    background,
                    grow,
                    border_radius,
                    border_width,
                    border_color,
                    children,
                }
            })
        }
        "Grid" => {
            // `columns:` is required and Int-strict; the rest are
            // Column's props with Column's sentinels — codegen's arm,
            // one primitive wider.
            let c = prop_of(el, "columns")
                .ok_or("Grid needs `columns:` (how many tracks wide it is)")?;
            let columns = eval_expr(c, env, scope, w)?.as_int()?;
            let rows = match prop_of(el, "rows") {
                Some(v) => eval_expr(v, env, scope, w)?.as_int()?,
                None => 0,
            };
            let spacing = match prop_of(el, "spacing") {
                Some(v) => eval_expr(v, env, scope, w)?.as_float()?,
                None => -1.0,
            };
            let padding = match prop_of(el, "padding") {
                Some(v) => eval_expr(v, env, scope, w)?.as_float()?,
                None => 0.0,
            };
            let background = match prop_of(el, "background") {
                Some(v) => eval_text(v, env, scope, w)?,
                None => Str::new(),
            };
            let grow = match prop_of(el, "grow") {
                Some(v) => eval_expr(v, env, scope, w)?.as_float()?,
                None => 0.0,
            };
            let (border_radius, border_width, border_color) = box_props_of(el, env, scope, w)?;
            let children = build_children(el, env, scope, w)?;
            Ok(Element::Grid {
                columns,
                rows,
                spacing,
                padding,
                background,
                grow,
                border_radius,
                border_width,
                border_color,
                children,
            })
        }
        "Stack" => Ok(Element::Stack(build_children(el, env, scope, w)?)),
        "ListView" => {
            // Mirrors codegen's strictness exactly: `virtualized:` must
            // be a Bool and `itemHeight:`/`height:` a Float when
            // present, with the same "unset" defaults.
            let virtualized = match prop_of(el, "virtualized") {
                Some(v) => eval_expr(v, env, scope, w)?.as_bool()?,
                None => false,
            };
            let item_height = match prop_of(el, "itemHeight") {
                Some(v) => eval_expr(v, env, scope, w)?.as_float()?,
                None => 0.0,
            };
            let height = match prop_of(el, "height") {
                Some(v) => eval_expr(v, env, scope, w)?.as_float()?,
                None => 0.0,
            };
            let grow = match prop_of(el, "grow") {
                Some(v) => eval_expr(v, env, scope, w)?.as_float()?,
                None => 0.0,
            };
            // The lazy detection mirrors codegen's rule verbatim: a
            // VIRTUALIZED body that is exactly one `for` repeater (no
            // static children) becomes LazyRows built on demand;
            // non-virtualized lists stay eager (§8.24 — the window's
            // clipped-viewport path renders `children`).
            if virtualized {
                if let Some((binding, index, iter, child)) = single_repeater_of(el)? {
                let len = match eval_expr(iter, env, scope, w)? {
                    Value::List(xs) => xs.len(),
                    other => {
                        return Err(format!("`for` needs a List, got {}", other.type_name()));
                    }
                };
                let iter_ast = iter.clone();
                let child_ast = child.clone();
                let binding_name = binding.to_string();
                let index_name = index.map(|i| i.to_string());
                let cenv = env.clone();
                let outer_vars = scope.vars.clone();
                let outer_path = scope.row_path.clone();
                // Trial-build row 0 so a reload-time validation pass
                // catches bad row expressions before install.
                if len > 0 {
                    let mut trial = Scope {
                        row_path: {
                            let mut p = outer_path.clone();
                            p.push(0);
                            p
                        },
                        vars: outer_vars.clone(),
                    };
                    if let Value::List(xs) = eval_expr(iter, env, scope, w)? {
                        trial.vars.push((binding_name.clone(), xs[0].clone()));
                        if let Some(ix) = &index_name {
                            trial.vars.push((ix.clone(), Value::Int(0)));
                        }
                        build_element(&child_ast, env, &trial, w)?;
                    }
                }
                let build = Rc::new(move |w: &World, range: std::ops::Range<usize>| {
                    let xs = match eval_expr(
                        &iter_ast,
                        &cenv,
                        &Scope {
                            row_path: outer_path.clone(),
                            vars: outer_vars.clone(),
                        },
                        w,
                    ) {
                        Ok(Value::List(xs)) => xs,
                        _ => return Vec::new(),
                    };
                    let mut rows = Vec::new();
                    for i in range {
                        if i >= xs.len() {
                            break;
                        }
                        // The lazy row's own index joins the path, so
                        // per-row state works in a virtualized list —
                        // `prepare` sized the seat from the FULL list
                        // length, not just the visible range (§8.34).
                        let mut row_scope = Scope {
                            row_path: {
                                let mut p = outer_path.clone();
                                p.push(i);
                                p
                            },
                            vars: outer_vars.clone(),
                        };
                        row_scope.vars.push((binding_name.clone(), xs[i].clone()));
                        if let Some(ix) = &index_name {
                            row_scope.vars.push((ix.clone(), Value::Int(i as i64)));
                        }
                        match build_element(&child_ast, &cenv, &row_scope, w) {
                            Ok(e) => rows.push(e),
                            Err(err) => {
                                eprintln!("pixie reload: lazy row {i}: {err}");
                            }
                        }
                    }
                    rows
                });
                return Ok(Element::ListView {
                    virtualized,
                    item_height,
                    height,
                    grow,
                    children: Vec::new(),
                    lazy: Some(LazyRows { len, build }),
                });
                }
            }
            Ok(Element::ListView {
                virtualized,
                item_height,
                height,
                grow,
                children: build_children(el, env, scope, w)?,
                lazy: None,
            })
        }
        "ScrollView" => {
            // The mirror of codegen's arm: optional, strict Float,
            // `0.0` = the engine's 320 px default.
            let height = match prop_of(el, "height") {
                Some(v) => eval_expr(v, env, scope, w)?.as_float()?,
                None => 0.0,
            };
            Ok(Element::ScrollView {
                height,
                children: build_children(el, env, scope, w)?,
            })
        }
        "HScrollView" => Ok(Element::HScrollView(build_children(el, env, scope, w)?)),
        "Image" => {
            let source = prop_of(el, "source").ok_or("Image needs `source:`")?;
            let (width, height) = eval_size(el, env, scope, w)?;
            Ok(Element::Image {
                source: eval_text(source, env, scope, w)?,
                width,
                height,
            })
        }
        "Svg" => {
            let source = prop_of(el, "source").ok_or("Svg needs `source:`")?;
            let (width, height) = eval_size(el, env, scope, w)?;
            Ok(Element::Svg {
                source: eval_text(source, env, scope, w)?,
                width,
                height,
            })
        }
        "DataTable" => Ok(Element::DataTable(build_children(el, env, scope, w)?)),
        "Modal" => {
            // `open:` went optional when `if` landed in views: a bare
            // Modal (cute_ui's propless shape) renders open, and the
            // view wraps it in `if` for visibility. When present it
            // must still be a Bool — no truthiness.
            let open = match prop_of(el, "open") {
                Some(o) => eval_expr(o, env, scope, w)?.as_bool()?,
                None => true,
            };
            Ok(Element::Modal {
                open,
                children: build_children(el, env, scope, w)?,
            })
        }
        // Both charts build the same kernel Lists the emitter does, so
        // the tier gate's dump comparison is parity by construction.
        // `data:` is required; `labels:` defaults to empty.
        "BarChart" => {
            let d = prop_of(el, "data").ok_or("BarChart needs `data:`")?;
            let (width, height) = eval_size(el, env, scope, w)?;
            Ok(Element::BarChart {
                data: eval_float_list(d, env, scope, w)?,
                labels: match prop_of(el, "labels") {
                    Some(l) => eval_str_list(l, env, scope, w)?,
                    None => List::new(),
                },
                width,
                height,
            })
        }
        "LineChart" => {
            let d = prop_of(el, "data").ok_or("LineChart needs `data:`")?;
            let (width, height) = eval_size(el, env, scope, w)?;
            Ok(Element::LineChart {
                data: eval_float_list(d, env, scope, w)?,
                labels: match prop_of(el, "labels") {
                    Some(l) => eval_str_list(l, env, scope, w)?,
                    None => List::new(),
                },
                width,
                height,
            })
        }
        "ProgressBar" => {
            let v = prop_of(el, "value").ok_or("ProgressBar needs `value:`")?;
            // Mirrors codegen exactly, Int widening included (§8.55).
            let value = eval_expr(v, env, scope, w)?.as_float()?;
            Ok(Element::ProgressBar { value })
        }
        "Spinner" => {
            // One square axis, unlike the charts' width/height pair;
            // same widening, same 0.0 = "engine default".
            let size = match prop_of(el, "size") {
                Some(v) => eval_expr(v, env, scope, w)?.as_float()?,
                None => 0.0,
            };
            Ok(Element::Spinner { size })
        }
        // Mirrors codegen exactly: `label:` and `checked:` required
        // (a bound Bool — no truthiness), `onToggle:` optional with
        // the NEW value as an implicit `checked`.
        "Checkbox" | "Switch" => {
            let name = el.name.name.as_str();
            let l = prop_of(el, "label").ok_or(format!("{name} needs `label:`"))?;
            let c = prop_of(el, "checked")
                .ok_or(format!("{name} needs `checked:` (the Bool state it shows)"))?;
            let label = eval_text(l, env, scope, w)?;
            let checked = eval_expr(c, env, scope, w)?.as_bool()?;
            let on_toggle = prop_of(el, "onToggle").map(|a| make_bool_listener(a, env));
            Ok(if name == "Checkbox" {
                Element::Checkbox {
                    label,
                    checked,
                    on_toggle,
                }
            } else {
                Element::Switch {
                    label,
                    checked,
                    on_toggle,
                }
            })
        }
        "Slider" => {
            // `value:` is required; codegen restricts it to a property
            // READ, and here any Float expression evaluates — the
            // charts' `data:` asymmetry, compile-time strictness with
            // a lenient mirror. The range props share codegen's
            // defaults exactly: min 0.0, max 1.0, step 0.0 (= continuous).
            let v = prop_of(el, "value").ok_or("Slider needs `value:`")?;
            let value = eval_expr(v, env, scope, w)?.as_float()?;
            let min = match prop_of(el, "min") {
                Some(v) => eval_expr(v, env, scope, w)?.as_float()?,
                None => 0.0,
            };
            let max = match prop_of(el, "max") {
                Some(v) => eval_expr(v, env, scope, w)?.as_float()?,
                None => 1.0,
            };
            let step = match prop_of(el, "step") {
                Some(v) => eval_expr(v, env, scope, w)?.as_float()?,
                None => 0.0,
            };
            Ok(Element::Slider {
                value,
                min,
                max,
                step,
                on_change: prop_of(el, "onChange").map(|a| make_float_listener(a, env)),
            })
        }
        // The choosers, mirroring codegen exactly: `options:` /
        // `labels:` and `selected:` / `active:` are required,
        // `onSelect` is optional and binds the implicit `index`.
        "Select" | "RadioGroup" => {
            let o = prop_of(el, "options")
                .ok_or_else(|| format!("{} needs `options:`", el.name.name))?;
            let s = prop_of(el, "selected")
                .ok_or_else(|| format!("{} needs `selected:`", el.name.name))?;
            let options = eval_str_list(o, env, scope, w)?;
            let selected = eval_expr(s, env, scope, w)?.as_int()?;
            let on_select = prop_of(el, "onSelect").map(|a| make_int_listener(a, env));
            Ok(if el.name.name == "Select" {
                Element::Select {
                    options,
                    selected,
                    on_select,
                }
            } else {
                Element::RadioGroup {
                    options,
                    selected,
                    on_select,
                }
            })
        }
        "TabBar" => {
            let l = prop_of(el, "labels").ok_or("TabBar needs `labels:`")?;
            let a = prop_of(el, "active").ok_or("TabBar needs `active:`")?;
            Ok(Element::TabBar {
                labels: eval_str_list(l, env, scope, w)?,
                active: eval_expr(a, env, scope, w)?.as_int()?,
                on_select: prop_of(el, "onSelect").map(|a| make_int_listener(a, env)),
            })
        }
        // Mirrors codegen exactly: `grow:` optional, 0.0 default.
        "Spacer" => {
            let grow = match prop_of(el, "grow") {
                Some(v) => eval_expr(v, env, scope, w)?.as_float()?,
                None => 0.0,
            };
            Ok(Element::Spacer { grow })
        }
        // Mirrors codegen exactly: `color:` a Str read (Text's
        // `color:` shape), `thickness:` an optional Float.
        "Divider" => {
            let color = match prop_of(el, "color") {
                Some(v) => eval_text(v, env, scope, w)?,
                None => Str::new(),
            };
            let thickness = match prop_of(el, "thickness") {
                Some(v) => eval_expr(v, env, scope, w)?.as_float()?,
                None => 0.0,
            };
            Ok(Element::Divider { color, thickness })
        }
        other => Err(format!("element `{other}` is not in the engine vocabulary")),
    }
}

/// Property keys a container element consumes in its own `build_element`
/// arm. The mirror of `pixie_codegen::container_prop_keys` — the two
/// tables must stay identical, or a view that the compiler rejects would
/// silently reload through rung 2 (and vice versa). Ledger §11.12.
/// The mirror of `pixie_codegen::lower_box_props` (§8.79): the
/// box-decoration props every element that paints a box reads, in the
/// same order and with the same sentinels.
fn box_props_of(
    el: &ast::Element,
    env: &ClosEnv,
    scope: &Scope,
    w: &World,
) -> Result<(f64, f64, Str), String> {
    let radius = match prop_of(el, "borderRadius") {
        Some(v) => eval_expr(v, env, scope, w)?.as_float()?,
        None => 0.0,
    };
    let width = match prop_of(el, "borderWidth") {
        Some(v) => eval_expr(v, env, scope, w)?.as_float()?,
        None => 0.0,
    };
    let color = match prop_of(el, "borderColor") {
        Some(v) => eval_text(v, env, scope, w)?,
        None => Str::new(),
    };
    Ok((radius, width, color))
}

pub fn container_prop_keys(element: &str) -> &'static [&'static str] {
    match element {
        "Column" | "Row" => &[
            "spacing",
            "padding",
            "background",
            "grow",
            "borderRadius",
            "borderWidth",
            "borderColor",
        ],
        "Grid" => &[
            "columns",
            "rows",
            "spacing",
            "padding",
            "background",
            "grow",
            "borderRadius",
            "borderWidth",
            "borderColor",
        ],
        "ListView" => &["virtualized", "itemHeight", "height", "grow"],
        "ScrollView" => &["height"],
        "Modal" => &["open"],
        _ => &[],
    }
}

/// The mirror of `pixie_codegen::grid_item_prop_keys`: placement props
/// every element accepts, stripped into an `Element::GridCell` by
/// `build_element` before any arm sees them.
pub fn grid_item_prop_keys() -> &'static [&'static str] {
    &["colSpan", "rowSpan"]
}

/// The mirror of `pixie_codegen::anim_prop_keys`: the animation
/// riders (§8.35), stripped into an `Element::Anim` by `build_element`
/// before any arm sees them.
pub fn anim_prop_keys() -> &'static [&'static str] {
    &["animate", "easing", "enter", "exit"]
}

/// The mirror of `pixie_codegen::semantic_prop_keys`: the
/// accessibility riders (§8.36).
pub fn semantic_prop_keys() -> &'static [&'static str] {
    &["role", "label"]
}

/// The mirror of `pixie_codegen::tooltip_prop_keys`.
pub fn tooltip_prop_keys() -> &'static [&'static str] {
    &["tooltip"]
}

/// The mirror of `pixie_codegen::theme_prop_keys` (§8.37).
pub fn theme_prop_keys() -> &'static [&'static str] {
    &["theme"]
}

/// `when some(..)` — the present half of a `T?` match (§8.69).
fn is_some_pattern(p: &ast::Pattern) -> bool {
    matches!(p, ast::Pattern::Ctor { name, .. } if name.name == "some")
}

fn build_children(
    el: &ast::Element,
    env: &ClosEnv,
    scope: &Scope,
    w: &World,
) -> Result<Vec<Element>, String> {
    // Same allowlist and same wording as codegen's `lower_children`:
    // an unknown container-level property is an error here too, never
    // a silently ignored member.
    for m in &el.members {
        if let ElementMember::Property { key, .. } = m {
            if !grid_item_prop_keys().contains(&key.as_str())
                && !anim_prop_keys().contains(&key.as_str())
                && !semantic_prop_keys().contains(&key.as_str())
                && !theme_prop_keys().contains(&key.as_str())
                && !tooltip_prop_keys().contains(&key.as_str())
                && !container_prop_keys(&el.name.name).contains(&key.as_str())
            {
                return Err(format!(
                    "element property `{key}` is not lowerable on `{}` (M0)",
                    el.name.name
                ));
            }
        }
    }
    let mut out = Vec::new();
    build_items(&items_of_members(&el.members), env, scope, w, &mut out)?;
    Ok(out)
}

/// Append what a run of view items contributes. `for` bodies and `if`
/// branches are runs of items too (§8.56), so this is the whole
/// recursion — a repeater body may hold several elements, another
/// repeater, or a conditional, and so may a branch.
fn build_items(
    items: &[ViewItem<'_>],
    env: &ClosEnv,
    scope: &Scope,
    w: &World,
    out: &mut Vec<Element>,
) -> Result<(), String> {
    for item in items {
        match item {
            ViewItem::Child(c) => out.push(build_element(c, env, scope, w)?),
            ViewItem::Repeat {
                binding,
                index,
                iter,
                body,
                ..
            } => {
                let xs = match eval_expr(iter, env, scope, w)? {
                    Value::List(xs) => xs,
                    other => {
                        return Err(format!("`for` needs a List, got {}", other.type_name()));
                    }
                };
                let inner_items = items_of_block(body);
                for (__ri, it) in xs.into_iter().enumerate() {
                    let mut inner = Scope {
                        vars: scope.vars.clone(),
                        row_path: {
                            let mut p = scope.row_path.clone();
                            p.push(__ri);
                            p
                        },
                    };
                    inner.vars.push((binding.name.clone(), it));
                    if let Some(ix) = index {
                        inner.vars.push((ix.name.clone(), Value::Int(__ri as i64)));
                    }
                    build_items(&inner_items, env, &inner, w, out)?;
                }
            }
            // Conditional render — mirrors codegen exactly: the
            // action-expression grammar for the condition (loop vars
            // in scope), strict Bool.
            ViewItem::Cond(e) => {
                let ExprKind::If {
                    cond,
                    then_b,
                    else_b,
                    let_binding,
                } = &e.kind
                else {
                    unreachable!("items_of_* only builds Cond from an If");
                };
                if let_binding.is_some() {
                    return Err("`if let` survived the desugar (§8.69) — this is a pixie bug".into());
                }
                let taken = if eval_expr(cond, env, scope, w)?.as_bool()? {
                    Some(then_b)
                } else {
                    else_b.as_ref()
                };
                if let Some(b) = taken {
                    build_items(&items_of_block(b), env, scope, w, out)?;
                }
            }
            // `case` in a view body (§8.69), and therefore `if let`.
            // The interpreted tier carries the value's own type, so
            // the arms dispatch on what the scrutinee IS rather than
            // on what it was declared to be.
            ViewItem::Match(e) => {
                let ExprKind::Case { scrutinee, arms } = &e.kind else {
                    unreachable!("items_of_* only builds Match from a Case");
                };
                let v = eval_expr(scrutinee, env, scope, w)?;
                match &v {
                    Value::Nil => {
                        let body = arms
                            .iter()
                            .find(|a| !is_some_pattern(&a.pattern))
                            .map(|a| &a.body)
                            .ok_or("matching a `T?` needs both a `some` and a `nil` arm")?;
                        build_items(&items_of_block(body), env, scope, w, out)?;
                    }
                    _ => {
                        // A present optional is the value itself, so a
                        // `some` arm binds it. Failing that, the arms
                        // name enum variants.
                        if let Some(arm) = arms.iter().find(|a| is_some_pattern(&a.pattern)) {
                            let mut inner = Scope {
                                vars: scope.vars.clone(),
                                row_path: scope.row_path.clone(),
                            };
                            if let ast::Pattern::Ctor { args, .. } = &arm.pattern {
                                if let [ast::Pattern::Bind { name, .. }] = args.as_slice() {
                                    inner.vars.push((name.name.clone(), v.clone()));
                                }
                            }
                            build_items(&items_of_block(&arm.body), env, &inner, w, out)?;
                        } else {
                            let (name, fields) = match &v {
                                Value::Struct(n2, fs) => (n2.clone(), Some(fs.clone())),
                                other => (other.render(), None),
                            };
                            let taken = arms
                                .iter()
                                .find(|a| match &a.pattern {
                                    ast::Pattern::Ctor { name: n, .. } => n.name == name,
                                    ast::Pattern::Wild { .. } => true,
                                    _ => false,
                                })
                                .map(|a| (&a.body, enum_arm_binds(&a.pattern, fields.as_deref())));
                            if let Some((b, binds)) = taken {
                                let mut inner = Scope {
                                    vars: scope.vars.clone(),
                                    row_path: scope.row_path.clone(),
                                };
                                if let Some(bs) = binds {
                                    for bnd in bs {
                                        inner.vars.push(bnd);
                                    }
                                }
                                build_items(&items_of_block(b), env, &inner, w, out)?;
                            }
                        }
                    }
                }
            }
            ViewItem::Other(_) => {
                return Err("this statement is not interpretable in views yet".into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_ignores_view_body_and_positions() {
        let a = parse_module(
            "class C {\n  pub prop n : Int, default: 0\n}\n\nview Main {\n  let c = C()\n  Column {\n    Text { text: \"n: #{c.n}\" }\n  }\n}\n",
        )
        .unwrap();
        let b = parse_module(
            "class C {\n  pub prop n : Int, default: 0\n}\n\nview Main {\n  let c = C()\n  Column {\n    Text { text: \"count is now #{c.n}!\" }\n    Text { text: \"more\" }\n  }\n}\n",
        )
        .unwrap();
        assert_eq!(module_fingerprint(&a), module_fingerprint(&b));

        let c = parse_module(
            "class C {\n  pub prop n : Int, default: 1\n}\n\nview Main {\n  let c = C()\n  Column {\n    Text { text: \"n: #{c.n}\" }\n  }\n}\n",
        )
        .unwrap();
        assert_ne!(module_fingerprint(&a), module_fingerprint(&c));
    }

    #[test]
    fn strip_positions_removes_spans_and_block_ids() {
        let s = "Ident { name: \"x\", span: Span { file: FileId(0), start: 3, end: 4 } }, block_id: Some(BlockId(7))";
        let t = strip_positions(s);
        assert!(!t.contains("start"));
        assert!(!t.contains("BlockId"));
        assert!(t.contains("\"x\""));
    }

    /// Component rung classification (§8.29): the fingerprint runs on
    /// the SPLICED tree, so a body edit inside a component stays rung
    /// 2 while adding a stateful use site (a new hoisted holder) is a
    /// rung-1 rebuild.
    #[test]
    fn component_edits_classify_by_hoisted_state() {
        let base = "view Chip(label: String) {\n  state n : Int = 0\n\n  Row {\n    Text { text: \"#{label}: #{n}\" }\n    Button { text: \"+\"; onClick: { n = n + 1 } }\n  }\n}\n\nview Main {\n  Column {\n    Chip { label: \"a\" }\n  }\n}\n";
        let (fp1, lv) = reload_from_source(base).expect("slices");
        // The hoisted holder rides the root's Object fields, not the
        // bare-cell list (both tiers resolve it as a member read).
        assert!(lv.state_cells.is_empty(), "cells: {:?}", lv.state_cells);

        // Body-only edit: same params, same instances — rung 2.
        let body_edit = base.replace("\"+\"", "\"more\"");
        let (fp2, _) = reload_from_source(&body_edit).expect("slices");
        assert_eq!(fp1, fp2, "a component body edit must stay rung 2");

        // A second stateful use site: new hoisted holder — rung 1.
        let more = base.replace(
            "Chip { label: \"a\" }",
            "Chip { label: \"a\" }\n    Chip { label: \"b\" }",
        );
        let (fp3, _) = reload_from_source(&more).expect("slices");
        assert_ne!(fp1, fp3, "a new stateful instance must be rung 1");
    }

    /// Per-row seats (§8.30): the seat marker carries the repeater's
    /// iter, so redirecting the `for` to another list is a rung-1
    /// rebuild while row-body edits stay rung 2.
    #[test]
    fn row_seat_iter_is_part_of_the_fingerprint() {
        let base = "store S {\n  state xs : List<String> = []\n  state ys : List<String> = []\n}\n\nview Chip(label: String) {\n  state n : Int = 0\n\n  Text { text: \"#{label}#{n}\" }\n}\n\nview Main {\n  Column {\n    for x in S.xs {\n      Chip { label: x }\n    }\n  }\n}\n";
        let (fp1, _) = reload_from_source(base).expect("slices");
        let body_edit = base.replace("#{label}#{n}", "#{label}: #{n}");
        let (fp2, _) = reload_from_source(&body_edit).expect("slices");
        assert_eq!(fp1, fp2, "row-body edit must stay rung 2");
        let redirect = base.replace("for x in S.xs", "for x in S.ys");
        let (fp3, _) = reload_from_source(&redirect).expect("slices");
        assert_ne!(fp1, fp3, "redirecting the repeater list must be rung 1");
    }
}
