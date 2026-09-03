//! pixie's Rust emitter.
//!
//! The output spec is the hand-written S3/S5 spike code: one struct per
//! class living in the World, one extension trait per class over
//! `Handle<C>` (getters / setters / methods), setters that dirty-check
//! scalars and always-notify collections, and closures that capture only
//! Copy handles and values. Anything the emitter cannot lower into one of
//! those borrow-clean stereotypes is a hard `EmitError` — per D10 a rustc
//! error escaping from generated code is a pixie bug, so the emitter
//! refuses rather than guesses.
//!
//! M0 surface: plain classes (prop with default / signal / fn), one
//! `view` with object state fields, element vocabulary Column / Text /
//! Button / ListView (the kernel's stub engine enum), `for` repeaters,
//! string interpolation, arithmetic, prop assignment, `List.push`.

mod escape;

use std::collections::HashMap;
use std::fmt::Write as _;

use pixie_syntax::ast::{
    self, AssignOp, BinOp, ClassMember, Element, ElementMember, Expr, ExprKind, Item, StrPart,
    Stmt, TypeExpr, TypeKind, UnaryOp, synth_notify_name, Ident,};
use pixie_syntax::span::{FileId, Span};
use pixie_syntax::view::{items_of_block, items_of_members, ViewItem};

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct EmitError {
    pub span: Span,
    pub message: String,
}

fn err<T>(span: Span, message: impl Into<String>) -> Result<T, EmitError> {
    Err(EmitError {
        span,
        message: message.into(),
    })
}

// ---------------------------------------------------------------------------
// Program model collected from the AST before emission.

#[derive(Clone, Debug, PartialEq)]
enum RustTy {
    Int,
    Float,
    Bool,
    Str,
    List(Box<RustTy>),
    /// `Void` — return-position only.
    Unit,
    /// `T?` — `Option<T>` across a binding return (§11.11). Consumed
    /// with `case x { some v { .. } none { .. } }`; construction in
    /// user code stays out of M1 (no `nil` in fn bodies yet).
    Opt(Box<RustTy>),
    /// `Bytes` — the kernel's COW byte string (§11.10): `Vec<u8>`
    /// returns and `&[u8]` params at the binding boundary.
    Bytes,
    /// `Map<K, V>` — the kernel's COW ordered map (§12 design).
    Map(Box<RustTy>, Box<RustTy>),
    /// A user enum or struct, by name. The checker already validated
    /// the name; codegen emits it nominally.
    Named(String),
    /// A CLASS, by name — carried as `Handle<Name>`. Classes live in
    /// the World and are reached by handle everywhere a type can
    /// appear, so this is the only shape a class-typed value takes.
    Handle(String),
    /// `!T` — `Result<T, E>` with the module's default error enum.
    Fallible { ok: Box<RustTy>, err: String },
}

impl RustTy {
    fn render(&self) -> String {
        match self {
            RustTy::Int => "i64".into(),
            RustTy::Float => "f64".into(),
            RustTy::Bool => "bool".into(),
            RustTy::Str => "Str".into(),
            RustTy::List(t) => format!("List<{}>", t.render()),
            RustTy::Unit => "()".into(),
            RustTy::Opt(t) => format!("Option<{}>", t.render()),
            RustTy::Bytes => "Bytes".into(),
            RustTy::Map(k, v) => format!("Map<{}, {}>", k.render(), v.render()),
            RustTy::Named(n) => n.clone(),
            RustTy::Handle(n) => format!("Handle<{n}>"),
            RustTy::Fallible { ok, err } => format!("Result<{}, {err}>", ok.render()),
        }
    }
    fn dirty_checks(&self) -> bool {
        !matches!(self, RustTy::List(_) | RustTy::Unit)
    }
}

struct PropInfo {
    camel: String,
    rust: String,
    ty: RustTy,
    /// `weak prop x : C` — a reference that does NOT count as an
    /// edge (§8.44). The cycle breaker: two objects naming each other
    /// keep each other alive under refcounting, and marking one side
    /// weak ends it. Reading one answers `T?`, which costs nothing to
    /// implement because handles are already generational — no side
    /// table, no zeroing pass.
    is_weak: bool,
    /// `None` = no `default:` — legal only when the class has an
    /// `init` that definitely assigns the prop (§8.25).
    default: Option<Expr>,
    notify_const: String,
    /// `false` for a `let` FIELD: init-once, assignable only inside
    /// `init` (§8.58). `prop` and `var` are both `true`.
    assignable: bool,
    /// The keyword the author wrote, for the error when they assign
    /// to something that does not take one.
    keyword: &'static str,
    /// `prop full : String, bind { first + " " + last }` — a DERIVED
    /// property (§8.61). Nothing is stored: the getter evaluates this
    /// on every read, and reactivity comes free because a view
    /// subscribes to its classes, not to individual properties.
    derived: Option<Expr>,
}

struct SignalInfo {
    camel: String,
    const_name: String,
    id: u32,
}

struct ClassInfo<'a> {
    name: String,
    props: Vec<PropInfo>,
    /// Explicit `signal` declarations plus the synthesized prop notifies.
    signals: Vec<SignalInfo>,
    /// Own methods first, then trait-impl-donated ones (resolution
    /// sees all; the Ref-trait emission stops at `own_method_count`
    /// — donated bodies emit into real `impl Trait for Handle<C>`
    /// blocks instead, §8.20).
    methods: Vec<&'a ast::FnDecl>,
    own_method_count: usize,
    /// The user constructor (§8.25) — at most one in v1.
    init: Option<&'a ast::InitDecl>,
    /// Class-level type params (§8.25 generic classes) — unbounded
    /// in v1, every one implicitly `Clone + 'static`.
    generics: Vec<String>,
    /// `static fn` members (§8.54): associated functions with no
    /// receiver and no World. They emit as `impl C { pub fn .. }`
    /// and are called `C.name(args)`.
    statics: Vec<&'a ast::FnDecl>,
    /// The class's `deinit` body (§8.60), run by the kernel when the
    /// last reference goes.
    deinit: Option<&'a ast::DeinitDecl>,
}

impl ClassInfo<'_> {
    fn prop(&self, camel: &str) -> Option<&PropInfo> {
        self.props.iter().find(|p| p.camel == camel)
    }
    fn signal(&self, camel: &str) -> Option<&SignalInfo> {
        self.signals.iter().find(|s| s.camel == camel)
    }
}

/// Rust keywords (2024 edition, strict + reserved). A pixie surface
/// name that lands on one gets a trailing underscore (§11.8) — the
/// concatenation-safe escape (`set_type_`, `__p_gen_` all stay valid
/// where `r#` would not). The kernel's own `gen` → `generation`
/// rename was this bug class biting pixie itself.
const RUST_KEYWORDS: &[&str] = &[
    "as", "abstract", "async", "await", "become", "box", "break", "const", "continue", "crate",
    "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl",
    "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub",
    "ref", "return", "self", "static", "struct", "super", "trait", "true", "try", "type",
    "typeof", "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

/// Names the generated crate already uses for its own types — the
/// runtime imports plus the Rust prelude (§8.67). A pixie type named
/// one of these emits a `pub struct Box` that shadows the real one,
/// and the failure lands hundreds of lines away inside machinery the
/// author never wrote (D10). Keywords get a trailing underscore
/// (§11.8); these cannot, because the name is also the reflection
/// table's key and the interpreted tier looks it up by what the
/// author wrote.
const RESERVED_TYPE_NAMES: &[&str] = &[
    // The runtime, as the generated preamble imports it.
    "Bytes", "Component", "Element", "Handle", "LazyRows", "List", "Map", "Runtime",
    "SignalId", "Str", "World", "Rc", "FromStr",
    // The Rust prelude types a body can mention.
    "Box", "Vec", "Option", "Result", "String", "Clone", "Copy", "Default", "Drop",
    "Iterator", "Into", "From", "Send", "Sync", "Sized", "Fn", "FnMut", "FnOnce",
    "Some", "None", "Ok", "Err", "Self",
];

/// A pixie type name that would collide with the generated crate's
/// own vocabulary.
fn check_type_name(name: &str, kind: &str, span: Span) -> Result<(), EmitError> {
    if !RESERVED_TYPE_NAMES.contains(&name) {
        return Ok(());
    }
    err(
        span,
        format!(
            "`{name}` is a name the generated program already uses, so a {kind} cannot \
             take it — pick another (`{name}Item`, `My{name}`)"
        ),
    )
}

fn escape_rust_keyword(s: String) -> String {
    if RUST_KEYWORDS.contains(&s.as_str()) {
        format!("{s}_")
    } else {
        s
    }
}

/// Temporaries the EMITTER writes into method bodies. A pixie local
/// named `w` shadowed the World and the failure landed on the setter
/// call two lines down (§8.68); the `__`-prefixed ones are the same
/// hazard for anyone who writes an identifier that looks generated.
///
/// Listed rather than matched on the `__` prefix, because the
/// component splice SYNTHESIZES names in that space
/// (`__c1___pixie_state`) and the interpreted tier reproduces them by
/// hand — renaming those would break the two tiers' agreement about
/// what a holder is called.
///
/// Renaming is safe where reserving a TYPE name was not (§8.67): a
/// local has no reflection-table entry, so nothing looks it up by
/// what the author wrote.
const EMITTER_LOCALS: &[&str] = &[
    "w", "__v", "__o", "__h", "__k", "__x", "__m", "__old", "__slot", "__xs", "__it",
    "__args", "__out", "__self", "__view", "__rt", "__tree", "__f", "__step",
];

fn escape_emitter_local(s: String) -> String {
    if EMITTER_LOCALS.contains(&s.as_str()) {
        format!("{s}_")
    } else {
        s
    }
}

fn camel_to_snake(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    escape_emitter_local(escape_rust_keyword(out))
}

fn scream(s: &str) -> String {
    camel_to_snake(s).to_uppercase()
}

/// The class names in scope, so `lower_type` can tell an object from
/// a value. A class is World-resident: wherever its name appears as a
/// TYPE — a prop, a struct field, a parameter, a return, a `List`
/// element — the thing carried is a `Handle`, never the struct
/// itself. Emitting the struct is what made `prop kid : Leaf`,
/// `fn take(l: Leaf)` and `List<Leaf>` all fail inside the generated
/// crate (§11.23).
type ClassNames<'a> = &'a std::collections::HashSet<String>;

/// Does this type hold World objects, directly or in a container?
/// Only these need edge counting (§8.44) — a class of plain values
/// costs nothing at all.
impl RustTy {
    fn holds_objects(&self) -> bool {
        match self {
            RustTy::Handle(_) => true,
            RustTy::List(inner) | RustTy::Opt(inner) => inner.holds_objects(),
            RustTy::Map(_, v) => v.holds_objects(),
            _ => false,
        }
    }
}

/// Emit a retain over every object `val` carries.
fn retain_expr(ty: &RustTy, val: &str) -> String {
    match ty {
        RustTy::Handle(_) => format!("w.retain(({val}).erase());"),
        RustTy::List(inner) if inner.holds_objects() => format!(
            "{{ let __xs = ({val}).clone(); for __e in __xs.iter() {{ {} }} }}",
            retain_expr(inner, "(*__e)")
        ),
        RustTy::Opt(inner) if inner.holds_objects() => format!(
            "if let Some(__o) = ({val}).clone() {{ {} }}",
            retain_expr(inner, "__o")
        ),
        RustTy::Map(_, v) if v.holds_objects() => format!(
            "{{ let __m = ({val}).clone(); for __e in __m.values().iter() {{ {} }} }}",
            retain_expr(v, "(*__e)")
        ),
        _ => String::new(),
    }
}

fn release_expr(ty: &RustTy, val: &str) -> String {
    retain_expr(ty, val).replace("w.retain(", "w.release(")
}

/// The class carried by a lowered type, if it carries one.
fn handle_of(t: &RustTy) -> Option<String> {
    match t {
        RustTy::Handle(n) => Some(n.clone()),
        _ => None,
    }
}

/// The class an expression denotes, when it denotes an OBJECT rather
/// than a value. This is what lets `o.kid.v` lower at all: `o.kid` is
/// a handle of class `Leaf`, so `.v` is the accessor call and not a
/// struct-field read (§11.23). Everything it needs is declared —
/// props carry their lowered type, methods their return type — so
/// this is a walk over declarations, not inference.
fn handle_class_of(e: &Expr, cx: &MethodCtx) -> Option<String> {
    match &e.kind {
        // `this` denotes the receiver, so it is a handle of the class
        // whose method is running (§8.63) — which is what makes
        // `this.kid.v` and passing `this` to a class-typed parameter
        // resolve like any other object expression.
        ExprKind::Ident(n) if n == "this" => Some(cx.class.name.clone()),
        ExprKind::Ident(n) => {
            if let Some(info) = cx.local_class(n) {
                return Some(info.name.clone());
            }
            if let Some((info, _)) = cx.global(n) {
                return Some(info.name.clone());
            }
            cx.class.prop(n).and_then(|p| handle_of(&p.ty))
        }
        ExprKind::AtIdent(n) => cx.class.prop(n).and_then(|p| handle_of(&p.ty)),
        ExprKind::Call { callee, .. } => match &callee.kind {
            ExprKind::Ident(c) if cx.classes.contains_key(c) => Some(c.clone()),
            _ => None,
        },
        ExprKind::Member { receiver, name } => {
            let c = handle_class_of(receiver, cx)?;
            let info = cx.classes.get(&c)?;
            info.prop(&name.name).and_then(|p| handle_of(&p.ty))
        }
        // Indexing a list OF objects hands back one of them.
        ExprKind::Index { receiver, .. } => match declared_ty_of(receiver, cx)? {
            RustTy::List(elem) => handle_of(&elem),
            _ => None,
        },
        ExprKind::MethodCall {
            receiver, method, ..
        } => {
            // A method that RETURNS an object hands back a handle.
            let owner = match &receiver.kind {
                ExprKind::SelfRef => Some(cx.class.name.clone()),
                _ => handle_class_of(receiver, cx),
            }?;
            let info = cx.classes.get(&owner)?;
            let decl = info.methods.iter().find(|m| m.name.name == method.name)?;
            named_class(decl.return_ty.as_ref()?, cx.class_names)
        }
        _ => None,
    }
}

/// Resolve a `push` receiver to (the holder's handle expression, the
/// property). Covers this class's own property, a global's, and one
/// reached through any object expression.
fn list_push_target<'a>(
    receiver: &Expr,
    cx: &'a MethodCtx<'a>,
) -> Option<(String, &'a PropInfo)> {
    match &receiver.kind {
        ExprKind::Ident(n) | ExprKind::AtIdent(n) => {
            let pi = cx.class.prop(n)?;
            matches!(pi.ty, RustTy::List(_)).then(|| ("self".to_string(), pi))
        }
        ExprKind::Member { receiver: r2, name } => {
            if let ExprKind::Ident(g) = &r2.kind {
                if let Some((info, handle)) = cx.global(g) {
                    let pi = info.prop(&name.name)?;
                    return matches!(pi.ty, RustTy::List(_)).then_some((handle, pi));
                }
            }
            // Through an object: the receiver denotes one, so its
            // class says which property this is.
            let c = handle_class_of(r2, cx)?;
            let info = cx.classes.get(&c)?;
            let pi = info.prop(&name.name)?;
            if !matches!(pi.ty, RustTy::List(_)) {
                return None;
            }
            let holder = lower_method_expr(r2, cx).ok()?;
            Some((holder, pi))
        }
        _ => None,
    }
}

fn map_insert_target<'a>(
    receiver: &Expr,
    cx: &'a MethodCtx<'a>,
) -> Option<(String, &'a PropInfo)> {
    match &receiver.kind {
        ExprKind::Ident(n) | ExprKind::AtIdent(n) => {
            let pi = cx.class.prop(n)?;
            matches!(pi.ty, RustTy::Map(_, _)).then(|| ("self".to_string(), pi))
        }
        ExprKind::Member { receiver: r2, name } => {
            if let ExprKind::Ident(g) = &r2.kind {
                if let Some((info, handle)) = cx.global(g) {
                    let pi = info.prop(&name.name)?;
                    return matches!(pi.ty, RustTy::Map(_, _)).then_some((handle, pi));
                }
            }
            let c = handle_class_of(r2, cx)?;
            let info = cx.classes.get(&c)?;
            let pi = info.prop(&name.name)?;
            if !matches!(pi.ty, RustTy::Map(_, _)) {
                return None;
            }
            let holder = lower_method_expr(r2, cx).ok()?;
            Some((holder, pi))
        }
        _ => None,
    }
}

/// The DECLARED lowered type of an expression, where a declaration
/// exists to read it off — a prop of the enclosing class, a global's
/// prop, or a prop reached through an object chain. Locals have no
/// declaration to consult, so they answer `None`.
fn declared_ty_of(e: &Expr, cx: &MethodCtx) -> Option<RustTy> {
    match &e.kind {
        ExprKind::Ident(n) | ExprKind::AtIdent(n) => cx
            .locals
            .iter()
            .find(|(l, _, _, _)| l == n)
            .and_then(|(_, _, _, t)| t.clone())
            .or_else(|| cx.class.prop(n).map(|p| p.ty.clone())),
        ExprKind::Member { receiver, name } => {
            if let ExprKind::Ident(r) = &receiver.kind {
                if let Some((info, _)) = cx.global(r) {
                    return info.prop(&name.name).map(|p| p.ty.clone());
                }
            }
            if let Some(c) = handle_class_of(receiver, cx) {
                return cx.classes.get(&c)?.prop(&name.name).map(|p| p.ty.clone());
            }
            // A field of a STRUCT value (§8.68) — a struct-typed prop,
            // a local, or a chain of either.
            let RustTy::Named(sname) = declared_ty_of(receiver, cx)? else {
                return None;
            };
            let st = cx.structs.get(&sname)?;
            st.fields
                .iter()
                .find(|(surface, _, _)| surface == &name.name)
                .map(|(_, _, t)| t.clone())
        }
        _ => None,
    }
}

/// The class a type expression names outright, if it names one. Used
/// to register class-typed parameters and locals as handles.
fn named_class(t: &TypeExpr, classes: ClassNames<'_>) -> Option<String> {
    match &t.kind {
        TypeKind::Named { path, args } if path.len() == 1 && args.is_empty() => {
            let n = path[0].name.as_str();
            classes.contains(n).then(|| n.to_string())
        }
        _ => None,
    }
}

fn lower_type(t: &TypeExpr, classes: ClassNames<'_>) -> Result<RustTy, EmitError> {
    match &t.kind {
        TypeKind::Named { path, args } if path.len() == 1 => {
            let name = path[0].name.as_str();
            match (name, args.len()) {
                ("Int", 0) => Ok(RustTy::Int),
                ("Float", 0) => Ok(RustTy::Float),
                ("Bool", 0) => Ok(RustTy::Bool),
                ("String", 0) => Ok(RustTy::Str),
                ("Bytes", 0) => Ok(RustTy::Bytes),
                ("Void", 0) => Ok(RustTy::Unit),
                // `Self` reaches the emitter as an ordinary name, and
                // it happens to mean the right thing inside a `Ref`
                // trait impl — while every use of the RESULT fails to
                // type-check, because the checker has no such type
                // (§8.63). A D10 violation waiting to happen, so it is
                // a pixie error naming what to write instead.
                ("Self", 0) => err(
                    t.span,
                    "pixie has no `Self` type — name the class. A method that answers \
                     its own object declares `C` and returns `this`",
                ),
                ("List", 1) => Ok(RustTy::List(Box::new(lower_type(&args[0], classes)?))),
                ("Map", 2) => Ok(RustTy::Map(
                    Box::new(lower_type(&args[0], classes)?),
                    Box::new(lower_type(&args[1], classes)?),
                )),
                (_, 0) if classes.contains(name) => Ok(RustTy::Handle(name.to_string())),
                (_, 0) => Ok(RustTy::Named(name.to_string())),
                // User generic type references (`Pair<T>`,
                // `Basket<Int>`) render with their lowered args —
                // rustc checks the instantiation (§8.25).
                _ => {
                    let mut rendered = Vec::new();
                    for a in args {
                        rendered.push(lower_type(a, classes)?.render());
                    }
                    let inst = format!("{name}<{}>", rendered.join(", "));
                    if classes.contains(name) {
                        return Ok(RustTy::Handle(inst));
                    }
                    Ok(RustTy::Named(inst))
                }
            }
        }
        // `T?` — an emitted `Option` (§11.11). Fn returns and params
        // coerce at the boundaries; props and `let` locals of `T?`
        // are gated separately.
        TypeKind::Nullable(inner) => Ok(RustTy::Opt(Box::new(lower_type(inner, classes)?))),
        // `TypeKind::Fn` is never produced by the parser (see its AST
        // comment) and `SelfType` is reserved — neither is reachable
        // today, but a shape that arrives here is a pixie gap, not a
        // design decision (§8.63).
        _ => err(t.span, "this type shape is not lowerable yet (M0)"),
    }
}

/// Return-position lowering: `!T` maps to `Result<T, DefaultError>`.
fn lower_return_type(
    t: &TypeExpr,
    default_error: Option<&str>,
    classes: ClassNames<'_>,
) -> Result<RustTy, EmitError> {
    if let TypeKind::ErrorUnion(inner) = &t.kind {
        let Some(err_name) = default_error else {
            return err(t.span, "`!T` needs a module `error` enum (declare `error E { ... }`)");
        };
        return Ok(RustTy::Fallible {
            ok: Box::new(lower_type(inner, classes)?),
            err: err_name.to_string(),
        });
    }
    lower_type(t, classes)
}

/// Escaping for a PLAIN Rust string literal — everything
/// `escape_fmt_text` does except the format!-only brace doubling.
fn escape_plain_text(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
}

fn escape_fmt_text(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '{' => out.push_str("{{"),
            '}' => out.push_str("}}"),
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
}

// ---------------------------------------------------------------------------
// Binding classes (.rpi): type surface for foreign Rust crates. Never
// emitted as structs — calls lower to inline adapter expressions, so a
// bound fn costs exactly its Rust call plus declared-type conversions.

struct BindingFn {
    rust_path: String,
    params: Vec<RustTy>,
    ret: RustTy,
    fallible: bool,
    /// rpi-gen marks a fn whose params/return are std `HashMap`s
    /// with a `stdmap:` path prefix (yokan's crate boundary); the adapter then
    /// converts the kernel Map at the boundary instead of passing
    /// it through (battery fns take the kernel Map directly).
    std_map: bool,
}

#[derive(Default)]
struct BindingClass {
    statics: HashMap<String, BindingFn>,
}

fn collect_binding_class(
    c: &ast::ClassDecl,
    classes: ClassNames<'_>,
) -> Result<(String, BindingClass), EmitError> {
    let mut out = BindingClass::default();
    for m in &c.members {
        let ClassMember::Fn(f) = m else { continue };
        if !f.is_static {
            continue;
        }
        let Some(path) = f
            .attributes
            .iter()
            .find_map(|a| (a.name.name == "rust").then(|| a.args.first().cloned()).flatten())
        else {
            return err(
                f.span,
                format!(
                    "binding fn `{}` needs a `@rust(\"path\")` attribute",
                    f.name.name
                ),
            );
        };
        let rust_path = path.trim().trim_matches('"').to_string();
        let (rust_path, std_map) = match rust_path.strip_prefix("stdmap:") {
            Some(rest) => (rest.to_string(), true),
            None => (rust_path, false),
        };
        let mut params = Vec::new();
        for p in &f.params {
            params.push(lower_type(&p.ty, classes)?);
        }
        // `T?` is legal in the binding RETURN position only (§11.11):
        // an `Option<T>`-returning Rust fn surfaces as `-> T?`, plain
        // or under `!`. Params and user-code positions keep erroring
        // through `lower_type` until construction lands.
        let lower_ret = |t: &TypeExpr| -> Result<RustTy, EmitError> {
            if let TypeKind::Nullable(inner) = &t.kind {
                return Ok(RustTy::Opt(Box::new(lower_type(inner, classes)?)));
            }
            lower_type(t, classes)
        };
        let (ret, fallible) = match &f.return_ty {
            None => (RustTy::Unit, false),
            Some(t) => match &t.kind {
                TypeKind::ErrorUnion(inner) => (lower_ret(inner)?, true),
                _ => (lower_ret(t)?, false),
            },
        };
        out.statics.insert(
            f.name.name.clone(),
            BindingFn {
                rust_path,
                params,
                ret,
                fallible,
                std_map,
            },
        );
    }
    Ok((c.name.name.clone(), out))
}

/// The element conversion going the OTHER way (§8.73): a pixie value
/// on its way INTO a foreign Rust fn, inside a `List<T>` or a `T?`.
/// It has to be owned rather than borrowed — you cannot hand out a
/// `&[String]` when what you hold is a `&[Str]`.
fn binding_arg_elem_conv(
    t: &RustTy,
    enums: &HashMap<String, EnumInfo<'_>>,
    structs: &HashMap<String, StructInfo<'_>>,
) -> Option<String> {
    match t {
        RustTy::Str => Some("|x: &Str| x.as_str().to_string()".into()),
        RustTy::Int | RustTy::Float | RustTy::Bool => Some("|x| *x".into()),
        RustTy::Bytes => Some("|x: &Bytes| x.as_slice().to_vec()".into()),
        RustTy::Named(n) => named_arg_conv(n, enums, structs),
        _ => None,
    }
}

/// How a binding return converts to its pixie value.
enum RetConv {
    /// Raw value passes through.
    Pass,
    /// An EXPRESSION over the returned value, which the caller binds
    /// as `__v`. Not a closure: `(|v| ..)(call)` leaves the parameter
    /// untyped and rustc refuses (E0282), while a `let` takes its type
    /// from the call. D10 makes that our bug, not the author's.
    Expr(String),
}

/// Declared-return conversion for a binding call, driven entirely by
/// the `.rpi` type (D6): `String` absorbs String and PathBuf through
/// `Str::from`, `Int` / `Float` widen any native number via `as`,
/// `List<T>` and `T?` convert element by element, and a declared
/// enum or struct follows the correspondence its `.rpi` wrote
/// (§8.74, §8.77). `Err(())` marks an unadaptable type.
#[allow(clippy::result_unit_err)]
/// Entry conversions for the std-HashMap boundary (yokan's crate boundary). map_kv
/// admits Str / Int / Float / Bool only, so these two are total over
/// what a `stdmap:` fn can declare.
fn std_map_out_conv(t: &RustTy, var: &str) -> String {
    match t {
        RustTy::Str => format!("{var}.as_str().to_string()"),
        _ => var.to_string(),
    }
}

fn std_map_in_conv(t: &RustTy, var: &str) -> String {
    match t {
        RustTy::Str => format!("Str::from({var})"),
        _ => var.to_string(),
    }
}

fn binding_ret_conv(
    ret: &RustTy,
    std_map: bool,
    enums: &HashMap<String, EnumInfo<'_>>,
    structs: &HashMap<String, StructInfo<'_>>,
) -> Result<RetConv, ()> {
    match ret {
        // A binding describes a foreign RUST surface, so a World
        // handle can never cross it: `.rpi` files name value types
        // only. `Fallible` is unwrapped by the caller, which knows
        // whether it is in a `try` position.
        RustTy::Handle(_) | RustTy::Fallible { .. } => Err(()),
        // A `stdmap:` fn answers std's HashMap — collect it into the
        // kernel Map (sorted by the BTreeMap underneath, so the
        // crossing is deterministic; yokan's crate boundary).
        RustTy::Map(mk, mv) if std_map => {
            let kf = std_map_in_conv(mk, "__k");
            let wf = std_map_in_conv(mv, "__w");
            Ok(RetConv::Expr(format!(
                "__v.into_iter().map(|(__k, __w)| ({kf}, {wf})).collect::<Map<_, _>>()"
            )))
        }
        // Kernel-typed returns pass through untouched.
        RustTy::Bool | RustTy::Unit | RustTy::Map(..) => Ok(RetConv::Pass),
        other => {
            let e = ret_expr_of(other, "__v", enums, structs, &mut Vec::new()).ok_or(())?;
            // A conversion that is the identity needs no binding —
            // `{ let __v = call; __v }` would be noise in every
            // generated line.
            Ok(if e == "__v" {
                RetConv::Pass
            } else {
                RetConv::Expr(e)
            })
        }
    }
}

/// Lower `Binding.fn(args)` to the adapted Rust call. Errors map to
/// `Str` messages (`.map_err(|e| Str::from(e.to_string()))`) — the M1
/// error-value story; typed error enums come later.
/// The STRUCT type of a method receiver, when declarations alone can
/// answer it: a struct-typed property (own prop or a global's), a
/// struct construction, or a chain of struct-method calls rooted at
/// one of those. Class-typed receivers answer None and keep their
/// designed rejection (class methods need `&mut World`).
fn struct_recv_of(e: &Expr, cx: &MethodCtx) -> Option<String> {
    let as_struct = |ty: &RustTy| match ty {
        RustTy::Named(s) if cx.structs.contains_key(s.as_str()) => Some(s.clone()),
        _ => None,
    };
    match &e.kind {
        ExprKind::AtIdent(n) => as_struct(&cx.class.prop(n)?.ty),
        ExprKind::Member { receiver, name } => match &receiver.kind {
            ExprKind::Ident(g) => {
                let (info, _) = cx.global(g)?;
                as_struct(&info.prop(&name.name)?.ty)
            }
            _ => None,
        },
        ExprKind::MethodCall { receiver, method, block: None, .. } => {
            let s = struct_recv_of(receiver, cx)?;
            let info = cx.structs.get(s.as_str())?;
            let f = info.methods.iter().find(|m| m.name.name == method.name)?;
            as_struct(&lower_type(f.return_ty.as_ref()?, cx.class_names).ok()?)
        }
        ExprKind::Call { callee, block: None, .. } => match &callee.kind {
            ExprKind::Ident(n) if cx.structs.contains_key(n.as_str()) => Some(n.clone()),
            _ => None,
        },
        _ => None,
    }
}

fn lower_binding_call(
    bf: &BindingFn,
    args: &[Expr],
    cx: &MethodCtx,
    span: Span,
) -> Result<String, EmitError> {
    if args.len() != bf.params.len() {
        return err(span, format!("expected {} argument(s)", bf.params.len()));
    }
    let mut lowered = Vec::new();
    for (a, ty) in args.iter().zip(&bf.params) {
        // A literal `nil` at a `T?` parameter has no value to lower:
        // the argument IS Rust's `None` (§8.18's coercion, literal
        // edition — found by yokan's Optional crate crossings).
        if matches!(ty, RustTy::Opt(_)) && matches!(a.kind, ExprKind::Nil) {
            lowered.push("None".into());
            continue;
        }
        let v = lower_method_expr(a, cx)?;
        lowered.push(match ty {
            // A `.rpi` names foreign RUST types; a World handle has no
            // meaning across that boundary, so it never appears here.
            RustTy::Handle(_) => {
                return err(span, "a class cannot cross a binding — bindings take value types");
            }
            RustTy::Str => format!("({v}).as_str()"),
            RustTy::Bytes => format!("({v}).as_slice()"),
            // The kernel's own COW map passes by value — battery fns
            // (§12.3's Http) take it directly. A `stdmap:` fn wants
            // std's HashMap instead: `pairs()` reads the entries out
            // sorted and the collect owns them (yokan's crate boundary).
            RustTy::Map(mk, mv) => {
                if bf.std_map {
                    let kc = std_map_out_conv(mk, "__k");
                    let wc = std_map_out_conv(mv, "__w");
                    format!(
                        "({v}).pairs().into_iter().map(|(__k, __w)| ({kc}, {wc}))\
                         .collect::<std::collections::HashMap<_, _>>()"
                    )
                } else {
                    v
                }
            }
            RustTy::Int | RustTy::Float | RustTy::Bool => v,
            // A LIST arrives as an owned `Vec` of the Rust element
            // type — the same `Vec<T>` shape the return side produces,
            // so a bound fn reads and writes one vocabulary (§8.73).
            RustTy::List(inner) => {
                let Some(conv) = binding_arg_elem_conv(inner, cx.enums, cx.structs) else {
                    return err(
                        a.span,
                        format!(
                            "a `List<{}>` cannot cross a binding — a list of numbers, \
                             bools, strings, bytes, or a type the `.rpi` mapped with \
                             `@rust(..)` can",
                            inner.render()
                        ),
                    );
                };
                format!("({v}).iter().map({conv}).collect::<Vec<_>>()")
            }
            // And a `T?` as an `Option` of the same.
            RustTy::Opt(inner) => {
                let Some(conv) = binding_arg_elem_conv(inner, cx.enums, cx.structs) else {
                    return err(
                        a.span,
                        format!(
                            "a `{}?` cannot cross a binding — an optional number, bool, \
                             string, byte string, or a type the `.rpi` mapped with \
                             `@rust(..)` can",
                            inner.render()
                        ),
                    );
                };
                // A plain value coerces into the optional at the call
                // site (§8.18), so the argument may not be an
                // `Option` yet — wrap it before mapping, or `as_ref`
                // lands on the bare value. The same test the `T?`
                // return and parameter coercions use.
                if expr_is_opt(a, cx) {
                    format!("({v}).as_ref().map({conv})")
                } else {
                    format!("Some({v}).as_ref().map({conv})")
                }
            }
            RustTy::Unit => {
                return err(a.span, "`Void` is not a value a binding can take");
            }
            // A declared type reads its correspondence right to left
            // (§8.74, §8.77). One closure serves here and inside a
            // list, so the two directions cannot drift apart.
            RustTy::Named(n) => {
                let Some(conv) = named_arg_conv(n, cx.enums, cx.structs) else {
                    return err(a.span, unmapped_type_msg(n, cx.enums, cx.structs));
                };
                format!("({conv})(&({v}))")
            }
            RustTy::Fallible { .. } => {
                return err(
                    a.span,
                    "a fallible is what a binding RETURNS, not something to hand it — \
                     unwrap it with `case` and pass the value",
                );
            }
        });
    }
    let call = format!("{}({})", bf.rust_path, lowered.join(", "));
    let Ok(conv) = binding_ret_conv(&bf.ret, bf.std_map, cx.enums, cx.structs) else {
        return err(
            span,
            match &bf.ret {
                RustTy::Named(n) => unmapped_type_msg(n, cx.enums, cx.structs),
                other => format!(
                    "a binding cannot return `{}` — values, lists, maps, optionals and \
                     `!T` cross, and an `enum` or a `struct` crosses when the `.rpi` \
                     says how",
                    other.render()
                ),
            },
        );
    };
    if bf.fallible {
        Ok(match conv {
            RetConv::Expr(t) => {
                format!("{call}.map(|__v| {t}).map_err(|e| Str::from(e.to_string()))")
            }
            RetConv::Pass => format!("{call}.map_err(|e| Str::from(e.to_string()))"),
        })
    } else {
        Ok(match conv {
            RetConv::Expr(t) => format!("{{ let __v = {call}; {t} }}"),
            RetConv::Pass => call,
        })
    }
}

/// Top-level `let name : Class = Class.new()` — a World singleton.
/// Maps let-name -> class name.
type Globals = HashMap<String, String>;

struct EnumInfo<'a> {
    name: String,
    variants: Vec<&'a ast::EnumVariant>,
    /// The Rust enum this one corresponds to, when a `.rpi` said so
    /// (§8.74). Its presence is what lets one cross a binding.
    rust_path: Option<String>,
}

impl EnumInfo<'_> {
    fn variant(&self, name: &str) -> Option<&ast::EnumVariant> {
        self.variants.iter().copied().find(|v| v.name.name == name)
    }

    /// The two halves of one variant's correspondence: the pixie
    /// path and the Rust one. A variant with no `@rust` uses its own
    /// name, so an enum whose names already agree needs one
    /// attribute, not one per variant.
    ///
    /// `None` when this enum cannot correspond to a Rust one at all
    /// — no `@rust`, or a variant that carries a PAYLOAD (§8.76). The
    /// generated conversion is a match over unit variants; a payload
    /// would need its fields related too, and emitting the unit form
    /// for one produced Rust that does not compile.
    fn variant_pairs(&self) -> Option<Vec<(String, String)>> {
        let rp = self.rust_path.as_ref()?;
        if self.variants.iter().any(|v| !v.fields.is_empty()) {
            return None;
        }
        Some(
            self.variants
                .iter()
                .map(|v| {
                    let mine = format!(
                        "{}::{}",
                        self.name,
                        escape_rust_keyword(v.name.name.clone())
                    );
                    let theirs = format!(
                        "{rp}::{}",
                        v.rust_name.clone().unwrap_or_else(|| v.name.name.clone())
                    );
                    (mine, theirs)
                })
                .collect(),
        )
    }
}

struct StructInfo<'a> {
    name: String,
    /// Value-type generics (§8.25) — unbounded, implicitly `Clone`.
    generics: Vec<String>,
    /// (surface name, rust name, type) in declaration order.
    fields: Vec<(String, String, RustTy)>,
    /// Per-field `= expr` defaults (§8.68), positionally. A trailing
    /// run of them may be omitted at a construction site.
    defaults: Vec<Option<Expr>>,
    /// The Rust struct this one corresponds to, when a `.rpi` said so
    /// (§8.77), and each field's counterpart name.
    rust_path: Option<String>,
    rust_fields: Vec<Option<String>>,
    /// Each field's Rust TYPE when the `.rpi` named one (§8.78).
    rust_types: Vec<Option<String>>,
    methods: Vec<&'a ast::FnDecl>,
}

// ---------------------------------------------------------------------------
// Expression lowering inside class method bodies.

struct MethodCtx<'a> {
    class: &'a ClassInfo<'a>,
    /// Class names, for `lower_type` (§11.23).
    class_names: &'a std::collections::HashSet<String>,
    /// Objects created in the current scopes that provably do not
    /// escape it (§8.42), innermost scope last. `lower_scope` drains
    /// its own and emits one `World::remove` per entry.
    reclaim: Vec<Vec<String>>,
    /// (surface name, Some(class) when the local holds a Handle,
    /// true when the local is `T?`-typed — an emitted `Option`, and
    /// the declared type when the emitter could work it out, §8.68).
    locals: Vec<(String, Option<String>, bool, Option<RustTy>)>,
    bindings: &'a HashMap<String, BindingClass>,
    classes: &'a HashMap<String, ClassInfo<'a>>,
    globals: &'a Globals,
    free_fns: &'a HashMap<String, String>,
    /// The module's free-fn declarations, for signature questions the
    /// name map can't answer (is the return `T?`).
    free_decls: &'a [&'a ast::FnDecl],
    /// Declared traits, for dispatching through generic bounds.
    traits: &'a HashMap<String, &'a ast::TraitDecl>,
    /// Params of the enclosing fn whose type is a trait-bounded
    /// generic (`thing` in `fn f<T: X>(thing: T)`) → the FIRST bound
    /// (the checker's first-bound-wins rule). Method calls on these
    /// thread `w` through the real Rust trait.
    generic_locals: HashMap<String, String>,
    enums: &'a HashMap<String, EnumInfo<'a>>,
    structs: &'a HashMap<String, StructInfo<'a>>,
    /// `Some` while lowering a struct method (`self` is the value).
    self_struct: Option<&'a str>,
    default_error: Option<&'a str>,
    /// The enclosing fn returns `!T` — `return` and trailing values wrap
    /// in `Ok(...)`.
    fallible_ret: bool,
    /// The enclosing fn returns `T?` — `return` and trailing values
    /// coerce (`nil` → `None`, plain values wrap in `Some`, values
    /// already `T?` pass through).
    nullable_ret: bool,
    /// Nesting depth of `for`/`while` — `break`/`continue` outside
    /// a loop are named errors, not rustc surprises.
    loop_depth: usize,
}

impl<'a> MethodCtx<'a> {
    fn is_local(&self, name: &str) -> bool {
        self.locals.iter().any(|(l, _, _, _)| l == name)
    }
    /// The local is `T?`-typed (an emitted `Option`).
    fn is_opt_local(&self, name: &str) -> bool {
        self.locals
            .iter()
            .rev()
            .find(|(l, _, _, _)| l == name)
            .is_some_and(|(_, _, opt, _)| *opt)
    }
    /// The class of a handle-holding local, if any.
    fn local_class(&self, name: &str) -> Option<&ClassInfo<'a>> {
        let class = self
            .locals
            .iter()
            .rev()
            .find(|(l, _, _, _)| l == name)?
            .1
            .as_deref()?;
        self.classes.get(class)
    }
    /// `name` names a top-level singleton: (class info, handle expr).
    fn global(&self, name: &str) -> Option<(&'a ClassInfo<'a>, String)> {
        let class_name = self.globals.get(name)?;
        let info = self.classes.get(class_name)?;
        Some((
            info,
            format!("w.singleton_ref::<{class_name}>()"),
        ))
    }
}

/// Every class a property type can hold a handle to.
fn handle_classes_of(ty: &RustTy) -> Vec<String> {
    match ty {
        RustTy::Handle(c) => vec![c.clone()],
        RustTy::List(inner) | RustTy::Opt(inner) => handle_classes_of(inner),
        RustTy::Map(k, v) => {
            let mut out = handle_classes_of(k);
            out.extend(handle_classes_of(v));
            out
        }
        _ => Vec::new(),
    }
}

/// How the author wrote an expression, for a diagnostic that wants to
/// quote it back. Member chains only — that is what the callers ask
/// about.
fn expr_source_name(e: &Expr) -> String {
    match &e.kind {
        ExprKind::Ident(n) | ExprKind::AtIdent(n) => n.clone(),
        ExprKind::Member { receiver, name } => {
            format!("{}.{}", expr_source_name(receiver), name.name)
        }
        _ => "this".to_string(),
    }
}

/// `xs.each { |v| .. }` — the block-passing idiom pixie deliberately
/// does not have (§8.62). Iteration is a STATEMENT here (`for v in
/// xs`, §8.27), which is why a loop body can `break`, `continue` and
/// `return` out of the enclosing method; none of that is available to
/// a method that takes a block.
fn block_call_error(e: &Expr) -> EmitError {
    let what = match &e.kind {
        ExprKind::Call { callee, .. } => match &callee.kind {
            ExprKind::Member { name, .. } => format!("`{}`", name.name),
            ExprKind::Ident(n) => format!("`{n}`"),
            _ => "this call".to_string(),
        },
        _ => "this call".to_string(),
    };
    EmitError {
        span: e.span,
        message: format!(
            "{what} is called with a block, and pixie has no block-passing. To iterate, \
             write `for v in <list> {{ .. }}` — which can also `break`, `continue` and \
             `return`. To group statements, just write them: they already run together"
        ),
    }
}

/// A `bind { .. }` body is evaluated while a view is being BUILT,
/// which only reads the World (§8.61). So it may read — properties,
/// locals of no kind, literals, operators, interpolation — and it may
/// not call anything, because a method takes `&mut World` and could
/// write. The restriction is checked here rather than left to rustc:
/// a borrow error inside generated code is a pixie bug (D10).
fn check_derivable(e: &Expr) -> Result<(), EmitError> {
    use ExprKind as K;
    match &e.kind {
        K::Int(_) | K::Float(_) | K::Bool(_) | K::Nil | K::Ident(_) | K::AtIdent(_) => Ok(()),
        K::Str(parts) => {
            for part in parts {
                match part {
                    StrPart::Text(_) => {}
                    StrPart::Interp(x) => check_derivable(x)?,
                    StrPart::InterpFmt { expr, .. } => check_derivable(expr)?,
                }
            }
            Ok(())
        }
        K::Member { receiver, .. } => check_derivable(receiver),
        K::Index { receiver, index } => {
            check_derivable(receiver)?;
            check_derivable(index)
        }
        K::Unary { expr, .. } => check_derivable(expr),
        K::Binary { lhs, rhs, .. } => {
            check_derivable(lhs)?;
            check_derivable(rhs)
        }
        _ => err(
            e.span,
            "a derived property is evaluated while a view is being built, which only \
             READS the World — so `bind { .. }` reads properties and combines them, and \
             cannot call anything",
        ),
    }
}

/// A Rust value expression converted to its pixie counterpart
/// (§8.74, §8.77), or `None` when the two sides have no stated
/// correspondence. `seen` is the chain of struct names being
/// converted, so a struct that contains itself stops rather than
/// recursing forever.
fn ret_expr_of(
    ty: &RustTy,
    read: &str,
    enums: &HashMap<String, EnumInfo<'_>>,
    structs: &HashMap<String, StructInfo<'_>>,
    seen: &mut Vec<String>,
) -> Option<String> {
    match ty {
        // Every native integer and float WIDENS on the way in — the
        // `.rpi` says `Int` and the Rust side may be a `u32`, here or
        // in a struct field.
        RustTy::Int => Some(format!("{read} as i64")),
        RustTy::Float => Some(format!("{read} as f64")),
        RustTy::Bool => Some(read.to_string()),
        RustTy::Str => Some(format!("Str::from({read})")),
        RustTy::Bytes => Some(format!("Bytes::from({read})")),
        RustTy::List(inner) => {
            let elem = ret_expr_of(inner, "__e", enums, structs, seen)?;
            Some(format!(
                "{read}.into_iter().map(|__e| {elem}).collect::<{}>()",
                ty.render()
            ))
        }
        RustTy::Opt(inner) => {
            let elem = ret_expr_of(inner, "__e", enums, structs, seen)?;
            Some(format!("{read}.map(|__e| {elem})"))
        }
        RustTy::Named(n) => Some(format!(
            "({})({read})",
            named_ret_conv_seen(n, enums, structs, seen)?
        )),
        _ => None,
    }
}

/// A type as its AUTHOR writes it. `render` answers the Rust
/// spelling, which is right inside generated code and wrong in a
/// message: someone who wrote `List<String>` should not be told
/// about a `List<Str>`.
fn pixie_ty_name(t: &RustTy) -> String {
    match t {
        RustTy::Int => "Int".to_string(),
        RustTy::Float => "Float".to_string(),
        RustTy::Bool => "Bool".to_string(),
        RustTy::Str => "String".to_string(),
        RustTy::Bytes => "Bytes".to_string(),
        RustTy::Unit => "Void".to_string(),
        RustTy::List(i) => format!("List<{}>", pixie_ty_name(i)),
        RustTy::Opt(i) => format!("{}?", pixie_ty_name(i)),
        RustTy::Map(k, v) => format!("Map<{}, {}>", pixie_ty_name(k), pixie_ty_name(v)),
        RustTy::Named(n) | RustTy::Handle(n) => n.clone(),
        RustTy::Fallible { ok, .. } => format!("!{}", pixie_ty_name(ok)),
    }
}

/// Writing ONE field back when the `.rpi` named its Rust type
/// (§8.78). Reading infers — `as i64` absorbs any integer width, and
/// `Str::from` absorbs a `PathBuf` — but writing has to name the
/// target, and the emitter never sees the Rust struct. `None` when
/// pixie has no way to write that type.
///
/// A width cast WRAPS, as `as` does everywhere: a negative `Int`
/// written into a `u64` field comes out enormous. The `.rpi` said
/// the width, so this follows it rather than second-guessing it.
fn arg_expr_declared(pix: &RustTy, read: &str, rust_ty: &str) -> Option<String> {
    const INT_WIDTHS: [&str; 10] = [
        "i64", "u64", "u32", "usize", "i32", "u16", "i16", "u8", "i8", "isize",
    ];
    match pix {
        RustTy::Int if INT_WIDTHS.contains(&rust_ty) => {
            Some(format!("({read}.clone()) as {rust_ty}"))
        }
        RustTy::Float if matches!(rust_ty, "f64" | "f32") => {
            Some(format!("({read}.clone()) as {rust_ty}"))
        }
        // A path is a string on this side, both ways: `Str::from`
        // absorbs one coming back, and this builds one going out.
        RustTy::Str if rust_ty.rsplit("::").next() == Some("PathBuf") => {
            Some(format!("{rust_ty}::from({read}.as_str())"))
        }
        // Naming the type the default rule would have written is
        // redundant, not wrong — a `.rpi` may spell every field out.
        RustTy::Str if rust_ty.rsplit("::").next() == Some("String") => {
            Some(format!("{read}.as_str().to_string()"))
        }
        RustTy::Bool if rust_ty == "bool" => Some(format!("{read}.clone()")),
        RustTy::Bytes if rust_ty.replace(' ', "") == "Vec<u8>" => {
            Some(format!("{read}.as_slice().to_vec()"))
        }
        _ => None,
    }
}

/// The same crossing read right to left: a pixie value expression
/// (held by reference) converted to its Rust counterpart.
fn arg_expr_of(
    ty: &RustTy,
    read: &str,
    enums: &HashMap<String, EnumInfo<'_>>,
    structs: &HashMap<String, StructInfo<'_>>,
    seen: &mut Vec<String>,
) -> Option<String> {
    match ty {
        RustTy::Int | RustTy::Float | RustTy::Bool => Some(format!("{read}.clone()")),
        RustTy::Str => Some(format!("{read}.as_str().to_string()")),
        RustTy::Bytes => Some(format!("{read}.as_slice().to_vec()")),
        RustTy::List(inner) => {
            let elem = arg_expr_of(inner, "__e", enums, structs, seen)?;
            Some(format!(
                "{read}.iter().map(|__e| {elem}).collect::<Vec<_>>()"
            ))
        }
        RustTy::Opt(inner) => {
            let elem = arg_expr_of(inner, "__e", enums, structs, seen)?;
            Some(format!("{read}.as_ref().map(|__e| {elem})"))
        }
        RustTy::Named(n) => Some(format!(
            "({})({read})",
            named_arg_conv_seen(n, enums, structs, seen)?
        )),
        _ => None,
    }
}

/// Rust value to pixie value for a DECLARED type: an enum matches
/// variant for variant, a struct is rebuilt field for field, and a
/// field crosses by the same rule the whole value does.
fn named_ret_conv_seen(
    n: &str,
    enums: &HashMap<String, EnumInfo<'_>>,
    structs: &HashMap<String, StructInfo<'_>>,
    seen: &mut Vec<String>,
) -> Option<String> {
    if let Some(pairs) = enums.get(n).and_then(|e| e.variant_pairs()) {
        let arms: Vec<String> = pairs
            .iter()
            .map(|(mine, theirs)| format!("{theirs} => {mine}"))
            .collect();
        return Some(format!("|v| match v {{ {} }}", arms.join(", ")));
    }
    let st = structs.get(n)?;
    let rp = st.rust_path.clone()?;
    if seen.iter().any(|s| s == n) {
        return None;
    }
    seen.push(n.to_string());
    let mut inits = Vec::with_capacity(st.fields.len());
    for (i, (surface, mine, ty)) in st.fields.iter().enumerate() {
        let theirs = st.rust_fields.get(i).cloned().flatten();
        let theirs = theirs.unwrap_or_else(|| surface.clone());
        // Reading does not need the declared Rust type (§8.78) — a
        // cast absorbs any width — but a declaration pixie cannot
        // WRITE is wrong wherever it appears, and saying so at the
        // first use beats saying it only when a value goes back.
        if let Some(rt) = st.rust_types.get(i).and_then(Option::as_ref) {
            if arg_expr_declared(ty, "v", rt).is_none() {
                seen.pop();
                return None;
            }
        }
        let conv = ret_expr_of(ty, &format!("v.{theirs}"), enums, structs, seen);
        let Some(conv) = conv else {
            seen.pop();
            return None;
        };
        inits.push(format!("{mine}: {conv}"));
    }
    seen.pop();
    // The parameter is annotated because nothing else pins it: an
    // enum's match arms name the Rust type, a struct's field reads do
    // not (E0282 otherwise, and D10 makes that our bug).
    Some(format!("|v: {rp}| {n} {{ {} }}", inits.join(", ")))
}

/// Pixie value to Rust value for a DECLARED type, by reference — the
/// same correspondence read right to left. Used for list elements,
/// where what you hold is a `&Stat`.
fn named_arg_conv(
    n: &str,
    enums: &HashMap<String, EnumInfo<'_>>,
    structs: &HashMap<String, StructInfo<'_>>,
) -> Option<String> {
    named_arg_conv_seen(n, enums, structs, &mut Vec::new())
}

fn named_arg_conv_seen(
    n: &str,
    enums: &HashMap<String, EnumInfo<'_>>,
    structs: &HashMap<String, StructInfo<'_>>,
    seen: &mut Vec<String>,
) -> Option<String> {
    if let Some(pairs) = enums.get(n).and_then(|e| e.variant_pairs()) {
        let arms: Vec<String> = pairs
            .iter()
            .map(|(mine, theirs)| format!("{mine} => {theirs}"))
            .collect();
        return Some(format!("|x: &{n}| match x {{ {} }}", arms.join(", ")));
    }
    let st = structs.get(n)?;
    let rp = st.rust_path.clone()?;
    if seen.iter().any(|s| s == n) {
        return None;
    }
    seen.push(n.to_string());
    let mut inits = Vec::with_capacity(st.fields.len());
    for (i, (surface, mine, ty)) in st.fields.iter().enumerate() {
        let theirs = st.rust_fields.get(i).cloned().flatten();
        let theirs = theirs.unwrap_or_else(|| surface.clone());
        let read = format!("(&x.{mine})");
        // A field whose Rust type the `.rpi` named writes through
        // that type (§8.78); the rest go by pixie type alone.
        let conv = match st.rust_types.get(i).and_then(Option::as_ref) {
            Some(rt) => arg_expr_declared(ty, &read, rt),
            None => arg_expr_of(ty, &read, enums, structs, seen),
        };
        let Some(conv) = conv else {
            seen.pop();
            return None;
        };
        inits.push(format!("{theirs}: {conv}"));
    }
    seen.pop();
    Some(format!("|x: &{n}| {rp} {{ {} }}", inits.join(", ")))
}

/// Why a declared type cannot cross a binding (§8.76). A payload
/// variant is a different answer from a missing `@rust`, and telling
/// them apart is the difference between "write this" and "you
/// cannot".
fn unmapped_type_msg(
    n: &str,
    enums: &HashMap<String, EnumInfo<'_>>,
    structs: &HashMap<String, StructInfo<'_>>,
) -> String {
    if let Some(e) = enums.get(n) {
        if e.variants.iter().any(|v| !v.fields.is_empty()) {
            return format!(
                "`{n}` has a variant with a payload, so it cannot correspond to a Rust \
                 enum — the conversion matches variant for variant, and a payload would \
                 need its fields related too. Pass the payload's fields instead"
            );
        }
    }
    if let Some(st) = structs.get(n) {
        if st.rust_path.is_some() {
            // The `@rust` is there, so the correspondence failed on a
            // FIELD: name the first one that cannot cross, and say
            // which direction failed.
            for (i, (f, _, ty)) in st.fields.iter().enumerate() {
                let declared = st.rust_types.get(i).and_then(Option::as_ref);
                if let Some(rt) = declared {
                    if arg_expr_declared(ty, "v", rt).is_none() {
                        return format!(
                            "`{n}`'s field `{f}` says its Rust type is `{rt}`, and pixie \
                             has no way to write `{}` into that. A number writes into \
                             any numeric width, a string into a `String` or a \
                             `PathBuf`, a byte string into a `Vec<u8>`",
                            pixie_ty_name(ty)
                        );
                    }
                    continue;
                }
                let mut seen = vec![n.to_string()];
                if ret_expr_of(ty, "v", enums, structs, &mut seen).is_none() {
                    return format!(
                        "`{n}` cannot correspond to a Rust struct: its field `{f}` is a \
                         `{}`, and a field crosses by the same rule the whole value does \
                         — a number, a bool, a string, a byte string, a list or optional \
                         of those, or another type the `.rpi` mapped",
                        pixie_ty_name(ty)
                    );
                }
            }
        }
    }
    format!(
        "`{n}` is a type pixie declares, and the Rust side has its own — say how they \
         correspond with `@rust(..)` in the `.rpi`, or pass the fields it needs"
    )
}

/// A `let` FIELD is init-once (§8.58): assignable inside `init`,
/// where the object is still being built, and nowhere else. `prop`
/// and `var` both take a write.
fn check_assignable(pi: &PropInfo, class: &str, span: Span) -> Result<(), EmitError> {
    if pi.assignable {
        return Ok(());
    }
    if pi.keyword == "bind" {
        return err(
            span,
            format!(
                "`{}` on `{class}` is derived — its value comes from the `bind {{ .. }}` \
                 body, so writing it would have nowhere to go. Assign what it reads.",
                pi.camel
            ),
        );
    }
    err(
        span,
        format!(
            "`{}` is a `let` field on `{class}` — it takes its value in `init` and \
             does not change after. Write `var {}` if it should.",
            pi.camel, pi.camel
        ),
    )
}

/// Lower an interpolated piece of a method body. Same as
/// `lower_method_expr` except that a `T?` prints as its value or as
/// nothing (§8.68) — `Option` has no `Display`, and the interpreted
/// tier renders `nil` as the empty string.
fn lower_method_display(e: &Expr, cx: &MethodCtx) -> Result<String, EmitError> {
    let v = lower_method_expr(e, cx)?;
    match declared_ty_of(e, cx) {
        Some(RustTy::Opt(_)) => Ok(format!("__pixie_show_opt({v})")),
        _ => Ok(v),
    }
}

fn lower_method_expr(e: &Expr, cx: &MethodCtx) -> Result<String, EmitError> {
    Ok(cast_if_widened(&lower_method_expr_inner(e, cx)?, e.span))
}

fn lower_method_expr_inner(e: &Expr, cx: &MethodCtx) -> Result<String, EmitError> {
    match &e.kind {
        ExprKind::Int(v) => Ok(format!("{v}i64")),
        ExprKind::Float(v) => Ok(format!("{v}f64")),
        ExprKind::Bool(v) => Ok(format!("{v}")),
        ExprKind::Str(parts) => lower_interp(parts, &mut |inner| lower_method_display(inner, cx)),
        // `this` is the receiver (§8.63): the handle the method was
        // called on, which is exactly what `self` already is.
        ExprKind::Ident(n) if n == "this" => Ok("self".to_string()),
        ExprKind::Ident(n) => {
            if cx.is_local(n) {
                Ok(format!("{}.clone()", camel_to_snake(n)))
            } else if let Some(p) = cx.class.prop(n) {
                Ok(format!("self.{}(w)", p.rust))
            } else {
                err(e.span, format!("`{n}` is not lowerable here yet (M0)"))
            }
        }
        // The parser rewrites bare member references inside class bodies
        // to `AtIdent` — member resolution already happened upstream.
        ExprKind::AtIdent(n) => match cx.class.prop(n) {
            Some(p) => Ok(format!("self.{}(w)", p.rust)),
            None => err(e.span, format!("`{n}` is not a lowerable member yet (M0)")),
        },
        ExprKind::Member { receiver, name } if name.name == "length" => {
            let inner = lower_list_value(receiver, cx)?;
            Ok(format!("({inner}.len() as i64)"))
        }
        // Member access: struct-self field, enum variant, global prop,
        // or a blind value-field read (checker-validated struct field).
        ExprKind::Member { receiver, name } => {
            if matches!(receiver.kind, ExprKind::SelfRef) && cx.self_struct.is_some() {
                return Ok(format!("self.{}.clone()", camel_to_snake(&name.name)));
            }
            if let ExprKind::Ident(r) = &receiver.kind {
                if let Some(en) = cx.enums.get(r) {
                    let Some(v) = en.variant(&name.name) else {
                        return err(e.span, format!("no variant `{}` on `{}`", name.name, en.name));
                    };
                    if !v.fields.is_empty() {
                        return err(
                            e.span,
                            format!("variant `{}` carries a payload — construct it with arguments", name.name),
                        );
                    }
                    return Ok(format!("{}::{}", en.name, escape_rust_keyword(name.name.clone())));
                }
                if let Some((info, handle)) = cx.global(r) {
                    let Some(p) = info.prop(&name.name) else {
                        return err(
                            e.span,
                            format!("no property `{}` on `{}`", name.name, info.name),
                        );
                    };
                    return Ok(format!("{handle}.{}(w)", p.rust));
                }
                if let Some(info) = cx.local_class(r) {
                    let Some(p) = info.prop(&name.name) else {
                        return err(
                            e.span,
                            format!("no property `{}` on `{}`", name.name, info.name),
                        );
                    };
                    return Ok(format!("{}.{}(w)", camel_to_snake(r), p.rust));
                }
            }
            // An object receiver anywhere in the chain: read the prop
            // through the World rather than as a struct field.
            if let Some(c) = handle_class_of(receiver, cx) {
                if let Some(info) = cx.classes.get(&c) {
                    if let Some(pi) = info.prop(&name.name) {
                        let base = lower_method_expr(receiver, cx)?;
                        return Ok(format!("({base}).{}(w)", pi.rust));
                    }
                }
            }
            let base = lower_method_expr(receiver, cx)?;
            Ok(format!("({base}).{}.clone()", camel_to_snake(&name.name)))
        }
        ExprKind::Try(inner) => {
            if is_binding_call(inner, cx) {
                return err(
                    e.span,
                    "binding errors are message-typed: handle them with `case` (error-type mapping across bindings is M2)",
                );
            }
            let v = lower_method_expr(inner, cx)?;
            Ok(format!("({v})?"))
        }
        ExprKind::MethodCall {
            receiver,
            method,
            args,
            block,
            ..
        } => {
            if block.is_some() {
                return Err(block_call_error(e));
            }
            if let ExprKind::Ident(recv_name) = &receiver.kind {
                if let Some(en) = cx.enums.get(recv_name) {
                    let Some(v) = en.variant(&method.name) else {
                        return err(
                            e.span,
                            format!("no variant `{}` on `{}`", method.name, en.name),
                        );
                    };
                    if v.fields.len() != args.len() {
                        return err(
                            e.span,
                            format!("variant `{}` takes {} value(s)", method.name, v.fields.len()),
                        );
                    }
                    let mut call = format!("{}::{}(", en.name, escape_rust_keyword(method.name.clone()));
                    for (i, a) in args.iter().enumerate() {
                        if i > 0 {
                            call.push_str(", ");
                        }
                        // The FIELD decides the wrapping, the same way
                        // a property and a struct field do (§8.70) —
                        // this coercion was the whole of the `T?`
                        // payload gate.
                        let fty = lower_type(&v.fields[i].ty, cx.class_names)?;
                        call.push_str(&lower_assign_value(a, &fty, cx)?);
                    }
                    call.push(')');
                    return Ok(call);
                }
                if let Some(bc) = cx.bindings.get(recv_name) {
                    let Some(bf) = bc.statics.get(&method.name) else {
                        return err(
                            e.span,
                            format!("no binding fn `{}` on `{recv_name}`", method.name),
                        );
                    };
                    return lower_binding_call(bf, args, cx, e.span);
                }
                if let Some((info, handle)) = cx.global(recv_name) {
                    if info.methods.iter().any(|m2| m2.name.name == method.name) {
                        let decl = info.methods.iter().find(|m2| m2.name.name == method.name).copied();
                        let head = format!("{handle}.{}(w", camel_to_snake(&method.name));
                        let lowered = lower_w_call_args(decl.map(|f| f.params.as_slice()), args, cx)?;
                        return Ok(finish_w_call(&head, lowered));
                    }
                    return err(
                        e.span,
                        format!("no method `{}` on `{}`", method.name, info.name),
                    );
                }
                // `C.name(args)` — an associated function, called
                // through the class NAME rather than an instance
                // (§8.54). Checked before the local/instance paths so
                // a class name never resolves to a receiver.
                if let Some(info) = cx.classes.get(recv_name) {
                    if let Some(f) = info
                        .statics
                        .iter()
                        .find(|f| f.name.name == method.name)
                        .copied()
                    {
                        let mut call =
                            format!("{recv_name}::{}(", camel_to_snake(&method.name));
                        for (i, a) in args.iter().enumerate() {
                            if i > 0 {
                                call.push_str(", ");
                            }
                            call.push_str(&lower_method_expr(a, cx)?);
                        }
                        call.push(')');
                        let _ = f;
                        return Ok(call);
                    }
                }
                if let Some(info) = cx.local_class(recv_name) {
                    let Some(decl) = info
                        .methods
                        .iter()
                        .find(|m2| m2.name.name == method.name)
                        .copied()
                    else {
                        return err(
                            e.span,
                            format!("no method `{}` on `{}`", method.name, info.name),
                        );
                    };
                    let head = format!(
                        "{}.{}(w",
                        camel_to_snake(recv_name),
                        camel_to_snake(&method.name)
                    );
                    let lowered = lower_w_call_args(Some(decl.params.as_slice()), args, cx)?;
                    return Ok(finish_w_call(&head, lowered));
                }
                if let Some(trait_name) = cx.generic_locals.get(recv_name) {
                    // Trait-bounded generic param: dispatch through
                    // the real Rust trait, w-threaded like any class
                    // method (§8.20). The trait's declared params
                    // drive arg coercion.
                    let decl = cx
                        .traits
                        .get(trait_name.as_str())
                        .and_then(|t| t.methods.iter().find(|m| m.name.name == method.name));
                    let head = format!(
                        "{}.{}(w",
                        camel_to_snake(recv_name),
                        camel_to_snake(&method.name)
                    );
                    let lowered = lower_w_call_args(decl.map(|f| f.params.as_slice()), args, cx)?;
                    return Ok(finish_w_call(&head, lowered));
                }
                if cx.is_local(recv_name) {
                    // Checker-validated method on a value local (struct
                    // methods; no World threading — values are pure).
                    let mut call = format!(
                        "{}.{}(",
                        camel_to_snake(recv_name),
                        camel_to_snake(&method.name)
                    );
                    for (i, a) in args.iter().enumerate() {
                        if i > 0 {
                            call.push_str(", ");
                        }
                        call.push_str(&lower_method_expr(a, cx)?);
                    }
                    call.push(')');
                    return Ok(call);
                }
            }
            if matches!(receiver.kind, ExprKind::SelfRef) {
                if cx.self_struct.is_some() {
                    let mut call = format!("self.{}(", camel_to_snake(&method.name));
                    for (i, a) in args.iter().enumerate() {
                        if i > 0 {
                            call.push_str(", ");
                        }
                        call.push_str(&lower_method_expr(a, cx)?);
                    }
                    call.push(')');
                    return Ok(call);
                }
                if cx.class.methods.iter().any(|m| m.name.name == method.name) {
                    let decl = cx
                        .class
                        .methods
                        .iter()
                        .find(|m| m.name.name == method.name)
                        .copied();
                    let head = format!("self.{}(w", camel_to_snake(&method.name));
                    let lowered = lower_w_call_args(decl.map(|f| f.params.as_slice()), args, cx)?;
                    return Ok(finish_w_call(&head, lowered));
                }
                if let Some(call) = lower_builtin_value_call(receiver, method, args, cx)? {
                    return Ok(call);
                }
                return err(e.span, format!("no lowerable method `{}` (M0)", method.name));
            }
            if let Some(call) = lower_builtin_value_call(receiver, method, args, cx)? {
                return Ok(call);
            }
            // A struct method through a PROPERTY read (`@a.opAdd(b)` —
            // a struct-typed store field): the checker validated the
            // method; values are pure, so the call threads no World.
            // The same shape on a class-typed prop stays refused (a
            // class method needs `&mut World`).
            if struct_recv_of(receiver, cx).is_some() {
                let recv = lower_method_expr(receiver, cx)?;
                let mut call = format!("({recv}).{}(", camel_to_snake(&method.name));
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        call.push_str(", ");
                    }
                    call.push_str(&lower_method_expr(a, cx)?);
                }
                call.push(')');
                return Ok(call);
            }
            err(e.span, "this method call is not lowerable yet (M0)")
        }
        ExprKind::Call {
            callee,
            args,
            block: None,
            type_args,
            ..
        } => {
            if let ExprKind::Path(p) = &callee.kind {
                if p.len() == 2 {
                    if let Some(bc) = cx.bindings.get(&p[0].name) {
                        let Some(bf) = bc.statics.get(&p[1].name) else {
                            return err(
                                e.span,
                                format!("no binding fn `{}` on `{}`", p[1].name, p[0].name),
                            );
                        };
                        return lower_binding_call(bf, args, cx, e.span);
                    }
                }
            }
            if let ExprKind::Ident(fname) = &callee.kind {
                if let Some(info) = cx.classes.get(fname.as_str()) {
                    // §8.25: `Class(args)` runs the user `init`;
                    // generic classes take explicit type args
                    // (`Stack<Int>()` → `Stack::<i64>::new()`).
                    let (_, rust_args) = instantiation_of(info, type_args, cx.class_names, e.span)?;
                    let turbofish = if rust_args.is_empty() {
                        String::new()
                    } else {
                        format!("::{rust_args}")
                    };
                    match info.init {
                        Some(init) => {
                            if args.len() != init.params.len() {
                                return err(
                                    e.span,
                                    format!(
                                        "`{fname}` takes {} constructor argument(s)",
                                        init.params.len()
                                    ),
                                );
                            }
                            let lowered =
                                lower_w_call_args(Some(init.params.as_slice()), args, cx)?;
                            let head = format!("w.insert({fname}{turbofish}::new(");
                            if lowered.is_empty() {
                                return Ok(format!("{head}))"));
                            }
                            let mut outs = String::from("{ ");
                            let mut names = Vec::new();
                            for (i, v) in lowered.iter().enumerate() {
                                write!(outs, "let __a{i} = {v}; ").unwrap();
                                names.push(format!("__a{i}"));
                            }
                            write!(outs, "{head}{})) }}", names.join(", ")).unwrap();
                            return Ok(outs);
                        }
                        None => {
                            if !args.is_empty() {
                                return err(
                                    e.span,
                                    format!("`{fname}` has no `init` — construct with `{fname}()`"),
                                );
                            }
                            return Ok(format!("w.insert({fname}{turbofish}::new())"));
                        }
                    }
                }
                if let Some(st) = cx.structs.get(fname) {
                    // A trailing run of defaulted fields may be left
                    // out (§8.68) — the fields are positional, so
                    // "omitted" can only mean "from the end".
                    let required = st
                        .defaults
                        .iter()
                        .rposition(|d| d.is_none())
                        .map(|i| i + 1)
                        .unwrap_or(0);
                    if args.len() > st.fields.len() || args.len() < required {
                        return err(
                            e.span,
                            if required == st.fields.len() {
                                format!("`{}` takes {} field value(s)", st.name, st.fields.len())
                            } else {
                                format!(
                                    "`{}` takes {} to {} field value(s) — the last {} have \
                                     defaults",
                                    st.name,
                                    required,
                                    st.fields.len(),
                                    st.fields.len() - required
                                )
                            },
                        );
                    }
                    let mut lit = format!("{} {{ ", st.name);
                    for (i, (_, rust, ty)) in st.fields.iter().enumerate() {
                        if i > 0 {
                            lit.push_str(", ");
                        }
                        let v = match args.get(i) {
                            Some(a) => lower_assign_value(a, ty, cx)?,
                            None => lower_default(
                                st.defaults[i].as_ref().expect("required checked above"),
                                ty,
                            )?,
                        };
                        write!(lit, "{rust}: {v}").unwrap();
                    }
                    lit.push_str(" }");
                    return Ok(lit);
                }
                if let Some(rust_name) = cx.free_fns.get(fname) {
                    // Declared param types drive `T?` arg coercion —
                    // the same rule as returns.
                    let decl = cx
                        .free_decls
                        .iter()
                        .find(|f| f.name.name == *fname)
                        .copied();
                    let head = format!("{rust_name}(w");
                    let lowered = lower_w_call_args(decl.map(|f| f.params.as_slice()), args, cx)?;
                    return Ok(finish_w_call(&head, lowered));
                }
            }
            err(e.span, "this call is not lowerable yet (M0)")
        }
        ExprKind::Binary { op, lhs, rhs } => {
            let l = lower_method_expr(lhs, cx)?;
            let r = lower_method_expr(rhs, cx)?;
            Ok(format!("({l} {} {r})", bin_op(op, e.span)?))
        }
        ExprKind::Unary { op, expr } => {
            let inner = lower_method_expr(expr, cx)?;
            match op {
                UnaryOp::Neg => Ok(format!("(-{inner})")),
                UnaryOp::Not => Ok(format!("(!{inner})")),
            }
        }
        ExprKind::Await(_) => err(
            e.span,
            "`await` is not lowerable here: in `async fn` bodies it must be the \
             entire right-hand side of a `let` / assignment / statement / `case` \
             scrutinee, and sync fns cannot await",
        ),
        // Map literals mirror the List story: `{}` from context,
        // entries as an insert-chain block. Identifier keys are
        // string sugar (`{ a: 1 }` ≡ `{ "a": 1 }` — the checker's
        // rule, mirrored).
        ExprKind::Map(entries) => {
            if entries.is_empty() {
                return Ok("Map::new()".into());
            }
            let mut out = String::from("{ let mut __lit = Map::new(); ");
            for (k, v) in entries {
                let kv = match &k.kind {
                    ExprKind::Ident(name) => format!("Str::from({:?})", name),
                    _ => lower_method_expr(k, cx)?,
                };
                let vv = lower_method_expr(v, cx)?;
                write!(out, "__lit.insert({kv}, {vv}); ").unwrap();
            }
            out.push_str("__lit }");
            Ok(out)
        }
        // List literals: the element type comes from the context rustc
        // sees (a typed prop assignment, an annotated let, an argument)
        // — the checker's literal inference validated it upstream.
        ExprKind::Array(items) => {
            if items.is_empty() {
                return Ok("List::new()".into());
            }
            let mut out = String::from("{ let mut __lit = List::new(); ");
            for item in items {
                let v = lower_method_expr(item, cx)?;
                write!(out, "__lit.push({v}); ").unwrap();
            }
            out.push_str("__lit }");
            Ok(out)
        }
        // `xs[i]` — the trapping index (§11.25). The receiver goes
        // through the list path, so a prop, a global's prop, a local
        // and a field path all subscript the same way.
        ExprKind::Index { receiver, index } => {
            let xs = lower_list_value(receiver, cx)?;
            let i = lower_method_expr(index, cx)?;
            Ok(format!("({xs}).at({i})"))
        }
        ExprKind::Call { block: Some(_), .. } => Err(block_call_error(e)),
        _ => err(e.span, "this expression is not lowerable yet (M0)"),
    }
}

/// The query methods the built-in VALUE types answer. This is a
/// language surface, not an emitter convenience: `List` and `Map` are
/// types the language ships, so what they respond to is part of the
/// spec and belongs in one table rather than in whatever the emitter
/// happened to special-case.
///
/// Only the pure ones live here. The mutators (`push` / `insert` /
/// `remove`) are statements with COW write-back and have their own
/// lowering. Dispatch is by RECEIVER type, which is rustc's job: the
/// emitted call is the same shape whether the receiver turns out to
/// be a `List`, a `Map`, or a user struct that happens to declare a
/// method by the same name.
fn builtin_value_method_arity(name: &str) -> Option<usize> {
    Some(match name {
        "get" => 1,
        "getOr" => 2,
        "contains" => 1,
        "first" => 0,
        "keys" => 0,
        "values" => 0,
        _ => return None,
    })
}

fn lower_builtin_value_call(
    receiver: &Expr,
    method: &Ident,
    args: &[Expr],
    cx: &MethodCtx,
) -> Result<Option<String>, EmitError> {
    let Some(want) = builtin_value_method_arity(&method.name) else {
        return Ok(None);
    };
    if args.len() != want {
        return err(
            method.span,
            format!("`{}` takes {want} argument(s)", method.name),
        );
    }
    let recv = lower_method_expr(receiver, cx)?;
    let mut call = format!("({recv}).{}(", camel_to_snake(&method.name));
    for (i, a) in args.iter().enumerate() {
        if i > 0 {
            call.push_str(", ");
        }
        call.push_str(&lower_method_expr(a, cx)?);
    }
    call.push(')');
    Ok(Some(call))
}

/// Lower an expression that must denote a List value (for `.length`,
/// `for` iteration, `.push` receivers).
fn lower_list_value(e: &Expr, cx: &MethodCtx) -> Result<String, EmitError> {
    match &e.kind {
        ExprKind::Ident(n) => {
            if cx.is_local(n) {
                Ok(camel_to_snake(n))
            } else if let Some(p) = cx.class.prop(n) {
                Ok(format!("self.{}(w)", p.rust))
            } else {
                err(e.span, format!("`{n}` is not lowerable here yet (M0)"))
            }
        }
        ExprKind::AtIdent(n) => match cx.class.prop(n) {
            Some(p) => Ok(format!("self.{}(w)", p.rust)),
            None => err(e.span, format!("`{n}` is not a lowerable member yet (M0)")),
        },
        ExprKind::Member { receiver, name } => {
            // A global's list property reads through the singleton.
            if let ExprKind::Ident(r) = &receiver.kind {
                if let Some((info, handle)) = cx.global(r) {
                    let Some(p) = info.prop(&name.name) else {
                        return err(
                            e.span,
                            format!("no property `{}` on `{}`", name.name, info.name),
                        );
                    };
                    return Ok(format!("{handle}.{}(w)", p.rust));
                }
            }
            // Anything else is an ordinary value expression — a struct
            // field holding a list, a field of a field, a method
            // result. The general lowerer already reads those, and
            // requiring a local first (§11.25's `(M0)` limit) only
            // made authors write the binding the emitter could have.
            lower_method_expr(e, cx)
        }
        // Same rule for the rest: if the general lowerer can produce
        // the value, it can be a list. The checker decided it IS one.
        _ => lower_method_expr(e, cx),
    }
}

/// Wrap a value produced in `!T` position: values built from the
/// module's error enum go to `Err(...)`, everything else to `Ok(...)`.

/// Validate a fn's generic params (bounds must name declared traits)
/// and render the Rust generic list — every param gets `+ Clone`
/// (pixie values all clone; handles are Copy through the trait's
/// supertrait). Empty string when the fn is not generic.
fn render_fn_generics(f: &ast::FnDecl, p: &Program) -> Result<String, EmitError> {
    if f.generics.is_empty() {
        return Ok(String::new());
    }
    for g in &f.generics {
        for b in &g.bounds {
            if !p.traits.contains_key(&b.name) {
                return err(
                    b.span,
                    format!("`{}` is not a declared trait (generic bounds name traits)", b.name),
                );
            }
        }
    }
    let params: Vec<String> = f
        .generics
        .iter()
        .map(|g| {
            let mut bounds: Vec<String> = g.bounds.iter().map(|b| b.name.clone()).collect();
            bounds.push("Clone".to_string());
            format!("{}: {}", g.name.name, bounds.join(" + "))
        })
        .collect();
    Ok(format!("<{}>", params.join(", ")))
}

/// Register the fn's trait-bound generic params so method calls on
/// them dispatch w-threaded through the real Rust trait.
fn register_generic_locals(f: &ast::FnDecl, cx: &mut MethodCtx) {
    for param in &f.params {
        if let TypeKind::Named { path, args } = &param.ty.kind {
            if args.is_empty() && path.len() == 1 {
                if let Some(g) = f.generics.iter().find(|g| g.name.name == path[0].name) {
                    if let Some(first_bound) = g.bounds.first() {
                        cx.generic_locals
                            .insert(param.name.name.clone(), first_bound.name.clone());
                    }
                }
            }
        }
    }
}

/// Lower the arguments of a w-threaded call, coercing any argument
/// whose declared param is `T?` (nil / passthrough / Some — the
/// return rule at the param boundary). Returns the lowered argument
/// expressions for `finish_w_call` to hoist.
fn lower_w_call_args(
    params: Option<&[ast::Param]>,
    args: &[Expr],
    cx: &MethodCtx,
) -> Result<Vec<String>, EmitError> {
    let mut out = Vec::with_capacity(args.len());
    for (i, a) in args.iter().enumerate() {
        let param_opt = params
            .and_then(|ps| ps.get(i))
            .is_some_and(|q| matches!(q.ty.kind, TypeKind::Nullable(_)));
        out.push(if param_opt {
            lower_nullable_slot(a, cx)?
        } else {
            lower_method_expr(a, cx)?
        });
    }
    Ok(out)
}

/// Assemble a w-threaded call with its arguments HOISTED into locals
/// (§11.20): an argument that itself threads `w` (`f(w, g(w))`)
/// would nest two `&mut World` borrows; evaluating every argument
/// before the call keeps the borrows sequential. `head` is the call
/// up to and including `(w` — e.g. `"describe_tag(w"` or
/// `"self.set_x(w"`.
fn finish_w_call(head: &str, lowered: Vec<String>) -> String {
    if lowered.is_empty() {
        return format!("{head})");
    }
    let mut out = String::from("{ ");
    let mut names = Vec::with_capacity(lowered.len());
    for (i, v) in lowered.iter().enumerate() {
        write!(out, "let __a{i} = {v}; ").unwrap();
        names.push(format!("__a{i}"));
    }
    write!(out, "{head}, {}) }}", names.join(", ")).unwrap();
    out
}

/// Is `e` an expression that already carries an `Option` (a `T?`
/// value)? The coercion sites (`T?` returns and `T?` fn params) pass
/// these through instead of wrapping in `Some`. Total by
/// construction: a `T?` can only enter a body as a `T?` param, a
/// direct call to a `T?`-returning fn/binding, or `nil` — `let`
/// locals of `T?` are gated (M2).
fn expr_is_opt(e: &Expr, cx: &MethodCtx) -> bool {
    let fn_ret_is_opt =
        |f: &ast::FnDecl| matches!(&f.return_ty, Some(t) if matches!(t.kind, TypeKind::Nullable(_)));
    match &e.kind {
        ExprKind::Nil => true,
        ExprKind::Ident(n) => cx.is_opt_local(n),
        // Free fn call: `helper(...)`.
        ExprKind::Call { callee, .. } => match &callee.kind {
            ExprKind::Ident(fname) => cx
                .free_decls
                .iter()
                .find(|f| f.name.name == *fname)
                .is_some_and(|f| fn_ret_is_opt(f)),
            // `Binding.f(...)` in path shape.
            ExprKind::Path(p) if p.len() == 2 => cx
                .bindings
                .get(&p[0].name)
                .and_then(|b| b.statics.get(&p[1].name))
                .is_some_and(|bf| matches!(bf.ret, RustTy::Opt(_)) && !bf.fallible),
            _ => false,
        },
        // `Receiver.f(...)`: a binding static, a method on a
        // class-typed local / global singleton, or `self.f(...)`.
        ExprKind::MethodCall {
            receiver, method, ..
        } => {
            if matches!(receiver.kind, ExprKind::SelfRef) {
                return cx
                    .class
                    .methods
                    .iter()
                    .find(|m| m.name.name == method.name)
                    .is_some_and(|m| fn_ret_is_opt(m));
            }
            if let ExprKind::Ident(r) = &receiver.kind {
                if let Some(b) = cx.bindings.get(r) {
                    return b
                        .statics
                        .get(&method.name)
                        .is_some_and(|bf| matches!(bf.ret, RustTy::Opt(_)) && !bf.fallible);
                }
                if let Some(trait_name) = cx.generic_locals.get(r) {
                    return cx
                        .traits
                        .get(trait_name.as_str())
                        .and_then(|t| t.methods.iter().find(|m| m.name.name == method.name))
                        .is_some_and(fn_ret_is_opt);
                }
                let class = cx
                    .local_class(r)
                    .or_else(|| cx.globals.get(r).and_then(|c| cx.classes.get(c)));
                if let Some(ci) = class {
                    return ci
                        .methods
                        .iter()
                        .find(|m| m.name.name == method.name)
                        .is_some_and(|m| fn_ret_is_opt(m));
                }
            }
            false
        }
        _ => false,
    }
}

/// Lower an expression into a `T?` slot: `nil` becomes `None` (before
/// the general lowering, which has no meaning for it), values already
/// `T?` pass through, plain values wrap in `Some`.
/// Lower an assignment's right-hand side against the SLOT's type. A
/// `T?` property takes `nil` and takes a bare value, wrapping the
/// second — the same automatic `some` a `T?` return gets (§8.68).
fn lower_assign_value(
    value: &Expr,
    ty: &RustTy,
    cx: &MethodCtx,
) -> Result<String, EmitError> {
    match ty {
        RustTy::Opt(_) => lower_nullable_slot(value, cx),
        _ => lower_method_expr(value, cx),
    }
}

fn lower_nullable_slot(src: &Expr, cx: &MethodCtx) -> Result<String, EmitError> {
    if matches!(src.kind, ExprKind::Nil) {
        return Ok("None".into());
    }
    let v = lower_method_expr(src, cx)?;
    Ok(if expr_is_opt(src, cx) {
        v
    } else {
        format!("Some({v})")
    })
}

fn fallible_wrap(src: &Expr, v: String, cx: &MethodCtx) -> String {
    let is_err_val = match &src.kind {
        ExprKind::Member { receiver, .. } | ExprKind::MethodCall { receiver, .. } => {
            matches!(&receiver.kind, ExprKind::Ident(r) if Some(r.as_str()) == cx.default_error)
        }
        _ => false,
    };
    if is_err_val {
        format!("Err({v})")
    } else {
        format!("Ok({v})")
    }
}

/// Does `e` call straight into a binding (whose errors are `Str`)?
fn is_binding_call(e: &Expr, cx: &MethodCtx) -> bool {
    match &e.kind {
        ExprKind::MethodCall { receiver, .. } => {
            matches!(&receiver.kind, ExprKind::Ident(r) if cx.bindings.contains_key(r))
        }
        ExprKind::Call { callee, .. } => {
            matches!(&callee.kind, ExprKind::Path(p) if p.len() == 2 && cx.bindings.contains_key(&p[0].name))
        }
        _ => false,
    }
}

fn bin_op(op: &BinOp, span: Span) -> Result<&'static str, EmitError> {
    Ok(match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Mod => "%",
        BinOp::Lt => "<",
        BinOp::LtEq => "<=",
        BinOp::Gt => ">",
        BinOp::GtEq => ">=",
        BinOp::Eq => "==",
        BinOp::NotEq => "!=",
        BinOp::And => "&&",
        BinOp::Or => "||",
        _ => return err(span, "this operator is not lowerable yet (M0)"),
    })
}

/// Translate a pixie format spec to a Rust one, or `None` when it is
/// not a spec at all. The two grammars agree except that pixie
/// accepts a printf-style trailing type letter (`.2f`, `04d`), which
/// Rust infers — so the letter is dropped after checking it names a
/// type the value could plausibly be.
///
/// Deliberately a whitelist: anything unrecognised is a named pixie
/// error, never a `format!` that fails inside generated code.
fn rust_format_spec(spec: &str) -> Option<String> {
    let core = match spec.chars().last() {
        // printf's type letters. `?` is Rust's own debug spec and is
        // kept, not stripped.
        Some(c @ ('f' | 'd' | 'e' | 'x' | 'X' | 'o' | 'b' | 's')) => {
            let head = &spec[..spec.len() - c.len_utf8()];
            // `x`/`X`/`o`/`b` ARE Rust specs on their own; keep them
            // when nothing precedes, strip them when they trail a
            // width or precision that Rust would reject alongside.
            if matches!(c, 'x' | 'X' | 'o' | 'b') {
                return check_rust_spec(spec);
            }
            head.to_string()
        }
        _ => spec.to_string(),
    };
    check_rust_spec(&core)
}

/// The subset of Rust's spec grammar pixie admits: an optional
/// fill+align, an optional zero-pad, an optional width, an optional
/// `.precision`, and an optional `?`.
fn check_rust_spec(spec: &str) -> Option<String> {
    let b: Vec<char> = spec.chars().collect();
    let mut i = 0;
    // fill + align, or align alone
    if b.len() >= 2 && matches!(b[1], '<' | '^' | '>') {
        i = 2;
    } else if !b.is_empty() && matches!(b[0], '<' | '^' | '>') {
        i = 1;
    }
    if i < b.len() && b[i] == '0' {
        i += 1;
    }
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i < b.len() && b[i] == '.' {
        i += 1;
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            return None;
        }
    }
    if i < b.len() && matches!(b[i], '?' | 'x' | 'X' | 'o' | 'b' | 'e' | 'E') {
        i += 1;
    }
    (i == b.len()).then(|| spec.to_string())
}

fn lower_interp(
    parts: &[StrPart],
    lower: &mut dyn FnMut(&Expr) -> Result<String, EmitError>,
) -> Result<String, EmitError> {
    let mut fmt = String::new();
    let mut args: Vec<String> = Vec::new();
    for p in parts {
        match p {
            StrPart::Text(t) => escape_fmt_text(t, &mut fmt),
            StrPart::Interp(e) => {
                fmt.push_str("{}");
                args.push(lower(e)?);
            }
            // `#{value:.2f}` — a width, an alignment, a precision
            // (§8.54). pixie's spec grammar is Rust's minus the type
            // suffix, which pixie writes as a trailing letter the way
            // printf does, so `.2f` becomes `.2` and the rest passes
            // through. It is CHECKED here rather than handed to
            // rustc: a bad spec has to be a pixie error naming the
            // spec, not a compile failure inside generated code (D10).
            StrPart::InterpFmt { expr, format_spec } => {
                let rust_spec = rust_format_spec(format_spec).ok_or_else(|| EmitError {
                    span: expr.span,
                    message: format!(
                        "`{format_spec}` is not a format spec — write a width (`8`), an \
                         alignment (`>8`), a precision (`.2`), a filler (`0>4`), or a \
                         float precision (`.2f`)"
                    ),
                })?;
                if format_spec.ends_with('f') {
                    // The trailing letter certifies Float (checked
                    // upstream; the binding is untyped so a by-ref
                    // loop variable auto-derefs), and NaN is spelled
                    // the way the interp tier and Python spell it.
                    fmt.push_str("{}");
                    args.push(format!(
                        "{{ let __f = {}; if __f.is_nan() {{ \"nan\".to_string() }} else {{ format!(\"{{:{rust_spec}}}\", __f) }} }}",
                        lower(expr)?
                    ));
                } else {
                    fmt.push_str(&format!("{{:{rust_spec}}}"));
                    args.push(lower(expr)?);
                }
            }
        }
    }
    if args.is_empty() {
        // No interpolations: a PLAIN string. The format!-only brace
        // doubling must not leak into it — `"{"` stays one brace
        // (found by castel's gate: a JSON literal arrived as `{{`).
        let mut plain = String::new();
        for p in parts {
            if let StrPart::Text(t) = p {
                escape_plain_text(t, &mut plain);
            }
        }
        Ok(format!("Str::from(\"{plain}\")"))
    } else {
        Ok(format!("Str::from(format!(\"{fmt}\", {}))", args.join(", ")))
    }
}

/// Lower a statement list as one SCOPE: every statement, then the
/// trailing expression, then a `World::remove` for each object the
/// scope created and provably never let out (§8.42).
///
/// Every block-shaped construct routes through here, so the reclaim
/// point is the same everywhere and a loop body reclaims once per
/// ITERATION — which is the whole point, since the measured leak was
/// a loop allocating objects it then dropped on the floor.
fn lower_scope(
    stmts: &[Stmt],
    trailing: Option<&Expr>,
    // Emit the trailing expression as a statement. A block does; a
    // FUNCTION body does not — there the trailing is the return
    // value, which the caller wraps and writes after the reclaims.
    // Either way it is read for the escape check, so an object handed
    // back is never reclaimed.
    emit_trailing: bool,
    cx: &mut MethodCtx,
    out: &mut String,
    ind: &str,
) -> Result<(), EmitError> {
    cx.reclaim.push(Vec::new());
    let run = |cx: &mut MethodCtx, out: &mut String| -> Result<(), EmitError> {
        for (i, s) in stmts.iter().enumerate() {
            // Before lowering a `let x = Class(..)`, ask whether `x`
            // gets out of this scope. Only the statements AFTER it can
            // let it out, plus the trailing expression.
            if let Stmt::Let { name, value, .. } = s {
                if is_construction(value, cx)
                    && !escape::escapes(&name.name, &stmts[i + 1..], trailing)
                {
                    cx.reclaim
                        .last_mut()
                        .expect("scope pushed")
                        .push(camel_to_snake(&name.name));
                }
            }
            lower_method_stmt(s, cx, out, ind)?;
        }
        if emit_trailing {
            if let Some(t) = trailing {
                let stmt = Stmt::Expr(t.clone());
                lower_method_stmt(&stmt, cx, out, ind)?;
            }
        }
        Ok(())
    };
    let r = run(cx, out);
    let mine = cx.reclaim.pop().expect("scope pushed");
    r?;
    // Reverse creation order, so a scope unwinds the way it built.
    for name in mine.iter().rev() {
        writeln!(out, "{ind}let _ = w.remove({name});").unwrap();
    }
    Ok(())
}

/// Is this initializer a direct `Class(..)` construction? Only those
/// are candidates — anything else is a value, or an object that came
/// from somewhere this scope does not own.
fn is_construction(e: &Expr, cx: &MethodCtx) -> bool {
    matches!(&e.kind, ExprKind::Call { callee, .. }
        if matches!(&callee.kind, ExprKind::Ident(c) if cx.classes.contains_key(c)))
}

fn lower_method_stmt(s: &Stmt, cx: &mut MethodCtx, out: &mut String, ind: &str) -> Result<(), EmitError> {
    match s {
        Stmt::Let {
            name, ty, value, ..
        }
        | Stmt::Var {
            name, ty, value, ..
        } => {
            let is_var = matches!(s, Stmt::Var { .. });
            // `T?` locals: an annotated `x : T?` coerces its
            // initializer (`nil` / plain / already-`T?`); without an
            // annotation the local is `T?` exactly when the RHS is.
            let ann_opt = matches!(ty, Some(t) if matches!(t.kind, TypeKind::Nullable(_)));
            if !ann_opt && matches!(value.kind, ExprKind::Nil) {
                return err(
                    name.span,
                    "`nil` needs a `T?` annotation here (`let x : String? = nil`)",
                );
            }
            let is_opt = ann_opt || expr_is_opt(value, cx);
            let v = if ann_opt {
                lower_nullable_slot(value, cx)?
            } else {
                lower_method_expr(value, cx)?
            };
            let ann = match ty {
                Some(t) => format!(": {}", lower_type(t, cx.class_names)?.render()),
                None => String::new(),
            };
            let rn = camel_to_snake(&name.name);
            let mutkw = if is_var { "mut " } else { "" };
            writeln!(out, "{ind}let {mutkw}{rn}{ann} = {v};").unwrap();
            // A local bound to an OBJECT is Handle-typed, so member
            // and method access on it dispatches through the World.
            // An annotation wins; otherwise the initializer decides —
            // `Class()`, a class-returning call, a class-typed prop.
            let handle_class = ty
                .as_ref()
                .and_then(|t| named_class(t, cx.class_names))
                .or_else(|| handle_class_of(value, cx));
            // The local's declared type, when the emitter can say it:
            // an annotation, or a struct/class construction on the
            // right (§8.68). Only used to answer "is this a struct
            // field read" and "does this print as an optional".
            let local_ty = match ty {
                Some(t) => lower_type(t, cx.class_names).ok(),
                None => match &value.kind {
                    ExprKind::Call { callee, .. } => match &callee.kind {
                        ExprKind::Ident(c) if cx.structs.contains_key(c) => {
                            Some(RustTy::Named(c.clone()))
                        }
                        _ => None,
                    },
                    _ => declared_ty_of(value, cx),
                },
            };
            cx.locals.push((name.name.clone(), handle_class, is_opt, local_ty));
            Ok(())
        }
        Stmt::Assign {
            target, op, value, span,
        } => {
            // `Global.prop = v` (and compound forms) through the singleton.
            if let ExprKind::Member { receiver, name } = &target.kind {
                if let ExprKind::Ident(r) = &receiver.kind {
                    if let Some((info, handle)) = cx.global(r) {
                        let Some(p) = info.prop(&name.name) else {
                            return err(
                                *span,
                                format!("no property `{}` on `{}`", name.name, info.name),
                            );
                        };
                        check_assignable(p, &info.name, *span)?;
                        let v = lower_assign_value(value, &p.ty, cx)?;
                        match op {
                            AssignOp::Eq => {
                                writeln!(out, "{ind}{{ let __v = {v}; {handle}.set_{}(w, __v); }}", p.rust).unwrap();
                            }
                            _ => {
                                let sym = match op {
                                    AssignOp::PlusEq => "+",
                                    AssignOp::MinusEq => "-",
                                    AssignOp::StarEq => "*",
                                    AssignOp::SlashEq => "/",
                                    AssignOp::Eq => unreachable!(),
                                };
                                writeln!(
                                    out,
                                    "{ind}{{ let __v = {handle}.{g}(w) {sym} {v}; {handle}.set_{g}(w, __v); }}",
                                    g = p.rust
                                )
                                .unwrap();
                            }
                        }
                        return Ok(());
                    }
                    if let Some(info) = cx.local_class(r) {
                        let Some(p) = info.prop(&name.name) else {
                            return err(
                                *span,
                                format!("no property `{}` on `{}`", name.name, info.name),
                            );
                        };
                        check_assignable(p, &info.name, *span)?;
                        let v = lower_assign_value(value, &p.ty, cx)?;
                        let h = camel_to_snake(r);
                        match op {
                            AssignOp::Eq => {
                                writeln!(out, "{ind}{{ let __v = {v}; {h}.set_{}(w, __v); }}", p.rust).unwrap();
                            }
                            _ => {
                                let sym = match op {
                                    AssignOp::PlusEq => "+",
                                    AssignOp::MinusEq => "-",
                                    AssignOp::StarEq => "*",
                                    AssignOp::SlashEq => "/",
                                    AssignOp::Eq => unreachable!(),
                                };
                                writeln!(
                                    out,
                                    "{ind}{{ let __v = {h}.{g}(w) {sym} {v}; {h}.set_{g}(w, __v); }}",
                                    g = p.rust
                                )
                                .unwrap();
                            }
                        }
                        return Ok(());
                    }
                }
                // Writing THROUGH an object chain (`a.kid.v = 7`). A
                // reference you cannot assign through is not much of a
                // reference, so any receiver that denotes an object
                // takes the same setter stereotype as a named one.
                if let Some(c) = handle_class_of(receiver, cx) {
                    if let Some(info) = cx.classes.get(&c) {
                        let Some(pi) = info.prop(&name.name) else {
                            return err(
                                *span,
                                format!("no property `{}` on `{}`", name.name, info.name),
                            );
                        };
                        check_assignable(pi, &info.name, *span)?;
                        // The receiver is evaluated ONCE into a local:
                        // a chain can read through several handles, and
                        // re-walking it for a compound assignment would
                        // read the World twice for one write.
                        let h = lower_method_expr(receiver, cx)?;
                        let v = lower_assign_value(value, &pi.ty, cx)?;
                        match op {
                            AssignOp::Eq => {
                                writeln!(
                                    out,
                                    "{ind}{{ let __o = {h}; let __v = {v}; __o.set_{}(w, __v); }}",
                                    pi.rust
                                )
                                .unwrap();
                            }
                            _ => {
                                let sym = match op {
                                    AssignOp::PlusEq => "+",
                                    AssignOp::MinusEq => "-",
                                    AssignOp::StarEq => "*",
                                    AssignOp::SlashEq => "/",
                                    AssignOp::Eq => unreachable!(),
                                };
                                writeln!(
                                    out,
                                    "{ind}{{ let __o = {h}; let __v = __o.{g}(w) {sym} {v}; __o.set_{g}(w, __v); }}",
                                    g = pi.rust
                                )
                                .unwrap();
                            }
                        }
                        return Ok(());
                    }
                }
                return err(*span, "only plain-name assignment is lowerable yet (M0)");
            }
            // `xs[i] = v` (§8.67). `xs[i]` traps on a read (§8.38) and
            // it traps on a write for the same reason: an index the
            // author wrote is a claim about the list. The list is
            // taken OUT first, so the COW clone-on-write finds a
            // single owner and the assignment lands in place — the
            // same shape `push_<prop>` uses.
            if let ExprKind::Index { receiver, index } = &target.kind {
                if !matches!(op, AssignOp::Eq) {
                    return err(
                        *span,
                        "a compound assignment through an index is not lowerable yet (M0) \
                         — read the element into a local, change it, and assign it back",
                    );
                }
                let i = lower_method_expr(index, cx)?;
                let v = lower_method_expr(value, cx)?;
                if let Some((holder, pi)) = list_push_target(receiver, cx) {
                    writeln!(
                        out,
                        "{ind}{{ let __h = {holder}; let mut __xs = __h.{g}(w); __xs.set({i}, {v}); \
                         __h.set_{g}(w, __xs); }}",
                        g = pi.rust
                    )
                    .unwrap();
                    return Ok(());
                }
                if let Some((holder, pi)) = map_insert_target(receiver, cx) {
                    if let RustTy::Map(_, vt) = &pi.ty {
                        if vt.holds_objects() {
                            return err(
                                *span,
                                "inserting into an object-valued map is not lowerable yet (M0)",
                            );
                        }
                    }
                    writeln!(
                        out,
                        "{ind}{{ let __h = {holder}; let __k = {i}; let __v = {v}; \
                         __h.insert_{}(w, __k, __v); }}",
                        pi.rust
                    )
                    .unwrap();
                    return Ok(());
                }
                return err(
                    *span,
                    "`[..] =` writes into a list or map PROPERTY — name the object and \
                     the container it holds",
                );
            }
            let n = match &target.kind {
                ExprKind::Ident(n) | ExprKind::AtIdent(n) => n,
                _ => return err(*span, "only plain-name assignment is lowerable yet (M0)"),
            };
            // The slot decides how the value is wrapped, so the prop
            // has to be resolved first (§8.68).
            let slot_ty = if cx.is_local(n) {
                None
            } else {
                cx.class.prop(n).map(|q| q.ty.clone())
            };
            let v = match &slot_ty {
                Some(t) => lower_assign_value(value, t, cx)?,
                None => lower_method_expr(value, cx)?,
            };
            if cx.is_local(n) {
                let rn = camel_to_snake(n);
                let sym = match op {
                    AssignOp::Eq => "=",
                    AssignOp::PlusEq => "+=",
                    AssignOp::MinusEq => "-=",
                    AssignOp::StarEq => "*=",
                    AssignOp::SlashEq => "/=",
                };
                writeln!(out, "{ind}{rn} {sym} {v};").unwrap();
                return Ok(());
            }
            let Some(p) = cx.class.prop(n) else {
                return err(*span, format!("`{n}` is not assignable here yet (M0)"));
            };
            check_assignable(p, &cx.class.name, *span)?;
            match op {
                AssignOp::Eq => {
                    writeln!(out, "{ind}{{ let __v = {v}; self.set_{}(w, __v); }}", p.rust).unwrap();
                }
                _ => {
                    let sym = match op {
                        AssignOp::PlusEq => "+",
                        AssignOp::MinusEq => "-",
                        AssignOp::StarEq => "*",
                        AssignOp::SlashEq => "/",
                        AssignOp::Eq => unreachable!(),
                    };
                    writeln!(
                        out,
                        "{ind}{{ let __v = self.{g}(w) {sym} {v}; self.set_{g}(w, __v); }}",
                        g = p.rust
                    )
                    .unwrap();
                }
            }
            Ok(())
        }
        Stmt::Expr(e) => {
            // `case fallible { when ok(..) / when err(..) }` — the M1
            // error-handling statement, lowered to a Rust match.
            if let ExprKind::Case { scrutinee, arms } = &e.kind {
                return lower_case_stmt(scrutinee, arms, cx, out, ind);
            }
            // `if cond { ... } else { ... }` in statement position.
            if let ExprKind::If {
                cond,
                then_b,
                else_b,
                let_binding,
            } = &e.kind
            {
                if let_binding.is_some() {
                    return err(e.span, "`if let` survived the desugar (§8.69) — this is a pixie bug");
                }
                let c = lower_method_expr(cond, cx)?;
                writeln!(out, "{ind}if {c} {{").unwrap();
                let inner = format!("{ind}    ");
                let depth = cx.locals.len();
                lower_scope(&then_b.stmts, then_b.trailing.as_deref(), true, cx, out, &inner)?;
                cx.locals.truncate(depth);
                if let Some(eb) = else_b {
                    writeln!(out, "{ind}}} else {{").unwrap();
                    lower_scope(&eb.stmts, eb.trailing.as_deref(), true, cx, out, &inner)?;
                    cx.locals.truncate(depth);
                }
                writeln!(out, "{ind}}}").unwrap();
                return Ok(());
            }
            // `prop.push(x)` — the COW read-modify-writeback stereotype.
            if let ExprKind::MethodCall {
                receiver,
                method,
                args,
                block: None,
                ..
            } = &e.kind
            {
                if method.name == "push" {
                    // `xs.push(v)` on ANY list property, whatever
                    // holds it: this class's own, a global's, or one
                    // reached through an object (a parameter, a
                    // local, a chain — `t.kids.push(k)`). The three
                    // used to be separate arms and the object case
                    // simply was not lowerable; they are one call to
                    // the property's own `push` now.
                    if let Some((holder, pi)) = list_push_target(receiver, cx) {
                        if args.len() != 1 {
                            return err(e.span, "`push` takes one argument");
                        }
                        let v = lower_method_expr(&args[0], cx)?;
                        writeln!(
                            out,
                            "{ind}{{ let __h = {holder}; let __v = {v}; __h.push_{}(w, __v); }}",
                            pi.rust
                        )
                        .unwrap();
                        return Ok(());
                    }
                }
            }
            // Test asserts lower to Rust assert macros (statement position).
            if let ExprKind::Call {
                callee,
                args,
                block: None,
                ..
            } = &e.kind
            {
                if let ExprKind::Ident(f) = &callee.kind {
                    match (f.as_str(), args.len()) {
                        ("assert_eq", 2) => {
                            let a = lower_method_expr(&args[0], cx)?;
                            let b = lower_method_expr(&args[1], cx)?;
                            writeln!(out, "{ind}assert_eq!({a}, {b});").unwrap();
                            return Ok(());
                        }
                        ("assert_neq", 2) => {
                            let a = lower_method_expr(&args[0], cx)?;
                            let b = lower_method_expr(&args[1], cx)?;
                            writeln!(out, "{ind}assert_ne!({a}, {b});").unwrap();
                            return Ok(());
                        }
                        ("assert_true", 1) => {
                            let a = lower_method_expr(&args[0], cx)?;
                            writeln!(out, "{ind}assert!({a});").unwrap();
                            return Ok(());
                        }
                        ("assert_false", 1) => {
                            let a = lower_method_expr(&args[0], cx)?;
                            writeln!(out, "{ind}assert!(!({a}));").unwrap();
                            return Ok(());
                        }
                        _ => {}
                    }
                }
            }
            let v = lower_method_expr(e, cx)?;
            writeln!(out, "{ind}{v};").unwrap();
            Ok(())
        }
        Stmt::Emit { signal, span, args } => {
            if let Some(a) = args.first() {
                return err(
                    a.span,
                    format!(
                        "`emit {}` carries no arguments — a view answers a signal by \
                         re-reading the object, so write the value to a `prop` first",
                        signal.name
                    ),
                );
            }
            let Some(sig) = cx.class.signal(&signal.name) else {
                return err(*span, format!("no signal `{}` on this class", signal.name));
            };
            writeln!(out, "{ind}w.notify(self.erase(), {});", sig.const_name).unwrap();
            Ok(())
        }
        Stmt::Return { value, span } => match value {
            Some(v) => {
                if cx.nullable_ret {
                    let wrapped = lower_nullable_slot(v, cx)?;
                    writeln!(out, "{ind}return {wrapped};").unwrap();
                    return Ok(());
                }
                let r = lower_method_expr(v, cx)?;
                if cx.fallible_ret {
                    let wrapped = fallible_wrap(v, r, cx);
                    writeln!(out, "{ind}return {wrapped};").unwrap();
                } else {
                    writeln!(out, "{ind}return {r};").unwrap();
                }
                Ok(())
            }
            None => {
                let _ = span;
                if cx.fallible_ret {
                    writeln!(out, "{ind}return Ok(());").unwrap();
                } else if cx.nullable_ret {
                    // A bare `return` in a `T?` fn returns the absent
                    // value.
                    writeln!(out, "{ind}return None;").unwrap();
                } else {
                    writeln!(out, "{ind}return;").unwrap();
                }
                Ok(())
            }
        },
        // Loops (§8.27). A List iteration hoists the list into a
        // local FIRST — `for x in (expr).iter()` would drop the
        // temporary while borrowed — and the `w` borrow ends at that
        // `let`, so the body is free to thread `w` again. Elements
        // clone out (values or Copy handles — the §3.2 economy).
        Stmt::For {
            binding,
            index,
            iter,
            body,
            ..
        } => {
            let rb = camel_to_snake(&binding.name);
            if index.is_some() {
                writeln!(out, "{ind}let mut __turn{} = 0i64;", cx.loop_depth).unwrap();
            }
            let range_form = matches!(&iter.kind, ExprKind::Range { .. });
            if let ExprKind::Range {
                start,
                end,
                inclusive,
            } = &iter.kind
            {
                let s = lower_method_expr(start, cx)?;
                let e2 = lower_method_expr(end, cx)?;
                let op = if *inclusive { "..=" } else { ".." };
                writeln!(out, "{ind}for {rb} in ({s}){op}({e2}) {{").unwrap();
            } else {
                let xs = lower_method_expr(iter, cx)?;
                writeln!(out, "{ind}{{ let __xs = {xs}; for __it in __xs.iter() {{").unwrap();
                writeln!(out, "{ind}    let {rb} = __it.clone();").unwrap();
            }
            let depth = cx.locals.len();
            // A repeater over a list of OBJECTS binds a HANDLE, so the
            // loop variable carries its class (§8.63) — otherwise
            // `n.method()` on a row lowered without threading the
            // World and rustc rejected generated code (D10). The view
            // repeater has done this since §8.41; the method-body one
            // never did.
            let elem_class = match declared_ty_of(iter, cx) {
                Some(RustTy::List(inner)) => match *inner {
                    RustTy::Handle(c) => Some(c),
                    _ => None,
                },
                _ => None,
            };
            cx.locals.push((binding.name.clone(), elem_class, false, None));
            if let Some(i) = index {
                // `for x, i in xs` — the row's position, counted as
                // the loop runs so a list and a range say the same.
                let iv = camel_to_snake(&i.name);
                writeln!(out, "{ind}    let {iv} = __turn{}; __turn{} += 1;", cx.loop_depth, cx.loop_depth).unwrap();
                cx.locals.push((i.name.clone(), None, false, None));
            }
            cx.loop_depth += 1;
            let inner = format!("{ind}    ");
            lower_scope(&body.stmts, body.trailing.as_deref(), true, cx, out, &inner)?;
            cx.loop_depth -= 1;
            cx.locals.truncate(depth);
            if range_form {
                writeln!(out, "{ind}}}").unwrap();
            } else {
                writeln!(out, "{ind}}} }}").unwrap();
            }
            Ok(())
        }
        Stmt::While { cond, body, .. } => {
            let c = lower_method_expr(cond, cx)?;
            writeln!(out, "{ind}while {c} {{").unwrap();
            let depth = cx.locals.len();
            cx.loop_depth += 1;
            let inner = format!("{ind}    ");
            lower_scope(&body.stmts, body.trailing.as_deref(), true, cx, out, &inner)?;
            cx.loop_depth -= 1;
            cx.locals.truncate(depth);
            writeln!(out, "{ind}}}").unwrap();
            Ok(())
        }
        Stmt::Break { span } => {
            if cx.loop_depth == 0 {
                return err(*span, "`break` outside a loop");
            }
            writeln!(out, "{ind}break;").unwrap();
            Ok(())
        }
        Stmt::Continue { span } => {
            if cx.loop_depth == 0 {
                return err(*span, "`continue` outside a loop");
            }
            writeln!(out, "{ind}continue;").unwrap();
            Ok(())
        }
        // Not "(M0) yet" (§8.62): `batch { .. }` grouped property
        // writes into one notification round. pixie defers every
        // notification to `flush` and collapses repeats on the way
        // (§8.43), so a method body IS a batch — no listener runs
        // until it returns. The block would be a no-op that reads
        // like a guarantee.
        Stmt::Batch { span, .. } => err(
            *span,
            "writes are already batched: no view rebuilds until the method returns, and \
             writing one property twice notifies once. Drop the `batch` block",
        ),
    }
}

/// Lower `case x { when ... }` to a Rust `match`. Two modes: `ok`/`err`
/// over a fallible (Result), or variants of one user enum (payload
/// bindings supported). A `when _` arm maps to `_`; a non-exhaustive
/// enum match gets a silent `_ => {}` tail.
fn lower_case_stmt(
    scrutinee: &Expr,
    arms: &[ast::CaseArm],
    cx: &mut MethodCtx,
    out: &mut String,
    ind: &str,
) -> Result<(), EmitError> {
    let is_result = !arms.is_empty()
        && arms.iter().all(|a| {
            matches!(&a.pattern, ast::Pattern::Ctor { name, .. } if name.name == "ok" || name.name == "err")
        });
    // `case x { when some(v) { .. } when nil { .. } }` over a `T?` —
    // the ok/err machinery pointed at `Option` (§11.11). `nil` is the
    // absent arm (cute's literal, the shape the exhaustiveness
    // heuristic in pixie-hir already recognizes).
    let nil_pat =
        |p: &ast::Pattern| matches!(p, ast::Pattern::Literal { value, .. } if matches!(value.kind, ExprKind::Nil));
    let is_option = !arms.is_empty()
        && arms.iter().any(
            |a| matches!(&a.pattern, ast::Pattern::Ctor { name, .. } if name.name == "some"),
        )
        && arms.iter().all(|a| {
            nil_pat(&a.pattern)
                || matches!(&a.pattern, ast::Pattern::Ctor { name, .. } if name.name == "some")
        });

    // `binds` carries each bound name with the CLASS it holds, when
    // it holds one (§8.68). A `T?` of class type binds a handle, and
    // registering it with no class made `l.v` lower as a struct-field
    // read — the §8.63 bug, one construct over.
    let emit_arm_body = |body: &ast::Block,
                         binds: Vec<(String, Option<String>, Option<RustTy>)>,
                         cx: &mut MethodCtx,
                         out: &mut String,
                         ind: &str|
     -> Result<(), EmitError> {
        let depth = cx.locals.len();
        cx.locals.extend(
            binds
                .into_iter()
                .map(|(b, c, t)| (b, c, matches!(t, Some(RustTy::Opt(_))), t)),
        );
        let inner = format!("{ind}    ");
        lower_scope(&body.stmts, body.trailing.as_deref(), true, cx, out, &inner)?;
        cx.locals.truncate(depth);
        Ok(())
    };

    let scrut = lower_method_expr(scrutinee, cx)?;

    if is_result {
        let mut ok_arm: Option<(Option<String>, &ast::Block)> = None;
        let mut err_arm: Option<(Option<String>, &ast::Block)> = None;
        for arm in arms {
            let ast::Pattern::Ctor { name, args, span } = &arm.pattern else {
                unreachable!("is_result checked");
            };
            let bind = match args.as_slice() {
                [] => None,
                [ast::Pattern::Bind { name, .. }] => Some(name.name.clone()),
                [ast::Pattern::Wild { .. }] => None,
                _ => return err(*span, "ok/err patterns take at most one binding (M1)"),
            };
            if name.name == "ok" {
                ok_arm = Some((bind, &arm.body));
            } else {
                err_arm = Some((bind, &arm.body));
            }
        }
        let (Some((ok_bind, ok_body)), Some((err_bind, err_body))) = (ok_arm, err_arm) else {
            return err(
                scrutinee.span,
                "`case` over a fallible needs both `ok` and `err` arms (M1)",
            );
        };
        writeln!(out, "{ind}match {scrut} {{").unwrap();
        for (variant, bind, body) in [("Ok", ok_bind, ok_body), ("Err", err_bind, err_body)] {
            let binder = match &bind {
                Some(surface) => camel_to_snake(surface),
                None => "_".to_string(),
            };
            writeln!(out, "{ind}    {variant}({binder}) => {{").unwrap();
            emit_arm_body(
                body,
                bind.into_iter().map(|b| (b, None, None)).collect(),
                cx,
                out,
                &format!("{ind}    "),
            )?;
            writeln!(out, "{ind}    }}").unwrap();
        }
        writeln!(out, "{ind}}}").unwrap();
        return Ok(());
    }

    if is_option {
        let mut some_arm: Option<(Option<String>, &ast::Block)> = None;
        let mut none_arm: Option<&ast::Block> = None;
        for arm in arms {
            match &arm.pattern {
                ast::Pattern::Ctor { args, span, .. } => {
                    let bind = match args.as_slice() {
                        [] => None,
                        [ast::Pattern::Bind { name, .. }] => Some(name.name.clone()),
                        [ast::Pattern::Wild { .. }] => None,
                        _ => return err(*span, "`some` takes at most one binding (M1)"),
                    };
                    some_arm = Some((bind, &arm.body));
                }
                _ => none_arm = Some(&arm.body),
            }
        }
        let (Some((some_bind, some_body)), Some(none_body)) = (some_arm, none_arm) else {
            return err(
                scrutinee.span,
                "`case` over a `T?` needs both `some` and `nil` arms (M1)",
            );
        };
        writeln!(out, "{ind}match {scrut} {{").unwrap();
        let binder = match &some_bind {
            Some(surface) => camel_to_snake(surface),
            None => "_".to_string(),
        };
        writeln!(out, "{ind}    Some({binder}) => {{").unwrap();
        // What the optional holds, when it holds an object.
        let some_elem = match declared_ty_of(scrutinee, cx) {
            Some(RustTy::Opt(inner)) => Some((*inner).clone()),
            _ => None,
        };
        let some_class = match &some_elem {
            Some(RustTy::Handle(c)) => Some(c.clone()),
            _ => None,
        };
        emit_arm_body(
            some_body,
            some_bind
                .into_iter()
                .map(|b| (b, some_class.clone(), some_elem.clone()))
                .collect(),
            cx,
            out,
            &format!("{ind}    "),
        )?;
        writeln!(out, "{ind}    }}").unwrap();
        writeln!(out, "{ind}    None => {{").unwrap();
        emit_arm_body(none_body, Vec::new(), cx, out, &format!("{ind}    "))?;
        writeln!(out, "{ind}    }}").unwrap();
        writeln!(out, "{ind}}}").unwrap();
        return Ok(());
    }

    // Enum mode: every Ctor arm must name a variant of one enum.
    let first_ctor = arms.iter().find_map(|a| match &a.pattern {
        ast::Pattern::Ctor { name, .. } => Some(name.name.clone()),
        _ => None,
    });
    let Some(first_ctor) = first_ctor else {
        return err(scrutinee.span, "`case` needs at least one variant arm (M1)");
    };
    let Some(en) = cx
        .enums
        .values()
        .find(|en| en.variant(&first_ctor).is_some())
    else {
        return err(
            scrutinee.span,
            format!("no enum declares a variant `{first_ctor}`"),
        );
    };
    let enum_name = en.name.clone();
    let mut covered: Vec<String> = Vec::new();
    let mut wild_body: Option<&ast::Block> = None;

    writeln!(out, "{ind}match {scrut} {{").unwrap();
    for arm in arms {
        match &arm.pattern {
            ast::Pattern::Ctor { name, args, span } => {
                let Some(v) = cx
                    .enums
                    .get(&enum_name)
                    .and_then(|en| en.variant(&name.name))
                else {
                    return err(
                        *span,
                        format!("no variant `{}` on `{enum_name}`", name.name),
                    );
                };
                let n_fields = v.fields.len();
                let mut binds: Vec<(String, Option<RustTy>)> = Vec::new();
                let pat = if args.is_empty() {
                    if n_fields == 0 {
                        format!("{enum_name}::{}", escape_rust_keyword(name.name.clone()))
                    } else {
                        format!("{enum_name}::{}(..)", escape_rust_keyword(name.name.clone()))
                    }
                } else {
                    if args.len() != n_fields {
                        return err(
                            *span,
                            format!("variant `{}` carries {} value(s)", name.name, n_fields),
                        );
                    }
                    let mut ps: Vec<String> = Vec::new();
                    for (fi, a) in args.iter().enumerate() {
                        match a {
                            ast::Pattern::Bind { name, .. } => {
                                ps.push(camel_to_snake(&name.name));
                                // The payload field's type, so an
                                // optional one prints as an optional
                                // (§8.70).
                                let fty = v
                                    .fields
                                    .get(fi)
                                    .map(|f| lower_type(&f.ty, cx.class_names))
                                    .transpose()?;
                                binds.push((name.name.clone(), fty));
                            }
                            ast::Pattern::Wild { .. } => ps.push("_".into()),
                            ast::Pattern::Ctor { span, .. }
                            | ast::Pattern::Literal { span, .. } => {
                                return err(*span, "nested patterns are not lowerable yet (M2)");
                            }
                        }
                    }
                    format!("{enum_name}::{}({})", escape_rust_keyword(name.name.clone()), ps.join(", "))
                };
                covered.push(name.name.clone());
                writeln!(out, "{ind}    {pat} => {{").unwrap();
                emit_arm_body(
                    &arm.body,
                    binds
                        .into_iter()
                        .map(|(b, t)| {
                            let c = match &t {
                                Some(RustTy::Handle(c)) => Some(c.clone()),
                                _ => None,
                            };
                            (b, c, t)
                        })
                        .collect(),
                    cx,
                    out,
                    &format!("{ind}    "),
                )?;
                writeln!(out, "{ind}    }}").unwrap();
            }
            ast::Pattern::Wild { .. } => {
                wild_body = Some(&arm.body);
            }
            ast::Pattern::Literal { span, .. } | ast::Pattern::Bind { span, .. } => {
                return err(*span, "this pattern is not lowerable yet (M2)");
            }
        }
    }
    if let Some(body) = wild_body {
        writeln!(out, "{ind}    _ => {{").unwrap();
        emit_arm_body(body, Vec::new(), cx, out, &format!("{ind}    "))?;
        writeln!(out, "{ind}    }}").unwrap();
    } else {
        let all = cx.enums.get(&enum_name).map(|en| en.variants.len()).unwrap_or(0);
        if covered.len() < all {
            writeln!(out, "{ind}    _ => {{}}").unwrap();
        }
    }
    writeln!(out, "{ind}}}").unwrap();
    Ok(())
}

// ---------------------------------------------------------------------------
// Async fn lowering (S5's shape, emitted). The spawned task captures
// only `self` (a Copy handle), params, and locals (values / Copy
// handles) — never a World borrow. Sync statements run inside
// `__ctx.with` re-entries; a supported `await` ends the current
// re-entry, ships the bound call to a worker thread with owned
// arguments, and awaits its completion.

/// Emit one async-body statement. `await` is accepted as the entire
/// value of a `let` / assignment / expression statement / `case`
/// scrutinee; anywhere deeper, the sync lowering surfaces its own
/// diagnostic.
fn emit_async_stmt(
    s: &Stmt,
    cx: &mut MethodCtx,
    out: &mut String,
    ind: &str,
    await_n: &mut usize,
) -> Result<(), EmitError> {
    match s {
        Stmt::Let {
            name, value, ..
        }
        | Stmt::Var {
            name, value, ..
        } => {
            let is_var = matches!(s, Stmt::Var { .. });
            let rn = camel_to_snake(&name.name);
            let mutkw = if is_var { "mut " } else { "" };
            if let ExprKind::Await(inner) = &value.kind {
                let conv = emit_await_dispatch(inner, cx, out, ind, await_n)?;
                writeln!(out, "{ind}let {mutkw}{rn} = {conv};").unwrap();
                cx.locals.push((name.name.clone(), None, false, None));
            } else {
                let v = lower_method_expr(value, cx)?;
                writeln!(
                    out,
                    "{ind}let {mutkw}{rn} = __ctx.with(|w: &mut World| {{ {v} }});"
                )
                .unwrap();
                let handle_class = match &value.kind {
                    ExprKind::Call { callee, .. } => match &callee.kind {
                        ExprKind::Ident(c) if cx.classes.contains_key(c) => Some(c.clone()),
                        _ => None,
                    },
                    _ => None,
                };
                cx.locals.push((name.name.clone(), handle_class, false, None));
            }
            Ok(())
        }
        Stmt::Assign {
            target, op, value, span,
        } => {
            if let ExprKind::Await(inner) = &value.kind {
                let conv = emit_await_dispatch(inner, cx, out, ind, await_n)?;
                let tmp = format!("__awaited{}", *await_n);
                writeln!(out, "{ind}let {tmp} = {conv};").unwrap();
                cx.locals.push((tmp.clone(), None, false, None));
                let synthetic = Stmt::Assign {
                    target: target.clone(),
                    op: *op,
                    value: Expr {
                        kind: ExprKind::Ident(tmp),
                        span: value.span,
                    },
                    span: *span,
                };
                writeln!(out, "{ind}__ctx.with(|w: &mut World| {{").unwrap();
                lower_method_stmt(&synthetic, cx, out, &format!("{ind}    "))?;
                writeln!(out, "{ind}}});").unwrap();
                cx.locals.pop();
                Ok(())
            } else {
                writeln!(out, "{ind}__ctx.with(|w: &mut World| {{").unwrap();
                lower_method_stmt(s, cx, out, &format!("{ind}    "))?;
                writeln!(out, "{ind}}});").unwrap();
                Ok(())
            }
        }
        Stmt::Expr(e) => match &e.kind {
            ExprKind::Await(inner) => {
                let conv = emit_await_dispatch(inner, cx, out, ind, await_n)?;
                writeln!(out, "{ind}let _ = {conv};").unwrap();
                Ok(())
            }
            ExprKind::Case { scrutinee, arms }
                if matches!(scrutinee.kind, ExprKind::Await(_)) =>
            {
                let ExprKind::Await(inner) = &scrutinee.kind else {
                    unreachable!("guarded");
                };
                let conv = emit_await_dispatch(inner, cx, out, ind, await_n)?;
                let tmp = format!("__awaited{}", *await_n);
                writeln!(out, "{ind}let {tmp} = {conv};").unwrap();
                cx.locals.push((tmp.clone(), None, false, None));
                let synthetic = Expr {
                    kind: ExprKind::Ident(tmp),
                    span: scrutinee.span,
                };
                writeln!(out, "{ind}__ctx.with(|w: &mut World| {{").unwrap();
                lower_case_stmt(&synthetic, arms, cx, out, &format!("{ind}    "))?;
                writeln!(out, "{ind}}});").unwrap();
                cx.locals.pop();
                Ok(())
            }
            // An `if` whose branches await: the condition is evaluated in
            // its own re-entry and the branches keep the async lowering,
            // so `await` reads the same inside a branch as outside one.
            // (Without this the branch went through the sync lowering,
            // where an `await` cannot appear at all — a shape an author
            // writes the moment a dialog's answer decides what happens
            // next.)
            ExprKind::If { .. } if stmt_awaits(s) => {
                let ExprKind::If {
                    cond,
                    then_b,
                    else_b,
                    let_binding,
                } = &e.kind
                else {
                    unreachable!("guarded above")
                };
                if let_binding.is_some() {
                    return err(e.span, "`if let` survived the desugar (§8.69) — this is a pixie bug");
                }
                let c = lower_method_expr(cond, cx)?;
                let n = *await_n;
                *await_n += 1;
                writeln!(
                    out,
                    "{ind}let __ac{n} = __ctx.with(|w: &mut World| {{ {c} }});"
                )
                .unwrap();
                writeln!(out, "{ind}if __ac{n} {{").unwrap();
                let inner = format!("{ind}    ");
                let depth = cx.locals.len();
                for st in &then_b.stmts {
                    emit_async_stmt(st, cx, out, &inner, await_n)?;
                }
                cx.locals.truncate(depth);
                if let Some(eb) = else_b {
                    writeln!(out, "{ind}}} else {{").unwrap();
                    for st in &eb.stmts {
                        emit_async_stmt(st, cx, out, &inner, await_n)?;
                    }
                    cx.locals.truncate(depth);
                }
                writeln!(out, "{ind}}}").unwrap();
                Ok(())
            }
            _ => {
                writeln!(out, "{ind}__ctx.with(|w: &mut World| {{").unwrap();
                lower_method_stmt(s, cx, out, &format!("{ind}    "))?;
                writeln!(out, "{ind}}});").unwrap();
                Ok(())
            }
        },
        Stmt::Return { span, .. } => err(
            *span,
            "`return` in async fns is not lowerable yet (M2); async fns run to completion",
        ),

        _ => {
            writeln!(out, "{ind}__ctx.with(|w: &mut World| {{").unwrap();
            lower_method_stmt(s, cx, out, &format!("{ind}    "))?;
            writeln!(out, "{ind}}});").unwrap();
            Ok(())
        }
    }
}

/// Does this statement contain an `await` anywhere inside it? The
/// async lowering asks before it takes a nested block: a branch with
/// no await keeps the cheaper sync path (one re-entry for the whole
/// block), and one with an await is lowered statement by statement.
fn stmt_awaits(s: &Stmt) -> bool {
    fn expr_awaits(e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Await(_) => true,
            ExprKind::If {
                cond,
                then_b,
                else_b,
                ..
            } => {
                expr_awaits(cond)
                    || then_b.stmts.iter().any(stmt_awaits)
                    || then_b.trailing.as_deref().is_some_and(expr_awaits)
                    || else_b.as_ref().is_some_and(|b| {
                        b.stmts.iter().any(stmt_awaits)
                            || b.trailing.as_deref().is_some_and(expr_awaits)
                    })
            }
            _ => false,
        }
    }
    match s {
        Stmt::Let { value, .. } | Stmt::Var { value, .. } => expr_awaits(value),
        Stmt::Assign { value, .. } => expr_awaits(value),
        Stmt::Expr(e) => expr_awaits(e),
        _ => false,
    }
}

/// Ship one awaited binding call to a worker: evaluate arguments to
/// owned values inside a re-entry, spawn the worker with a completion
/// handle, and return the expression that awaits + converts the result
/// back into pixie types (errors become `Str`, matching the sync
/// adapter path).
fn emit_await_dispatch(
    inner: &Expr,
    cx: &MethodCtx,
    out: &mut String,
    ind: &str,
    await_n: &mut usize,
) -> Result<String, EmitError> {
    // Resolve the binding call: `Binding.f(args)` in either call shape.
    let (bf, args) = match &inner.kind {
        ExprKind::MethodCall {
            receiver,
            method,
            args,
            block: None,
            ..
        } => {
            let ExprKind::Ident(r) = &receiver.kind else {
                return err(
                    inner.span,
                    "only binding calls can be awaited in this milestone (async composition is M2)",
                );
            };
            let Some(bc) = cx.bindings.get(r) else {
                return err(
                    inner.span,
                    "only binding calls can be awaited in this milestone (async composition is M2)",
                );
            };
            let Some(bf) = bc.statics.get(&method.name) else {
                return err(
                    inner.span,
                    format!("no binding fn `{}` on `{r}`", method.name),
                );
            };
            (bf, args)
        }
        ExprKind::Call {
            callee,
            args,
            block: None,
            ..
        } => {
            let ExprKind::Path(p) = &callee.kind else {
                return err(
                    inner.span,
                    "only binding calls can be awaited in this milestone (async composition is M2)",
                );
            };
            if p.len() != 2 {
                return err(
                    inner.span,
                    "only binding calls can be awaited in this milestone (async composition is M2)",
                );
            }
            let Some(bc) = cx.bindings.get(&p[0].name) else {
                return err(
                    inner.span,
                    "only binding calls can be awaited in this milestone (async composition is M2)",
                );
            };
            let Some(bf) = bc.statics.get(&p[1].name) else {
                return err(
                    inner.span,
                    format!("no binding fn `{}` on `{}`", p[1].name, p[0].name),
                );
            };
            (bf, args)
        }
        _ => {
            return err(
                inner.span,
                "only binding calls can be awaited in this milestone (async composition is M2)",
            );
        }
    };
    if args.len() != bf.params.len() {
        return err(inner.span, format!("expected {} argument(s)", bf.params.len()));
    }
    let n = *await_n;
    *await_n += 1;

    // Owned argument tuple, evaluated inside a re-entry (args may read
    // props); COW values convert to Send-able owned forms.
    let mut owned: Vec<String> = Vec::new();
    let mut call_sites: Vec<String> = Vec::new();
    for (i, (a, ty)) in args.iter().zip(&bf.params).enumerate() {
        let v = lower_method_expr(a, cx)?;
        match ty {
            RustTy::Str => {
                owned.push(format!("({v}).as_str().to_string()"));
                call_sites.push(format!("__args{n}.{i}.as_str()"));
            }
            // Bytes owns an Rc (not Send): the worker gets a Vec<u8>.
            RustTy::Bytes => {
                owned.push(format!("({v}).as_slice().to_vec()"));
                call_sites.push(format!("__args{n}.{i}.as_slice()"));
            }
            // Map too: plain pairs cross the thread, the worker
            // rebuilds the COW map (§12.3 headers).
            RustTy::Map(..) => {
                owned.push(format!("pixie_kernel::map_to_send(&({v}))"));
                call_sites.push(format!("pixie_kernel::map_from_send(&__args{n}.{i})"));
            }
            RustTy::Int | RustTy::Float | RustTy::Bool => {
                owned.push(v);
                call_sites.push(format!("__args{n}.{i}"));
            }
            _ => {
                return err(a.span, "this argument type is not adaptable in `await` yet (M2)");
            }
        }
    }
    if !args.is_empty() {
        writeln!(
            out,
            "{ind}let __args{n} = __ctx.with(|w: &mut World| ({},));",
            owned.join(", ")
        )
        .unwrap();
    }
    writeln!(out, "{ind}let (__h{n}, __c{n}) = pixie_kernel::completion();").unwrap();
    let call = format!("{}({})", bf.rust_path, call_sites.join(", "));
    let sent = if bf.fallible {
        format!("{call}.map_err(|e| e.to_string())")
    } else {
        call
    };
    writeln!(
        out,
        "{ind}pixie_kernel::spawn_worker(move || {{ __h{n}.complete({sent}); }});"
    )
    .unwrap();
    // The worker completes the native value (Send); conversion to
    // pixie types happens main-side, by declared type — the same rule
    // as the sync adapter.
    let Ok(ok_conv) = binding_ret_conv(&bf.ret, bf.std_map, cx.enums, cx.structs) else {
        return err(inner.span, "this return type is not adaptable in `await` yet (M2)");
    };
    let conv = if bf.fallible {
        match ok_conv {
            RetConv::Expr(t) => format!("__c{n}.await.map(|__v| {t}).map_err(Str::from)"),
            RetConv::Pass => format!("__c{n}.await.map_err(Str::from)"),
        }
    } else {
        match ok_conv {
            RetConv::Expr(t) => format!("{{ let __v = __c{n}.await; {t} }}"),
            RetConv::Pass => format!("__c{n}.await"),
        }
    };
    Ok(conv)
}

/// Emit the spawned-task body of an `async fn`.
fn emit_async_body(
    body: &ast::Block,
    cx: &mut MethodCtx,
    out: &mut String,
    ind: &str,
) -> Result<(), EmitError> {
    let mut await_n = 0usize;
    for s in &body.stmts {
        emit_async_stmt(s, cx, out, ind, &mut await_n)?;
    }
    if let Some(t) = &body.trailing {
        let stmt = Stmt::Expr((**t).clone());
        emit_async_stmt(&stmt, cx, out, ind, &mut await_n)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// View lowering.

struct ViewCtx<'a> {
    /// Object state fields: view-local name -> class name.
    fields: HashMap<String, String>,
    classes: &'a HashMap<String, ClassInfo<'a>>,
    /// Binding classes (`.rpi`), so a view can call a PURE binding
    /// static the way it calls a local `static fn` (§8.54).
    bindings: &'a HashMap<String, BindingClass>,
    /// Struct layouts, so a view can read a field off a struct-typed
    /// property or repeater row (§8.68).
    structs: &'a HashMap<String, StructInfo<'a>>,
    /// Enum declarations, for a `case` in a view body (§8.69).
    enums: &'a HashMap<String, EnumInfo<'a>>,
    globals: &'a Globals,
    /// Class names, for `lower_type` (§11.23).
    class_names: &'a std::collections::HashSet<String>,
    /// `for` loop variables in scope (each is a `&Str` element binding).
    /// (surface name, element class when the list holds OBJECTS).
    /// A repeater over `List<Handle<C>>` binds a handle, so member
    /// access on the loop variable has to dispatch through the World
    /// rather than read a value field (§8.41).
    /// (surface name, element class when it holds objects, element
    /// TYPE — the third is what a struct row needs, §8.68).
    loop_vars: Vec<(String, Option<String>, RustTy)>,
    depth: usize,
    /// Nesting depth of view-body `for` repeaters. Each level binds
    /// its own `__row_idx{d}`, and a `__PixieRowScope` at depth `d`
    /// indexes its seat by the path `[__row_idx0 .. __row_idx{d-1}]`
    /// (§8.34 — nested repeaters keep per-row state per path).
    repeat_depth: usize,
}

impl ViewCtx<'_> {
    fn is_loop_var(&self, n: &str) -> bool {
        self.loop_vars.iter().any(|(v, _, _)| v == n)
    }

    /// The class an expression denotes when it denotes an OBJECT.
    /// `handle_class_of`'s view-side twin (§8.40/§8.41): a state
    /// field, a global, a repeater variable over a list of objects,
    /// or any prop chain rooted at one of those.
    fn object_class(&self, e: &Expr) -> Option<String> {
        match &e.kind {
            ExprKind::Ident(n) => {
                if let Some((_, Some(c), _)) = self.loop_vars.iter().find(|(v, _, _)| v == n) {
                    return Some(c.clone());
                }
                // A STORE is an object too (§8.64): `Board.root.name`
                // reaches through a store's class-typed prop exactly
                // as `note.tag.label` reaches through a row's. Only
                // view fields were listed here, so a chain rooted at a
                // global stopped at the first hop.
                self.fields
                    .get(n)
                    .cloned()
                    .or_else(|| self.globals.get(n).cloned())
            }
            ExprKind::Member { receiver, name } => {
                if let ExprKind::Ident(r) = &receiver.kind {
                    if let Some((info, _)) = self.handle_for(r) {
                        return match &info.prop(&name.name)?.ty {
                            RustTy::Handle(c) => Some(c.clone()),
                            _ => None,
                        };
                    }
                }
                let c = self.object_class(receiver)?;
                match &self.classes.get(&c)?.prop(&name.name)?.ty {
                    RustTy::Handle(c2) => Some(c2.clone()),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Lower a read of `name` off an object expression, when the
    /// receiver is an object and the prop exists. `None` leaves the
    /// caller's own diagnostics in charge.
    fn object_prop_read(&self, receiver: &Expr, name: &str) -> Option<(String, RustTy)> {
        let c = self.object_class(receiver)?;
        let info = self.classes.get(&c)?;
        let pi = info.prop(name)?;
        let base = match &receiver.kind {
            ExprKind::Ident(n) if self.is_loop_var(n) => camel_to_snake(n),
            ExprKind::Ident(n) => self.handle_for(n).map(|(_, h)| h)?,
            ExprKind::Member { .. } => {
                let (inner, _) = self.object_prop_read(
                    match &receiver.kind {
                        ExprKind::Member { receiver: r, .. } => r,
                        _ => return None,
                    },
                    match &receiver.kind {
                        ExprKind::Member { name, .. } => &name.name,
                        _ => return None,
                    },
                )?;
                inner
            }
            _ => return None,
        };
        Some((format!("({base}).{}(w)", pi.rust), pi.ty.clone()))
    }

    /// Read a FIELD off a struct value the view can name (§8.68):
    /// a struct-typed property, a repeater row over `List<Struct>`,
    /// or a chain through either. Answers the read and the field's
    /// type, so a chain can continue.
    fn struct_field_read(&self, receiver: &Expr, name: &str) -> Option<(String, RustTy)> {
        let (base, ty) = self.struct_value_read(receiver)?;
        let RustTy::Named(sname) = ty else { return None };
        let st = self.structs.get(&sname)?;
        let (_, rust, fty) = st.fields.iter().find(|(surface, _, _)| surface == name)?;
        Some((format!("({base}).{rust}.clone()", ), fty.clone()))
    }

    /// The element type a repeater variable binds.
    fn loop_elem_ty(&self, n: &str) -> Option<RustTy> {
        self.loop_vars
            .iter()
            .find(|(v, _, _)| v == n)
            .map(|(_, _, t)| t.clone())
    }

    /// A value expression the view can name, with its type — the
    /// receiver side of `struct_field_read`.
    fn struct_value_read(&self, e: &Expr) -> Option<(String, RustTy)> {
        match &e.kind {
            // A repeater row: the loop variable is the value itself.
            ExprKind::Ident(n) if self.is_loop_var(n) => {
                let ty = self.loop_elem_ty(n)?;
                Some((camel_to_snake(n), ty))
            }
            ExprKind::Member { receiver, name } => match &receiver.kind {
                ExprKind::Ident(f) if self.handle_for(f).is_some() => {
                    let (class, handle) = self.handle_for(f)?;
                    let p = class.prop(&name.name)?;
                    Some((format!("{handle}.{}(w)", p.rust), p.ty.clone()))
                }
                _ => self
                    .object_prop_read(receiver, &name.name)
                    .or_else(|| self.struct_field_read(receiver, &name.name)),
            },
            _ => None,
        }
    }

    /// Resolve `name` to (class info, handle expression). Fields resolve
    /// to their captured Copy local; globals to a `singleton_ref` read —
    /// `w` deliberately binds to whichever World is in scope (build's
    /// `&World`, or the action closure's `&mut World`).
    fn handle_for(&self, name: &str) -> Option<(&ClassInfo<'_>, String)> {
        if let Some(class_name) = self.fields.get(name) {
            let info = self.classes.get(class_name)?;
            return Some((info, camel_to_snake(name)));
        }
        let class_name = self.globals.get(name)?;
        let info = self.classes.get(class_name)?;
        Some((info, format!("w.singleton_ref::<{class_name}>()")))
    }
}

/// Lower an expression appearing inside a view body to a display-ready
/// Rust expression (used as a format! argument).
/// The declared type of a value expression a view reads, when the
/// view can resolve it. Used to tell a map subscript (which answers
/// `T?`) from a list one (§8.68).
fn view_value_ty(e: &Expr, cx: &ViewCtx) -> Option<RustTy> {
    match &e.kind {
        ExprKind::Member { receiver, name } => match &receiver.kind {
            ExprKind::Ident(f) if cx.handle_for(f).is_some() => {
                let (class, _) = cx.handle_for(f)?;
                class.prop(&name.name).map(|p| p.ty.clone())
            }
            _ => cx.object_prop_read(receiver, &name.name).map(|(_, t)| t),
        },
        _ => None,
    }
}

/// `<map>.keys` / `<map>.values` in a view — the two built-ins a
/// repeater can iterate (§8.68). Answers the read expression and the
/// LIST type it produces, so the repeater types its loop variable
/// exactly as it does for a list property.
fn view_map_view(
    receiver: &Expr,
    name: &str,
    cx: &ViewCtx,
) -> Option<(String, RustTy)> {
    if name != "keys" && name != "values" {
        return None;
    }
    let (read, ty) = match &receiver.kind {
        ExprKind::Ident(f) => {
            let (class, handle) = cx.handle_for(f)?;
            let p = class.prop(name)?;
            (format!("{handle}.{}(w)", p.rust), p.ty.clone())
        }
        ExprKind::Member { receiver: r2, name: n2 } => match &r2.kind {
            ExprKind::Ident(f) if cx.handle_for(f).is_some() => {
                let (class, handle) = cx.handle_for(f)?;
                let p = class.prop(&n2.name)?;
                (format!("{handle}.{}(w)", p.rust), p.ty.clone())
            }
            _ => cx.object_prop_read(r2, &n2.name)?,
        },
        _ => return None,
    };
    let RustTy::Map(k, v) = ty else { return None };
    let elem = if name == "keys" { k } else { v };
    Some((format!("{read}.{name}()"), RustTy::List(elem)))
}

fn lower_view_display(e: &Expr, cx: &ViewCtx) -> Result<String, EmitError> {
    Ok(cast_if_widened(&lower_view_display_inner(e, cx)?, e.span))
}

fn lower_view_display_inner(e: &Expr, cx: &ViewCtx) -> Result<String, EmitError> {
    match &e.kind {
        ExprKind::Int(v) => Ok(format!("{v}i64")),
        ExprKind::Float(v) => Ok(format!("{v}f64")),
        ExprKind::Bool(v) => Ok(format!("{v}")),
        ExprKind::Str(parts) => lower_interp(parts, &mut |inner| lower_view_display(inner, cx)),
        // Arithmetic in an interpolation (§8.54). `#{S.n * 2}` and
        // `#{a} of #{b}` are the same shape of thing, and reading a
        // value is exactly what a view body is allowed to do — this
        // was a gap in the lowerer, not a rule about views.
        ExprKind::Binary { op, lhs, rhs } => {
            let l = lower_view_display(lhs, cx)?;
            let r = lower_view_display(rhs, cx)?;
            Ok(format!("({l} {} {r})", bin_op(op, e.span)?))
        }
        ExprKind::Unary { op, expr } => {
            let inner = lower_view_display(expr, cx)?;
            match op {
                ast::UnaryOp::Neg => Ok(format!("(-{inner})")),
                ast::UnaryOp::Not => Ok(format!("(!{inner})")),
            }
        }
        ExprKind::Ident(n) if cx.is_loop_var(n) => Ok(camel_to_snake(n)),
        ExprKind::Member { receiver, name } => {
            if name.name == "length" {
                let inner = lower_view_display(receiver, cx)?;
                return Ok(format!("({inner}.len() as i64)"));
            }
            if let ExprKind::Ident(f) = &receiver.kind {
                if let Some((class, handle)) = cx.handle_for(f) {
                    let Some(p) = class.prop(&name.name) else {
                        return err(
                            e.span,
                            format!("no property `{}` on `{}`", name.name, class.name),
                        );
                    };
                    let read = format!("{handle}.{}(w)", p.rust);
                    return Ok(match p.ty {
                        RustTy::Opt(_) => format!("__pixie_show_opt({read})"),
                        _ => read,
                    });
                }
            }
            // Through an OBJECT: a repeater variable over a list of
            // objects, or a prop chain rooted at a field (§8.41).
            if let Some((read, _)) = cx.object_prop_read(receiver, &name.name) {
                return Ok(read);
            }
            // Through a STRUCT: a struct-typed property, a repeater
            // row over `List<Struct>`, or a chain of either (§8.68).
            // This is the shape the memory model recommends — data as
            // `struct` on a store property — and a view could not read
            // one field of it.
            if let Some((read, ty)) = cx.struct_field_read(receiver, &name.name) {
                return Ok(match ty {
                    RustTy::Opt(_) => format!("__pixie_show_opt({read})"),
                    _ => read,
                });
            }
            err(e.span, "this member access is not lowerable in views yet (M0)")
        }
        // `C.staticFn(args)` in a view: a `static fn` has no
        // receiver and NO WORLD (§8.54's own definition), so it is
        // view-safe by construction — the blanket method rejection
        // below was catching it by shape alone.
        ExprKind::MethodCall {
            receiver,
            method,
            args,
            block: None,
            ..
        } if matches!(&receiver.kind, ExprKind::Ident(r)
            if cx.classes.get(r.as_str()).is_some_and(|info| {
                info.statics.iter().any(|f| f.name.name == method.name)
            })) =>
        {
            let ExprKind::Ident(recv_name) = &receiver.kind else {
                unreachable!("guarded")
            };
            let mut call = format!("{recv_name}::{}(", camel_to_snake(&method.name));
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    call.push_str(", ");
                }
                call.push_str(&lower_view_display_inner(a, cx)?);
            }
            call.push(')');
            Ok(call)
        }
        // A method call in a view body. CLASS methods really cannot
        // work — build takes `&World`, methods `&mut World` — but
        // the declared BUILT-IN VALUE surface is pure by spec
        // (`builtin_value_method_arity`'s "only the pure ones"), so
        // `m.getOr(k, d)` and friends read fine here.
        ExprKind::MethodCall {
            receiver,
            method,
            args,
            block: None,
            ..
        } if builtin_value_method_arity(&method.name).is_some() => {
            let recv = lower_view_display_inner(receiver, cx)?;
            let mut call = format!("({recv}).{}(", camel_to_snake(&method.name));
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    call.push_str(", ");
                }
                call.push_str(&lower_view_display_inner(a, cx)?);
            }
            call.push(')');
            Ok(call)
        }
        // A PURE binding static in a view (`Py.floatRepr(x)`): no
        // World in a binding signature ever, and with `!T` and
        // non-value shapes excluded it is exactly as view-safe as a
        // local `static fn` (§8.54). Fallible or non-value shapes
        // fall through to the rejection below.
        ExprKind::MethodCall {
            receiver,
            method,
            args,
            block: None,
            ..
        } if matches!(&receiver.kind, ExprKind::Ident(r)
            if cx.bindings.get(r.as_str()).is_some_and(|bc| {
                bc.statics.get(&method.name).is_some_and(|bf| {
                    !bf.fallible
                        && matches!(bf.ret, RustTy::Int | RustTy::Float | RustTy::Bool | RustTy::Str)
                        && bf.params.iter().all(|t| {
                            matches!(t, RustTy::Int | RustTy::Float | RustTy::Bool | RustTy::Str)
                        })
                })
            })) =>
        {
            let ExprKind::Ident(recv_name) = &receiver.kind else {
                unreachable!("guarded")
            };
            let bf = &cx.bindings[recv_name.as_str()].statics[&method.name];
            if args.len() != bf.params.len() {
                return err(e.span, format!("expected {} argument(s)", bf.params.len()));
            }
            let mut lowered = Vec::new();
            for (a, ty) in args.iter().zip(&bf.params) {
                let v = lower_view_display_inner(a, cx)?;
                lowered.push(match ty {
                    RustTy::Str => format!("({v}).as_str()"),
                    _ => v,
                });
            }
            let call = format!("{}({})", bf.rust_path, lowered.join(", "));
            Ok(match &bf.ret {
                RustTy::Int => format!("(({call}) as i64)"),
                RustTy::Float => format!("(({call}) as f64)"),
                RustTy::Str => format!("Str::from({call})"),
                _ => call,
            })
        }
        // A method call in a view body. This one really cannot work,
        // and the reason is the design rather than the lowering: a
        // view's `build` takes `&World` so that rebuilding can never
        // change what it is rebuilding from, and every class method
        // takes `&mut World`. The error says that instead of "not
        // lowerable", and names the two shapes that do work.
        ExprKind::MethodCall { receiver, method, .. } => {
            let who = match &receiver.kind {
                ExprKind::Ident(r) => r.clone(),
                _ => "it".to_string(),
            };
            err(
                e.span,
                format!(
                    "a view body cannot call `{}.{}()` — building a view only READS the \
                     World, and a method may write to it. Read a property instead, or have \
                     an action call the method and store its result",
                    who, method.name
                ),
            )
        }
        // `xs[i]` and `m[k]` in a view (§8.68). A list subscript traps
        // and a map subscript answers `T?` — the same `at` twin the
        // method side uses, so the emitter needs no types to tell
        // them apart, and an absent map value prints as nothing.
        ExprKind::Index { receiver, index } => {
            let xs = lower_view_display(receiver, cx)?;
            // A repeater binds its row by reference and `at` takes the
            // key by value, so the index is cloned — free for an Int
            // and a refcount bump for a `Str`.
            let i = format!("({}).clone()", lower_view_display(index, cx)?);
            let read = format!("({xs}).at({i})");
            Ok(match view_value_ty(receiver, cx) {
                Some(RustTy::Map(..)) => format!("__pixie_show_opt({read})"),
                _ => read,
            })
        }
        _ => err(e.span, "this expression is not lowerable in views yet (M0)"),
    }
}

/// Lower a `text:` property value to a `Str` expression.
fn lower_view_text(e: &Expr, cx: &ViewCtx) -> Result<String, EmitError> {
    match &e.kind {
        ExprKind::Str(parts) => lower_interp(parts, &mut |inner| lower_view_display(inner, cx)),
        // Routed to `lower_view_display` for its explanation of why
        // a view body cannot call a method (§8.53).
        ExprKind::MethodCall { .. } => lower_view_display(e, cx),
        ExprKind::Ident(n) if cx.is_loop_var(n) => {
            Ok(format!("{}.clone()", camel_to_snake(n)))
        }
        // `text: name` — a String prop / state-cell read (state cells
        // arrive here as `__pixie_state.name` after the desugar).
        ExprKind::Member { receiver, name } => {
            if !matches!(&receiver.kind, ExprKind::Ident(f) if cx.handle_for(f).is_some()) {
                // Through an object chain, same rule as a direct prop.
                if let Some((read, ty)) = cx.object_prop_read(receiver, &name.name) {
                    if ty != RustTy::Str {
                        return err(e.span, "this text binding must be a String property");
                    }
                    return Ok(read);
                }
            }
            let ExprKind::Ident(f) = &receiver.kind else {
                return err(e.span, "`text:` must be a string, a loop variable, or a String property");
            };
            let Some((class, handle)) = cx.handle_for(f) else {
                return err(e.span, format!("`{f}` is not a view state field or global"));
            };
            let Some(p) = class.prop(&name.name) else {
                return err(e.span, format!("no property `{}` on `{}`", name.name, class.name));
            };
            if p.ty != RustTy::Str {
                return err(e.span, "this text binding must be a String property");
            }
            Ok(format!("{handle}.{}(w)", p.rust))
        }
        _ => err(e.span, "`text:` must be a string, a loop variable, or a String property"),
    }
}

/// Lower a Float property (`value:` on ProgressBar, `width:`/`height:`
/// on Image/Svg/the charts, `size:` on Spinner) to an `f64` expression.
/// Mirrors `lower_view_text`'s shape: a literal arm plus a Member arm
/// that only accepts a Float-typed property. `key` names the property
/// in the errors so a mistyped `width:` does not read as `value:`.
fn lower_view_float(e: &Expr, cx: &ViewCtx, key: &str) -> Result<String, EmitError> {
    match &e.kind {
        ExprKind::Float(v) => Ok(format!("{v}f64")),
        // `fontSize: 14` — an Int in a Float slot. The checker widens
        // it, so the view layer widens with it (§8.55). Writing the
        // `.0` is not something a size property should demand.
        ExprKind::Int(v) => Ok(format!("{v}f64")),
        // `value: name.prop` — a Float prop / state-cell read.
        ExprKind::Member { receiver, name } => {
            let ExprKind::Ident(f) = &receiver.kind else {
                return err(e.span, format!("`{key}:` must be a number or a numeric property"));
            };
            let Some((class, handle)) = cx.handle_for(f) else {
                return err(e.span, format!("`{f}` is not a view state field or global"));
            };
            let Some(p) = class.prop(&name.name) else {
                return err(e.span, format!("no property `{}` on `{}`", name.name, class.name));
            };
            match p.ty {
                RustTy::Float => Ok(format!("{handle}.{}(w)", p.rust)),
                // An Int-typed prop widens the same way a literal does.
                RustTy::Int => Ok(format!("({}.{}(w) as f64)", handle, p.rust)),
                _ => err(e.span, format!("this {key} binding must be a numeric property")),
            }
        }
        // `fontSize: unit * qty` — arithmetic over numbers and numeric
        // reads. The interpreting tier evaluates the property with the
        // ordinary expression evaluator, so the compiled one lowers it
        // too; anything else stays a named error.
        ExprKind::Binary { op, lhs, rhs } => {
            let l = lower_view_float(lhs, cx, key)?;
            let r = lower_view_float(rhs, cx, key)?;
            Ok(format!("({l} {} {r})", bin_op(op, e.span)?))
        }
        _ => err(e.span, format!("`{key}:` must be a number, a numeric property, or arithmetic over them")),
    }
}

/// Lower a Float property that must be a property READ (the Slider's
/// `value:`) — `lower_view_float`'s Member arm and nothing else, the
/// way the charts' `data:` restricts itself: a literal could never
/// reflect state across rebuilds, so it is a named error rather than
/// a frozen control.
fn lower_view_float_prop(e: &Expr, cx: &ViewCtx, key: &str) -> Result<String, EmitError> {
    match &e.kind {
        ExprKind::Member { .. } => lower_view_float(e, cx, key),
        _ => err(
            e.span,
            format!(
                "`{key}:` must be a Float property (a literal cannot reflect \
                 state — bind a store prop or state cell)"
            ),
        ),
    }
}

/// Lower an Int property (`columns:`, `colSpan:`, `rowSpan:`) —
/// `lower_view_float`'s shape, one primitive over.
fn lower_view_int(e: &Expr, cx: &ViewCtx, key: &str) -> Result<String, EmitError> {
    match &e.kind {
        ExprKind::Int(v) => Ok(format!("{v}i64")),
        // `columns: name.prop` — an Int prop / state-cell read.
        ExprKind::Member { receiver, name } => {
            let ExprKind::Ident(f) = &receiver.kind else {
                return err(e.span, format!("`{key}:` must be an int literal or an Int property"));
            };
            let Some((class, handle)) = cx.handle_for(f) else {
                return err(e.span, format!("`{f}` is not a view state field or global"));
            };
            let Some(p) = class.prop(&name.name) else {
                return err(e.span, format!("no property `{}` on `{}`", name.name, class.name));
            };
            if p.ty != RustTy::Int {
                return err(e.span, format!("this {key} binding must be an Int property"));
            }
            Ok(format!("{handle}.{}(w)", p.rust))
        }
        ExprKind::Binary { op, lhs, rhs } => {
            let l = lower_view_int(lhs, cx, key)?;
            let r = lower_view_int(rhs, cx, key)?;
            Ok(format!("({l} {} {r})", bin_op(op, e.span)?))
        }
        _ => err(e.span, format!("`{key}:` must be an int literal, an Int property, or arithmetic over them")),
    }
}

/// `lower_view_float_prop`'s Int twin (the IntField's `value:`): the
/// Member arm and nothing else, because a literal could never reflect
/// state across rebuilds.
fn lower_view_int_prop(e: &Expr, cx: &ViewCtx, key: &str) -> Result<String, EmitError> {
    match &e.kind {
        ExprKind::Member { .. } => lower_view_int(e, cx, key),
        _ => err(
            e.span,
            format!(
                "`{key}:` must be an Int property (a literal cannot reflect \
                 state — bind a store prop or state cell)"
            ),
        ),
    }
}

/// The optional `width:`/`height:` pair every sized leaf shares, as a
/// lowered `(width, height)` — `0f64` for an axis the view left unset.
fn lower_view_size(el: &Element, cx: &ViewCtx) -> Result<(String, String), EmitError> {
    let width = match element_prop(el, "width") {
        Some(v) => lower_view_float(v, cx, "width")?,
        None => "0f64".into(),
    };
    let height = match element_prop(el, "height") {
        Some(v) => lower_view_float(v, cx, "height")?,
        None => "0f64".into(),
    };
    Ok((width, height))
}

/// Lower an `open:` property (Modal) to a `bool` expression.
/// Mirrors `lower_view_float`'s shape: a literal arm plus a Member arm
/// that only accepts a Bool-typed property.
fn lower_view_bool(e: &Expr, cx: &ViewCtx) -> Result<String, EmitError> {
    lower_view_bool_keyed(e, cx, "open")
}

/// The keyed body behind `lower_view_bool` — `key` names the property
/// in the errors (`lower_view_float`'s rule), so the toggles' required
/// `checked:` does not misreport itself as Modal's `open:`.
fn lower_view_bool_keyed(e: &Expr, cx: &ViewCtx, key: &str) -> Result<String, EmitError> {
    match &e.kind {
        ExprKind::Bool(v) => Ok(format!("{v}")),
        // `open: name.prop` / `checked: name.prop` — a Bool prop /
        // state-cell read.
        ExprKind::Member { receiver, name } => {
            let ExprKind::Ident(f) = &receiver.kind else {
                return err(e.span, format!("`{key}:` must be a bool literal or a Bool property"));
            };
            let Some((class, handle)) = cx.handle_for(f) else {
                return err(e.span, format!("`{f}` is not a view state field or global"));
            };
            let Some(p) = class.prop(&name.name) else {
                return err(e.span, format!("no property `{}` on `{}`", name.name, class.name));
            };
            if p.ty != RustTy::Bool {
                return err(e.span, format!("this {key} binding must be a Bool property"));
            }
            Ok(format!("{handle}.{}(w)", p.rust))
        }
        _ => err(e.span, format!("`{key}:` must be a bool literal or a Bool property")),
    }
}

/// Lower a `data:` property (BarChart / LineChart) to a `List<f64>`
/// expression. Mirrors `lower_view_text`'s Member arm — and only that
/// arm: a list literal in a view body has no lowering yet, so chart
/// data must come from a bound `List<Float>` prop or state cell.
fn lower_view_float_list(e: &Expr, cx: &ViewCtx) -> Result<String, EmitError> {
    match &e.kind {
        // `data: name.prop` — a List<Float> prop / state-cell read.
        ExprKind::Member { receiver, name } => {
            let ExprKind::Ident(f) = &receiver.kind else {
                return err(e.span, "`data:` must be a List<Float> property");
            };
            let Some((class, handle)) = cx.handle_for(f) else {
                return err(e.span, format!("`{f}` is not a view state field or global"));
            };
            let Some(p) = class.prop(&name.name) else {
                return err(e.span, format!("no property `{}` on `{}`", name.name, class.name));
            };
            // List<Int> widens element by element (§8.55) — the
            // interp tier's as_float always accepted it, so refusing
            // here was a tier disagreement, not a rule.
            if p.ty == RustTy::List(Box::new(RustTy::Int)) {
                return Ok(format!(
                    "{handle}.{}(w).iter().map(|v| *v as f64).collect::<List<f64>>()",
                    p.rust
                ));
            }
            if p.ty != RustTy::List(Box::new(RustTy::Float)) {
                return err(e.span, "this data binding must be a List<Float> or List<Int> property");
            }
            Ok(format!("{handle}.{}(w)", p.rust))
        }
        _ => err(
            e.span,
            "`data:` must be a List<Float> property (list literals are not \
             lowerable in views yet — bind a store prop or state cell)",
        ),
    }
}

/// Lower a `List<String>` property (`labels:` on the charts and
/// TabBar, `options:` on Select / RadioGroup) to a `List<Str>`
/// expression. The `List<String>` twin of `lower_view_float_list`;
/// `key` names the property in the errors (the `lower_view_float`
/// rule — a mistyped `options:` must not read as `labels:`).
fn lower_view_str_list(e: &Expr, cx: &ViewCtx, key: &str) -> Result<String, EmitError> {
    match &e.kind {
        // `labels: name.prop` — a List<String> prop / state-cell read.
        ExprKind::Member { receiver, name } => {
            let ExprKind::Ident(f) = &receiver.kind else {
                return err(e.span, format!("`{key}:` must be a List<String> property"));
            };
            let Some((class, handle)) = cx.handle_for(f) else {
                return err(e.span, format!("`{f}` is not a view state field or global"));
            };
            let Some(p) = class.prop(&name.name) else {
                return err(e.span, format!("no property `{}` on `{}`", name.name, class.name));
            };
            if p.ty != RustTy::List(Box::new(RustTy::Str)) {
                return err(e.span, format!("this {key} binding must be a List<String> property"));
            }
            Ok(format!("{handle}.{}(w)", p.rust))
        }
        // A literal list of strings: `options: ["a", "b"]`. A view
        // rebuild evaluates it again, which is exactly what a literal
        // in the source says.
        ExprKind::Array(items) => {
            let mut out = String::from("{ let mut __lit = List::<Str>::new(); ");
            for item in items {
                let v = lower_view_text(item, cx)?;
                write!(out, "__lit.push({v}); ").unwrap();
            }
            out.push_str("__lit }");
            Ok(out)
        }
        _ => err(
            e.span,
            format!(
                "`{key}:` must be a List<String> property or a list of string \
                 literals — bind a store prop or state cell for anything else"
            ),
        ),
    }
}

/// Statement context inside an action closure (`onClick: { ... }`).
/// Resolution mirrors method bodies minus `self`: names are locals,
/// view fields (captured Copy handles), or globals.
struct ActionCtx<'a, 'v> {
    view: &'v ViewCtx<'a>,
    locals: Vec<String>,
    /// Handler locals that hold an OBJECT, and its class (§8.53).
    /// `let c = C()` binds a handle, so `c.n = 5` and `S.take(c)`
    /// have to dispatch through the World rather than read a value.
    local_objects: Vec<(String, String)>,
}

/// The class a handler expression denotes when it denotes an object.
fn action_object_class(e: &Expr, cx: &ActionCtx) -> Option<String> {
    match &e.kind {
        ExprKind::Call { callee, .. } => match &callee.kind {
            ExprKind::Ident(c) if cx.view.classes.contains_key(c) => Some(c.clone()),
            _ => None,
        },
        ExprKind::Ident(n) => cx
            .local_objects
            .iter()
            .find(|(l, _)| l == n)
            .map(|(_, c)| c.clone())
            .or_else(|| cx.view.fields.get(n).cloned()),
        _ => None,
    }
}

fn lower_action_expr(e: &Expr, cx: &ActionCtx) -> Result<String, EmitError> {
    Ok(cast_if_widened(&lower_action_expr_inner(e, cx)?, e.span))
}

fn lower_action_expr_inner(e: &Expr, cx: &ActionCtx) -> Result<String, EmitError> {
    match &e.kind {
        ExprKind::Int(v) => Ok(format!("{v}i64")),
        ExprKind::Float(v) => Ok(format!("{v}f64")),
        ExprKind::Bool(v) => Ok(format!("{v}")),
        ExprKind::Str(parts) => lower_interp(parts, &mut |inner| lower_action_expr(inner, cx)),
        ExprKind::Ident(n) if cx.locals.iter().any(|l| l == n) => {
            Ok(format!("{}.clone()", camel_to_snake(n)))
        }
        // A view's state OBJECT, by name (§8.47). An action can hand
        // one to a method or push it into a list now — the handle is
        // Copy, and the view's own edge table keeps it alive whatever
        // the receiving container later does with its own reference.
        ExprKind::Ident(n)
            if cx.view.fields.contains_key(n)
                || cx.local_objects.iter().any(|(l, _)| l == n) =>
        {
            Ok(camel_to_snake(n))
        }
        // Constructing an object in a handler (§8.53). The World is
        // right there — a handler is `Fn(&mut World)` — so the only
        // thing that was missing is this arm.
        ExprKind::Call { callee, args, block: None, .. }
            if matches!(&callee.kind, ExprKind::Ident(c) if cx.view.classes.contains_key(c)) =>
        {
            let ExprKind::Ident(class_name) = &callee.kind else {
                unreachable!("guarded")
            };
            let info = &cx.view.classes[class_name];
            let want = info.init.map(|i| i.params.len()).unwrap_or(0);
            if args.len() != want {
                return err(
                    e.span,
                    format!("`{class_name}` takes {want} constructor argument(s)"),
                );
            }
            // Arguments hoist (the §11.20 rule): each one may itself
            // touch the World, and nesting them inside `w.insert`'s
            // call would take two mutable borrows at once.
            let mut lowered = Vec::with_capacity(args.len());
            for a in args {
                lowered.push(lower_action_expr(a, cx)?);
            }
            if lowered.is_empty() {
                return Ok(format!("w.insert({class_name}::new())"));
            }
            let mut out = String::from("{ ");
            let mut names = Vec::with_capacity(lowered.len());
            for (i, v) in lowered.iter().enumerate() {
                write!(out, "let __c{i} = {v}; ").unwrap();
                names.push(format!("__c{i}"));
            }
            write!(out, "w.insert({class_name}::new({})) }}", names.join(", ")).unwrap();
            Ok(out)
        }
        // A method call that produces a VALUE — `S.label()` in a
        // handler, the same call the view body can already make.
        ExprKind::MethodCall { receiver, method, args, block: None, .. } => {
            // On a handler local that holds an object.
            if let Some(c) = action_object_class(receiver, cx) {
                if let Some(info) = cx.view.classes.get(&c) {
                    if info.methods.iter().any(|m| m.name.name == method.name) {
                        let h = lower_action_expr(receiver, cx)?;
                        let head =
                            format!("({h}).{}(w", camel_to_snake(&method.name));
                        let mut lowered = Vec::with_capacity(args.len());
                        for a in args {
                            lowered.push(lower_action_expr(a, cx)?);
                        }
                        return Ok(finish_w_call(&head, lowered));
                    }
                }
            }
            let ExprKind::Ident(r) = &receiver.kind else {
                return err(e.span, "this method call is not lowerable in actions yet (M0)");
            };
            let Some((class, handle)) = cx.view.handle_for(r) else {
                return err(e.span, format!("`{r}` is not a view state field or global"));
            };
            if !class.methods.iter().any(|m| m.name.name == method.name) {
                return err(
                    e.span,
                    format!("no method `{}` on `{}`", method.name, class.name),
                );
            }
            let head = format!("{handle}.{}(w", camel_to_snake(&method.name));
            let mut lowered = Vec::with_capacity(args.len());
            for a in args {
                lowered.push(lower_action_expr(a, cx)?);
            }
            Ok(finish_w_call(&head, lowered))
        }
        ExprKind::Member { receiver, name } => {
            if name.name == "length" {
                let inner = lower_action_expr(receiver, cx)?;
                return Ok(format!("({inner}.len() as i64)"));
            }
            if let ExprKind::Ident(r) = &receiver.kind {
                if let Some((class, handle)) = cx.view.handle_for(r) {
                    let Some(p) = class.prop(&name.name) else {
                        return err(
                            e.span,
                            format!("no property `{}` on `{}`", name.name, class.name),
                        );
                    };
                    return Ok(format!("{handle}.{}(w)", p.rust));
                }
            }
            // Through a handler local that holds an object.
            if let Some(c) = action_object_class(receiver, cx) {
                if let Some(info) = cx.view.classes.get(&c) {
                    if let Some(pi) = info.prop(&name.name) {
                        let h = lower_action_expr(receiver, cx)?;
                        return Ok(format!("({h}).{}(w)", pi.rust));
                    }
                }
            }
            // Through an OBJECT chain, the same reach a method body
            // has (§8.41's `object_prop_read`).
            if let Some((read, _)) = cx.view.object_prop_read(receiver, &name.name) {
                return Ok(read);
            }
            err(e.span, "this member access is not lowerable in actions yet (M0)")
        }
        ExprKind::Binary { op, lhs, rhs } => {
            let l = lower_action_expr(lhs, cx)?;
            let r = lower_action_expr(rhs, cx)?;
            Ok(format!("({l} {} {r})", bin_op(op, e.span)?))
        }
        ExprKind::Unary { op, expr } => {
            let inner = lower_action_expr(expr, cx)?;
            match op {
                UnaryOp::Neg => Ok(format!("(-{inner})")),
                UnaryOp::Not => Ok(format!("(!{inner})")),
            }
        }
        ExprKind::Array(items) => {
            if items.is_empty() {
                return Ok("List::new()".into());
            }
            let mut out = String::from("{ let mut __lit = List::new(); ");
            for item in items {
                let v = lower_action_expr(item, cx)?;
                write!(out, "__lit.push({v}); ").unwrap();
            }
            out.push_str("__lit }");
            Ok(out)
        }
        _ => err(e.span, "this expression is not lowerable in actions yet (M0)"),
    }
}

fn lower_action_stmt(
    s: &Stmt,
    cx: &mut ActionCtx,
    out: &mut String,
    ind: &str,
) -> Result<(), EmitError> {
    match s {
        Stmt::Let {
            name, ty, value, ..
        }
        | Stmt::Var {
            name, ty, value, ..
        } => {
            let is_var = matches!(s, Stmt::Var { .. });
            let v = lower_action_expr(value, cx)?;
            let ann = match ty {
                Some(t) => format!(": {}", lower_type(t, cx.view.class_names)?.render()),
                None => String::new(),
            };
            let rn = camel_to_snake(&name.name);
            let mutkw = if is_var { "mut " } else { "" };
            writeln!(out, "{ind}let {mutkw}{rn}{ann} = {v};").unwrap();
            if let Some(c) = action_object_class(value, cx) {
                cx.local_objects.push((name.name.clone(), c));
            }
            cx.locals.push(name.name.clone());
            Ok(())
        }
        Stmt::Assign {
            target,
            op,
            value,
            span,
        } => {
            // A LOCAL the action itself declared (§8.53). `var i = 0`
            // was lowerable and `i = i + 1` was not, which is not a
            // constraint so much as a missing arm — a loop counter is
            // the first thing anyone writes after `while`.
            if let ExprKind::Ident(n) = &target.kind {
                if cx.locals.iter().any(|l| l == n) {
                    let v = lower_action_expr(value, cx)?;
                    let rn = camel_to_snake(n);
                    match op {
                        AssignOp::Eq => writeln!(out, "{ind}{rn} = {v};").unwrap(),
                        _ => {
                            let sym = match op {
                                AssignOp::PlusEq => "+",
                                AssignOp::MinusEq => "-",
                                AssignOp::StarEq => "*",
                                AssignOp::SlashEq => "/",
                                AssignOp::Eq => unreachable!(),
                            };
                            writeln!(out, "{ind}{rn} = {rn} {sym} {v};").unwrap();
                        }
                    }
                    return Ok(());
                }
            }
            // `c.prop = v` where `c` is a handler local holding an
            // object: the ordinary setter, same as a method body.
            if let ExprKind::Member { receiver, name } = &target.kind {
                if let Some(c) = action_object_class(receiver, cx) {
                    if !cx.view.classes.contains_key(&c) {
                        return err(*span, format!("unknown class `{c}`"));
                    }
                    let info = &cx.view.classes[&c];
                    let Some(pi) = info.prop(&name.name) else {
                        return err(
                            *span,
                            format!("no property `{}` on `{}`", name.name, info.name),
                        );
                    };
                    check_assignable(pi, &info.name, *span)?;
                    let h = lower_action_expr(receiver, cx)?;
                    let v = lower_action_expr(value, cx)?;
                    match op {
                        AssignOp::Eq => writeln!(
                            out,
                            "{ind}{{ let __o = {h}; let __v = {v}; __o.set_{}(w, __v); }}",
                            pi.rust
                        )
                        .unwrap(),
                        _ => {
                            let sym = match op {
                                AssignOp::PlusEq => "+",
                                AssignOp::MinusEq => "-",
                                AssignOp::StarEq => "*",
                                AssignOp::SlashEq => "/",
                                AssignOp::Eq => unreachable!(),
                            };
                            writeln!(
                                out,
                                "{ind}{{ let __o = {h}; let __v = __o.{g}(w) {sym} {v}; __o.set_{g}(w, __v); }}",
                                g = pi.rust
                            )
                            .unwrap();
                        }
                    }
                    return Ok(());
                }
            }
            let ExprKind::Member { receiver, name } = &target.kind else {
                return err(
                    *span,
                    "an action assigns to a local it declared, or to `field.prop` / `Global.prop`",
                );
            };
            let ExprKind::Ident(r) = &receiver.kind else {
                return err(*span, "actions assign to `field.prop` / `Global.prop` (M1)");
            };
            let Some((class, handle)) = cx.view.handle_for(r) else {
                return err(*span, format!("`{r}` is not a view state field or global"));
            };
            let Some(p) = class.prop(&name.name) else {
                return err(
                    *span,
                    format!("no property `{}` on `{}`", name.name, class.name),
                );
            };
            check_assignable(p, &class.name, *span)?;
            let v = lower_action_expr(value, cx)?;
            match op {
                AssignOp::Eq => {
                    writeln!(out, "{ind}{handle}.set_{}(w, {v});", p.rust).unwrap();
                }
                _ => {
                    let sym = match op {
                        AssignOp::PlusEq => "+",
                        AssignOp::MinusEq => "-",
                        AssignOp::StarEq => "*",
                        AssignOp::SlashEq => "/",
                        AssignOp::Eq => unreachable!(),
                    };
                    writeln!(
                        out,
                        "{ind}{{ let __v = {handle}.{g}(w) {sym} {v}; {handle}.set_{g}(w, __v); }}",
                        g = p.rust
                    )
                    .unwrap();
                }
            }
            Ok(())
        }
        // `case` in a handler (§8.69), and therefore `if let`. A
        // handler is a method body written at the use site (§8.53),
        // and a method body matches — so this one does too. The
        // shapes are the ones a handler can name: a `T?` property and
        // an enum one.
        Stmt::Expr(e) if matches!(e.kind, ExprKind::Case { .. }) => {
            let ExprKind::Case { scrutinee, arms } = &e.kind else {
                unreachable!("guarded")
            };
            let Some((scrut, scrut_ty)) = cx.view.struct_value_read(scrutinee) else {
                return err(
                    scrutinee.span,
                    "a handler matches a `T?` property or an enum one — for anything else, \
                     call a method that does the matching",
                );
            };
            let inner = format!("{ind}    ");
            match scrut_ty {
                RustTy::Opt(elem) => {
                    let (bind, some_body, none_body) = split_opt_arms(arms, e.span)?;
                    let binder = bind
                        .clone()
                        .map(|b| camel_to_snake(&b))
                        .unwrap_or_else(|| "_".to_string());
                    writeln!(out, "{ind}match {scrut} {{").unwrap();
                    writeln!(out, "{ind}    Some({binder}) => {{").unwrap();
                    let depth = cx.locals.len();
                    if let Some(b) = bind {
                        cx.locals.push(b.clone());
                        if let RustTy::Handle(c) = &*elem {
                            cx.local_objects.push((b, c.clone()));
                        }
                    }
                    lower_action_block(some_body, cx, out, &format!("{inner}    "))?;
                    cx.locals.truncate(depth);
                    writeln!(out, "{ind}    }}").unwrap();
                    writeln!(out, "{ind}    None => {{").unwrap();
                    lower_action_block(none_body, cx, out, &format!("{inner}    "))?;
                    writeln!(out, "{ind}    }}").unwrap();
                    writeln!(out, "{ind}}}").unwrap();
                }
                RustTy::Named(n) if cx.view.enums.contains_key(&n) => {
                    let en = &cx.view.enums[&n];
                    writeln!(out, "{ind}match {scrut} {{").unwrap();
                    for arm in arms {
                        let pat = match &arm.pattern {
                            ast::Pattern::Wild { .. } => "_".to_string(),
                            ast::Pattern::Ctor { name, args, span } => {
                                if !args.is_empty() {
                                    return err(
                                        *span,
                                        "a handler arm matches a variant by name — a payload \
                                         needs a method to read it",
                                    );
                                }
                                if en.variant(&name.name).is_none() {
                                    return err(
                                        *span,
                                        format!("no variant `{}` on `{n}`", name.name),
                                    );
                                }
                                format!("{n}::{}", escape_rust_keyword(name.name.clone()))
                            }
                            other => {
                                return err(
                                    view_pattern_span(other),
                                    "a handler arm matches a variant name or `_`",
                                );
                            }
                        };
                        writeln!(out, "{ind}    {pat} => {{").unwrap();
                        lower_action_block(&arm.body, cx, out, &format!("{inner}    "))?;
                        writeln!(out, "{ind}    }}").unwrap();
                    }
                    writeln!(out, "{ind}    #[allow(unreachable_patterns)] _ => {{}}").unwrap();
                    writeln!(out, "{ind}}}").unwrap();
                }
                _ => {
                    return err(
                        scrutinee.span,
                        "a handler matches a `T?` property or an enum one — for anything \
                         else, call a method that does the matching",
                    );
                }
            }
            Ok(())
        }
        Stmt::Expr(e) if matches!(e.kind, ExprKind::If { .. }) => {
            let ExprKind::If {
                cond,
                then_b,
                else_b,
                let_binding,
            } = &e.kind
            else {
                unreachable!("guarded")
            };
            if let_binding.is_some() {
                return err(e.span, "`if let` survived the desugar (§8.69) — this is a pixie bug");
            }
            let c = lower_action_expr(cond, cx)?;
            let inner = format!("{ind}    ");
            writeln!(out, "{ind}if {c} {{").unwrap();
            let depth = cx.locals.len();
            lower_action_block(then_b, cx, out, &inner)?;
            cx.locals.truncate(depth);
            if let Some(eb) = else_b {
                writeln!(out, "{ind}}} else {{").unwrap();
                lower_action_block(eb, cx, out, &inner)?;
                cx.locals.truncate(depth);
            }
            writeln!(out, "{ind}}}").unwrap();
            Ok(())
        }
        Stmt::Expr(e) => {
            // `field.list.push(x)` / `Global.list.push(x)` write-back.
            if let ExprKind::MethodCall {
                receiver,
                method,
                args,
                block: None,
                ..
            } = &e.kind
            {
                if method.name == "push" {
                    if let ExprKind::Member {
                        receiver: r2,
                        name: pname,
                    } = &receiver.kind
                    {
                        if let ExprKind::Ident(r) = &r2.kind {
                            if let Some((class, handle)) = cx.view.handle_for(r) {
                                if let Some(p) = class.prop(&pname.name) {
                                    if args.len() != 1 {
                                        return err(e.span, "`push` takes one argument");
                                    }
                                    let v = lower_action_expr(&args[0], cx)?;
                                    // The property's own `push`: one
                                    // call, no read-modify-write, so
                                    // an action filling a list is
                                    // linear like a method's is.
                                    writeln!(
                                        out,
                                        "{ind}{{ let __h = {handle}; let __v = {v}; __h.push_{g}(w, __v); }}",
                                        g = p.rust
                                    )
                                    .unwrap();
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
                // A method call in STATEMENT position on a handler
                // local holding an object — `c.bump()` (§8.53). The
                // expression path already handled it; this is the
                // same call with its result discarded.
                if action_object_class(receiver, cx).is_some() {
                    let call = lower_action_expr(e, cx)?;
                    writeln!(out, "{ind}{call};").unwrap();
                    return Ok(());
                }
                if let ExprKind::Ident(r) = &receiver.kind {
                    if let Some((class, handle)) = cx.view.handle_for(r) {
                        if !class.methods.iter().any(|m| m.name.name == method.name) {
                            return err(
                                e.span,
                                format!("no method `{}` on `{}`", method.name, class.name),
                            );
                        }
                        // Same hoist as everywhere else a call takes
                        // the World: an argument may touch it too.
                        let head =
                            format!("{handle}.{}(w", camel_to_snake(&method.name));
                        let mut lowered = Vec::with_capacity(args.len());
                        for a in args {
                            lowered.push(lower_action_expr(a, cx)?);
                        }
                        writeln!(out, "{ind}{};", finish_w_call(&head, lowered)).unwrap();
                        return Ok(());
                    }
                }
            }
            err(e.span, "this statement is not lowerable in actions yet (M0)")
        }
        // Control flow in an action body (§8.53). A handler is a
        // method body that happens to be written at the use site, so
        // the same statements belong in both. These were an M1 error
        // with no reason anyone had written down.
        Stmt::For {
            binding,
            index,
            iter,
            body,
            span,
        } => {
            let rb = camel_to_snake(&binding.name);
            if index.is_some() {
                writeln!(out, "{ind}let mut __turn = 0i64;").unwrap();
            }
            match &iter.kind {
                ExprKind::Range {
                    start,
                    end,
                    inclusive,
                } => {
                    let a = lower_action_expr(start, cx)?;
                    let b = lower_action_expr(end, cx)?;
                    let op = if *inclusive { "..=" } else { ".." };
                    writeln!(out, "{ind}for {rb} in ({a}){op}({b}) {{").unwrap();
                }
                _ => {
                    let xs = lower_action_expr(iter, cx)?;
                    writeln!(out, "{ind}{{ let __xs = {xs}; for __it in __xs.iter() {{").unwrap();
                    writeln!(out, "{ind}    let {rb} = __it.clone();").unwrap();
                }
            }
            if let Some(i) = index {
                writeln!(out, "{ind}    let {} = __turn; __turn += 1;", camel_to_snake(&i.name)).unwrap();
            }
            let depth = cx.locals.len();
            cx.locals.push(binding.name.clone());
            if let Some(i) = index {
                cx.locals.push(i.name.clone());
            }
            let inner = format!("{ind}    ");
            lower_action_block(body, cx, out, &inner)?;
            cx.locals.truncate(depth);
            if matches!(iter.kind, ExprKind::Range { .. }) {
                writeln!(out, "{ind}}}").unwrap();
            } else {
                writeln!(out, "{ind}}} }}").unwrap();
            }
            let _ = span;
            Ok(())
        }
        Stmt::While { cond, body, .. } => {
            let c = lower_action_expr(cond, cx)?;
            writeln!(out, "{ind}while {c} {{").unwrap();
            let depth = cx.locals.len();
            lower_action_block(body, cx, out, &format!("{ind}    "))?;
            cx.locals.truncate(depth);
            writeln!(out, "{ind}}}").unwrap();
            Ok(())
        }
        Stmt::Break { .. } => {
            writeln!(out, "{ind}break;").unwrap();
            Ok(())
        }
        Stmt::Continue { .. } => {
            writeln!(out, "{ind}continue;").unwrap();
            Ok(())
        }
        // `return` in a handler is an early exit (§8.62) — the same
        // thing it means in a method whose body runs for effect. It
        // has nothing to return TO, so a value is a named error
        // rather than a silently dropped expression.
        Stmt::Return { value: None, .. } => {
            writeln!(out, "{ind}return;").unwrap();
            Ok(())
        }
        Stmt::Return { value: Some(v), .. } => err(
            v.span,
            "a handler runs for effect and returns nothing — write a bare `return` to \
             stop early, or store the value in a property",
        ),
        // `emit` names a signal on `self`, and a handler has no self:
        // it acts on objects by name.
        Stmt::Emit { signal, span, .. } => err(
            *span,
            format!(
                "`emit` sends a signal from the object that owns it, and a handler is \
                 not inside one. Call a method that emits `{}`",
                signal.name
            ),
        ),
        Stmt::Batch { span, .. } => err(
            *span,
            "writes are already batched: no view rebuilds until the handler returns, and \
             writing one property twice notifies once. Drop the `batch` block",
        ),
    }
}

/// A block of action statements plus its trailing expression.
fn lower_action_block(
    b: &ast::Block,
    cx: &mut ActionCtx,
    out: &mut String,
    ind: &str,
) -> Result<(), EmitError> {
    for st in &b.stmts {
        lower_action_stmt(st, cx, out, ind)?;
    }
    if let Some(t) = &b.trailing {
        let stmt = Stmt::Expr((**t).clone());
        lower_action_stmt(&stmt, cx, out, ind)?;
    }
    Ok(())
}

/// Lower an `onClick:` value to a `Listener` expression.
fn lower_view_action(e: &Expr, cx: &ViewCtx) -> Result<String, EmitError> {
    lower_view_action_with(e, cx, "onClick", &[])
}

/// Lower a handler value to an `Rc` closure. Two shapes: a direct
/// method call on a field/global, or a `{ ... }` block of action
/// statements. `params` are the handler's implicit arguments (e.g.
/// `text: Str` for `onTextChanged`), in scope in either shape.
/// Closures capture only Copy handles and values — the D10 rule holds
/// in generated views.
fn lower_view_action_with(
    e: &Expr,
    cx: &ViewCtx,
    key: &str,
    params: &[(&str, &str)],
) -> Result<String, EmitError> {
    let mut sig = String::from("move |w: &mut World");
    for (n, t) in params {
        write!(sig, ", {}: {t}", camel_to_snake(n)).unwrap();
    }
    sig.push('|');
    // The handler's own parameters, plus the repeater bindings in
    // scope: a row's action is built inside the loop, so the row and
    // its index are ordinary captures of the closure.
    let mut implicit: Vec<String> = params.iter().map(|(n, _)| n.to_string()).collect();
    implicit.extend(cx.loop_vars.iter().map(|(n, _, _)| n.clone()));
    match &e.kind {
        ExprKind::MethodCall {
            receiver,
            method,
            args,
            block: None,
            ..
        } => {
            let ExprKind::Ident(f) = &receiver.kind else {
                return err(e.span, format!("`{key}:` must call a state-field/global method or be a block"));
            };
            let Some((class, handle)) = cx.handle_for(f) else {
                return err(e.span, format!("`{f}` is not a view state field or global"));
            };
            if !class.methods.iter().any(|m| m.name.name == method.name) {
                return err(e.span, format!("no method `{}` on `{}`", method.name, class.name));
            }
            let acx = ActionCtx {
                view: cx,
                locals: implicit,
                local_objects: Vec::new(),
            };
            let head = format!("{handle}.{}(w", camel_to_snake(&method.name));
            let mut lowered = Vec::with_capacity(args.len());
            for a in args {
                lowered.push(lower_action_expr(a, &acx)?);
            }
            let call = finish_w_call(&head, lowered);
            Ok(format!("Rc::new({sig} {{ {call}; }})"))
        }
        ExprKind::Block(b) => {
            let mut acx = ActionCtx {
                view: cx,
                locals: implicit,
                local_objects: Vec::new(),
            };
            let mut body = String::new();
            for s in &b.stmts {
                lower_action_stmt(s, &mut acx, &mut body, "    ")?;
            }
            if let Some(trailing) = &b.trailing {
                let stmt = Stmt::Expr((**trailing).clone());
                lower_action_stmt(&stmt, &mut acx, &mut body, "    ")?;
            }
            Ok(format!("Rc::new({sig} {{\n{body}}})"))
        }
        _ => err(e.span, format!("`{key}:` must call a state-field/global method or be a block")),
    }
}

fn element_prop<'e>(el: &'e Element, key: &str) -> Option<&'e Expr> {
    el.members.iter().find_map(|m| match m {
        ElementMember::Property { key: k, value, .. } if k == key => Some(value),
        _ => None,
    })
}

/// Is the element body exactly one `for` repeater and nothing else
/// (properties aside)? Mirrors pixie-interp's `single_repeater_of` —
/// the two predicates decide lazy rows together, and the tiers
/// diverge if they disagree.
/// Ordinary `for` bodies hold as many elements as they like (§8.56).
/// A VIRTUALIZED one does not, and cannot: a lazy row is built on
/// demand as one `Element` for one index, so "one row is one element"
/// is what virtualization means rather than a lowering limit.
fn single_repeater_of(
    el: &Element,
) -> Result<Option<(&ast::Ident, Option<&ast::Ident>, &Expr, &Element)>, EmitError> {
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
                span,
            })),
            None,
        ) => {
            if !body.stmts.is_empty() {
                return err(*span, VIRTUAL_ROW_RULE);
            }
            let Some(trailing) = &body.trailing else {
                return err(*span, VIRTUAL_ROW_RULE);
            };
            let ExprKind::Element(child) = &trailing.kind else {
                return err(*span, VIRTUAL_ROW_RULE);
            };
            Ok(Some((binding, index.as_ref(), iter, child)))
        }
        _ => Ok(None),
    }
}

/// Both tiers say this, word for word.
const VIRTUAL_ROW_RULE: &str =
    "a virtualized ListView builds one element per row, so its `for` body \
     holds exactly one element — wrap several in a Column";

/// A string literal's text, when the expr is a plain literal.
fn str_lit_of(e: &Expr) -> Option<String> {
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

/// Lower one element, wrapping it in a `GridCell` when it carries the
/// universal grid-item spans. `colSpan:` / `rowSpan:` belong to the
/// PARENT grid's placement, not to the element's own vocabulary, so
/// they are stripped here and every element gets them for free; in a
/// non-grid parent the cell is inert (the engine's flex containers
/// ignore span props, exactly as CSS does).
fn lower_element(el: &Element, cx: &mut ViewCtx, ind: &str) -> Result<String, EmitError> {
    let col = element_prop(el, "colSpan");
    let row = element_prop(el, "rowSpan");
    let inner = lower_element_inner(el, cx, ind)?;
    // Innermost of the three wrappers: `role:`/`label:` describe the
    // ELEMENT, and the accessibility walk reads the role off whatever
    // `Semantics` wraps directly.
    let inner = lower_semantics(el, inner, cx)?;
    // The tooltip sits just outside the semantics wrapper: it belongs
    // to the element, and what it says is not the element's own name.
    let inner = lower_tooltip(el, inner, cx)?;
    // Outside the semantics wrapper, inside the animation one: the
    // scope has to CONTAIN the element whose tokens it re-resolves.
    let inner = lower_themed(el, inner, cx)?;
    // The animation wrapper goes INSIDE the grid cell: a span belongs
    // to the grid's direct child, so a fading div between the two
    // would take the placement with it.
    let inner = lower_anim(el, inner, cx)?;
    if col.is_none() && row.is_none() {
        return Ok(inner);
    }
    let col_span = match col {
        Some(v) => lower_view_int(v, cx, "colSpan")?,
        None => "1i64".into(),
    };
    let row_span = match row {
        Some(v) => lower_view_int(v, cx, "rowSpan")?,
        None => "1i64".into(),
    };
    Ok(format!(
        "Element::GridCell {{ col_span: {col_span}, row_span: {row_span}, children: vec![{inner}] }}"
    ))
}

/// Wrap `inner` in an `Element::Themed` when the element carries the
/// universal `theme:` rider (§8.37) — the whole subtree then resolves
/// its color tokens against that palette instead of the root one.
fn lower_themed(el: &Element, inner: String, cx: &ViewCtx) -> Result<String, EmitError> {
    let Some(t) = element_prop(el, "theme") else {
        return Ok(inner);
    };
    // A literal is checked here, where a typo can still be a build
    // error. Anything else is an ordinary String expression, so an
    // app can own its theme as reactive state (`theme: App.mode`) —
    // which is the only way a view can offer a theme switcher. A name
    // the palette does not know keeps the inherited theme at runtime,
    // the same way a bad color falls back instead of aborting a frame.
    let name = match str_lit_of(t) {
        Some(name) => {
            if !theme_names().contains(&name.as_str()) {
                return err(
                    t.span,
                    format!("unknown theme `{name}` — one of {}", theme_names().join(", ")),
                );
            }
            format!("Str::from({name:?})")
        }
        None => lower_view_text(t, cx)?,
    };
    Ok(format!(
        "Element::Themed {{ theme: {name}, children: vec![{inner}] }}"
    ))
}

/// Wrap `inner` in an `Element::Tooltip` when the element carries the
/// universal `tooltip:` rider — a line the window shows when the
/// pointer rests on it, and a dumped output either way.
fn lower_tooltip(el: &Element, inner: String, cx: &ViewCtx) -> Result<String, EmitError> {
    let Some(t) = element_prop(el, "tooltip") else {
        return Ok(inner);
    };
    let text = lower_view_text(t, cx)?;
    Ok(format!(
        "Element::Tooltip {{ text: {text}, children: vec![{inner}] }}"
    ))
}

/// Wrap `inner` in an `Element::Semantics` when the element carries
/// the universal accessibility riders (§8.36). `role:` comes from a
/// closed vocabulary and is checked here; `label:` is any string
/// expression, so an icon's alt text can name what it stands for.
fn lower_semantics(el: &Element, inner: String, cx: &ViewCtx) -> Result<String, EmitError> {
    let role = element_prop(el, "role");
    // The toggles OWN `label:` — it is their visible text, and their
    // accessible name DERIVES from it — so only `role:` rides on
    // them. Letting the rider fire too would wrap every toggle in a
    // Semantics carrying the same string and shadow that derivation.
    let label = if matches!(el.name.name.as_str(), "Checkbox" | "Switch") {
        None
    } else {
        element_prop(el, "label")
    };
    if role.is_none() && label.is_none() {
        return Ok(inner);
    }
    // A literal is checked against the vocabulary here, where the
    // error can name the span. Anything else is an ordinary String
    // expression (the `theme:` rule, §8.37), so a role can be
    // computed — a list whose rows are headings or items depending on
    // the data. An unknown name at run time reports no role, the way
    // an unknown theme keeps the inherited one.
    let role_lit = match role {
        Some(e) => match str_lit_of(e) {
            Some(name) => {
                if !a11y_roles().contains(&name.as_str()) {
                    return err(
                        e.span,
                        format!("unknown role `{name}` — one of {}", a11y_roles().join(", ")),
                    );
                }
                format!("Str::from({name:?})")
            }
            None => lower_view_text(e, cx)?,
        },
        None => "Str::new()".to_string(),
    };
    let label_expr = match label {
        Some(v) => lower_view_text(v, cx)?,
        None => "Str::new()".to_string(),
    };
    Ok(format!(
        "Element::Semantics {{ role: {role_lit}, label: {label_expr}, children: vec![{inner}] }}"
    ))
}

/// Wrap `inner` in an `Element::Anim` when the element carries any of
/// the universal animation riders (§8.35). `animate:` is the one that
/// turns the machinery on — `easing:` / `enter:` / `exit:` without it
/// would silently do nothing, so they say so instead.
fn lower_anim(el: &Element, inner: String, cx: &ViewCtx) -> Result<String, EmitError> {
    let duration = element_prop(el, "animate");
    let easing = element_prop(el, "easing");
    let enter = element_prop(el, "enter");
    let exit = element_prop(el, "exit");
    if duration.is_none() {
        if let Some(e) = easing.or(enter).or(exit) {
            return err(
                e.span,
                "`easing:` / `enter:` / `exit:` describe a tween, and `animate:` is what \
                 starts one — without it there is nothing for them to shape",
            );
        }
        return Ok(inner);
    }
    let dur = lower_view_float(duration.expect("guarded"), cx, "animate")?;
    // Same rule as `role:` and `theme:`: a literal is checked here, an
    // expression is resolved at run time and falls back rather than
    // aborting a frame.
    let ease = match easing {
        Some(e) => match str_lit_of(e) {
            Some(name) => {
                let variant = easing_variant(&name).ok_or_else(|| EmitError {
                    span: e.span,
                    message: format!(
                        "unknown easing `{name}` — one of linear, in, out, inOut"
                    ),
                })?;
                format!("pixie_kernel::Easing::{variant}")
            }
            None => {
                let x = lower_view_text(e, cx)?;
                format!(
                    "pixie_kernel::Easing::parse(({x}).as_str())\
                     .unwrap_or(pixie_kernel::Easing::Out)"
                )
            }
        },
        None => "pixie_kernel::Easing::Out".to_string(),
    };
    // `enter:` / `exit:` take the same Bool grammar `open:` does, so
    // a fade can be conditional on state.
    let flag = |e: Option<&Expr>, key: &str| -> Result<String, EmitError> {
        match e {
            None => Ok("false".to_string()),
            Some(x) => lower_view_bool(x, cx).map_err(|mut err| {
                err.message = err.message.replace("`open:`", &format!("`{key}:`"));
                err
            }),
        }
    };
    let enter = flag(enter, "enter")?;
    let exit = flag(exit, "exit")?;
    Ok(format!(
        "Element::Anim {{ duration: {dur}, easing: {ease}, enter: {enter}, exit: {exit}, opacity: 1f64, children: vec![{inner}] }}"
    ))
}

/// The easing vocabulary, spelled once per tier. `pixie_interp` runs
/// `Easing::parse` on the same names; a divergence here shows up as a
/// tier difference in the gate rather than as a silent curve swap.
fn easing_variant(name: &str) -> Option<&'static str> {
    match name {
        "linear" => Some("Linear"),
        "in" => Some("In"),
        "out" => Some("Out"),
        "inOut" => Some("InOut"),
        _ => None,
    }
}

fn lower_element_inner(el: &Element, cx: &mut ViewCtx, ind: &str) -> Result<String, EmitError> {
    if !el.module_path.is_empty() {
        // Unreachable in practice (§8.62): the component splice runs
        // before either lowerer and resolves every qualified element,
        // reporting an unknown module or an unknown `view` there. Kept
        // as a guard rather than an `unreachable!` — a bug in the
        // splice should surface as a pixie error, not a panic.
        return err(
            el.span,
            format!(
                "`{}` was left qualified after component resolution — this is a pixie bug",
                el.name.name
            ),
        );
    }
    match el.name.name.as_str() {
        // Per-row component state (§8.30): bind this row's state
        // handle (ensured in `prepare`) as a local, then lower the
        // wrapped element with it in scope — bindings and action
        // closures capture the Copy handle per row.
        "__PixieRowScope" => {
            let seat = element_prop(el, "__seat")
                .and_then(str_lit_of)
                .ok_or_else(|| EmitError {
                    span: el.span,
                    message: "__PixieRowScope needs `__seat:`".into(),
                })?;
            let row = element_prop(el, "__row")
                .and_then(str_lit_of)
                .ok_or_else(|| EmitError {
                    span: el.span,
                    message: "__PixieRowScope needs `__row:`".into(),
                })?;
            let holder = element_prop(el, "__holder")
                .and_then(str_lit_of)
                .ok_or_else(|| EmitError {
                    span: el.span,
                    message: "__PixieRowScope needs `__holder:`".into(),
                })?;
            let depth: usize = element_prop(el, "__depth")
                .and_then(str_lit_of)
                .and_then(|d| d.parse().ok())
                .ok_or_else(|| EmitError {
                    span: el.span,
                    message: "__PixieRowScope needs `__depth:`".into(),
                })?;
            if depth == 0 || depth > cx.repeat_depth {
                return err(
                    el.span,
                    format!(
                        "per-row state at repeater depth {depth} lowered under {} enclosing \
                         `for`s — the component splice and the emitter disagree",
                        cx.repeat_depth
                    ),
                );
            }
            let child = el
                .members
                .iter()
                .find_map(|m| match m {
                    ElementMember::Child(c) => Some(c),
                    _ => None,
                })
                .ok_or_else(|| EmitError {
                    span: el.span,
                    message: "__PixieRowScope holds one element".into(),
                })?;
            let seat_local = camel_to_snake(&seat);
            let row_local = camel_to_snake(&row);
            let prev = cx.fields.insert(row.clone(), holder.clone());
            let inner = lower_element(child, cx, &format!("{ind}    "));
            match prev {
                Some(v) => {
                    cx.fields.insert(row.clone(), v);
                }
                None => {
                    cx.fields.remove(&row);
                }
            }
            let inner = inner?;
            let path = (0..depth)
                .map(|d| format!("__row_idx{d}"))
                .collect::<Vec<_>>()
                .join(", ");
            Ok(format!(
                "{{ let {row_local} = pixie_kernel::row_at(w, {seat_local}, &[{path}]); {inner} }}"
            ))
        }
        "Text" => {
            let text = element_prop(el, "text")
                .ok_or_else(|| EmitError {
                    span: el.span,
                    message: "Text needs `text:`".into(),
                })?;
            let font_size = match element_prop(el, "fontSize") {
                Some(v) => lower_view_float(v, cx, "fontSize")?,
                None => "0f64".into(),
            };
            let color = match element_prop(el, "color") {
                Some(v) => lower_view_text(v, cx)?,
                None => "Str::new()".into(),
            };
            let align = match element_prop(el, "align") {
                Some(v) => lower_view_text(v, cx)?,
                None => "Str::new()".into(),
            };
            let grow = match element_prop(el, "grow") {
                Some(v) => lower_view_float(v, cx, "grow")?,
                None => "0f64".into(),
            };
            Ok(format!(
                "Element::Text {{ text: {}, font_size: {font_size}, color: {color}, align: {align}, grow: {grow} }}",
                lower_view_text(text, cx)?
            ))
        }
        "Button" => {
            let label = element_prop(el, "text")
                .or_else(|| element_prop(el, "label"))
                .ok_or_else(|| EmitError {
                    span: el.span,
                    message: "Button needs `text:`".into(),
                })?;
            let action = element_prop(el, "onClick").ok_or_else(|| EmitError {
                span: el.span,
                message: "Button needs `onClick:`".into(),
            })?;
            let background = match element_prop(el, "background") {
                Some(v) => lower_view_text(v, cx)?,
                None => "Str::new()".into(),
            };
            let hover_background = match element_prop(el, "hover.background") {
                Some(v) => lower_view_text(v, cx)?,
                None => "Str::new()".into(),
            };
            let active_background = match element_prop(el, "active.background") {
                Some(v) => lower_view_text(v, cx)?,
                None => "Str::new()".into(),
            };
            let width = match element_prop(el, "width") {
                Some(v) => lower_view_float(v, cx, "width")?,
                None => "0f64".into(),
            };
            let height = match element_prop(el, "height") {
                Some(v) => lower_view_float(v, cx, "height")?,
                None => "0f64".into(),
            };
            let font_size = match element_prop(el, "fontSize") {
                Some(v) => lower_view_float(v, cx, "fontSize")?,
                None => "0f64".into(),
            };
            let color = match element_prop(el, "color") {
                Some(v) => lower_view_text(v, cx)?,
                None => "Str::new()".into(),
            };
            let grow = match element_prop(el, "grow") {
                Some(v) => lower_view_float(v, cx, "grow")?,
                None => "0f64".into(),
            };
            let basis = match element_prop(el, "basis") {
                Some(v) => lower_view_float(v, cx, "basis")?,
                None => "0f64".into(),
            };
            let boxed = lower_box_props(el, cx)?;
            Ok(format!(
                "Element::Button {{ label: {}, background: {background}, hover_background: {hover_background}, active_background: {active_background}, width: {width}, height: {height}, font_size: {font_size}, color: {color}, grow: {grow}, basis: {basis}, {boxed}, on_click: {} }}",
                lower_view_text(label, cx)?,
                lower_view_action(action, cx)?
            ))
        }
        "TextField" => {
            let value = match element_prop(el, "text") {
                Some(t) => lower_view_text(t, cx)?,
                None => "Str::new()".into(),
            };
            let placeholder = match element_prop(el, "placeholder") {
                Some(t) => lower_view_text(t, cx)?,
                None => "Str::new()".into(),
            };
            // The handler's implicit `text` argument carries the new
            // (for onTextChanged) / current (for onSubmitted) content —
            // cute_ui's `signal textChanged(text: String)` convention.
            let on_change = match element_prop(el, "onTextChanged") {
                Some(a) => format!(
                    "Some({})",
                    lower_view_action_with(a, cx, "onTextChanged", &[("text", "Str")])?
                ),
                None => "None".into(),
            };
            let on_submit = match element_prop(el, "onSubmitted") {
                Some(a) => format!(
                    "Some({})",
                    lower_view_action_with(a, cx, "onSubmitted", &[("text", "Str")])?
                ),
                None => "None".into(),
            };
            // A field that holds paragraphs. `rows` is how many lines
            // are visible; `0` means the default.
            let multiline = match element_prop(el, "multiline") {
                Some(v) => lower_view_bool_keyed(v, cx, "multiline")?,
                None => "false".into(),
            };
            let rows = match element_prop(el, "rows") {
                Some(v) => lower_view_float(v, cx, "rows")?,
                None => "0f64".into(),
            };
            Ok(format!(
                "Element::TextField {{ value: {value}, placeholder: {placeholder}, on_change: {on_change}, on_submit: {on_submit}, multiline: {multiline}, rows: {rows} }}"
            ))
        }
        "Column" | "Row" => {
            // Style props (spliced from `style:` or written directly).
            // `spacing` keeps `-1f64` = unset so `spacing: 0` can
            // honestly remove the default gap; the other two follow
            // the house sentinels. Consumed here, allowlisted in
            // `container_prop_keys`, walked past by `lower_children`.
            let spacing = match element_prop(el, "spacing") {
                Some(v) => lower_view_float(v, cx, "spacing")?,
                None => "-1f64".into(),
            };
            let padding = match element_prop(el, "padding") {
                Some(v) => lower_view_float(v, cx, "padding")?,
                None => "0f64".into(),
            };
            let background = match element_prop(el, "background") {
                Some(v) => lower_view_text(v, cx)?,
                None => "Str::new()".into(),
            };
            let grow = match element_prop(el, "grow") {
                Some(v) => lower_view_float(v, cx, "grow")?,
                None => "0f64".into(),
            };
            let boxed = lower_box_props(el, cx)?;
            let children = lower_children(el, cx, ind)?;
            Ok(format!(
                "Element::{} {{ spacing: {spacing}, padding: {padding}, background: {background}, grow: {grow}, {boxed}, children: {children} }}",
                el.name.name
            ))
        }
        "Grid" => {
            // `columns:` is the one required prop — tracks are what
            // makes a grid a grid. Everything else mirrors Column,
            // sentinels included (`spacing` `-1` = the engine default,
            // so `spacing: 0` can honestly close the gaps).
            let columns = element_prop(el, "columns").ok_or_else(|| EmitError {
                span: el.span,
                message: "Grid needs `columns:` (how many tracks wide it is)".into(),
            })?;
            let columns = lower_view_int(columns, cx, "columns")?;
            // `rows:` is optional — `0` leaves the row tracks implicit
            // (content-sized), which is what a grid that only wraps
            // wants; a grid that divides its own height sets it.
            let rows = match element_prop(el, "rows") {
                Some(v) => lower_view_int(v, cx, "rows")?,
                None => "0i64".into(),
            };
            let spacing = match element_prop(el, "spacing") {
                Some(v) => lower_view_float(v, cx, "spacing")?,
                None => "-1f64".into(),
            };
            let padding = match element_prop(el, "padding") {
                Some(v) => lower_view_float(v, cx, "padding")?,
                None => "0f64".into(),
            };
            let background = match element_prop(el, "background") {
                Some(v) => lower_view_text(v, cx)?,
                None => "Str::new()".into(),
            };
            let grow = match element_prop(el, "grow") {
                Some(v) => lower_view_float(v, cx, "grow")?,
                None => "0f64".into(),
            };
            let boxed = lower_box_props(el, cx)?;
            let children = lower_children(el, cx, ind)?;
            Ok(format!(
                "Element::Grid {{ columns: {columns}, rows: {rows}, spacing: {spacing}, padding: {padding}, background: {background}, grow: {grow}, {boxed}, children: {children} }}"
            ))
        }
        "Stack" => {
            let children = lower_children(el, cx, ind)?;
            Ok(format!("Element::Stack({children})"))
        }
        "ListView" => {
            // Container props: both optional, both consumed here before
            // `lower_children` walks the same member list (which
            // allowlists them for ListView alone).
            let virtualized = match element_prop(el, "virtualized") {
                Some(v) => lower_view_bool(v, cx)?,
                None => "false".into(),
            };
            let item_height = match element_prop(el, "itemHeight") {
                Some(v) => lower_view_float(v, cx, "itemHeight")?,
                None => "0f64".into(),
            };
            // The viewport's own height: `0f64` keeps the engine's
            // 320 px default, so an untouched list renders exactly as
            // it did before the prop existed.
            let height = match element_prop(el, "height") {
                Some(v) => lower_view_float(v, cx, "height")?,
                None => "0f64".into(),
            };
            // `grow:` — flex share of the parent instead of a fixed
            // viewport height (0f64 = unset, sized behavior).
            let grow = match element_prop(el, "grow") {
                Some(v) => lower_view_float(v, cx, "grow")?,
                None => "0f64".into(),
            };
            // A VIRTUALIZED body that is exactly one `for` repeater
            // goes lazy: rows are (re)built on demand for the range
            // gpui's uniform_list asks for. Non-virtualized lists stay
            // EAGER — every row is visible anyway, and the engine's
            // clipped-viewport path renders `children` (§8.14's lazy
            // detection briefly covered both and blanked plain lists
            // in the window; the dump materializes lazy rows, so the
            // tier gate never saw it — §8.24). The detection predicate
            // MUST match the interpreter's, or the tiers diverge.
            if virtualized == "true" {
                if let Some((binding, index, iter, child)) = single_repeater_of(el)? {
                // Same reach as the eager repeater (§8.65): any list
                // this view can name, through however many objects.
                let ExprKind::Member { receiver, name } = &iter.kind else {
                    return err(
                        iter.span,
                        "`for` iterates a list PROPERTY — name the object and the \
                         property it holds",
                    );
                };
                let (xs_expr, elem_ty) = match &receiver.kind {
                    ExprKind::Ident(f) if cx.handle_for(f).is_some() => {
                        let (class, handle) = cx.handle_for(f).expect("guarded");
                        let Some(p) = class.prop(&name.name) else {
                            return err(
                                iter.span,
                                format!("no property `{}` on `{}`", name.name, class.name),
                            );
                        };
                        (format!("{handle}.{}(w)", p.rust), p.ty.clone())
                    }
                    _ => match cx.object_prop_read(receiver, &name.name) {
                        Some(pair) => pair,
                        None => {
                            return err(
                                iter.span,
                                format!(
                                    "`{}` is not a list this view can reach",
                                    expr_source_name(iter)
                                ),
                            );
                        }
                    },
                };
                let bind_rust = camel_to_snake(&binding.name);
                let ri = format!("__row_idx{}", cx.repeat_depth);
                let index_bind = match index {
                    Some(i) => format!("let {} = {ri} as i64;", camel_to_snake(&i.name)),
                    None => String::new(),
                };
                // A repeater over a list of OBJECTS binds a handle,
                // so the loop variable carries its class (§8.41).
                let elem_class = match &elem_ty {
                    RustTy::List(inner) => match &**inner {
                        RustTy::Handle(c) => Some(c.clone()),
                        _ => None,
                    },
                    _ => None,
                };
                let row_ty = match &elem_ty {
                    RustTy::List(inner) => (**inner).clone(),
                    _ => RustTy::Unit,
                };
                cx.loop_vars.push((binding.name.clone(), elem_class, row_ty));
                if let Some(i) = index {
                    cx.loop_vars.push((i.name.clone(), None, RustTy::Int));
                }
                cx.repeat_depth += 1;
                let row = lower_element(child, cx, &format!("{ind}        "));
                cx.repeat_depth -= 1;
                if index.is_some() {
                    cx.loop_vars.pop();
                }
                cx.loop_vars.pop();
                let row = row?;
                // The closure captures only the Copy handles inside
                // `xs_expr` / `row`; its own `w` parameter shadows
                // build()'s, so the same lowered text serves both
                // scopes.
                return Ok(format!(
                    "Element::ListView {{\n\
                     {ind}    virtualized: {virtualized},\n\
                     {ind}    item_height: {item_height},\n\
                     {ind}    height: {height},\n\
                     {ind}    grow: {grow},\n\
                     {ind}    children: Vec::new(),\n\
                     {ind}    lazy: Some(LazyRows {{\n\
                     {ind}        len: {xs_expr}.len(),\n\
                     {ind}        build: Rc::new(move |w: &World, __range: std::ops::Range<usize>| {{\n\
                     {ind}            let __xs = {xs_expr};\n\
                     {ind}            let mut __rows: Vec<Element> = Vec::new();\n\
                     {ind}            for {ri} in __range {{\n\
                     {ind}                if {ri} >= __xs.len() {{ break; }}\n\
                     {ind}                let {bind_rust} = __xs.at({ri} as i64);\n\
                     {ind}                {index_bind}\n\
                     {ind}                __rows.push({row});\n\
                     {ind}            }}\n\
                     {ind}            __rows\n\
                     {ind}        }}),\n\
                     {ind}    }}),\n\
                     {ind}}}"
                ));
                }
            }
            let children = lower_children(el, cx, ind)?;
            Ok(format!(
                "Element::ListView {{ virtualized: {virtualized}, item_height: {item_height}, height: {height}, grow: {grow}, children: {children}, lazy: None }}"
            ))
        }
        "ScrollView" => {
            // Same shape as ListView's `height:`: optional, strict
            // Float, `0f64` meaning "keep the engine's 320 px default".
            let height = match element_prop(el, "height") {
                Some(v) => lower_view_float(v, cx, "height")?,
                None => "0f64".into(),
            };
            let children = lower_children(el, cx, ind)?;
            Ok(format!(
                "Element::ScrollView {{ height: {height}, children: {children} }}"
            ))
        }
        "HScrollView" => {
            let children = lower_children(el, cx, ind)?;
            Ok(format!("Element::HScrollView({children})"))
        }
        "Image" => {
            let source = element_prop(el, "source").ok_or_else(|| EmitError {
                span: el.span,
                message: "Image needs `source:`".into(),
            })?;
            let (width, height) = lower_view_size(el, cx)?;
            Ok(format!(
                "Element::Image {{ source: {}, width: {width}, height: {height} }}",
                lower_view_text(source, cx)?
            ))
        }
        "Svg" => {
            let source = element_prop(el, "source").ok_or_else(|| EmitError {
                span: el.span,
                message: "Svg needs `source:`".into(),
            })?;
            let (width, height) = lower_view_size(el, cx)?;
            Ok(format!(
                "Element::Svg {{ source: {}, width: {width}, height: {height} }}",
                lower_view_text(source, cx)?
            ))
        }
        "DataTable" => {
            let children = lower_children(el, cx, ind)?;
            Ok(format!("Element::DataTable({children})"))
        }
        "Modal" => {
            // `open:` went optional when `if` landed in views: a bare
            // Modal (cute_ui's propless shape) renders open, and the
            // view wraps it in `if` for visibility.
            let open = match element_prop(el, "open") {
                Some(o) => lower_view_bool(o, cx)?,
                None => "true".into(),
            };
            let children = lower_children(el, cx, ind)?;
            Ok(format!(
                "Element::Modal {{ open: {open}, children: {children} }}"
            ))
        }
        "BarChart" => {
            // `data:` is the widget — an empty chart is a bound empty
            // list, never a missing binding. `labels:` is optional, and
            // so is sizing (`0f64` = "unset", the Image rule).
            let data = element_prop(el, "data").ok_or_else(|| EmitError {
                span: el.span,
                message: "BarChart needs `data:`".into(),
            })?;
            let labels = match element_prop(el, "labels") {
                Some(l) => lower_view_str_list(l, cx, "labels")?,
                None => "List::new()".into(),
            };
            let (width, height) = lower_view_size(el, cx)?;
            Ok(format!(
                "Element::BarChart {{ data: {}, labels: {labels}, width: {width}, height: {height} }}",
                lower_view_float_list(data, cx)?
            ))
        }
        "LineChart" => {
            let data = element_prop(el, "data").ok_or_else(|| EmitError {
                span: el.span,
                message: "LineChart needs `data:`".into(),
            })?;
            let labels = match element_prop(el, "labels") {
                Some(l) => lower_view_str_list(l, cx, "labels")?,
                None => "List::new()".into(),
            };
            let (width, height) = lower_view_size(el, cx)?;
            Ok(format!(
                "Element::LineChart {{ data: {}, labels: {labels}, width: {width}, height: {height} }}",
                lower_view_float_list(data, cx)?
            ))
        }
        "ProgressBar" => {
            let value = element_prop(el, "value").ok_or_else(|| EmitError {
                span: el.span,
                message: "ProgressBar needs `value:`".into(),
            })?;
            Ok(format!(
                "Element::ProgressBar {{ value: {} }}",
                lower_view_float(value, cx, "value")?
            ))
        }
        "Spinner" => {
            // One square axis, unlike the charts' width/height pair:
            // `0f64` keeps the engine's 24 px default.
            let size = match element_prop(el, "size") {
                Some(v) => lower_view_float(v, cx, "size")?,
                None => "0f64".into(),
            };
            Ok(format!("Element::Spinner {{ size: {size} }}"))
        }
        // One contract, two paints. `checked:` is REQUIRED — bound
        // state the app owns, never widget-internal — and `onToggle:`
        // is optional, receiving the NEW value as an implicit
        // `checked` (the `text` convention on TextField's handlers).
        "Checkbox" | "Switch" => {
            let name = el.name.name.as_str();
            let label = element_prop(el, "label").ok_or_else(|| EmitError {
                span: el.span,
                message: format!("{name} needs `label:`"),
            })?;
            let checked = element_prop(el, "checked").ok_or_else(|| EmitError {
                span: el.span,
                message: format!("{name} needs `checked:` (the Bool state it shows)"),
            })?;
            let checked = lower_view_bool_keyed(checked, cx, "checked")?;
            let on_toggle = match element_prop(el, "onToggle") {
                Some(a) => format!(
                    "Some({})",
                    lower_view_action_with(a, cx, "onToggle", &[("checked", "bool")])?
                ),
                None => "None".into(),
            };
            Ok(format!(
                "Element::{name} {{ label: {}, checked: {checked}, on_toggle: {on_toggle} }}",
                lower_view_text(label, cx)?
            ))
        }
        "Slider" => {
            // `value:` is required and must be a property READ — the
            // control reflects state, so a literal is a named error
            // (the charts' `data:` rule). The range props are ordinary
            // Float exprs: `min` 0.0 / `max` 1.0 defaults, `step` 0.0
            // = continuous. `onChange:` binds an implicit `value`
            // argument carrying the new value — `onTextChanged`'s
            // machinery with the payload one primitive over (the Rust
            // type in the closure signature, so `f64`).
            let value = element_prop(el, "value").ok_or_else(|| EmitError {
                span: el.span,
                message: "Slider needs `value:` (a Float property to reflect)".into(),
            })?;
            let value = lower_view_float_prop(value, cx, "value")?;
            let min = match element_prop(el, "min") {
                Some(v) => lower_view_float(v, cx, "min")?,
                None => "0f64".into(),
            };
            let max = match element_prop(el, "max") {
                Some(v) => lower_view_float(v, cx, "max")?,
                None => "1f64".into(),
            };
            let step = match element_prop(el, "step") {
                Some(v) => lower_view_float(v, cx, "step")?,
                None => "0f64".into(),
            };
            let on_change = match element_prop(el, "onChange") {
                Some(a) => format!(
                    "Some({})",
                    lower_view_action_with(a, cx, "onChange", &[("value", "f64")])?
                ),
                None => "None".into(),
            };
            Ok(format!(
                "Element::Slider {{ value: {value}, min: {min}, max: {max}, step: {step}, on_change: {on_change} }}"
            ))
        }
        "Select" | "RadioGroup" => {
            let options = element_prop(el, "options").ok_or_else(|| EmitError {
                span: el.span,
                message: format!("{} needs `options:`", el.name.name),
            })?;
            let selected = element_prop(el, "selected").ok_or_else(|| EmitError {
                span: el.span,
                message: format!("{} needs `selected:`", el.name.name),
            })?;
            let on_select = match element_prop(el, "onSelect") {
                Some(a) => format!(
                    "Some({})",
                    lower_view_action_with(a, cx, "onSelect", &[("index", "i64")])?
                ),
                None => "None".into(),
            };
            Ok(format!(
                "Element::{} {{ options: {}, selected: {}, on_select: {on_select} }}",
                el.name.name,
                lower_view_str_list(options, cx, "options")?,
                lower_view_int(selected, cx, "selected")?
            ))
        }
        "TabBar" => {
            let labels = element_prop(el, "labels").ok_or_else(|| EmitError {
                span: el.span,
                message: "TabBar needs `labels:`".into(),
            })?;
            let active = element_prop(el, "active").ok_or_else(|| EmitError {
                span: el.span,
                message: "TabBar needs `active:`".into(),
            })?;
            let on_select = match element_prop(el, "onSelect") {
                Some(a) => format!(
                    "Some({})",
                    lower_view_action_with(a, cx, "onSelect", &[("index", "i64")])?
                ),
                None => "None".into(),
            };
            Ok(format!(
                "Element::TabBar {{ labels: {}, active: {}, on_select: {on_select} }}",
                lower_view_str_list(labels, cx, "labels")?,
                lower_view_int(active, cx, "active")?
            ))
        }
        // The typed number fields, mirroring Slider: `value:` is
        // required and must be a property READ, since the field
        // SHOWS the app's number and a literal could never move.
        // `min`/`max` default to 0, which means unbounded (a slider
        // is a range by construction, a typed field is not);
        // `placeholder:` is TextField's. `onChange:` binds an
        // implicit `value` carrying the committed number — Float for
        // NumberField, Int for IntField.
        "NumberField" => {
            let value = element_prop(el, "value").ok_or_else(|| EmitError {
                span: el.span,
                message: "NumberField needs `value:` (a Float property to reflect)".into(),
            })?;
            let value = lower_view_float_prop(value, cx, "value")?;
            let mut range = Vec::new();
            for key in ["min", "max", "step"] {
                range.push(match element_prop(el, key) {
                    Some(v) => lower_view_float(v, cx, key)?,
                    None => "0f64".to_string(),
                });
            }
            let placeholder = match element_prop(el, "placeholder") {
                Some(v) => lower_view_text(v, cx)?,
                None => "Str::new()".into(),
            };
            let on_change = match element_prop(el, "onChange") {
                Some(a) => format!(
                    "Some({})",
                    lower_view_action_with(a, cx, "onChange", &[("value", "f64")])?
                ),
                None => "None".into(),
            };
            Ok(format!(
                "Element::NumberField {{ value: {value}, min: {}, max: {}, step: {}, \
                 placeholder: {placeholder}, on_change: {on_change} }}",
                range[0], range[1], range[2]
            ))
        }
        "IntField" => {
            let value = element_prop(el, "value").ok_or_else(|| EmitError {
                span: el.span,
                message: "IntField needs `value:` (an Int property to reflect)".into(),
            })?;
            let value = lower_view_int_prop(value, cx, "value")?;
            let min = match element_prop(el, "min") {
                Some(v) => lower_view_int(v, cx, "min")?,
                None => "0i64".to_string(),
            };
            let max = match element_prop(el, "max") {
                Some(v) => lower_view_int(v, cx, "max")?,
                None => "0i64".to_string(),
            };
            // A step of 1 IS every integer, and so is 0 — the default
            // matches `int_field(step=1)` in the dialect.
            let step = match element_prop(el, "step") {
                Some(v) => lower_view_int(v, cx, "step")?,
                None => "1i64".to_string(),
            };
            let placeholder = match element_prop(el, "placeholder") {
                Some(v) => lower_view_text(v, cx)?,
                None => "Str::new()".into(),
            };
            let on_change = match element_prop(el, "onChange") {
                Some(a) => format!(
                    "Some({})",
                    lower_view_action_with(a, cx, "onChange", &[("value", "i64")])?
                ),
                None => "None".into(),
            };
            Ok(format!(
                "Element::IntField {{ value: {value}, min: {min}, max: {max}, step: {step}, \
                 placeholder: {placeholder}, on_change: {on_change} }}"
            ))
        }
        other => err(
            el.span,
            format!(
                "element `{other}` is not in the engine vocabulary yet \
                 (Column / Row / Grid / Stack / Text / Button / TextField / ListView / \
                 ScrollView / HScrollView / Image / Svg / DataTable / Modal / \
                 BarChart / LineChart / ProgressBar / Spinner / Checkbox / Switch / Slider / Select / RadioGroup / TabBar / \
                 NumberField / IntField), and no \
                 `view {other}` component is declared in this module; the \
                 catalog grows widget by widget"
            ),
        ),
    }
}

/// The box-decoration props (yokan's crate boundary), lowered once for every element
/// that paints a box. Returns the three initializer fragments in
/// declaration order; each unset prop keeps its sentinel so an
/// element that sets none emits exactly what it did before.
fn lower_box_props(el: &Element, cx: &ViewCtx<'_>) -> Result<String, EmitError> {
    let radius = match element_prop(el, "borderRadius") {
        Some(v) => lower_view_float(v, cx, "borderRadius")?,
        None => "0f64".into(),
    };
    let width = match element_prop(el, "borderWidth") {
        Some(v) => lower_view_float(v, cx, "borderWidth")?,
        None => "0f64".into(),
    };
    let color = match element_prop(el, "borderColor") {
        Some(v) => lower_view_text(v, cx)?,
        None => "Str::new()".into(),
    };
    Ok(format!(
        "border_radius: {radius}, border_width: {width}, border_color: {color}"
    ))
}

/// Property keys a container element consumes in its own `lower_element`
/// arm — everything else among a container's members is a child, a
/// statement, or an error. `pixie_interp::container_prop_keys` is the
/// mirror of this table; the two must stay identical (ledger §11.12).
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

/// Placement props every element accepts because they belong to the
/// PARENT grid, not to the element's own vocabulary: `lower_element`
/// strips them into an `Element::GridCell` before an arm ever sees
/// them. Kept out of `container_prop_keys` so that table keeps meaning
/// "props THIS element consumes"; mirrored in `pixie_interp` and
/// equality-tested next to the container tables (ledger §11.12).
pub fn grid_item_prop_keys() -> &'static [&'static str] {
    &["colSpan", "rowSpan"]
}

/// The animation riders (§8.35) — the same "every element takes
/// them, the wrapper consumes them" contract as the grid spans, and
/// the same mirror-table equality test across tiers.
pub fn anim_prop_keys() -> &'static [&'static str] {
    &["animate", "easing", "enter", "exit"]
}

/// The accessibility riders (§8.36) — the fourth universal table,
/// same contract and same cross-tier equality test as the others.
pub fn semantic_prop_keys() -> &'static [&'static str] {
    &["role", "label"]
}

/// The tooltip rider — the sixth universal table, same contract and
/// same cross-tier equality test as the others.
pub fn tooltip_prop_keys() -> &'static [&'static str] {
    &["tooltip"]
}

/// The theme-scope rider (§8.37) — the fifth universal table.
pub fn theme_prop_keys() -> &'static [&'static str] {
    &["theme"]
}

/// The palettes a `theme:` rider may name. `pixie_kernel::theme::NAMES`
/// owns the real list (the easing/role rule: this crate does not
/// depend on the kernel), and the accept test asserts they agree.
pub fn theme_names() -> &'static [&'static str] {
    &["dark", "light"]
}

/// The `role:` vocabulary, spelled here because this crate does not
/// depend on the kernel (the easing table's rule). `pixie_kernel::
/// a11y::Role` owns the real definition and the accept test — the
/// only place that can see both — asserts the two agree.
pub fn a11y_roles() -> &'static [&'static str] {
    &[
        "button",
        "label",
        "heading",
        "textInput",
        "image",
        "list",
        "listItem",
        "table",
        "dialog",
        "progress",
        "slider",
        "group",
        "checkbox",
        "switch",
        "comboBox",
        "radioGroup",
        "tabList",
    ]
}

/// The `some` / `nil` halves of a two-armed `case` over a `T?`,
/// shared by the view lowering and its interpreted mirror (§8.69).
fn split_opt_arms<'a>(
    arms: &'a [ast::CaseArm],
    span: Span,
) -> Result<(Option<String>, &'a ast::Block, &'a ast::Block), EmitError> {
    let mut some_arm: Option<(Option<String>, &ast::Block)> = None;
    let mut none_arm: Option<&ast::Block> = None;
    for arm in arms {
        match &arm.pattern {
            ast::Pattern::Ctor { name, args, span } if name.name == "some" => {
                let bind = match args.as_slice() {
                    [] => None,
                    [ast::Pattern::Bind { name, .. }] => Some(name.name.clone()),
                    _ => return err(*span, "`some` takes at most one binding"),
                };
                some_arm = Some((bind, &arm.body));
            }
            _ => none_arm = Some(&arm.body),
        }
    }
    let (Some((bind, some_body)), Some(none_body)) = (some_arm, none_arm) else {
        return err(span, "matching a `T?` needs both a `some` and a `nil` arm");
    };
    Ok((bind, some_body, none_body))
}

fn view_pattern_span(p: &ast::Pattern) -> Span {
    match p {
        ast::Pattern::Ctor { span, .. }
        | ast::Pattern::Literal { span, .. }
        | ast::Pattern::Wild { span }
        | ast::Pattern::Bind { span, .. } => *span,
    }
}

fn lower_children(el: &Element, cx: &mut ViewCtx, ind: &str) -> Result<String, EmitError> {
    // Container-property allowlist (§5.11/R3, ledger §11.12): only the
    // keys THIS element consumes in its own arm before calling us may
    // sit among the children. Interp's `build_children` runs the
    // identical table and the identical error, so a rung-2 reload of a
    // bad view fails the same way instead of silently ignoring it.
    for m in &el.members {
        if let ElementMember::Property { key, span, .. } = m {
            if !grid_item_prop_keys().contains(&key.as_str())
                && !anim_prop_keys().contains(&key.as_str())
                && !semantic_prop_keys().contains(&key.as_str())
                && !theme_prop_keys().contains(&key.as_str())
                && !tooltip_prop_keys().contains(&key.as_str())
                && !container_prop_keys(&el.name.name).contains(&key.as_str())
            {
                return err(
                    *span,
                    format!(
                        "element property `{key}` is not lowerable on `{}` (M0)",
                        el.name.name
                    ),
                );
            }
        }
    }
    let var = format!("__c{}", cx.depth);
    cx.depth += 1;
    let inner_ind = format!("{ind}    ");
    let mut out = String::new();
    writeln!(out, "{{").unwrap();
    writeln!(out, "{inner_ind}let mut {var}: Vec<Element> = Vec::new();").unwrap();
    lower_items(
        &items_of_members(&el.members),
        &var,
        cx,
        &inner_ind,
        &mut out,
    )?;
    write!(out, "{ind}    {var}\n{ind}}}").unwrap();
    Ok(out)
}

/// Emit the pushes a run of view items contributes into `var`.
/// `for` bodies and `if` branches are runs of items too (§8.56), so
/// this is the whole recursion: a repeater body may hold several
/// elements, another repeater, or a conditional, and so may a branch.
fn lower_items(
    items: &[ViewItem<'_>],
    var: &str,
    cx: &mut ViewCtx,
    ind: &str,
    out: &mut String,
) -> Result<(), EmitError> {
    for item in items {
        match item {
            ViewItem::Child(child) => {
                let c = lower_element(child, cx, ind)?;
                writeln!(out, "{ind}{var}.push({c});").unwrap();
            }
            ViewItem::Repeat {
                binding,
                index,
                iter,
                body,
                span,
            } => {
                let ExprKind::Member { receiver, name } = &iter.kind else {
                    return err(
                        *span,
                        "`for` iterates a list PROPERTY — name the object and the \
                         property it holds",
                    );
                };
                // The list can be reached through any object
                // expression (§8.65), not only a bare field or global:
                // `for c in row.cells` inside `for row in Store.rows`
                // is a table, and it was the shape a nested repeater
                // could not express.
                let (xs_read, elem_ty) = match &receiver.kind {
                    ExprKind::Ident(f) if cx.handle_for(f).is_some() => {
                        let (class, handle) = cx.handle_for(f).expect("guarded");
                        let Some(p) = class.prop(&name.name) else {
                            return err(
                                *span,
                                format!("no property `{}` on `{}`", name.name, class.name),
                            );
                        };
                        (format!("{handle}.{}(w)", p.rust), p.ty.clone())
                    }
                    _ => match cx.object_prop_read(receiver, &name.name) {
                        Some(pair) => pair,
                        None => match view_map_view(receiver, &name.name, cx) {
                            Some(pair) => pair,
                            None => {
                                return err(
                                    *span,
                                    format!(
                                        "`{}` is not a list this view can reach",
                                        expr_source_name(iter)
                                    ),
                                );
                            }
                        },
                    },
                };
                let bind_rust = camel_to_snake(&binding.name);
                // The list binding is named by REPEATER DEPTH, not by
                // container depth: a `for` directly inside a `for`
                // body has no container between them, and two `let
                // __xs0` in nested scopes would shadow rather than
                // collide — legal Rust, unreadable output.
                let xs = format!("__xs{}", cx.repeat_depth);
                let ri = format!("__row_idx{}", cx.repeat_depth);
                writeln!(out, "{ind}let {xs} = {xs_read};").unwrap();
                writeln!(
                    out,
                    "{ind}for ({ri}, {bind_rust}) in {xs}.iter().enumerate() {{"
                )
                .unwrap();
                if let Some(i) = index {
                    writeln!(out, "{ind}    let {} = {ri} as i64;", camel_to_snake(&i.name)).unwrap();
                }
                // A repeater over a list of OBJECTS binds a handle,
                // so the loop variable carries its class (§8.41).
                let elem_class = match &elem_ty {
                    RustTy::List(inner) => match &**inner {
                        RustTy::Handle(c) => Some(c.clone()),
                        _ => None,
                    },
                    _ => None,
                };
                let row_ty = match &elem_ty {
                    RustTy::List(inner) => (**inner).clone(),
                    _ => RustTy::Unit,
                };
                cx.loop_vars.push((binding.name.clone(), elem_class, row_ty));
                if let Some(i) = index {
                    cx.loop_vars.push((i.name.clone(), None, RustTy::Int));
                }
                cx.repeat_depth += 1;
                let r = lower_items(
                    &items_of_block(body),
                    var,
                    cx,
                    &format!("{ind}    "),
                    out,
                );
                cx.repeat_depth -= 1;
                if index.is_some() {
                    cx.loop_vars.pop();
                }
                cx.loop_vars.pop();
                r?;
                writeln!(out, "{ind}}}").unwrap();
            }
            // Conditional render: `if cond { .. } [else { .. }]`. The
            // condition gets the action-expression grammar, with loop
            // variables in scope, and the checker already enforced it
            // is a Bool. A false branch contributes nothing, so child
            // indices below a toggled `if` shift — positional
            // semantics, same as Flutter's no-key rule.
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
                    return err(e.span, "`if let` survived the desugar (§8.69) — this is a pixie bug");
                }
                let acx = ActionCtx {
                    view: cx,
                    locals: cx.loop_vars.iter().map(|(n, _, _)| n.clone()).collect(),
                    local_objects: Vec::new(),
                };
                let c = lower_action_expr(cond, &acx)?;
                writeln!(out, "{ind}if {c} {{").unwrap();
                lower_items(
                    &items_of_block(then_b),
                    var,
                    cx,
                    &format!("{ind}    "),
                    out,
                )?;
                if let Some(eb) = else_b {
                    writeln!(out, "{ind}}} else {{").unwrap();
                    lower_items(&items_of_block(eb), var, cx, &format!("{ind}    "), out)?;
                }
                writeln!(out, "{ind}}}").unwrap();
            }
            // `case` in a view body (§8.69) — and therefore `if let`,
            // which desugars into one. A view holds `&World`, so the
            // scrutinee is a READ and the arms push elements; the
            // shapes a view can name are a `T?` property and an enum
            // one, because a view cannot call anything that returns a
            // fallible.
            ViewItem::Match(e) => {
                let ExprKind::Case { scrutinee, arms } = &e.kind else {
                    unreachable!("items_of_* only builds Match from a Case");
                };
                // The RAW read, not the display one: a match needs
                // the `Option` itself, and display would have turned
                // it into text (§8.68).
                let Some((scrut, scrut_ty)) = cx.struct_value_read(scrutinee) else {
                    return err(
                        scrutinee.span,
                        "a view can match a `T?` property or an enum one — nothing else \
                         reaches a view body with a shape to match",
                    );
                };
                match Some(scrut_ty) {
                    Some(RustTy::Opt(inner)) => {
                        let (some_bind, some_body, none_body) = split_opt_arms(arms, e.span)?;
                        let binder = some_bind
                            .clone()
                            .map(|b| camel_to_snake(&b))
                            .unwrap_or_else(|| "_".to_string());
                        writeln!(out, "{ind}match {scrut} {{").unwrap();
                        writeln!(out, "{ind}    Some({binder}) => {{").unwrap();
                        let elem_class = match &*inner {
                            RustTy::Handle(c) => Some(c.clone()),
                            _ => None,
                        };
                        if let Some(b) = some_bind {
                            cx.loop_vars.push((b, elem_class, (*inner).clone()));
                        }
                        let r = lower_items(
                            &items_of_block(some_body),
                            var,
                            cx,
                            &format!("{ind}        "),
                            out,
                        );
                        cx.loop_vars.pop();
                        r?;
                        writeln!(out, "{ind}    }}").unwrap();
                        writeln!(out, "{ind}    None => {{").unwrap();
                        lower_items(
                            &items_of_block(none_body),
                            var,
                            cx,
                            &format!("{ind}        "),
                            out,
                        )?;
                        writeln!(out, "{ind}    }}").unwrap();
                        writeln!(out, "{ind}}}").unwrap();
                    }
                    Some(RustTy::Named(n)) if cx.enums.contains_key(&n) => {
                        let en = &cx.enums[&n];
                        writeln!(out, "{ind}match {scrut} {{").unwrap();
                        for arm in arms {
                            // Payload arms BIND now, the way a view
                            // if-let binds `some(v)` (§8.69) — the
                            // checker already demanded the patterns;
                            // refusing them here was the
                            // contradiction castel's front end hit.
                            let mut binders: Vec<(String, Option<String>, RustTy)> = Vec::new();
                            let pat = match &arm.pattern {
                                ast::Pattern::Wild { .. } => "_".to_string(),
                                ast::Pattern::Ctor { name, args, span } => {
                                    let Some(variant) = en.variant(&name.name) else {
                                        return err(
                                            *span,
                                            format!("no variant `{}` on `{n}`", name.name),
                                        );
                                    };
                                    if args.is_empty() {
                                        if variant.fields.is_empty() {
                                            format!(
                                                "{n}::{}",
                                                escape_rust_keyword(name.name.clone())
                                            )
                                        } else {
                                            format!(
                                                "{n}::{}(..)",
                                                escape_rust_keyword(name.name.clone())
                                            )
                                        }
                                    } else {
                                        let mut parts: Vec<String> = Vec::new();
                                        for (a, f) in args.iter().zip(&variant.fields) {
                                            match a {
                                                ast::Pattern::Bind { name: b, .. } => {
                                                    parts.push(camel_to_snake(&b.name));
                                                    let fty = lower_type(&f.ty, cx.class_names)?;
                                                    let fclass = match &fty {
                                                        RustTy::Handle(c) => Some(c.clone()),
                                                        _ => None,
                                                    };
                                                    binders.push((b.name.clone(), fclass, fty));
                                                }
                                                ast::Pattern::Wild { .. } => parts.push("_".to_string()),
                                                other => {
                                                    return err(
                                                        view_pattern_span(other),
                                                        "a view arm binds payload names or `_`",
                                                    );
                                                }
                                            }
                                        }
                                        format!(
                                            "{n}::{}({})",
                                            escape_rust_keyword(name.name.clone()),
                                            parts.join(", ")
                                        )
                                    }
                                }
                                other => {
                                    return err(
                                        view_pattern_span(other),
                                        "a view arm matches a variant name or `_`",
                                    );
                                }
                            };
                            writeln!(out, "{ind}    {pat} => {{").unwrap();
                            for (b, c, t) in &binders {
                                cx.loop_vars.push((b.clone(), c.clone(), t.clone()));
                            }
                            let r = lower_items(
                                &items_of_block(&arm.body),
                                var,
                                cx,
                                &format!("{ind}        "),
                                out,
                            );
                            for _ in &binders {
                                cx.loop_vars.pop();
                            }
                            r?;
                            writeln!(out, "{ind}    }}").unwrap();
                        }
                        // A view build cannot fail, so an unlisted
                        // variant contributes nothing rather than
                        // panicking mid-frame.
                        writeln!(out, "{ind}    #[allow(unreachable_patterns)] _ => {{}}").unwrap();
                        writeln!(out, "{ind}}}").unwrap();
                    }
                    _ => {
                        return err(
                            scrutinee.span,
                            "a view can match a `T?` property or an enum one — nothing else \
                             reaches a view body with a shape to match",
                        );
                    }
                }
            }
            ViewItem::Other(span) => {
                return err(*span, "this statement is not lowerable in views yet (M0)");
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Whole-program emission.

struct Program<'a> {
    classes: HashMap<String, ClassInfo<'a>>,
    /// Just the class NAMES, collected before anything else is built.
    /// `lower_type` needs them to decide handle-or-value, and it runs
    /// while the rest of this struct is still being filled in.
    class_names: std::collections::HashSet<String>,
    order: Vec<String>,
    bindings: HashMap<String, BindingClass>,
    globals: Globals,
    /// Insertion order for main-side singleton init: (let name, class).
    global_order: Vec<(String, String)>,
    free_fns: HashMap<String, String>,
    free_order: Vec<&'a ast::FnDecl>,
    tests: Vec<&'a ast::FnDecl>,
    views: Vec<&'a ast::ViewDecl>,
    enums: HashMap<String, EnumInfo<'a>>,
    enum_order: Vec<String>,
    structs: HashMap<String, StructInfo<'a>>,
    struct_order: Vec<String>,
    /// Declared traits — emitted as REAL Rust traits (§8.20) so
    /// generic fns lower to Rust generics and rustc monomorphizes.
    traits: HashMap<String, &'a ast::TraitDecl>,
    trait_order: Vec<String>,
    /// (trait name, class name, impl) — emitted as
    /// `impl Trait for Handle<Class>` blocks after the classes.
    trait_impls: Vec<(String, String, &'a ast::ImplDecl)>,
    /// The same, for value types: `impl Trait for P` (§8.49).
    struct_trait_impls: Vec<(String, String, &'a ast::ImplDecl)>,
    /// The module's `!T` error enum (one `error E` decl, else one plain
    /// enum — the resolve-side rule, mirrored).
    default_error: Option<String>,
    /// Receiver-less context for free fns and tests.
    empty_class: ClassInfo<'a>,
}

impl<'a> Program<'a> {
    /// Seed a method context from the fn's PARAMETERS, so a
    /// class-typed param registers as a handle of that class (§11.23)
    /// — otherwise `l.v` on a `Leaf` param lowered as a struct field
    /// read instead of the accessor.
    fn method_ctx(&'a self, class: &'a ClassInfo<'a>, params: &[ast::Param]) -> MethodCtx<'a> {
        MethodCtx {
            class_names: &self.class_names,
            reclaim: Vec::new(),
            class,
            locals: params
                .iter()
                .map(|q| {
                    (
                        q.name.name.clone(),
                        named_class(&q.ty, &self.class_names),
                        matches!(q.ty.kind, TypeKind::Nullable(_)),
                        lower_type(&q.ty, &self.class_names).ok(),
                    )
                })
                .collect(),
            bindings: &self.bindings,
            classes: &self.classes,
            globals: &self.globals,
            free_fns: &self.free_fns,
            free_decls: &self.free_order,
            traits: &self.traits,
            generic_locals: HashMap::new(),
            enums: &self.enums,
            structs: &self.structs,
            self_struct: None,
            default_error: self.default_error.as_deref(),
            fallible_ret: false,
            nullable_ret: false,
            loop_depth: 0,
        }
    }
}

fn collect_program(
    module: &ast::Module,
    binding_items: usize,
) -> Result<Program<'_>, EmitError> {
    // Pass zero: every class name in the module. A class is reached by
    // handle wherever its name appears as a type (§11.23), and the
    // passes below lower types as they go, so this has to be known
    // before any of them run.
    let mut class_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    for item in &module.items {
        if let ast::Item::Class(c) = item {
            class_names.insert(c.name.name.clone());
        }
    }
    let mut p = Program {
        class_names: class_names.clone(),
        classes: HashMap::new(),
        order: Vec::new(),
        bindings: HashMap::new(),
        globals: HashMap::new(),
        global_order: Vec::new(),
        free_fns: HashMap::new(),
        free_order: Vec::new(),
        tests: Vec::new(),
        views: Vec::new(),
        enums: HashMap::new(),
        enum_order: Vec::new(),
        structs: HashMap::new(),
        struct_order: Vec::new(),
        traits: HashMap::new(),
        trait_order: Vec::new(),
        trait_impls: Vec::new(),
        struct_trait_impls: Vec::new(),
        default_error: None,
        empty_class: ClassInfo {
            name: String::new(),
            props: Vec::new(),
            signals: Vec::new(),
            methods: Vec::new(),
            own_method_count: 0,
            init: None,
            generics: Vec::new(),
        statics: Vec::new(),
            deinit: None,
        },
    };
    let mut next_signal: u32 = 1;
    let mut raw_lets: Vec<&ast::LetDecl> = Vec::new();
    let mut raw_impls: Vec<&ast::ImplDecl> = Vec::new();
    let mut error_enums: Vec<String> = Vec::new();
    let mut plain_enums: Vec<String> = Vec::new();

    for (idx, item) in module.items.iter().enumerate() {
        if idx < binding_items {
            if let Item::Class(c) = item {
                let (name, bc) = collect_binding_class(c, &class_names)?;
                p.bindings.insert(name, bc);
            }
            // An ENUM or STRUCT declared in a `.rpi` is a real pixie
            // type that happens to know its Rust counterpart (§8.74,
            // §8.77), so it falls through to the ordinary collection
            // below rather than being skipped as binding surface.
            if !matches!(item, Item::Enum(_) | Item::Struct(_)) {
                continue;
            }
        }
        match item {
            Item::Class(c) => {
                check_type_name(&c.name.name, "class", c.name.span)?;
                let info = collect_class(c, &class_names, &mut next_signal)?;
                p.order.push(info.name.clone());
                p.classes.insert(info.name.clone(), info);
            }
            Item::View(v) => p.views.push(v),
            Item::Fn(f) => {
                if f.is_test {
                    p.tests.push(f);
                } else {
                    p.free_fns
                        .insert(f.name.name.clone(), camel_to_snake(&f.name.name));
                    p.free_order.push(f);
                }
            }
            Item::Let(l) => raw_lets.push(l),
            Item::Struct(s) => {
                check_type_name(&s.name.name, "struct", s.name.span)?;
                let mut fields = Vec::new();
                let mut defaults: Vec<Option<Expr>> = Vec::new();
                for f in &s.fields {
                    // §8.68: a field's `= expr` is a default a
                    // construction site may leave out. It is checked
                    // where it is USED, because a struct default can
                    // itself construct a struct that has not been
                    // collected yet.
                    defaults.push(f.default.clone());
                    // A struct is a VALUE: assigning it copies it,
                    // and a copy of a handle is a second reference to
                    // one object. Counting edges through something
                    // that copies silently would mean a struct is not
                    // a value any more — so a struct holds values,
                    // and a reference is spelled `class` (§8.46).
                    // Until now this passed the checker and broke
                    // rustc inside the generated crate, which D10
                    // says is our bug rather than the author's.
                    let fty = lower_type(&f.ty, &class_names)?;
                    if fty.holds_objects() {
                        return err(
                            f.name.span,
                            format!(
                                "a `struct` holds values, and `{}` holds an object — copying \
                                 the struct would copy the reference. Make the holder a \
                                 `class`, or store an id and look the object up",
                                f.name.name
                            ),
                        );
                    }
                    fields.push((
                        f.name.name.clone(),
                        camel_to_snake(&f.name.name),
                        fty,
                    ));
                }
                for m in &s.methods {
                    // Unbounded generics are fine on value methods;
                    // trait-bound ones dispatch w-threaded and struct
                    // fns have no World.
                    if m.generics.iter().any(|g| !g.bounds.is_empty()) {
                        return err(m.span, "trait-bound generics need World access — use a class method or a free fn");
                    }
                }
                if s.generics.iter().any(|g| !g.bounds.is_empty()) {
                    return err(s.span, "bounded struct generics are not lowerable yet (M2)");
                }
                let info = StructInfo {
                    name: s.name.name.clone(),
                    generics: s.generics.iter().map(|g| g.name.name.clone()).collect(),
                    rust_path: s.rust_path.clone(),
                    rust_fields: s.fields.iter().map(|f| f.rust_name.clone()).collect(),
                    rust_types: s.fields.iter().map(|f| f.rust_ty.clone()).collect(),
                    fields,
                    defaults,
                    methods: s.methods.iter().collect(),
                };
                p.struct_order.push(info.name.clone());
                p.structs.insert(info.name.clone(), info);
            }
            Item::Enum(e) => {
                check_type_name(&e.name.name, "enum", e.name.span)?;
                if e.is_extern {
                    return err(e.span, "`extern enum` was replaced by `.rpi` bindings (M1) — declare the Rust enum in a binding file and `use` it");
                }
                let info = EnumInfo {
                    name: e.name.name.clone(),
                    rust_path: e.rust_path.clone(),
                    variants: e.variants.iter().collect(),
                };
                if e.is_error {
                    error_enums.push(info.name.clone());
                } else {
                    plain_enums.push(info.name.clone());
                }
                p.enum_order.push(info.name.clone());
                p.enums.insert(info.name.clone(), info);
            }
            Item::Flags(f) => return err(f.span, "flags are not lowerable yet (M2)"),
            Item::Trait(t) => {
                if t.methods.iter().any(|m| !m.generics.is_empty()) {
                    return err(t.span, "generic trait methods are not lowerable yet (M2)");
                }
                p.trait_order.push(t.name.name.clone());
                p.traits.insert(t.name.name.clone(), t);
            }
            Item::Impl(i) => raw_impls.push(i),
            // The driver desugars or gates these before codegen runs.
            Item::Use(_) | Item::UseQml(_) | Item::Widget(_) | Item::Style(_)
            | Item::Store(_) | Item::Suite(_) => {}
        }
    }

    p.default_error = match (error_enums.as_slice(), plain_enums.as_slice()) {
        ([one], _) => Some(one.clone()),
        ([], [one]) => Some(one.clone()),
        _ => None,
    };

    // `impl Trait for Type` splices its methods into the for-type — the
    // trait itself has no runtime shape (the checker enforced conformance).
    for i in raw_impls {
        let TypeKind::Named { path, args } = &i.for_type.kind else {
            return err(i.span, "impl for this type shape is not lowerable yet (M2)");
        };
        if path.len() != 1 || !args.is_empty() || !i.generics.is_empty() {
            return err(i.span, "generic impls are not lowerable yet (M2)");
        }
        let target = path[0].name.as_str();
        if let Some(info) = p.classes.get_mut(target) {
            if !info.generics.is_empty() {
                return err(i.span, "trait impls on generic classes are not lowerable yet (M2)");
            }
            // Declared trait on a class: the methods emit into a REAL
            // `impl Trait for Handle<C>` block (so the class satisfies
            // the Rust bound generic fns compile against) — but they
            // still register here so call resolution and arg coercion
            // see them. `own_method_count` keeps them out of the Ref
            // trait. An impl of an UNDECLARED trait keeps the old
            // inherent splice.
            info.methods.extend(i.methods.iter());
            if p.traits.contains_key(&i.trait_name.name) {
                p.trait_impls.push((
                    i.trait_name.name.clone(),
                    target.to_string(),
                    i,
                ));
            } else {
                info.own_method_count = info.methods.len();
            }
        } else if let Some(info) = p.structs.get_mut(target) {
            // A VALUE implementing a declared trait needs a real
            // `impl Trait for P` block too, or it cannot satisfy a
            // bound — which is how `impl Labeled for P` came to pass
            // the checker and break rustc (§8.49). Its methods stay
            // out of the inherent block, exactly as a class's do.
            if p.traits.contains_key(&i.trait_name.name) {
                p.struct_trait_impls.push((
                    i.trait_name.name.clone(),
                    target.to_string(),
                    i,
                ));
            } else {
                info.methods.extend(i.methods.iter());
            }
        } else {
            return err(
                i.span,
                format!("impl target `{target}` must be a class or struct (M2 widens this)"),
            );
        }
    }

    // Top-level `let x : Class = ...` — World singletons (one per class:
    // `singleton_ref` addresses by type).
    for l in raw_lets {
        let TypeKind::Named { path, args } = &l.ty.kind else {
            return err(l.span, "top-level `let` must hold a class in M1");
        };
        if path.len() != 1 || !args.is_empty() {
            return err(l.span, "top-level `let` must hold a class in M1");
        }
        let class_name = path[0].name.clone();
        if p.classes.get(&class_name).is_some_and(|c| !c.generics.is_empty()) {
            return err(l.span, "generic classes cannot be stores or top-level lets yet (M2)");
        }
        if !p.classes.contains_key(&class_name) {
            return err(l.span, format!("unknown class `{class_name}`"));
        }
        if p.global_order.iter().any(|(_, c)| *c == class_name) {
            return err(
                l.span,
                format!("only one top-level instance of `{class_name}` (singletons address by type)"),
            );
        }
        p.globals.insert(l.name.name.clone(), class_name.clone());
        p.global_order.push((l.name.name.clone(), class_name));
    }

    Ok(p)
}

fn emit_header(out: &mut String) {
    out.push_str("// Generated by pixie — do not edit.\n");
    out.push_str("#![allow(unused_imports, unused_variables, unused_mut, unused_parens, unused_assignments, dead_code, non_camel_case_types, non_snake_case, clippy::all)]\n\n");
    out.push_str("use pixie_kernel::{mount, Bytes, Component, Element, Handle, LazyRows, List, Map, Runtime, SignalId, Str, World};\n");
    out.push_str("use std::rc::Rc;\n");
    // FromStr in scope so bindings may name `i64::from_str` and friends.
    out.push_str("use std::str::FromStr;\n\n");
    // Interpolating a `T?` (§8.68). An absent optional prints as
    // nothing, which is what the interpreted tier renders for `nil` —
    // the two tiers have to agree, and `Option` has no `Display`.
    out.push_str(
        "fn __pixie_show_opt<T: std::fmt::Display>(v: Option<T>) -> Str {\n\
         \x20   match v {\n\
         \x20       Some(x) => Str::from(format!(\"{x}\")),\n\
         \x20       None => Str::new(),\n\
         \x20   }\n\
         }\n\n",
    );
}

/// Rung-2 wiring: where the running binary finds its own source, and
/// the fingerprint of everything outside the view body (computed on
/// the raw, pre-desugar parse — the same parse the reload does).
pub struct ReloadInfo {
    pub source_path: String,
    pub fingerprint: u64,
    /// The entry's direct imports as (module name, absolute path).
    /// The reload REREADS these (§8.72): a `pub style` or a component
    /// body in another module is view-slice material like any other,
    /// and baking their text froze them into the binary.
    pub foreign_paths: Vec<(String, String)>,
}

/// What the type checker knows and the emitter cannot work out on
/// its own. The emitter has no types by design — rustc is the second
/// verifier (D10) — but a few lowerings need one specific fact, and
/// guessing produces code that does not compile.
///
/// `Default` is the empty answer, which is what the emitter assumed
/// implicitly before this existed.
#[derive(Default)]
pub struct CheckInfo {
    /// Operand spans widened `Int` → `Float` (§8.55).
    pub int_to_float: std::collections::HashSet<Span>,
}

pub fn emit_program(
    module: &ast::Module,
    binding_items: usize,
    reload: Option<&ReloadInfo>,
) -> Result<String, EmitError> {
    emit_program_with(module, binding_items, reload, &CheckInfo::default())
}

pub fn emit_program_with(
    module: &ast::Module,
    binding_items: usize,
    reload: Option<&ReloadInfo>,
    info: &CheckInfo,
) -> Result<String, EmitError> {
    emit_program_with_window(module, binding_items, reload, info, &WindowOpts::default())
}

/// The app's window request: pixie.toml `[window]` (title / width /
/// height), yokan's `ui.run(title=, width=, height=)`. All optional —
/// absent keeps the historical exe-stem title and 420x560 bounds.
/// width/height apply as a pair; one without the other is ignored.
#[derive(Default, Clone)]
pub struct WindowOpts {
    pub title: Option<String>,
    pub width: Option<f64>,
    pub height: Option<f64>,
}

pub fn emit_program_with_window(
    module: &ast::Module,
    binding_items: usize,
    reload: Option<&ReloadInfo>,
    info: &CheckInfo,
    win: &WindowOpts,
) -> Result<String, EmitError> {
    CHECK_INFO.with(|c| *c.borrow_mut() = info.int_to_float.clone());
    WINDOW_OPTS.with(|w| *w.borrow_mut() = win.clone());
    let r = emit_program_inner(module, binding_items, reload);
    CHECK_INFO.with(|c| c.borrow_mut().clear());
    WINDOW_OPTS.with(|w| *w.borrow_mut() = WindowOpts::default());
    r
}

thread_local! {
    /// The current emission's widening set. A thread-local rather
    /// than a parameter threaded through forty lowering functions:
    /// emission is single-threaded and non-reentrant, and every read
    /// is one `contains`.
    static CHECK_INFO: std::cell::RefCell<std::collections::HashSet<Span>> =
        std::cell::RefCell::new(std::collections::HashSet::new());
    /// Struct layouts, for the one place that needs them without a
    /// `Program` in hand: a constant DEFAULT that constructs a struct
    /// (§8.68). `lower_default` is reached from fourteen sites and
    /// recurses through itself; threading a map through all of them
    /// to serve one arm is worse than the borrow this costs.
    static STRUCT_LAYOUTS: std::cell::RefCell<
        HashMap<String, Vec<(String, RustTy, Option<Expr>)>>,
    > = std::cell::RefCell::new(HashMap::new());
    /// The current emission's window request — same rationale as
    /// CHECK_INFO: emission is single-threaded and non-reentrant, and
    /// exactly one site (`emit_main`) reads it.
    static WINDOW_OPTS: std::cell::RefCell<WindowOpts> =
        std::cell::RefCell::new(WindowOpts::default());
}

/// Did the checker widen this operand from `Int` to `Float`?
fn widened_to_float(span: Span) -> bool {
    CHECK_INFO.with(|c| c.borrow().contains(&span))
}

/// The lowered operand, cast when the checker widened it. Rust has no
/// implicit numeric conversion, so `30.0 * n` needs the `as f64` the
/// checker already decided on (§8.55).
fn cast_if_widened(lowered: &str, span: Span) -> String {
    if widened_to_float(span) {
        format!("(({lowered}) as f64)")
    } else {
        lowered.to_string()
    }
}

fn emit_program_inner(
    module: &ast::Module,
    binding_items: usize,
    reload: Option<&ReloadInfo>,
) -> Result<String, EmitError> {
    let p = collect_program(module, binding_items)?;
    // Struct layouts for `lower_default`'s constructing arm (§8.68).
    STRUCT_LAYOUTS.with(|m| {
        let mut m = m.borrow_mut();
        m.clear();
        for (name, st) in &p.structs {
            m.insert(
                name.clone(),
                st.fields
                    .iter()
                    .zip(st.defaults.iter())
                    .map(|((_, rust, ty), d)| (rust.clone(), ty.clone(), d.clone()))
                    .collect(),
            );
        }
    });
    if p.views.len() != 1 {
        return err(
            module.span,
            format!("programs declare exactly one `view` for now (found {})", p.views.len()),
        );
    }
    let view = p.views[0];

    let mut out = String::new();
    emit_header(&mut out);
    for name in &p.enum_order {
        emit_enum(&p.enums[name], &p.class_names, &mut out)?;
    }
    for name in &p.struct_order {
        emit_struct(&p.structs[name], &p, &mut out)?;
    }
    for name in &p.trait_order {
        emit_trait(p.traits[name], &p, &mut out)?;
    }
    for name in &p.order {
        emit_class(&p.classes[name], &p, &mut out)?;
    }
    emit_trait_impls(&p, &mut out)?;
    for f in &p.free_order {
        emit_free_fn(f, &p, &mut out)?;
    }
    emit_view(view, &p, &mut out, reload.is_some())?;
    if let Some(ri) = reload {
        emit_reload_support(view, &p, ri, &mut out)?;
    }
    emit_main(view, &p, &mut out, reload.is_some())?;
    Ok(out)
}

/// `pixie test`: every `test fn` becomes a runner entry; TAP output,
/// fresh World (plus singletons) per test, failures via catch_unwind.
pub fn emit_test_program(module: &ast::Module, binding_items: usize) -> Result<String, EmitError> {
    let p = collect_program(module, binding_items)?;
    if p.tests.is_empty() {
        return err(module.span, "no tests found (`test fn x { ... }` / `suite`)");
    }
    let mut out = String::new();
    emit_header(&mut out);
    for name in &p.enum_order {
        emit_enum(&p.enums[name], &p.class_names, &mut out)?;
    }
    for name in &p.struct_order {
        emit_struct(&p.structs[name], &p, &mut out)?;
    }
    for name in &p.trait_order {
        emit_trait(p.traits[name], &p, &mut out)?;
    }
    for name in &p.order {
        emit_class(&p.classes[name], &p, &mut out)?;
    }
    emit_trait_impls(&p, &mut out)?;
    for f in &p.free_order {
        emit_free_fn(f, &p, &mut out)?;
    }
    for (i, t) in p.tests.iter().enumerate() {
        let Some(body) = &t.body else {
            return err(t.span, "test fns need a body");
        };
        writeln!(out, "fn __pixie_test_{i}(w: &mut World) {{").unwrap();
        let mut cx = p.method_ctx(&p.empty_class, &[]);
        let mut body_s = String::new();
        lower_scope(&body.stmts, body.trailing.as_deref(), true, &mut cx, &mut body_s, "    ")?;
        out.push_str(&body_s);
        writeln!(out, "}}\n").unwrap();
    }

    writeln!(out, "fn main() {{").unwrap();
    writeln!(
        out,
        "    let tests: [(&str, fn(&mut World)); {}] = [",
        p.tests.len()
    )
    .unwrap();
    for (i, t) in p.tests.iter().enumerate() {
        let label = t
            .display_name
            .clone()
            .unwrap_or_else(|| t.name.name.clone());
        writeln!(out, "        ({:?}, __pixie_test_{i}),", label).unwrap();
    }
    writeln!(out, "    ];").unwrap();
    writeln!(out, "    println!(\"1..{}\");", p.tests.len()).unwrap();
    writeln!(out, "    std::panic::set_hook(Box::new(|_| {{}}));").unwrap();
    writeln!(out, "    let mut failed = 0usize;").unwrap();
    writeln!(out, "    for (i, (name, f)) in tests.iter().enumerate() {{").unwrap();
    writeln!(
        out,
        "        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {{"
    )
    .unwrap();
    writeln!(out, "            let mut w = World::new();").unwrap();
    for (_, class) in &p.global_order {
        writeln!(out, "            w.singleton({class}::new);").unwrap();
    }
    // Runtime wrap so test bodies may call async fns; spawned tasks
    // settle (bounded) before the test is scored.
    writeln!(out, "            let __rt = Runtime::new(w);").unwrap();
    writeln!(out, "            __rt.with(|w| f(w));").unwrap();
    writeln!(out, "            let mut __spins = 0usize;").unwrap();
    writeln!(out, "            while __rt.has_tasks() {{").unwrap();
    writeln!(out, "                __rt.turn();").unwrap();
    writeln!(out, "                __rt.with(|w| w.flush());").unwrap();
    writeln!(out, "                __spins += 1;").unwrap();
    writeln!(out, "                if __spins > 5000 {{ panic!(\"async tasks did not settle\"); }}").unwrap();
    writeln!(out, "                std::thread::sleep(std::time::Duration::from_millis(1));").unwrap();
    writeln!(out, "            }}").unwrap();
    writeln!(out, "        }}));").unwrap();
    writeln!(out, "        match r {{").unwrap();
    writeln!(
        out,
        "            Ok(()) => println!(\"ok {{}} - {{}}\", i + 1, name),"
    )
    .unwrap();
    writeln!(out, "            Err(e) => {{").unwrap();
    writeln!(out, "                failed += 1;").unwrap();
    writeln!(
        out,
        "                let msg = e.downcast_ref::<String>().map(|s| s.as_str())"
    )
    .unwrap();
    writeln!(
        out,
        "                    .or_else(|| e.downcast_ref::<&str>().copied())"
    )
    .unwrap();
    writeln!(out, "                    .unwrap_or(\"panic\");").unwrap();
    writeln!(
        out,
        "                println!(\"not ok {{}} - {{}}: {{}}\", i + 1, name, msg);"
    )
    .unwrap();
    writeln!(out, "            }}").unwrap();
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "    if failed > 0 {{ std::process::exit(1); }}").unwrap();
    writeln!(out, "}}").unwrap();
    Ok(out)
}

fn emit_free_fn(
    f: &ast::FnDecl,
    p: &Program,
    out: &mut String,
) -> Result<(), EmitError> {
    let Some(body) = &f.body else {
        return err(
            f.span,
            format!(
                "`{}` has no body. A declaration without one belongs on a `trait`, \
                 where it is a requirement",
                f.name.name
            ),
        );
    };
    if !f.generics.is_empty() && f.is_async {
        return err(f.span, "generic async fns are not lowerable yet (M2)");
    }
    if f.is_async {
        if f.return_ty.is_some() {
            return err(
                f.span,
                "async fns don't declare return types yet (M2): spawned tasks are fire-and-forget",
            );
        }
        let mut cx = p.method_ctx(&p.empty_class, &f.params);
        write!(out, "fn {}(w: &mut World", camel_to_snake(&f.name.name)).unwrap();
        for param in &f.params {
            write!(
                out,
                ", {}: {}",
                camel_to_snake(&param.name.name),
                lower_type(&param.ty, &p.class_names)?.render()
            )
            .unwrap();
        }
        writeln!(out, ") {{").unwrap();
        writeln!(out, "    let __ctx = w.async_ctx();").unwrap();
        writeln!(out, "    w.spawn(async move {{").unwrap();
        let mut body_s = String::new();
        emit_async_body(body, &mut cx, &mut body_s, "        ")?;
        out.push_str(&body_s);
        writeln!(out, "    }});").unwrap();
        writeln!(out, "}}\n").unwrap();
        return Ok(());
    }
    let mut cx = p.method_ctx(&p.empty_class, &f.params);
    // Generic fns lower to REAL Rust generics — rustc owns the
    // monomorphization (§8.20).
    let generics = render_fn_generics(f, p)?;
    register_generic_locals(f, &mut cx);
    let ret = match &f.return_ty {
        Some(t) => Some(lower_return_type(t, p.default_error.as_deref(), &p.class_names)?),
        None => None,
    };
    cx.fallible_ret = matches!(&ret, Some(RustTy::Fallible { .. }));
    cx.nullable_ret = matches!(&ret, Some(RustTy::Opt(_)));
    write!(out, "fn {}{generics}(w: &mut World", camel_to_snake(&f.name.name)).unwrap();
    for param in &f.params {
        write!(
            out,
            ", {}: {}",
            camel_to_snake(&param.name.name),
            lower_type(&param.ty, &p.class_names)?.render()
        )
        .unwrap();
    }
    match &ret {
        Some(t) => writeln!(out, ") -> {} {{", t.render()).unwrap(),
        None => writeln!(out, ") {{").unwrap(),
    }
    let mut body_s = String::new();
    lower_scope(&body.stmts, body.trailing.as_deref(), false, &mut cx, &mut body_s, "    ")?;
    if let Some(trailing) = &body.trailing {
        if ret.is_some() {
            if cx.nullable_ret {
                let wrapped = lower_nullable_slot(trailing, &cx)?;
                writeln!(body_s, "    {wrapped}").unwrap();
            } else {
                let v = lower_method_expr(trailing, &cx)?;
                if cx.fallible_ret {
                    let wrapped = fallible_wrap(trailing, v, &cx);
                    writeln!(body_s, "    {wrapped}").unwrap();
                } else {
                    writeln!(body_s, "    {v}").unwrap();
                }
            }
        } else {
            let stmt = Stmt::Expr((**trailing).clone());
            lower_method_stmt(&stmt, &mut cx, &mut body_s, "    ")?;
        }
    } else if let Some(RustTy::Fallible { ok, .. }) = &ret {
        if matches!(**ok, RustTy::Unit) {
            writeln!(body_s, "    Ok(())").unwrap();
        }
    } else if matches!(&ret, Some(RustTy::Opt(_))) {
        writeln!(body_s, "    None").unwrap();
    }
    out.push_str(&body_s);
    writeln!(out, "}}\n").unwrap();
    Ok(())
}

fn collect_class<'a>(c: &'a ast::ClassDecl, classes: ClassNames<'_>, next_signal: &mut u32) -> Result<ClassInfo<'a>, EmitError> {
    if c.is_arc {
        return err(c.span, "`arc` was folded into plain classes in pixie");
    }
    if c.generics.iter().any(|g| !g.bounds.is_empty()) {
        return err(c.span, "bounded class-level generics are not lowerable yet (M2)");
    }
    if c.is_extern_value {
        return err(c.span, "`extern value` is replaced by `.rpi` bindings (M1)");
    }

    if let Some(s) = &c.super_class {
        return err(s.span, "pixie classes have no inheritance (D9): compose, or use a trait");
    }

    let mut info = ClassInfo {
        name: c.name.name.clone(),
        props: Vec::new(),
        signals: Vec::new(),
        methods: Vec::new(),
        own_method_count: 0,
        init: None,
        generics: Vec::new(),
        statics: Vec::new(),
        deinit: None,
    };

    for m in &c.members {
        match m {
            ClassMember::Property(p) => {
                // §8.61. `bind { .. }` is a DERIVED property and
                // lowers below. The other three are Qt storage
                // decisions with no pixie meaning, and each has a
                // pixie spelling that does the job.
                if p.bindable {
                    return err(
                        p.span,
                        "`bindable` chose a storage strategy for Qt's binding system. \
                         pixie has one reactive loop and every prop is already in it — \
                         drop the flag, or write `bind { .. }` for a derived value",
                    );
                }
                if p.constant {
                    return err(
                        p.span,
                        format!(
                            "`constant` opted a Qt property out of change notification. \
                             In pixie a value that never changes is a field: write \
                             `pub let {} : T` and give it its value in `init`",
                            p.name.name
                        ),
                    );
                }
                if p.model {
                    return err(
                        p.span,
                        "the item-model surface was retired — a `List<T>` prop IS the \
                         list a `for` repeater reads",
                    );
                }
                if let Some(f) = &p.fresh {
                    let _ = f;
                    return err(
                        p.span,
                        "`fresh { .. }` and `bind { .. }` differed only in Qt's caching \
                         strategy. A pixie derivation is evaluated on every read, so \
                         there is one spelling — write `bind { .. }`",
                    );
                }
                // `default:` may be omitted when an `init` assigns
                // the prop (validated below, §8.25).
                let default = p.default.clone();
                let notify_camel = match &p.notify {
                    Some(n) => n.name.clone(),
                    None => synth_notify_name(&p.name.name),
                };
                let const_name = format!("{}_{}", scream(&c.name.name), scream(&notify_camel));
                let ty = lower_type(&p.ty, classes)?;
                // `weak` says "this reference does not own". On a
                // property that holds no objects there is nothing to
                // own, so the modifier would be decoration that reads
                // like a lifetime decision (§8.44).
                if p.weak && !ty.holds_objects() {
                    return err(
                        p.span,
                        format!(
                            "`weak` needs a property that holds objects — `{}` holds values, \
                             which are copied, not referenced",
                            p.name.name
                        ),
                    );
                }
                let derived = p.binding.clone();
                if let Some(d) = &derived {
                    check_derivable(d)?;
                }
                info.props.push(PropInfo {
                    camel: p.name.name.clone(),
                    rust: camel_to_snake(&p.name.name),
                    ty,
                    default,
                    is_weak: p.weak,
                    notify_const: const_name.clone(),
                    assignable: derived.is_none(),
                    keyword: if derived.is_some() { "bind" } else { "prop" },
                    derived,
                });
                // A derived property has no signal of its own: it
                // changes exactly when something it reads changes, and
                // that already fires (§8.61).
                if info.props.last().is_some_and(|q| q.derived.is_some()) {
                    continue;
                }
                info.signals.push(SignalInfo {
                    camel: notify_camel,
                    const_name,
                    id: *next_signal,
                });
                *next_signal += 1;
            }
            ClassMember::Signal(s) => {
                // Not "(M0) yet" (§8.60): a pixie signal is a REBUILD
                // notification, and the only thing that subscribes to
                // one is a view — which re-reads the World rather than
                // receiving a value. There is no subscribe form for a
                // payload to arrive at, so a parameter list would name
                // an argument nothing can take.
                if let Some(q) = s.params.first() {
                    return err(
                        q.span,
                        format!(
                            "a signal says something happened, and a view answers it by \
                             re-reading the object — nothing receives an argument. Put \
                             `{}` in a `prop` and emit `{}` after writing it",
                            q.name.name, s.name.name
                        ),
                    );
                }
                // Skip if a prop already synthesized this notify.
                if info.signal(&s.name.name).is_some() {
                    continue;
                }
                let const_name = format!("{}_{}", scream(&c.name.name), scream(&s.name.name));
                info.signals.push(SignalInfo {
                    camel: s.name.name.clone(),
                    const_name,
                    id: *next_signal,
                });
                *next_signal += 1;
            }
            ClassMember::Fn(f) => {
                if f.is_static {
                    // An associated function: no receiver, no World,
                    // so it is exactly a free fn that lives under a
                    // class's name (§8.54).
                    if f.is_async {
                        return err(
                            f.span,
                            "a `static fn` has no World, so it cannot be async — make it a \
                             method or a free fn",
                        );
                    }
                    info.statics.push(f);
                    continue;
                }
                // `test fn` is an ITEM, so this is unreachable from
                // the parser (§8.63) — kept as a guard rather than an
                // `unreachable!`.
                if f.is_test {
                    return err(
                        f.span,
                        "a test is a top-level `test fn`, not a class method — move it \
                         out of the class",
                    );
                }
                info.methods.push(f);
            }
            ClassMember::Slot(f) => {
                return err(f.span, "`slot` was dropped in pixie; write `fn`");
            }
            // A plain `let` / `var` field (§8.58). Same machinery a
            // `prop` gets — the reactive edge included, so a view
            // bound to one still rebuilds — with two differences that
            // are the reason to write one: it is not part of the
            // class's declared interface, and `let` means init-once.
            ClassMember::Field(f) => {
                if f.unowned {
                    return err(
                        f.span,
                        "`unowned` was removed in pixie — a handle cannot dangle, so \
                         non-null non-owning has nothing to add. Use `weak` to break a \
                         reference cycle",
                    );
                }
                let ty = lower_type(&f.ty, classes)?;
                if f.weak && !ty.holds_objects() {
                    return err(
                        f.span,
                        format!(
                            "`weak` needs a field that holds objects — `{}` holds values, \
                             which are copied, not referenced",
                            f.name.name
                        ),
                    );
                }
                let notify_camel = synth_notify_name(&f.name.name);
                let const_name = format!("{}_{}", scream(&c.name.name), scream(&notify_camel));
                info.props.push(PropInfo {
                    camel: f.name.name.clone(),
                    rust: camel_to_snake(&f.name.name),
                    ty,
                    default: f.default.clone(),
                    is_weak: f.weak,
                    notify_const: const_name.clone(),
                    assignable: f.is_mut,
                    keyword: if f.is_mut { "var" } else { "let" },
                    derived: None,
                });
                info.signals.push(SignalInfo {
                    camel: notify_camel,
                    const_name,
                    id: *next_signal,
                });
                *next_signal += 1;
                continue;
            }
            ClassMember::Init(i) => {
                if info.init.is_some() {
                    return err(i.span, "one `init` per class in v1 (overloads are M2)");
                }
                info.init = Some(i);
            }
            ClassMember::Deinit(d) => {
                if info.deinit.is_some() {
                    return err(d.span, "one `deinit` per class");
                }
                info.deinit = Some(d);
            }
        }
    }
    info.own_method_count = info.methods.len();
    info.generics = c.generics.iter().map(|g| g.name.name.clone()).collect();
    // Definite assignment: every default-less prop must be assigned
    // (a top-level `name = …`) by the init. rustc's deferred-init
    // check backs this up; the point here is a pixie-named error.
    for prop in &info.props {
        // A derived property needs neither: its value comes from the
        // `bind { .. }` body every time it is read (§8.61).
        if prop.derived.is_some() {
            continue;
        }
        if prop.default.is_none() {
            let assigned = info.init.is_some_and(|i| {
                i.body.stmts.iter().any(|s| matches!(s,
                    ast::Stmt::Assign { target, op: ast::AssignOp::Eq, .. }
                        if matches!(&target.kind, ExprKind::Ident(n) | ExprKind::AtIdent(n) if *n == prop.camel)))
            });
            if !assigned {
                return err(
                    c.span,
                    format!(
                        "prop `{}` has no `default:` and no unconditional `init` assignment",
                        prop.camel
                    ),
                );
            }
        }
    }
    Ok(info)
}

/// One interpolated piece of a constant string default. It has no
/// declared type of its own, so `lower_default` is tried against each
/// primitive in turn — the one that matches is the piece's type.
fn lower_default_display(e: &Expr) -> Result<String, EmitError> {
    for ty in [RustTy::Int, RustTy::Float, RustTy::Bool, RustTy::Str] {
        if let Ok(x) = lower_default(e, &ty) {
            return Ok(x);
        }
    }
    err(
        e.span,
        "a default is evaluated before the object exists, so every piece of an \
         interpolated one has to be a constant too",
    )
}

fn lower_default(e: &Expr, ty: &RustTy) -> Result<String, EmitError> {
    // Enum defaults: `default: Color.Red`.
    if let (ExprKind::Member { receiver, name }, RustTy::Named(n)) = (&e.kind, ty) {
        if matches!(&receiver.kind, ExprKind::Ident(r) if r == n) {
            return Ok(format!("{n}::{}", escape_rust_keyword(name.name.clone())));
        }
    }
    match (&e.kind, ty) {
        (ExprKind::Int(v), RustTy::Int) => Ok(format!("{v}i64")),
        (ExprKind::Float(v), RustTy::Float) => Ok(format!("{v}f64")),
        // §8.55: an Int literal in a Float slot. The checker widens it,
        // so the default has to widen with it.
        (ExprKind::Int(v), RustTy::Float) => Ok(format!("{v}f64")),
        (ExprKind::Bool(v), RustTy::Bool) => Ok(format!("{v}")),
        (ExprKind::Str(parts), RustTy::Str) => {
            // An interpolation whose pieces are themselves constant
            // (§8.59): `default: "v#{MAJOR}.#{MINOR}"` builds at
            // construction time like any other constant expression.
            // The pieces recurse through this same function, so a
            // piece that reads the World reports the constant rule
            // rather than a missing receiver.
            let all_text = parts.iter().all(|p| matches!(p, StrPart::Text(_)));
            if !all_text {
                return lower_interp(parts, &mut |inner| lower_default_display(inner));
            }
            let mut s = String::new();
            for p in parts {
                if let StrPart::Text(t) = p {
                    escape_fmt_text(t, &mut s);
                }
            }
            Ok(format!("Str::from(\"{s}\")"))
        }
        // A class-typed default (§8.64). The object cannot exist yet —
        // `new()` has no World — so the slot starts empty and `main`
        // constructs it right after the store, where the World does
        // exist. `pending_object_default` is what decides that this
        // arm applies; keep the two in step.
        (ExprKind::Call { .. }, RustTy::Handle(c)) => {
            Ok(format!("Handle::<{c}>::PENDING"))
        }
        // `T?` defaults (§8.68): `nil` is the empty one, and anything
        // else is the value wrapped — the same automatic `some` a
        // return position gets.
        (ExprKind::Nil, RustTy::Opt(_)) => Ok("None".into()),
        (_, RustTy::Opt(inner)) => Ok(format!("Some({})", lower_default(e, inner)?)),
        // A map literal, keys and values both constant.
        (ExprKind::Map(entries), RustTy::Map(kt, vt)) => {
            let mut out = String::from("{ let mut __m = Map::new();");
            for (k, v) in entries {
                // An identifier key is string-key sugar (the parser
                // keeps it as an Ident so the checker can tell the
                // two apart).
                let kx = match (&k.kind, &**kt) {
                    (ExprKind::Ident(n), RustTy::Str) => format!("Str::from({n:?})"),
                    _ => lower_default(k, kt)?,
                };
                out.push_str(&format!(" __m.insert({kx}, {});", lower_default(v, vt)?));
            }
            out.push_str(" __m }");
            Ok(out)
        }
        // `[]` against `Bytes` — the empty byte string (§8.68). Bytes
        // arrive from a file or a response and there is no literal
        // for one, so the only value a default can name is the empty
        // one, and this is the empty-sequence spelling pixie has.
        (ExprKind::Array(items), RustTy::Bytes) if items.is_empty() => Ok("Bytes::new()".into()),
        // A default that CONSTRUCTS a struct (§8.68). A struct is a
        // value, so `P(1, 2)` is as constant as its arguments — the
        // same trailing-defaults rule construction sites use.
        (ExprKind::Call { callee, args, .. }, RustTy::Named(n))
            if matches!(&callee.kind, ExprKind::Ident(c) if c == n)
                && STRUCT_LAYOUTS.with(|m| m.borrow().contains_key(n)) =>
        {
            let layout = STRUCT_LAYOUTS.with(|m| m.borrow().get(n).cloned()).expect("guarded");
            let required = layout
                .iter()
                .rposition(|(_, _, d)| d.is_none())
                .map(|i| i + 1)
                .unwrap_or(0);
            if args.len() > layout.len() || args.len() < required {
                return err(
                    e.span,
                    format!("`{n}` takes {required} to {} field value(s)", layout.len()),
                );
            }
            let mut lit = format!("{n} {{ ");
            for (i, (rust, fty, fdefault)) in layout.iter().enumerate() {
                if i > 0 {
                    lit.push_str(", ");
                }
                let v = match args.get(i) {
                    Some(a) => lower_default(a, fty)?,
                    None => lower_default(fdefault.as_ref().expect("required checked"), fty)?,
                };
                write!(lit, "{rust}: {v}").unwrap();
            }
            lit.push_str(" }");
            Ok(lit)
        }
        (ExprKind::Array(items), RustTy::List(_)) if items.is_empty() => Ok("List::new()".into()),
        // A default is evaluated before any object exists, so it has
        // to be constant — but "constant" includes arithmetic and a
        // sign (§8.54). `default: -1` and `default: 60 * 60` were
        // errors, which is a limit of this match rather than of the
        // rule, and `-1` in particular is a common default.
        (ExprKind::Unary { op, expr }, _) => {
            let inner = lower_default(expr, ty)?;
            match op {
                ast::UnaryOp::Neg => Ok(format!("(-{inner})")),
                ast::UnaryOp::Not => Ok(format!("(!{inner})")),
            }
        }
        (ExprKind::Binary { op, lhs, rhs }, _) => {
            let l = lower_default(lhs, ty)?;
            let r = lower_default(rhs, ty)?;
            Ok(format!(
                "({} {} {})",
                cast_if_widened(&l, lhs.span),
                bin_op(op, e.span)?,
                cast_if_widened(&r, rhs.span)
            ))
        }
        // A non-empty list of constants.
        (ExprKind::Array(items), RustTy::List(elem)) => {
            let mut out = String::from("{ let mut __d = List::new(); ");
            for it in items {
                let v = lower_default(it, elem)?;
                write!(out, "__d.push({v}); ").unwrap();
            }
            out.push_str("__d }");
            Ok(out)
        }
        _ => err(
            e.span,
            "a default is evaluated before the object exists, so it has to be a \
             constant — a literal, a list of literals, or arithmetic over them",
        ),
    }
}


/// `<T: Clone + 'static, …>` for a generic class's declaration side
/// (`'static` because instances live in the Any-keyed World), and
/// the bare `<T, …>` use side. Empty strings for plain classes.
fn class_generics_decl(info: &ClassInfo) -> String {
    if info.generics.is_empty() {
        return String::new();
    }
    let ps: Vec<String> = info
        .generics
        .iter()
        .map(|g| format!("{g}: Clone + 'static"))
        .collect();
    format!("<{}>", ps.join(", "))
}

fn class_generics_use(info: &ClassInfo) -> String {
    if info.generics.is_empty() {
        return String::new();
    }
    format!("<{}>", info.generics.join(", "))
}

/// The `init` body's expression subset — World-free by construction
/// (the object does not exist yet): params/locals, prop slots
/// (`__p_<name>`), literals, arithmetic, interpolation, list/map
/// literals. Everything else is a named error (D10).
fn lower_init_expr(
    e: &Expr,
    info: &ClassInfo,
    locals: &std::collections::HashSet<String>,
) -> Result<String, EmitError> {
    Ok(cast_if_widened(&lower_init_expr_inner(e, info, locals)?, e.span))
}

fn lower_init_expr_inner(
    e: &Expr,
    info: &ClassInfo,
    locals: &std::collections::HashSet<String>,
) -> Result<String, EmitError> {
    use ExprKind as K;
    match &e.kind {
        K::Int(v) => Ok(format!("{v}i64")),
        K::Float(v) => Ok(format!("{v}f64")),
        K::Bool(v) => Ok(format!("{v}")),
        K::Str(parts) => lower_interp(parts, &mut |inner| lower_init_expr(inner, info, locals)),
        K::Ident(n) | K::AtIdent(n) => {
            if let Some(prop) = info.prop(n) {
                return Ok(format!("__p_{}.clone()", prop.rust));
            }
            if locals.contains(n) {
                return Ok(format!("{}.clone()", camel_to_snake(n)));
            }
            if n == "this" {
                return err(
                    e.span,
                    "`init` runs before the object exists, so there is no `this` yet — \
                     assign the properties and use it after construction",
                );
            }
            err(e.span, format!("`{n}` is not a param, local, or prop (init bodies are World-free)"))
        }
        K::Binary { op, lhs, rhs } => {
            let l = lower_init_expr(lhs, info, locals)?;
            let r = lower_init_expr(rhs, info, locals)?;
            Ok(format!("({l} {} {r})", bin_op(op, e.span)?))
        }
        K::Unary { op, expr } => {
            let inner = lower_init_expr(expr, info, locals)?;
            match op {
                ast::UnaryOp::Neg => Ok(format!("(-{inner})")),
                ast::UnaryOp::Not => Ok(format!("(!{inner})")),
            }
        }
        K::Array(items) => {
            if items.is_empty() {
                return Ok("List::new()".into());
            }
            let mut out = String::from("{ let mut __lit = List::new(); ");
            for item in items {
                let v = lower_init_expr(item, info, locals)?;
                write!(out, "__lit.push({v}); ").unwrap();
            }
            out.push_str("__lit }");
            Ok(out)
        }
        K::Map(entries) => {
            if entries.is_empty() {
                return Ok("Map::new()".into());
            }
            let mut out = String::from("{ let mut __lit = Map::new(); ");
            for (k, v) in entries {
                let kv = match &k.kind {
                    K::Ident(name) => format!("Str::from({:?})", name),
                    _ => lower_init_expr(k, info, locals)?,
                };
                let vv = lower_init_expr(v, info, locals)?;
                write!(out, "__lit.insert({kv}, {vv}); ").unwrap();
            }
            out.push_str("__lit }");
            Ok(out)
        }
        _ => err(e.span, "this expression is not lowerable in `init` yet (init bodies are World-free)"),
    }
}

fn lower_init_stmt(
    s: &Stmt,
    info: &ClassInfo,
    locals: &mut std::collections::HashSet<String>,
    out: &mut String,
    ind: &str,
) -> Result<(), EmitError> {
    match s {
        Stmt::Let { name, value, .. } | Stmt::Var { name, value, .. } => {
            let v = lower_init_expr(value, info, locals)?;
            let mutkw = if matches!(s, Stmt::Var { .. }) { "mut " } else { "" };
            writeln!(out, "{ind}let {mutkw}{} = {v};", camel_to_snake(&name.name)).unwrap();
            locals.insert(name.name.clone());
            Ok(())
        }
        Stmt::Assign { target, op, value, span } => {
            let slot = match &target.kind {
                ExprKind::Ident(n) | ExprKind::AtIdent(n) => {
                    if let Some(prop) = info.prop(n) {
                        format!("__p_{}", prop.rust)
                    } else if locals.contains(n) {
                        camel_to_snake(n)
                    } else {
                        return err(*span, format!("`{n}` is not assignable in `init`"));
                    }
                }
                _ => return err(*span, "only prop / local assignment is lowerable in `init`"),
            };
            let v = lower_init_expr(value, info, locals)?;
            let sym = match op {
                ast::AssignOp::Eq => "=",
                ast::AssignOp::PlusEq => "+=",
                ast::AssignOp::MinusEq => "-=",
                ast::AssignOp::StarEq => "*=",
                ast::AssignOp::SlashEq => "/=",
            };
            writeln!(out, "{ind}{slot} {sym} {v};").unwrap();
            Ok(())
        }
        Stmt::Expr(e) => {
            if let ExprKind::If { cond, then_b, else_b, let_binding } = &e.kind {
                if let_binding.is_some() {
                    return err(e.span, "`if let` survived the desugar (§8.69) — this is a pixie bug");
                }
                let c = lower_init_expr(cond, info, locals)?;
                writeln!(out, "{ind}if {c} {{").unwrap();
                let inner = format!("{ind}    ");
                for s2 in &then_b.stmts {
                    lower_init_stmt(s2, info, locals, out, &inner)?;
                }
                if let Some(eb) = else_b {
                    writeln!(out, "{ind}}} else {{").unwrap();
                    for s2 in &eb.stmts {
                        lower_init_stmt(s2, info, locals, out, &inner)?;
                    }
                }
                writeln!(out, "{ind}}}").unwrap();
                return Ok(());
            }
            err(e.span, "this statement is not lowerable in `init` yet")
        }
        _ => err(
            match s {
                Stmt::Return { span, .. } | Stmt::For { span, .. } | Stmt::While { span, .. } => *span,
                _ => Span::new(FileId(0), 0, 0),
            },
            "this statement is not lowerable in `init` yet",
        ),
    }
}

fn emit_class(info: &ClassInfo, p: &Program, out: &mut String) -> Result<(), EmitError> {
    let name = &info.name;

    // Storage struct + constructor (prop defaults, or the user
    // `init` — §8.25). Generic classes carry their params on every
    // shape; rustc monomorphizes per instantiation.
    let gd = class_generics_decl(info);
    let gu = class_generics_use(info);
    writeln!(out, "pub struct {name}{gd} {{").unwrap();
    for p in &info.props {
        // A derived property stores nothing (§8.61).
        if p.derived.is_some() {
            continue;
        }
        writeln!(out, "    {}: {},", p.rust, p.ty.render()).unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    writeln!(out, "impl{gd} {name}{gu} {{").unwrap();
    match info.init {
        None => {
            writeln!(out, "    pub fn new() -> Self {{").unwrap();
            writeln!(out, "        Self {{").unwrap();
            for p in &info.props {
                if p.derived.is_some() {
                    continue;
                }
                let d = p.default.as_ref().expect("no-default props require init (validated)");
                writeln!(out, "            {}: {},", p.rust, lower_default(d, &p.ty)?).unwrap();
            }
            writeln!(out, "        }}").unwrap();
            writeln!(out, "    }}").unwrap();
        }
        Some(init) => {
            write!(out, "    pub fn new(").unwrap();
            for (i, param) in init.params.iter().enumerate() {
                if i > 0 {
                    write!(out, ", ").unwrap();
                }
                write!(out, "{}: {}", camel_to_snake(&param.name.name), lower_type(&param.ty, &p.class_names)?.render()).unwrap();
            }
            writeln!(out, ") -> Self {{").unwrap();
            // Prop slots: defaults seed them; default-less ones are
            // deferred lets — rustc's definite-assignment check backs
            // the collector's syntactic one.
            for p in &info.props {
                if p.derived.is_some() {
                    continue;
                }
                match &p.default {
                    Some(d) => writeln!(
                        out,
                        "        let mut __p_{}: {} = {};",
                        p.rust,
                        p.ty.render(),
                        lower_default(d, &p.ty)?
                    )
                    .unwrap(),
                    None => writeln!(out, "        let __p_{}: {};", p.rust, p.ty.render()).unwrap(),
                }
            }
            let mut locals: std::collections::HashSet<String> =
                init.params.iter().map(|q| q.name.name.clone()).collect();
            let mut body_s = String::new();
            for s in &init.body.stmts {
                lower_init_stmt(s, info, &mut locals, &mut body_s, "        ")?;
            }
            out.push_str(&body_s);
            writeln!(out, "        Self {{").unwrap();
            for p in &info.props {
                if p.derived.is_some() {
                    continue;
                }
                writeln!(out, "            {}: __p_{},", p.rust, p.rust).unwrap();
            }
            writeln!(out, "        }}").unwrap();
            writeln!(out, "    }}").unwrap();
        }
    }
    writeln!(out, "}}\n").unwrap();

    for s in &info.signals {
        writeln!(out, "pub const {}: SignalId = {};", s.const_name, s.id).unwrap();
    }
    out.push('\n');

    // Extension trait over Handle<C> — the cross-crate-legal stereotype.
    writeln!(out, "pub trait {name}Ref{gd}: Copy {{").unwrap();
    for p in &info.props {
        writeln!(out, "    fn {}(self, w: &World) -> {};", p.rust, p.ty.render()).unwrap();
        // A `let` field takes its value in `init`, which writes the
        // struct field directly — so it needs no setter at all, and
        // emitting a dead one would misdescribe the class (§8.58).
        if !p.assignable {
            continue;
        }
        writeln!(out, "    fn set_{}(self, w: &mut World, v: {});", p.rust, p.ty.render()).unwrap();
        if let RustTy::List(elem) = &p.ty {
            writeln!(
                out,
                "    fn push_{}(self, w: &mut World, v: {});",
                p.rust,
                elem.render()
            )
            .unwrap();
        }
        // `m[k] = v` (map twin of §8.67): value-typed maps get an
        // in-place insert; object-valued maps wait on the replaced
        // value's release story.
        if let RustTy::Map(kt, vt) = &p.ty {
            if !vt.holds_objects() {
                writeln!(
                    out,
                    "    fn insert_{}(self, w: &mut World, k: {}, v: {});",
                    p.rust,
                    kt.render(),
                    vt.render()
                )
                .unwrap();
            }
        }
    }
    for m in &info.methods[..info.own_method_count] {
        if m.is_async && m.return_ty.is_some() {
            return err(
                m.span,
                "async fns don't declare return types yet (M2): spawned tasks are fire-and-forget",
            );
        }
        if m.is_async && !m.generics.is_empty() {
            return err(m.span, "generic async fns are not lowerable yet (M2)");
        }
        let generics = render_fn_generics(m, p)?;
        write!(out, "    fn {}{generics}(self, w: &mut World", camel_to_snake(&m.name.name)).unwrap();
        for param in &m.params {
            write!(out, ", {}: {}", camel_to_snake(&param.name.name), lower_type(&param.ty, &p.class_names)?.render()).unwrap();
        }
        match &m.return_ty {
            Some(t) => writeln!(
                out,
                ") -> {};",
                lower_return_type(t, p.default_error.as_deref(), &p.class_names)?.render()
            )
            .unwrap(),
            None => writeln!(out, ");").unwrap(),
        }
    }
    // `deinit` (§8.60) rides the method machinery — it IS a method
    // body with a receiver and a World — but the kernel calls it, not
    // pixie code, so it wears a reserved name.
    if info.deinit.is_some() {
        writeln!(out, "    fn __pixie_deinit(self, w: &mut World);").unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    // `static fn` members (§8.54): plain associated functions. No
    // receiver, no World — a free fn that lives under the class's
    // name, which is what makes `C.parse(s)` spell what it means.
    if !info.statics.is_empty() {
        writeln!(out, "impl{gd} {name}{gu} {{").unwrap();
        for f in &info.statics {
            let Some(body) = &f.body else {
                return err(
                    f.span,
                    format!(
                        "`{}` has no body. A class method is the implementation; a \
                         requirement without one belongs on a `trait`",
                        f.name.name
                    ),
                );
            };
            let mut cx = p.method_ctx(&p.empty_class, &f.params);
            let ret = match &f.return_ty {
                Some(t) => Some(lower_return_type(t, p.default_error.as_deref(), &p.class_names)?),
                None => None,
            };
            cx.fallible_ret = matches!(&ret, Some(RustTy::Fallible { .. }));
            cx.nullable_ret = matches!(&ret, Some(RustTy::Opt(_)));
            let generics = render_fn_generics(f, p)?;
            write!(out, "    pub fn {}{generics}(", camel_to_snake(&f.name.name)).unwrap();
            for (i, param) in f.params.iter().enumerate() {
                if i > 0 {
                    write!(out, ", ").unwrap();
                }
                write!(
                    out,
                    "{}: {}",
                    camel_to_snake(&param.name.name),
                    lower_type(&param.ty, &p.class_names)?.render()
                )
                .unwrap();
            }
            match &ret {
                Some(t) => writeln!(out, ") -> {} {{", t.render()).unwrap(),
                None => writeln!(out, ") {{").unwrap(),
            }
            let mut body_s = String::new();
            lower_scope(&body.stmts, body.trailing.as_deref(), false, &mut cx, &mut body_s, "        ")?;
            if let Some(trailing) = &body.trailing {
                if ret.is_some() {
                    let v = lower_method_expr(trailing, &cx)?;
                    writeln!(body_s, "        {v}").unwrap();
                } else {
                    let stmt = Stmt::Expr((**trailing).clone());
                    lower_method_stmt(&stmt, &mut cx, &mut body_s, "        ")?;
                }
            }
            out.push_str(&body_s);
            writeln!(out, "    }}").unwrap();
        }
        writeln!(out, "}}\n").unwrap();
    }

    writeln!(out, "impl{gd} {name}Ref{gu} for Handle<{name}{gu}> {{").unwrap();
    // `pi` rather than `p`: the Program is `p` here, and a derived
    // getter needs it to build a method context.
    for pi in &info.props {
        writeln!(out, "    fn {g}(self, w: &World) -> {t} {{", g = pi.rust, t = pi.ty.render()).unwrap();
        match &pi.derived {
            // Evaluated on every read (§8.61). `check_derivable`
            // already proved the body only reads, so `&World` is
            // enough — the same `self.<prop>(w)` a method body emits.
            Some(d) => {
                let cx = p.method_ctx(info, &[]);
                let v = lower_method_expr(d, &cx)?;
                writeln!(out, "        {v}").unwrap();
            }
            // A weak `T?` read answers nil once the target is gone —
            // §8.44's contract. The stored handle goes stale rather
            // than null (nothing owns it), so the getter filters
            // liveness; returning the stale handle would panic at the
            // NEXT read instead of reading as None (gate-caught).
            // Other weak shapes (a weak LIST) keep the raw clone —
            // their per-element staleness story is a recorded residue.
            None if pi.is_weak
                && matches!(&pi.ty, RustTy::Opt(inner) if matches!(**inner, RustTy::Handle(_))) =>
            {
                writeln!(
                    out,
                    "        w.get(self).{}.clone().filter(|__h| w.try_get(*__h).is_some())",
                    pi.rust
                )
                .unwrap()
            }
            None => writeln!(out, "        w.get(self).{}.clone()", pi.rust).unwrap(),
        }
        writeln!(out, "    }}").unwrap();
        // The trait declared no setter for a `let` field (§8.58).
        if !pi.assignable {
            continue;
        }
        writeln!(out, "    fn set_{g}(self, w: &mut World, v: {t}) {{", g = pi.rust, t = pi.ty.render()).unwrap();
        // §8.44: a property that holds OBJECTS is a counted edge, so
        // overwriting it retains the arrivals and releases the
        // departures. Retain first — assigning a value to itself must
        // not drop the count to zero in between. A `weak` property
        // counts nothing: it is the cycle breaker, and its reads are
        // already stale-aware because handles are generational.
        let counted = !pi.is_weak && pi.ty.holds_objects();
        if counted {
            writeln!(out, "        {}", retain_expr(&pi.ty, "v")).unwrap();
        }
        if pi.ty.dirty_checks() {
            // One `get_mut`, not a `get` for the comparison and a
            // `get_mut` for the write: each World access is a bounds
            // check, a generation compare and a downcast, and the
            // setter was paying for two of them where one does
            // (§8.50 — measured at 38 % of the cost of a property
            // assignment). The old value comes out of the same borrow.
            writeln!(out, "        let __slot = w.get_mut(self);").unwrap();
            writeln!(out, "        if __slot.{} != v {{", pi.rust).unwrap();
            if counted {
                writeln!(out, "            let __old = std::mem::replace(&mut __slot.{}, v);", pi.rust).unwrap();
                writeln!(out, "            {}", release_expr(&pi.ty, "__old")).unwrap();
            } else {
                writeln!(out, "            __slot.{} = v;", pi.rust).unwrap();
            }
            writeln!(out, "            w.notify_changed(self.erase(), {});", pi.notify_const).unwrap();
            writeln!(out, "        }}").unwrap();
            if counted {
                // The no-change branch already retained; balance it.
                writeln!(out, "        else {{ {} }}", release_expr(&pi.ty, "v")).unwrap();
            }
        } else {
            if counted {
                writeln!(out, "        let __old = w.get(self).{}.clone();", pi.rust).unwrap();
            }
            writeln!(out, "        w.get_mut(self).{} = v;", pi.rust).unwrap();
            if counted {
                writeln!(out, "        {}", release_expr(&pi.ty, "__old")).unwrap();
            }
            writeln!(out, "        w.notify_changed(self.erase(), {});", pi.notify_const).unwrap();
        }
        writeln!(out, "    }}").unwrap();
        // Appending, as one operation rather than read-modify-write.
        //
        // The old stereotype was `let mut xs = self.g(w); xs.push(v);
        // self.set_g(w, xs)`, which CLONED the whole vector on every
        // push: the field still held a reference, so `List::push`'s
        // `Rc::make_mut` had to copy. Filling a list was quadratic
        // long before §8.44 counted anything (measured: 160 000
        // pushes took 1.8 s of pure copying). Taking the list OUT
        // first makes the local its only owner, so the push lands in
        // place — and it also means an object list retains exactly
        // the arriving element instead of re-counting every element
        // it already held.
        if let RustTy::List(elem) = &pi.ty {
            writeln!(
                out,
                "    fn push_{g}(self, w: &mut World, v: {t}) {{",
                g = pi.rust,
                t = elem.render()
            )
            .unwrap();
            if !pi.is_weak && elem.holds_objects() {
                writeln!(out, "        {}", retain_expr(elem, "v")).unwrap();
            }
            writeln!(
                out,
                "        let mut __xs = std::mem::take(&mut w.get_mut(self).{});",
                pi.rust
            )
            .unwrap();
            writeln!(out, "        __xs.push(v);").unwrap();
            writeln!(out, "        w.get_mut(self).{} = __xs;", pi.rust).unwrap();
            writeln!(
                out,
                "        w.notify_changed(self.erase(), {});",
                pi.notify_const
            )
            .unwrap();
            writeln!(out, "    }}").unwrap();
        }
        // The map twin: take the map OUT so `Rc::make_mut` finds one
        // owner and the insert lands in place — no per-write clone.
        if let RustTy::Map(kt, vt) = &pi.ty {
            if !vt.holds_objects() {
                writeln!(
                    out,
                    "    fn insert_{g}(self, w: &mut World, k: {k}, v: {v}) {{",
                    g = pi.rust,
                    k = kt.render(),
                    v = vt.render()
                )
                .unwrap();
                writeln!(
                    out,
                    "        let mut __m = std::mem::take(&mut w.get_mut(self).{});",
                    pi.rust
                )
                .unwrap();
                writeln!(out, "        __m.insert(k, v);").unwrap();
                writeln!(out, "        w.get_mut(self).{} = __m;", pi.rust).unwrap();
                writeln!(
                    out,
                    "        w.notify_changed(self.erase(), {});",
                    pi.notify_const
                )
                .unwrap();
                writeln!(out, "    }}").unwrap();
            }
        }
    }
    for m in &info.methods[..info.own_method_count] {
        let Some(body) = &m.body else {
            return err(
                m.span,
                format!(
                    "`{}` has no body. A class method is the implementation; a \
                     requirement without one belongs on a `trait`",
                    m.name.name
                ),
            );
        };
        if m.is_async {
            // Call sites stay sync: the method spawns its body as a
            // task and returns immediately.
            let mut cx = p.method_ctx(info, &m.params);
            write!(out, "    fn {}(self, w: &mut World", camel_to_snake(&m.name.name)).unwrap();
            for param in &m.params {
                write!(
                    out,
                    ", {}: {}",
                    camel_to_snake(&param.name.name),
                    lower_type(&param.ty, &p.class_names)?.render()
                )
                .unwrap();
            }
            writeln!(out, ") {{").unwrap();
            writeln!(out, "        let __ctx = w.async_ctx();").unwrap();
            writeln!(out, "        w.spawn(async move {{").unwrap();
            let mut body_s = String::new();
            emit_async_body(body, &mut cx, &mut body_s, "            ")?;
            out.push_str(&body_s);
            writeln!(out, "        }});").unwrap();
            writeln!(out, "    }}").unwrap();
            continue;
        }
        emit_sync_method(m, info, p, out)?;
    }
    if let Some(d) = info.deinit {
        if !info.generics.is_empty() {
            return err(
                d.span,
                "a generic class cannot declare `deinit` yet (M2): the kernel holds one \
                 fn pointer per concrete type",
            );
        }
        let mut cx = p.method_ctx(info, &[]);
        writeln!(out, "    fn __pixie_deinit(self, w: &mut World) {{").unwrap();
        let mut body_s = String::new();
        lower_scope(
            &d.body.stmts,
            d.body.trailing.as_deref(),
            true,
            &mut cx,
            &mut body_s,
            "        ",
        )?;
        out.push_str(&body_s);
        writeln!(out, "    }}").unwrap();
    }
    writeln!(out, "}}\n").unwrap();
    Ok(())
}

/// One w-threaded `fn m(self, w: &mut World, ...)` body — shared by
/// the ClassRef extension impl and the real trait-impl blocks
/// (§8.20). The receiver is the class handle either way.
fn emit_sync_method(
    m: &ast::FnDecl,
    info: &ClassInfo,
    p: &Program,
    out: &mut String,
) -> Result<(), EmitError> {
    let Some(body) = &m.body else {
        return err(
            m.span,
            format!(
                "`{}` has no body. A class method is the implementation; a requirement \
                 without one belongs on a `trait`",
                m.name.name
            ),
        );
    };
    let mut cx = p.method_ctx(info, &m.params);
    // Method-level generics ride the free-fn machinery (§8.24).
    let generics = render_fn_generics(m, p)?;
    register_generic_locals(m, &mut cx);
    let ret = match &m.return_ty {
        Some(t) => Some(lower_return_type(t, p.default_error.as_deref(), &p.class_names)?),
        None => None,
    };
    cx.fallible_ret = matches!(&ret, Some(RustTy::Fallible { .. }));
    cx.nullable_ret = matches!(&ret, Some(RustTy::Opt(_)));
    write!(out, "    fn {}{generics}(self, w: &mut World", camel_to_snake(&m.name.name)).unwrap();
    for param in &m.params {
        write!(out, ", {}: {}", camel_to_snake(&param.name.name), lower_type(&param.ty, &p.class_names)?.render()).unwrap();
    }
    match &ret {
        Some(t) => writeln!(out, ") -> {} {{", t.render()).unwrap(),
        None => writeln!(out, ") {{").unwrap(),
    }
    let mut body_s = String::new();
    lower_scope(&body.stmts, body.trailing.as_deref(), false, &mut cx, &mut body_s, "        ")?;
    if let Some(trailing) = &body.trailing {
        if ret.is_some() {
            if cx.nullable_ret {
                let wrapped = lower_nullable_slot(trailing, &cx)?;
                writeln!(body_s, "        {wrapped}").unwrap();
            } else {
                let v = lower_method_expr(trailing, &cx)?;
                if cx.fallible_ret {
                    let wrapped = fallible_wrap(trailing, v, &cx);
                    writeln!(body_s, "        {wrapped}").unwrap();
                } else {
                    writeln!(body_s, "        {v}").unwrap();
                }
            }
        } else {
            // A void fn's trailing expression is just its last
            // statement — route it through the statement path so
            // statement-only stereotypes (prop.push) apply.
            let stmt = Stmt::Expr((**trailing).clone());
            lower_method_stmt(&stmt, &mut cx, &mut body_s, "        ")?;
        }
    } else if let Some(RustTy::Fallible { ok, .. }) = &ret {
        if matches!(**ok, RustTy::Unit) {
            writeln!(body_s, "        Ok(())").unwrap();
        }
    } else if matches!(&ret, Some(RustTy::Opt(_))) {
        writeln!(body_s, "        None").unwrap();
    }
    out.push_str(&body_s);
    writeln!(out, "    }}").unwrap();
    Ok(())
}

/// A declared pixie trait as a REAL Rust trait over class handles —
/// what generic fns bound against it compile to (§8.20). `Copy` is
/// the supertrait because implementors are `Handle<C>`.
fn emit_trait(t: &ast::TraitDecl, p: &Program, out: &mut String) -> Result<(), EmitError> {
    // `Clone`, not `Copy`: a handle is Copy and a value is not, and a
    // trait that demanded Copy could only ever be implemented by
    // objects (§8.49). Every pixie type clones.
    writeln!(out, "pub trait {}: Clone {{", t.name.name).unwrap();
    for m in &t.methods {
        if m.is_async {
            return err(m.span, "async trait methods are not lowerable yet (M2)");
        }
        write!(out, "    fn {}(self, w: &mut World", camel_to_snake(&m.name.name)).unwrap();
        for param in &m.params {
            write!(out, ", {}: {}", camel_to_snake(&param.name.name), lower_type(&param.ty, &p.class_names)?.render()).unwrap();
        }
        match &m.return_ty {
            Some(ty) => writeln!(
                out,
                ") -> {};",
                lower_return_type(ty, p.default_error.as_deref(), &p.class_names)?.render()
            )
            .unwrap(),
            None => writeln!(out, ");").unwrap(),
        }
    }
    writeln!(out, "}}\n").unwrap();
    Ok(())
}

/// `impl Trait for Handle<Class>` blocks — the bodies the collection
/// pass routed away from the ClassRef splice.
fn emit_trait_impls(p: &Program, out: &mut String) -> Result<(), EmitError> {
    for (trait_name, class_name, i) in &p.trait_impls {
        let info = &p.classes[class_name];
        writeln!(out, "impl {trait_name} for Handle<{class_name}> {{").unwrap();
        for m in &i.methods {
            emit_sync_method(m, info, p, out)?;
        }
        writeln!(out, "}}\n").unwrap();
    }
    // Value types implement the same trait with the same signature
    // (§8.49). The World parameter comes along and goes unused — a
    // value has no World to consult — which is what lets ONE trait
    // abstract over both halves of the type system instead of two.
    for (trait_name, struct_name, i) in &p.struct_trait_impls {
        let st = &p.structs[struct_name];
        writeln!(out, "impl {trait_name} for {struct_name} {{").unwrap();
        for m in &i.methods {
            emit_struct_trait_method(m, st, p, out)?;
        }
        writeln!(out, "}}\n").unwrap();
    }
    Ok(())
}

/// One trait method on a value type: the trait's signature (`self`,
/// `w`), a struct's body context (`self.field` reads the value).
fn emit_struct_trait_method(
    m: &ast::FnDecl,
    st: &StructInfo,
    p: &Program,
    out: &mut String,
) -> Result<(), EmitError> {
    let Some(body) = &m.body else {
        return err(
            m.span,
            format!(
                "`{}` has no body. A declaration without one belongs on a `trait`, \
                 where it is a requirement",
                m.name.name
            ),
        );
    };
    if m.is_async {
        return err(m.span, "trait methods on a value type cannot be async");
    }
    let mut cx = p.method_ctx(&p.empty_class, &m.params);
    cx.self_struct = Some(&st.name);
    let ret = match &m.return_ty {
        Some(t) => Some(lower_return_type(t, p.default_error.as_deref(), &p.class_names)?),
        None => None,
    };
    cx.fallible_ret = matches!(&ret, Some(RustTy::Fallible { .. }));
    cx.nullable_ret = matches!(&ret, Some(RustTy::Opt(_)));
    write!(out, "    fn {}(self, _w: &mut World", camel_to_snake(&m.name.name)).unwrap();
    for param in &m.params {
        write!(
            out,
            ", {}: {}",
            camel_to_snake(&param.name.name),
            lower_type(&param.ty, &p.class_names)?.render()
        )
        .unwrap();
    }
    match &ret {
        Some(t) => writeln!(out, ") -> {} {{", t.render()).unwrap(),
        None => writeln!(out, ") {{").unwrap(),
    }
    let mut body_s = String::new();
    lower_scope(&body.stmts, body.trailing.as_deref(), false, &mut cx, &mut body_s, "        ")?;
    if let Some(trailing) = &body.trailing {
        if ret.is_some() {
            let v = lower_method_expr(trailing, &cx)?;
            writeln!(body_s, "        {v}").unwrap();
        } else {
            let stmt = Stmt::Expr((**trailing).clone());
            lower_method_stmt(&stmt, &mut cx, &mut body_s, "        ")?;
        }
    }
    out.push_str(&body_s);
    writeln!(out, "    }}").unwrap();
    Ok(())
}

fn emit_enum(en: &EnumInfo, classes: ClassNames<'_>, out: &mut String) -> Result<(), EmitError> {
    writeln!(out, "#[derive(Clone, Debug, PartialEq)]").unwrap();
    writeln!(out, "pub enum {} {{", en.name).unwrap();
    for v in &en.variants {
        if v.fields.is_empty() {
            writeln!(out, "    {},", escape_rust_keyword(v.name.name.clone())).unwrap();
        } else {
            let mut tys = Vec::new();
            for f in &v.fields {
                tys.push(lower_type(&f.ty, classes)?.render());
            }
            writeln!(out, "    {}({}),", escape_rust_keyword(v.name.name.clone()), tys.join(", ")).unwrap();
        }
    }
    writeln!(out, "}}\n").unwrap();
    // Display so `#{e}` interpolation renders variants; Debug-backed.
    writeln!(out, "impl std::fmt::Display for {} {{", en.name).unwrap();
    writeln!(
        out,
        "    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{"
    )
    .unwrap();
    writeln!(out, "        write!(f, \"{{:?}}\", self)").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}\n").unwrap();
    Ok(())
}

fn emit_struct(st: &StructInfo, p: &Program, out: &mut String) -> Result<(), EmitError> {
    let (gd, gu) = if st.generics.is_empty() {
        (String::new(), String::new())
    } else {
        let ps: Vec<String> = st.generics.iter().map(|g| format!("{g}: Clone")).collect();
        (format!("<{}>", ps.join(", ")), format!("<{}>", st.generics.join(", ")))
    };
    writeln!(out, "#[derive(Clone, Debug, PartialEq)]").unwrap();
    writeln!(out, "pub struct {}{gd} {{", st.name).unwrap();
    for (_, rust, ty) in &st.fields {
        writeln!(out, "    pub {rust}: {},", ty.render()).unwrap();
    }
    writeln!(out, "}}\n").unwrap();
    if st.methods.is_empty() {
        return Ok(());
    }
    writeln!(out, "impl{gd} {}{gu} {{", st.name).unwrap();
    for m in &st.methods {
        let Some(body) = &m.body else {
            return err(
                m.span,
                format!(
                    "`{}` has no body. A declaration without one belongs on a `trait`, \
                     where it is a requirement",
                    m.name.name
                ),
            );
        };
        if m.is_async {
            return err(m.span, "struct fns cannot be async (value types have no World context)");
        }
        let mut cx = p.method_ctx(&p.empty_class, &m.params);
        cx.self_struct = Some(&st.name);
        let ret = match &m.return_ty {
            Some(t) => Some(lower_return_type(t, p.default_error.as_deref(), &p.class_names)?),
            None => None,
        };
        cx.fallible_ret = matches!(&ret, Some(RustTy::Fallible { .. }));
        cx.nullable_ret = matches!(&ret, Some(RustTy::Opt(_)));
        let generics = render_fn_generics(m, p)?;
        write!(out, "    pub fn {}{generics}(&self", camel_to_snake(&m.name.name)).unwrap();
        for param in &m.params {
            write!(
                out,
                ", {}: {}",
                camel_to_snake(&param.name.name),
                lower_type(&param.ty, &p.class_names)?.render()
            )
            .unwrap();
        }
        match &ret {
            Some(t) => writeln!(out, ") -> {} {{", t.render()).unwrap(),
            None => writeln!(out, ") {{").unwrap(),
        }
        let mut body_s = String::new();
        lower_scope(&body.stmts, body.trailing.as_deref(), false, &mut cx, &mut body_s, "        ")?;
        if let Some(trailing) = &body.trailing {
            if ret.is_some() {
                if cx.nullable_ret {
                    let wrapped = lower_nullable_slot(trailing, &cx)?;
                    writeln!(body_s, "        {wrapped}").unwrap();
                } else {
                    let v = lower_method_expr(trailing, &cx)?;
                    if cx.fallible_ret {
                        let wrapped = fallible_wrap(trailing, v, &cx);
                        writeln!(body_s, "        {wrapped}").unwrap();
                    } else {
                        writeln!(body_s, "        {v}").unwrap();
                    }
                }
            } else {
                let stmt = Stmt::Expr((**trailing).clone());
                lower_method_stmt(&stmt, &mut cx, &mut body_s, "        ")?;
            }
        }
        out.push_str(&body_s);
        writeln!(out, "    }}").unwrap();
    }
    writeln!(out, "}}\n").unwrap();
    Ok(())
}


/// One view state field's class binding (§8.25): the base class for
/// member resolution, the pixie-side instantiation key ("Stack<Int>"
/// — also the reflection-table key the interp looks up), and the
/// Rust type arguments ("<i64>"; empty for plain classes).
struct StateBinding {
    field: String,
    class: String,
    pixie_key: String,
    rust_args: String,
    rust_tys: Vec<RustTy>,
    /// Lowered constructor arguments (§8.26).
    ctor_args: Vec<String>,
    /// Statements this binding's arguments need FIRST — one `let` per
    /// nested construction. Mount has the World in scope, so a state
    /// field may build an object graph (§8.41); nesting the inserts
    /// would take two mutable borrows at once, so they hoist (the
    /// §11.20 w-call rule).
    ctor_prelude: Vec<String>,
}

/// One per-row state seat (§8.30): a component instance with `state`
/// inside a `for` repeater. The splice hoists a marker Object field
/// `__pixie_row_seat(HolderClass, <iter>)`; the emitter turns it into
/// a `RowSeat<Holder>` field, ensured to the driving list's length in
/// `prepare` and only read during build.
struct RowSeatInfo {
    field: String,
    holder: String,
    /// One driving list per enclosing repeater, outermost first, each
    /// a `recv.prop` (view field or global). `prepare` sizes one seat
    /// dimension from each.
    dims: Vec<(String, String)>,
    sig_const: String,
    sig_id: u32,
}

fn is_row_seat_marker(sf: &ast::StateField) -> bool {
    matches!(
        &sf.init_expr.kind,
        ExprKind::Call { callee, .. }
            if matches!(&callee.kind, ExprKind::Ident(n) if n == "__pixie_row_seat")
    )
}

fn row_seats(view: &ast::ViewDecl) -> Result<Vec<RowSeatInfo>, EmitError> {
    let mut out = Vec::new();
    for sf in &view.state_fields {
        if !is_row_seat_marker(sf) {
            continue;
        }
        let ExprKind::Call { args, .. } = &sf.init_expr.kind else {
            unreachable!("guarded by is_row_seat_marker");
        };
        let Some((holder_e, iter_es)) = args.split_first() else {
            return err(sf.span, "__pixie_row_seat needs (Holder, iter...)");
        };
        if iter_es.is_empty() {
            return err(sf.span, "__pixie_row_seat needs (Holder, iter...)");
        }
        let ExprKind::Ident(holder) = &holder_e.kind else {
            return err(sf.span, "__pixie_row_seat holder must be a class name");
        };
        let mut dims = Vec::with_capacity(iter_es.len());
        for iter_e in iter_es {
            let ExprKind::Member { receiver, name } = &iter_e.kind else {
                return err(
                    sf.span,
                    "a stateful component in a repeater needs every enclosing `for` to \
                     iterate a field/global list property (M1)",
                );
            };
            let ExprKind::Ident(recv) = &receiver.kind else {
                return err(
                    sf.span,
                    "a stateful component in a repeater needs every enclosing `for` to \
                     iterate a field/global list property (M1)",
                );
            };
            dims.push((recv.clone(), name.name.clone()));
        }
        let field = sf.name.name.clone();
        let idx = out.len() as u32;
        out.push(RowSeatInfo {
            sig_const: format!("__PIXIE_SEAT_SIG_{idx}"),
            // A disjoint id range — class signals count up from 1 and
            // never reach here.
            sig_id: 0x4000_0000 + idx,
            field,
            holder: holder.clone(),
            dims,
        });
    }
    Ok(out)
}

fn state_bindings(view: &ast::ViewDecl, p: &Program) -> Result<Vec<StateBinding>, EmitError> {
    let mut out = Vec::new();
    for sf in &view.state_fields {
        if matches!(sf.kind, ast::StateFieldKind::Property { .. }) {
            continue;
        }
        if is_row_seat_marker(sf) {
            continue;
        }
        // §8.64: a view's object state is one construction per field.
        // Anything else — an alias of a sibling, a call, a literal —
        // either names an object the view already owns or is not an
        // object at all.
        let ExprKind::Call { callee, args, type_args, .. } = &sf.init_expr.kind else {
            return err(
                sf.span,
                format!(
                    "a view's `let {}` owns an object, so it constructs one: \
                     `let {} = SomeClass(..)`",
                    sf.name.name, sf.name.name
                ),
            );
        };
        let ExprKind::Ident(class_name) = &callee.kind else {
            return err(
                sf.span,
                format!(
                    "a view's `let {}` owns an object, so it constructs one: \
                     `let {} = SomeClass(..)`",
                    sf.name.name, sf.name.name
                ),
            );
        };
        let Some(info) = p.classes.get(class_name) else {
            return err(sf.span, format!("unknown class `{class_name}`"));
        };
        // Constructor args run the class `init` at mount (§8.26).
        // The init BODY is World-free, but the mount SITE is not: `w`
        // is in scope there, so an argument may construct another
        // object or name an earlier state field of the same view.
        // That is what lets a view own a shared graph (§8.41).
        let mut ctor_args = Vec::new();
        let mut ctor_prelude = Vec::new();
        match info.init {
            Some(init) => {
                if args.len() != init.params.len() {
                    return err(
                        sf.span,
                        format!(
                            "`{class_name}` takes {} constructor argument(s)",
                            init.params.len()
                        ),
                    );
                }
                let no_locals = std::collections::HashSet::new();
                for a in args {
                    // An object-valued argument: an earlier state
                    // field by name, or a nested construction hoisted
                    // into its own `let`.
                    match &a.kind {
                        ExprKind::Ident(n) if out.iter().any(|b: &StateBinding| b.field == *n) => {
                            ctor_args.push(camel_to_snake(n));
                            continue;
                        }
                        ExprKind::Call { callee, args: inner, type_args: ta, .. } => {
                            if let ExprKind::Ident(c) = &callee.kind {
                                if let Some(nested) = p.classes.get(c) {
                                    if nested.init.map(|i| i.params.len()).unwrap_or(0)
                                        != inner.len()
                                    {
                                        return err(
                                            a.span,
                                            format!(
                                                "`{c}` takes {} constructor argument(s)",
                                                nested.init.map(|i| i.params.len()).unwrap_or(0)
                                            ),
                                        );
                                    }
                                    let mut nested_args = Vec::new();
                                    for ia in inner {
                                        nested_args
                                            .push(lower_init_expr(ia, &p.empty_class, &no_locals)?);
                                    }
                                    let (_, rust_args) =
                                        instantiation_of(nested, ta, &p.class_names, a.span)?;
                                    let turbofish = if rust_args.is_empty() {
                                        String::new()
                                    } else {
                                        format!("::{rust_args}")
                                    };
                                    let tmp = format!("__ctor{}", ctor_prelude.len());
                                    ctor_prelude.push(format!(
                                        "let {tmp} = w.insert({c}{turbofish}::new({}));",
                                        nested_args.join(", ")
                                    ));
                                    ctor_args.push(tmp);
                                    continue;
                                }
                            }
                        }
                        _ => {}
                    }
                    ctor_args.push(lower_init_expr(a, &p.empty_class, &no_locals)?);
                }
            }
            None => {
                if !args.is_empty() {
                    return err(
                        sf.span,
                        format!("`{class_name}` has no `init` — construct with `{class_name}()`"),
                    );
                }
            }
        }
        let (pixie_key, rust_args) = instantiation_of(info, type_args, &p.class_names, sf.span)?;
        let mut rust_tys = Vec::new();
        for t in type_args {
            rust_tys.push(lower_type(t, &p.class_names)?);
        }
        out.push(StateBinding {
            field: sf.name.name.clone(),
            class: class_name.clone(),
            pixie_key,
            rust_args,
            rust_tys,
            ctor_args,
            ctor_prelude,
        });
    }
    Ok(out)
}

/// Resolve explicit type args against a class's generics: the
/// pixie-side key and the Rust `<…>` argument list.
fn instantiation_of(
    info: &ClassInfo,
    type_args: &[ast::TypeExpr],
    classes: ClassNames<'_>,
    span: Span,
) -> Result<(String, String), EmitError> {
    if info.generics.is_empty() {
        if !type_args.is_empty() {
            return err(span, format!("`{}` takes no type arguments", info.name));
        }
        return Ok((info.name.clone(), String::new()));
    }
    if type_args.len() != info.generics.len() {
        return err(
            span,
            format!(
                "`{}` takes {} type argument(s) — construction needs them explicit (`{}<…>()`)",
                info.name,
                info.generics.len(),
                info.name
            ),
        );
    }
    let mut pixie: Vec<String> = Vec::new();
    let mut rust: Vec<String> = Vec::new();
    for t in type_args {
        pixie.push(ast::type_expr_render(t));
        rust.push(lower_type(t, classes)?.render());
    }
    Ok((
        format!("{}<{}>", info.name, pixie.join(", ")),
        format!("<{}>", rust.join(", ")),
    ))
}

/// Substitute class type params with concrete RustTys (reflection
/// tables register per instantiation).
fn substitute_rustty(ty: &RustTy, map: &HashMap<String, RustTy>) -> RustTy {
    match ty {
        RustTy::Named(n) => map.get(n).cloned().unwrap_or_else(|| ty.clone()),
        RustTy::List(t) => RustTy::List(Box::new(substitute_rustty(t, map))),
        RustTy::Opt(t) => RustTy::Opt(Box::new(substitute_rustty(t, map))),
        RustTy::Map(k, v) => RustTy::Map(
            Box::new(substitute_rustty(k, map)),
            Box::new(substitute_rustty(v, map)),
        ),
        other => other.clone(),
    }
}

fn emit_view(
    view: &ast::ViewDecl,
    p: &Program,
    out: &mut String,
    reload: bool,
) -> Result<(), EmitError> {
    let classes = &p.classes;
    if !view.params.is_empty() {
        // Unreachable from the driver, which reports this earlier with
        // the same meaning; kept as a guard (§8.63).
        return err(
            view.span,
            "the root view is the window and nothing calls it, so it takes no \
             parameters — a view WITH parameters is a component, used from another view",
        );
    }
    let mut cx = ViewCtx {
        fields: HashMap::new(),
        classes,
        bindings: &p.bindings,
        structs: &p.structs,
        enums: &p.enums,
        globals: &p.globals,
        class_names: &p.class_names,
        loop_vars: Vec::new(),
        depth: 0,
        repeat_depth: 0,
    };
    let bindings = state_bindings(view, p)?;
    for b in &bindings {
        cx.fields.insert(b.field.clone(), b.class.clone());
    }

    let seats = row_seats(view)?;
    let vname = format!("{}View", view.name.name);
    writeln!(out, "#[derive(Clone)]").unwrap();
    writeln!(out, "pub struct {vname} {{").unwrap();
    for b in &bindings {
        writeln!(out, "    {}: Handle<{}{}>,", camel_to_snake(&b.field), b.class, b.rust_args).unwrap();
    }
    for s in &seats {
        writeln!(
            out,
            "    {}: Handle<pixie_kernel::RowSeat<{}>>,",
            camel_to_snake(&s.field),
            s.holder
        )
        .unwrap();
    }
    writeln!(out, "}}\n").unwrap();
    // §8.47: the view and its row seats are counted holders too.
    //
    // Leaving them out was safe only because nothing could give one
    // of their objects a SECOND, counted edge — and that was true by
    // accident, resting on an unrelated M1 limit rather than on any
    // decision. Registering them makes the model right on its own
    // terms: a view field's object is alive because the view holds
    // it, so a counted edge elsewhere can come and go freely.
    writeln!(out, "pub fn __pixie_register_view_edges(w: &mut World) {{").unwrap();
    if !bindings.is_empty() || !seats.is_empty() {
        writeln!(out, "    w.register_edges::<{vname}>(|w, h| {{").unwrap();
        writeln!(out, "        let __v = w.get(h.typed::<{vname}>());").unwrap();
        writeln!(out, "        let mut __out: Vec<pixie_kernel::ErasedHandle> = Vec::new();").unwrap();
        for b in &bindings {
            writeln!(out, "        __out.push(__v.{}.erase());", camel_to_snake(&b.field)).unwrap();
        }
        for s in &seats {
            writeln!(out, "        __out.push(__v.{}.erase());", camel_to_snake(&s.field)).unwrap();
        }
        writeln!(out, "        __out").unwrap();
        writeln!(out, "    }});").unwrap();
    }
    for s in &seats {
        writeln!(
            out,
            "    w.register_edges::<pixie_kernel::RowSeat<{}>>(|w, h| w.get(h.typed::<pixie_kernel::RowSeat<{}>>()).edges());",
            s.holder, s.holder
        )
        .unwrap();
    }
    writeln!(out, "}}\n").unwrap();
    for s in &seats {
        writeln!(out, "pub const {}: SignalId = {};", s.sig_const, s.sig_id).unwrap();
    }
    if !seats.is_empty() {
        out.push('\n');
    }

    writeln!(out, "impl Component for {vname} {{").unwrap();
    if !seats.is_empty() {
        // Row seats grow to the driving list's CURRENT length here —
        // the only mutable phase. `build` then reads `row_of` only.
        writeln!(out, "    fn prepare(&self, w: &mut World) {{").unwrap();
        for b in &bindings {
            let rf = camel_to_snake(&b.field);
            writeln!(out, "        let {rf} = self.{rf};").unwrap();
        }
        for s in &seats {
            let sf = camel_to_snake(&s.field);
            let mut dim_names = Vec::with_capacity(s.dims.len());
            for (d, (recv, prop)) in s.dims.iter().enumerate() {
                let recv_expr = if let Some((info, handle)) = cx.handle_for(recv) {
                    let p2 = info.prop(prop).ok_or_else(|| EmitError {
                        span: view.span,
                        message: format!("no property `{prop}` on `{}` (row seat)", info.name),
                    })?;
                    format!("{handle}.{}(w)", p2.rust)
                } else {
                    return err(
                        view.span,
                        format!("`{recv}` is not a view state field or global (row seat)"),
                    );
                };
                writeln!(out, "        let __n{d} = {recv_expr}.len();").unwrap();
                dim_names.push(format!("__n{d}"));
            }
            let dims_rust = dim_names.join(", ");
            writeln!(out, "        let __seat_e = self.{sf}.erase();").unwrap();
            writeln!(
                out,
                "        pixie_kernel::ensure_row_grid(w, self.{sf}, &[{dims_rust}], |w| {{"
            )
            .unwrap();
            writeln!(out, "            let h = w.insert({}::new());", s.holder).unwrap();
            let hinfo = cx.classes.get(&s.holder).ok_or_else(|| EmitError {
                span: view.span,
                message: format!("unknown row holder class `{}`", s.holder),
            })?;
            for sig in &hinfo.signals {
                writeln!(
                    out,
                    "            w.connect(h.erase(), {}, std::rc::Rc::new(move |w: &mut World| w.notify(__seat_e, {})));",
                    sig.const_name, s.sig_const
                )
                .unwrap();
            }
            writeln!(out, "            h").unwrap();
            writeln!(out, "        }});").unwrap();
        }
        writeln!(out, "    }}").unwrap();
    }
    writeln!(out, "    fn build(&self, w: &World) -> Element {{").unwrap();
    if reload {
        // Rung 2: a live view body installed by the reload watcher
        // takes over; interpreter failures fall back to the compiled
        // body instead of tearing the app down — except under
        // PIXIE_TIER=interp (the divergence gate), where a silent
        // fallback would mask exactly what the gate exists to catch.
        writeln!(out, "        if let Some(__lv) = __PIXIE_LIVE.with(|l| l.borrow().clone()) {{").unwrap();
        writeln!(out, "            if let Some(__tables) = __PIXIE_TABLES.with(|t| t.borrow().clone()) {{").unwrap();
        writeln!(out, "                match pixie_interp::build_view(&__lv, &__interp_env(self), &__tables, w) {{").unwrap();
        writeln!(out, "                    Ok(el) => return el,").unwrap();
        writeln!(out, "                    Err(e) => {{").unwrap();
        writeln!(out, "                        if std::env::var(\"PIXIE_TIER\").as_deref() == Ok(\"interp\") {{").unwrap();
        writeln!(out, "                            panic!(\"tier divergence: the interpreter cannot build this view: {{e}}\");").unwrap();
        writeln!(out, "                        }}").unwrap();
        writeln!(out, "                        eprintln!(\"pixie reload: {{e}}; keeping the compiled view\");").unwrap();
        writeln!(out, "                    }}").unwrap();
        writeln!(out, "                }}").unwrap();
        writeln!(out, "            }}").unwrap();
        writeln!(out, "        }}").unwrap();
    }
    for b in &bindings {
        let rf = camel_to_snake(&b.field);
        writeln!(out, "        let {rf} = self.{rf};").unwrap();
    }
    for s in &seats {
        let sf = camel_to_snake(&s.field);
        writeln!(out, "        let {sf} = self.{sf};").unwrap();
    }
    let root = lower_element(&view.root, &mut cx, "        ")?;
    writeln!(out, "        {root}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}\n").unwrap();
    Ok(())
}

/// `Value` construction for a prop read, by declared type. `None`
/// marks a type the interpreter can't see (skipped, not mis-bound).
/// A payload field's type, when it is one of the scalars a payload
/// can cross with today (richer payloads stay un-crossed rather
/// than half-crossed).
fn scalar_payload_ty(t: &TypeExpr) -> Option<RustTy> {
    match &t.kind {
        TypeKind::Named { path, args } if path.len() == 1 && args.is_empty() => {
            match path[0].name.as_str() {
                "Int" => Some(RustTy::Int),
                "Float" => Some(RustTy::Float),
                "Bool" => Some(RustTy::Bool),
                "String" => Some(RustTy::Str),
                _ => None,
            }
        }
        _ => None,
    }
}

fn interp_value_expr(ty: &RustTy, read: &str, p: &Program) -> Option<String> {
    match ty {
        RustTy::Int => Some(format!("pixie_interp::Value::Int({read})")),
        RustTy::Float => Some(format!("pixie_interp::Value::Float({read})")),
        RustTy::Bool => Some(format!("pixie_interp::Value::Bool({read})")),
        RustTy::Str => Some(format!("pixie_interp::Value::Str({read})")),
        // An OBJECT crosses as its erased handle plus the class key
        // its reflection entries are registered under (§8.41), so a
        // view can read on through it exactly like the compiled tier.
        RustTy::Handle(c) => Some(format!(
            "pixie_interp::Value::Object(({read}).erase(), {c:?}.to_string())"
        )),
        RustTy::List(inner) => {
            // Every element shape this function can produce — a list
            // of structs included (§8.68), which is the shape the
            // memory model recommends for app data.
            let elem = interp_value_expr(inner, "x.clone()", p)?;
            Some(format!(
                "pixie_interp::Value::List({read}.iter().map(|x| {elem}).collect())"
            ))
        }
        // A MAP crosses as its sorted pair list (§8.68). `BTreeMap`
        // iterates in key order, and the interpreted tier keeps that
        // order, so `keys` answers the same sequence in both tiers.
        RustTy::Map(k, v) => {
            let kx = interp_value_expr(k, "__k.clone()", p)?;
            let vx = interp_value_expr(v, "__v.clone()", p)?;
            Some(format!(
                "pixie_interp::Value::Map({read}.pairs().into_iter().map(|(__k, __v)| ({kx}, {vx})).collect())"
            ))
        }
        RustTy::Bytes => Some(format!("pixie_interp::Value::Bytes({read})")),
        // `T?`: a present optional is the value itself and an absent
        // one is `Nil`, which is what makes `case` over one need no
        // unwrapping on the interpreted side.
        RustTy::Opt(inner) => {
            let some = interp_value_expr(inner, "__x", p)?;
            Some(format!(
                "match {read} {{ Some(__x) => {some}, None => pixie_interp::Value::Nil }}"
            ))
        }
        // Name-only enums render through their Display; a PAYLOAD
        // enum crosses STRUCTURALLY as Value::Struct(variant,
        // fields), which is what lets an interpreted `when V(x)` arm
        // match and bind the way the compiled tier does.
        RustTy::Named(n) if p.enums.contains_key(n) => {
            let en = &p.enums[n];
            if en.variants.iter().all(|v| v.fields.is_empty()) {
                return Some(format!(
                    "pixie_interp::Value::Str(Str::from(format!(\"{{}}\", {read})))"
                ));
            }
            let mut arms: Vec<String> = Vec::new();
            for v in &en.variants {
                let vn = &v.name.name;
                let rust_v = escape_rust_keyword(vn.clone());
                if v.fields.is_empty() {
                    arms.push(format!(
                        "{n}::{rust_v} => pixie_interp::Value::Struct({vn:?}.to_string(), vec![])"
                    ));
                } else {
                    let mut binds = Vec::new();
                    let mut parts = Vec::new();
                    for (i, f) in v.fields.iter().enumerate() {
                        let b = format!("__f{i}");
                        let fty = scalar_payload_ty(&f.ty)?;
                        let conv = interp_value_expr(&fty, &format!("{b}.clone()"), p)?;
                        parts.push(format!("({:?}.to_string(), {conv})", f.name.name));
                        binds.push(b);
                    }
                    arms.push(format!(
                        "{n}::{rust_v}({}) => pixie_interp::Value::Struct({vn:?}.to_string(), vec![{}])",
                        binds.join(", "),
                        parts.join(", ")
                    ));
                }
            }
            Some(format!("match {read} {{ {} }}", arms.join(", ")))
        }
        // A STRUCT crosses as its fields by surface name (§8.68), so
        // the interpreted tier reads `p.x` through the same walk the
        // compiled one does.
        RustTy::Named(n) if p.structs.contains_key(n) => {
            let st = &p.structs[n];
            let mut parts = Vec::with_capacity(st.fields.len());
            for (surface, rust, fty) in &st.fields {
                let fx = interp_value_expr(fty, &format!("__s.{rust}.clone()"), p)?;
                parts.push(format!("({surface:?}.to_string(), {fx})"));
            }
            Some(format!(
                "{{ let __s = {read}; pixie_interp::Value::Struct({n:?}.to_string(), vec![{}]) }}",
                parts.join(", ")
            ))
        }
        _ => None,
    }
}

/// `Value` extraction for scalar types (setter / method-arg side).
fn interp_extract_expr(ty: &RustTy, val: &str) -> Option<String> {
    match ty {
        // An OBJECT argument: the value carries the erased handle, so
        // re-typing it is all that is needed (§8.53). Without this a
        // method taking a class was simply absent from the table, and
        // an interpreted handler calling it said "no invokable
        // method" while the compiled one worked.
        RustTy::Handle(c) => Some(format!("{val}.as_object()?.typed::<{c}>()")),
        RustTy::Int => Some(format!("{val}.as_int()?")),
        RustTy::Float => Some(format!("{val}.as_float()?")),
        RustTy::Bool => Some(format!("{val}.as_bool()?")),
        RustTy::Str => Some(format!("{val}.as_str_value()?")),
        RustTy::Bytes => Some(format!("{val}.as_bytes_value()?")),
        RustTy::Map(k, v) => {
            let kx = interp_extract_expr(k, "__k")?;
            let vx = interp_extract_expr(v, "__v")?;
            Some(format!(
                "{{ let __kv = {val}.as_map_value()?; let mut __m = Map::new(); \
                 for (__k, __v) in __kv.into_iter() {{ __m.insert({kx}, {vx}); }} __m }}"
            ))
        }
        RustTy::Opt(inner) => {
            let some = interp_extract_expr(inner, "__x")?;
            Some(format!(
                "{{ let __x = {val}; if matches!(__x, pixie_interp::Value::Nil) \
                 {{ None }} else {{ Some({some}) }} }}"
            ))
        }
        _ => None,
    }
}

/// Everything the running binary needs for rung 2: the live-view
/// statics, the source location + fingerprint, the field env, and the
/// reflection tables the interpreter reaches compiled classes through.
fn emit_reload_support(
    view: &ast::ViewDecl,
    p: &Program,
    ri: &ReloadInfo,
    out: &mut String,
) -> Result<(), EmitError> {
    writeln!(out, "thread_local! {{").unwrap();
    writeln!(out, "    static __PIXIE_LIVE: std::cell::RefCell<Option<std::rc::Rc<pixie_interp::LiveView>>> = std::cell::RefCell::new(None);").unwrap();
    writeln!(out, "    static __PIXIE_TABLES: std::cell::RefCell<Option<std::rc::Rc<pixie_interp::Tables>>> = std::cell::RefCell::new(None);").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out, "const __PIXIE_SRC: &str = {:?};", ri.source_path).unwrap();
    writeln!(out, "const __PIXIE_FINGERPRINT: u64 = {}u64;", ri.fingerprint).unwrap();
    // The imports, by name and path. Their snippets are rebuilt from
    // disk at every reload (§8.72), so editing another module's
    // `pub style` or a component it exports lands without a rebuild.
    writeln!(out, "const __PIXIE_FOREIGN_PATHS: &[(&str, &str)] = &[").unwrap();
    for (name, path) in &ri.foreign_paths {
        writeln!(out, "    ({name:?}, {path:?}),").unwrap();
    }
    writeln!(out, "];\n").unwrap();
    writeln!(out, "fn __pixie_foreign_paths() -> Vec<(String, String)> {{").unwrap();
    writeln!(
        out,
        "    __PIXIE_FOREIGN_PATHS.iter().map(|(n, p)| (n.to_string(), p.to_string())).collect()"
    )
    .unwrap();
    writeln!(out, "}}\n").unwrap();
    writeln!(out, "fn __pixie_foreign() -> pixie_interp::ForeignReload {{").unwrap();
    writeln!(
        out,
        "    pixie_interp::foreign_reload_from_paths(&__pixie_foreign_paths())"
    )
    .unwrap();
    writeln!(out, "}}\n").unwrap();

    // The field env: view object fields by surface name (the reloaded
    // AST resolves `counter` / bare state cells against these).
    let vname = format!("{}View", view.name.name);
    writeln!(out, "fn __interp_env(v: &{vname}) -> pixie_interp::FieldEnv {{").unwrap();
    writeln!(out, "    pixie_interp::FieldEnv {{").unwrap();
    writeln!(out, "        fields: vec![").unwrap();
    for b in &state_bindings(view, p)? {
        let fname = &b.field;
        let rf = camel_to_snake(fname);
        // The class string is the INSTANTIATION key ("Stack<Int>") —
        // reflection tables register per instantiation (§8.25).
        writeln!(
            out,
            "            ({fname:?}.to_string(), {:?}.to_string(), v.{rf}.erase()),",
            b.pixie_key
        )
        .unwrap();
    }
    for s in &row_seats(view)? {
        let rf = camel_to_snake(&s.field);
        writeln!(
            out,
            "            ({:?}.to_string(), \"RowSeat\".to_string(), v.{rf}.erase()),",
            s.field
        )
        .unwrap();
    }
    writeln!(out, "        ],").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}\n").unwrap();

    writeln!(out, "fn __pixie_tables() -> pixie_interp::Tables {{").unwrap();
    writeln!(out, "    let mut t = pixie_interp::Tables::new();").unwrap();
    for s in &row_seats(view)? {
        writeln!(
            out,
            "    t.row({:?}, {:?}, |w, h, path| pixie_kernel::row_at_erased(w, h.typed::<pixie_kernel::RowSeat<{}>>(), path));",
            s.field, s.holder, s.holder
        )
        .unwrap();
    }
    // Constructors, so an interpreted handler can build an object the
    // way a compiled one does (§8.53). Generic classes are skipped:
    // their instantiation is a type-level fact the table cannot key.
    for name in &p.order {
        let info = &p.classes[name];
        if !info.generics.is_empty() {
            continue;
        }
        let mut extracts = Vec::new();
        let mut ok = true;
        match info.init {
            Some(init) => {
                for (i, param) in init.params.iter().enumerate() {
                    let Ok(ty) = lower_type(&param.ty, &p.class_names) else {
                        ok = false;
                        break;
                    };
                    match interp_extract_expr(&ty, &format!("__args[{i}]")) {
                        Some(ex) => extracts.push(ex),
                        None => {
                            ok = false;
                            break;
                        }
                    }
                }
            }
            None => {}
        }
        if !ok {
            continue;
        }
        let arity = info.init.map(|i| i.params.len()).unwrap_or(0);
        writeln!(out, "    t.ctor({name:?}, |w, __args| {{").unwrap();
        writeln!(
            out,
            "        if __args.len() != {arity} {{ return Err(\"`{name}` takes {arity} constructor argument(s)\".to_string()) }}"
        )
        .unwrap();
        writeln!(
            out,
            "        Ok(w.insert({name}::new({})).erase())",
            extracts.join(", ")
        )
        .unwrap();
        writeln!(out, "    }});").unwrap();
    }
    let view_bindings = state_bindings(view, p)?;
    for name in &p.order {
        let info = &p.classes[name];
        // Registration set: a plain class registers once under its
        // own name; a generic class registers once per DISTINCT
        // instantiation used by the view's state fields, keyed
        // "Stack<Int>" with every prop type substituted (§8.25).
        let mut insts: Vec<(String, String, HashMap<String, RustTy>)> = Vec::new();
        if info.generics.is_empty() {
            insts.push((name.clone(), name.clone(), HashMap::new()));
        } else {
            for b in view_bindings.iter().filter(|b| &b.class == name) {
                let rust_ty = format!("{}{}", name, b.rust_args);
                if insts.iter().any(|(k, _, _)| *k == b.pixie_key) {
                    continue;
                }
                let subst: HashMap<String, RustTy> = info
                    .generics
                    .iter()
                    .cloned()
                    .zip(b.rust_tys.iter().cloned())
                    .collect();
                insts.push((b.pixie_key.clone(), rust_ty, subst));
            }
        }
        for (key, rust_ty, subst) in &insts {
        let name = key;
        for prop in &info.props {
            let prop_ty = substitute_rustty(&prop.ty, subst);
            let prop = &PropInfo {
                camel: prop.camel.clone(),
                rust: prop.rust.clone(),
                ty: prop_ty,
                default: prop.default.clone(),
                is_weak: prop.is_weak,
                notify_const: prop.notify_const.clone(),
                assignable: prop.assignable,
                keyword: prop.keyword,
                derived: prop.derived.clone(),
            };
            let read = format!("h.typed::<{rust_ty}>().{}(w)", prop.rust);
            if let Some(v) = interp_value_expr(&prop.ty, &read, p) {
                writeln!(
                    out,
                    "    t.getter({name:?}, {:?}, |w, h| {v});",
                    prop.camel
                )
                .unwrap();
            }
            // A `let` field has no setter to register (§8.58), so a
            // rung-2 reload that introduces a write to one fails the
            // same way the compiler would have.
            match &prop.ty {
                _ if !prop.assignable => {}
                RustTy::List(inner) => {
                    if let Some(ex) = interp_extract_expr(inner, "__x") {
                        writeln!(out, "    t.setter({name:?}, {:?}, |w, h, v| {{", prop.camel).unwrap();
                        writeln!(out, "        let pixie_interp::Value::List(__xs) = v else {{ return Err(\"expected List\".to_string()) }};").unwrap();
                        writeln!(out, "        let mut __out: {} = List::new();", prop.ty.render()).unwrap();
                        writeln!(out, "        for __x in &__xs {{ __out.push({ex}); }}").unwrap();
                        writeln!(out, "        h.typed::<{rust_ty}>().set_{}(w, __out);", prop.rust).unwrap();
                        writeln!(out, "        Ok(())").unwrap();
                        writeln!(out, "    }});").unwrap();
                    }
                }
                ty => {
                    if let Some(ex) = interp_extract_expr(ty, "v") {
                        writeln!(
                            out,
                            "    t.setter({name:?}, {:?}, |w, h, v| {{ h.typed::<{rust_ty}>().set_{}(w, {ex}); Ok(()) }});",
                            prop.camel, prop.rust
                        )
                        .unwrap();
                    }
                }
            }
        }
        for m in &info.methods {
            // Method-level generics can't sit behind a table fn
            // pointer — reload-time actions skip them (call through
            // a plain wrapper method if needed).
            if !m.generics.is_empty() {
                continue;
            }
            let mut extracts: Vec<String> = Vec::new();
            let mut ok = true;
            for (i, param) in m.params.iter().enumerate() {
                let Ok(ty) = lower_type(&param.ty, &p.class_names) else {
                    ok = false;
                    break;
                };
                let ty = substitute_rustty(&ty, subst);
                match interp_extract_expr(&ty, &format!("__args[{i}]")) {
                    Some(ex) => extracts.push(ex),
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            if !ok {
                continue;
            }
            let mname = &m.name.name;
            writeln!(out, "    t.method({name:?}, {mname:?}, |w, h, __args| {{").unwrap();
            writeln!(
                out,
                "        if __args.len() != {} {{ return Err(\"{mname} takes {} argument(s)\".to_string()); }}",
                m.params.len(),
                m.params.len()
            )
            .unwrap();
            let mut call = format!("h.typed::<{rust_ty}>().{}(w", camel_to_snake(mname));
            for ex in &extracts {
                write!(call, ", {ex}").unwrap();
            }
            call.push(')');
            writeln!(out, "        let _ = {call};").unwrap();
            writeln!(out, "        Ok(())").unwrap();
            writeln!(out, "    }});").unwrap();
        }
        }
    }
    for (gname, class) in &p.global_order {
        writeln!(
            out,
            "    t.global({gname:?}, {class:?}, |w| w.singleton_ref::<{class}>().erase());"
        )
        .unwrap();
    }
    // `static fn`s (§8.54): World-free, so the interpreter may call
    // them from VIEW expressions — register each crossable one.
    {
        let mut names: Vec<&String> = p.classes.keys().collect();
        names.sort();
        for name in names {
            let info = &p.classes[name];
            if !info.generics.is_empty() {
                continue;
            }
            for f in &info.statics {
                let Some(ret) = f.return_ty.as_ref() else { continue };
                let Ok(rty) = lower_type(ret, &p.class_names) else { continue };
                let mut extracts = Vec::new();
                let mut ok = true;
                for (i, par) in f.params.iter().enumerate() {
                    let Ok(pty) = lower_type(&par.ty, &p.class_names) else {
                        ok = false;
                        break;
                    };
                    match interp_extract_expr(&pty, &format!("__args[{i}].clone()")) {
                        Some(x) => extracts.push(x),
                        None => {
                            ok = false;
                            break;
                        }
                    }
                }
                if !ok {
                    continue;
                }
                let fname = &f.name.name;
                let mut call = format!("{name}::{}(", camel_to_snake(fname));
                for (i, ex) in extracts.iter().enumerate() {
                    if i > 0 {
                        call.push_str(", ");
                    }
                    call.push_str(ex);
                }
                call.push(')');
                let Some(wrap) = interp_value_expr(&rty, "__r", p) else { continue };
                writeln!(out, "    t.static_fn({name:?}, {fname:?}, |__args| {{").unwrap();
                writeln!(
                    out,
                    "        if __args.len() != {} {{ return Err(\"{fname} takes {} argument(s)\".to_string()); }}",
                    f.params.len(),
                    f.params.len()
                )
                .unwrap();
                writeln!(
                    out,
                    "        let __extract = || -> Result<pixie_interp::Value, String> {{"
                )
                .unwrap();
                writeln!(out, "            let __r = {call};").unwrap();
                writeln!(out, "            Ok({wrap})").unwrap();
                writeln!(out, "        }};").unwrap();
                writeln!(out, "        __extract()").unwrap();
                writeln!(out, "    }});").unwrap();
            }
        }
    }
    // PURE binding statics (§8.54 extended): register them too, so
    // the interpreter tier can evaluate `Py.floatRepr(x)` in a view
    // exactly where the compiled tier calls the Rust fn directly.
    {
        let mut bnames: Vec<&String> = p.bindings.keys().collect();
        bnames.sort();
        for name in bnames {
            let bc = &p.bindings[name];
            let mut fnames: Vec<&String> = bc.statics.keys().collect();
            fnames.sort();
            for fname in fnames {
                let bf = &bc.statics[fname];
                if bf.fallible
                    || !matches!(bf.ret, RustTy::Int | RustTy::Float | RustTy::Bool | RustTy::Str)
                    || !bf.params.iter().all(|t| {
                        matches!(t, RustTy::Int | RustTy::Float | RustTy::Bool | RustTy::Str)
                    })
                {
                    continue;
                }
                let mut binds = Vec::new();
                let mut argv = Vec::new();
                for (i, pty) in bf.params.iter().enumerate() {
                    let Some(x) = interp_extract_expr(pty, &format!("__args[{i}].clone()")) else {
                        continue;
                    };
                    binds.push(format!("let __a{i} = {x};"));
                    argv.push(match pty {
                        RustTy::Str => format!("(__a{i}).as_str()"),
                        _ => format!("__a{i}"),
                    });
                }
                if binds.len() != bf.params.len() {
                    continue;
                }
                let call = format!("{}({})", bf.rust_path, argv.join(", "));
                let pix = match &bf.ret {
                    RustTy::Int => "(__r as i64)".to_string(),
                    RustTy::Float => "(__r as f64)".to_string(),
                    RustTy::Str => "Str::from(__r)".to_string(),
                    _ => "__r".to_string(),
                };
                let Some(wrap) = interp_value_expr(&bf.ret, "__v", p) else { continue };
                writeln!(out, "    t.static_fn({name:?}, {fname:?}, |__args| {{").unwrap();
                writeln!(
                    out,
                    "        if __args.len() != {} {{ return Err(\"{fname} takes {} argument(s)\".to_string()); }}",
                    bf.params.len(),
                    bf.params.len()
                )
                .unwrap();
                writeln!(
                    out,
                    "        let __extract = || -> Result<pixie_interp::Value, String> {{"
                )
                .unwrap();
                for b in &binds {
                    writeln!(out, "            {b}").unwrap();
                }
                writeln!(out, "            let __r = {call};").unwrap();
                writeln!(out, "            let __v = {pix};").unwrap();
                writeln!(out, "            Ok({wrap})").unwrap();
                writeln!(out, "        }};").unwrap();
                writeln!(out, "        __extract()").unwrap();
                writeln!(out, "    }});").unwrap();
            }
        }
    }
    writeln!(out, "    t").unwrap();
    writeln!(out, "}}\n").unwrap();
    Ok(())
}

/// Teach the World every class's outgoing edges (§8.44). Only
/// classes that actually hold objects register; a class of plain
/// values costs nothing, not even a table entry.
///
/// This is the same declaration walk `lower_type` does — a property
/// whose lowered type carries a `Handle` is an edge — so the table
/// cannot drift from what the setters count.
fn emit_edge_registrations(
    p: &Program,
    view: &ast::ViewDecl,
    out: &mut String,
) -> Result<(), EmitError> {
    // The table is keyed by CONCRETE Rust type, so a generic class
    // registers once per instantiation — the same enumeration the
    // reflection tables do (§8.25), reused here rather than
    // reinvented, so the two cannot disagree about which
    // instantiations exist.
    let view_bindings = state_bindings(view, p)?;
    for name in &p.order {
        let info = &p.classes[name];
        let mut insts: Vec<(String, HashMap<String, RustTy>)> = Vec::new();
        if info.generics.is_empty() {
            insts.push((name.clone(), HashMap::new()));
        } else {
            for b in view_bindings.iter().filter(|b| &b.class == name) {
                let rust_ty = format!("{}{}", name, b.rust_args);
                if insts.iter().any(|(t, _)| *t == rust_ty) {
                    continue;
                }
                let subst: HashMap<String, RustTy> = info
                    .generics
                    .iter()
                    .cloned()
                    .zip(b.rust_tys.iter().cloned())
                    .collect();
                insts.push((rust_ty, subst));
            }
        }
        // `deinit` (§8.60) registers here too — same keying, and a
        // class that declares one is never generic (checked at
        // emission), so there is exactly one concrete type.
        if info.deinit.is_some() {
            writeln!(
                out,
                "    w.register_deinit::<{name}>(|w, h| h.typed::<{name}>().__pixie_deinit(w));"
            )
            .unwrap();
        }
        for (rust_ty, subst) in &insts {
            let edges: Vec<(String, RustTy)> = info
                .props
                .iter()
                .filter(|pr| !pr.is_weak)
                .map(|pr| (pr.rust.clone(), substitute_rustty(&pr.ty, subst)))
                .filter(|(_, ty)| ty.holds_objects())
                .collect();
            if edges.is_empty() {
                continue;
            }
            writeln!(out, "    w.register_edges::<{rust_ty}>(|w, h| {{").unwrap();
            writeln!(
                out,
                "        let mut __out: Vec<pixie_kernel::ErasedHandle> = Vec::new();"
            )
            .unwrap();
            writeln!(out, "        let __h = h.typed::<{rust_ty}>();").unwrap();
            for (rust, ty) in edges {
                let read = format!("__h.{rust}(w)");
                writeln!(out, "        {}", edge_push_expr(&ty, &read)).unwrap();
            }
            writeln!(out, "        __out").unwrap();
            writeln!(out, "    }});").unwrap();
        }
    }
    Ok(())
}

/// Push every object `val` carries onto `__out`.
fn edge_push_expr(ty: &RustTy, val: &str) -> String {
    match ty {
        RustTy::Handle(_) => format!("__out.push(({val}).erase());"),
        RustTy::List(inner) if inner.holds_objects() => format!(
            "{{ let __xs = {val}; for __e in __xs.iter() {{ {} }} }}",
            edge_push_expr(inner, "(*__e)")
        ),
        RustTy::Opt(inner) if inner.holds_objects() => format!(
            "if let Some(__o) = {val} {{ {} }}",
            edge_push_expr(inner, "__o")
        ),
        RustTy::Map(_, v) if v.holds_objects() => format!(
            "{{ let __m = {val}; for __e in __m.values().iter() {{ {} }} }}",
            edge_push_expr(v, "(*__e)")
        ),
        _ => String::new(),
    }
}

/// The `__title` initializer and the `win` argument for the emitted
/// `run_app` call, from the current WindowOpts. A declared title is a
/// baked literal (Rust debug formatting IS a valid string literal);
/// otherwise the historical exe-stem read. width/height bake as a
/// pair or not at all.
fn window_exprs() -> (String, String) {
    WINDOW_OPTS.with(|w| {
        let w = w.borrow();
        let title = match &w.title {
            Some(t) => format!("{t:?}.to_string()"),
            None => "std::env::current_exe().ok().and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned())).unwrap_or_else(|| \"pixie\".into())".to_string(),
        };
        let win = match (w.width, w.height) {
            (Some(x), Some(y)) => format!("Some(({x:?}f64, {y:?}f64))"),
            _ => "None".to_string(),
        };
        (title, win)
    })
}

fn emit_main(
    view: &ast::ViewDecl,
    p: &Program,
    out: &mut String,
    reload: bool,
) -> Result<(), EmitError> {
    let classes = &p.classes;
    let vname = format!("{}View", view.name.name);
    writeln!(out, "fn main() {{").unwrap();
    writeln!(out, "    let mut w = World::new();").unwrap();
    emit_edge_registrations(p, view, out)?;
    // The view's own table has to be installed before the view is
    // built, so its fields count from the moment it exists.
    writeln!(out, "    __pixie_register_view_edges(&mut w);").unwrap();
    let mut deps: Vec<String> = Vec::new();
    // Singletons first (stores / top-level lets); every view currently
    // subscribes to all of their signals — over-subscription is cheap
    // and keeps the wiring static.
    for (i, (gname, class)) in p.global_order.iter().enumerate() {
        let var = format!("__g{i}");
        writeln!(out, "    let {var} = w.singleton({class}::new); // {gname}").unwrap();
        // A store is a ROOT: nothing ever releases it, so its count
        // never reaches zero and neither does anything it holds.
        writeln!(out, "    w.root({var}.erase());").unwrap();
        // `fn tick @every(1000)` — a repeating callback, declared on
        // the store and registered the moment the store exists. The
        // clock is the animation clock, so a headless `advance:` fires
        // exactly the ticks a window would have.
        for m in &classes[class].methods {
            let Some(a) = m.attributes.iter().find(|a| a.name.name == "every") else {
                continue;
            };
            let ms: f64 = match a.args.first().map(|s| s.trim().parse::<f64>()) {
                Some(Ok(v)) if v > 0.0 => v,
                _ => {
                    return err(a.span, "`@every(ms)` takes the period in milliseconds, a positive number");
                }
            };
            if !m.params.is_empty() {
                return err(a.span, "a timer's method takes no parameters — it is called by the clock");
            }
            writeln!(
                out,
                "    pixie_kernel::timer::every(&mut w, {ms}f64, std::rc::Rc::new(move |w: &mut World| {{ {var}.{}(w); }}));",
                camel_to_snake(&m.name.name)
            )
            .unwrap();
        }
        // `fn save @key("cmd-s")` — a shortcut, and `fn typed(k:
        // String) @key` — every key, as the chord it was. Bound the
        // moment the store exists, like a timer, so a window and a
        // headless `key:` step reach the same handler.
        for m in &classes[class].methods {
            let Some(a) = m.attributes.iter().find(|a| a.name.name == "key") else {
                continue;
            };
            let snake = camel_to_snake(&m.name.name);
            match a.args.first().map(|s| s.trim().trim_matches('"').to_string()) {
                Some(chord) if !chord.is_empty() => {
                    if !m.params.is_empty() {
                        return err(
                            a.span,
                            "a shortcut's method takes no parameters — the chord is in the attribute",
                        );
                    }
                    writeln!(
                        out,
                        "    pixie_kernel::keys::bind(&mut w, \"{chord}\", std::rc::Rc::new(move |w: &mut World| {{ {var}.{snake}(w); }}));"
                    )
                    .unwrap();
                }
                _ => {
                    if m.params.len() != 1 {
                        return err(
                            a.span,
                            "`@key` with no chord takes the key: one `String` parameter (`fn typed(k: String) @key`)",
                        );
                    }
                    writeln!(
                        out,
                        "    pixie_kernel::keys::on_key(&mut w, std::rc::Rc::new(move |w: &mut World, k: Str| {{ {var}.{snake}(w, k); }}));"
                    )
                    .unwrap();
                }
            }
        }
        // `fn save @menu("File", "Save")` — an item in the
        // application's menu bar, declared beside the shortcuts and
        // registered the same way. A window hands the list to the
        // platform; a headless `menu:<item>` step picks one.
        for m in &classes[class].methods {
            let Some(a) = m.attributes.iter().find(|a| a.name.name == "menu") else {
                continue;
            };
            let strip = |s: &String| s.trim().trim_matches('"').to_string();
            let (Some(menu), Some(item)) = (a.args.first().map(strip), a.args.get(1).map(strip))
            else {
                return err(a.span, "`@menu(\"File\", \"Save\")` takes the menu and the item name");
            };
            if menu.is_empty() || item.is_empty() {
                return err(a.span, "`@menu(\"File\", \"Save\")` takes the menu and the item name");
            }
            if !m.params.is_empty() {
                return err(a.span, "a menu item's method takes no parameters — it is called by the menu");
            }
            writeln!(
                out,
                "    pixie_kernel::menu::item(&mut w, \"{menu}\", \"{item}\", std::rc::Rc::new(move |w: &mut World| {{ {var}.{}(w); }}));",
                camel_to_snake(&m.name.name)
            )
            .unwrap();
        }
        // `fn opened(path: String) @drop` — what happens to a file
        // dragged onto the window, declared like a shortcut.
        for m in &classes[class].methods {
            let Some(a) = m.attributes.iter().find(|a| a.name.name == "drop") else {
                continue;
            };
            if m.params.len() != 1 {
                return err(
                    a.span,
                    "`@drop` takes the path: one `String` parameter (`fn opened(p: String) @drop`)",
                );
            }
            writeln!(
                out,
                "    pixie_kernel::drop::on_file(&mut w, std::rc::Rc::new(move |w: &mut World, p: Str| {{ {var}.{}(w, p); }}));",
                camel_to_snake(&m.name.name)
            )
            .unwrap();
        }
        // Class-typed props whose default CONSTRUCTS (§8.64). The
        // slot came out of `new()` empty, because `new()` has no
        // World; here it does, so the object is built and assigned
        // through the ordinary setter — which counts the edge.
        for pr in &classes[class].props {
            let RustTy::Handle(target) = &pr.ty else {
                continue;
            };
            let Some(d) = &pr.default else { continue };
            let ExprKind::Call { callee, args, .. } = &d.kind else {
                continue;
            };
            if !matches!(&callee.kind, ExprKind::Ident(c) if c == target) {
                return err(
                    d.span,
                    format!(
                        "`{}` holds a `{target}`, so its default has to construct one",
                        pr.camel
                    ),
                );
            }
            let Some(target_info) = classes.get(target) else {
                return err(d.span, format!("unknown class `{target}`"));
            };
            let mut ctor_args = Vec::with_capacity(args.len());
            for (n, a) in args.iter().enumerate() {
                let want = target_info
                    .init
                    .and_then(|it| it.params.get(n))
                    .map(|q| lower_type(&q.ty, &p.class_names))
                    .transpose()?
                    .unwrap_or(RustTy::Unit);
                ctor_args.push(lower_default(a, &want)?);
            }
            writeln!(
                out,
                "    {{ let __o = w.insert({target}::new({})); {var}.set_{}(&mut w, __o); }}",
                ctor_args.join(", "),
                pr.rust
            )
            .unwrap();
        }
        for s in &classes[class].signals {
            deps.push(format!("({var}.erase(), {})", s.const_name));
        }
    }
    let mut fields: Vec<String> = Vec::new();
    for b in &state_bindings(view, p)? {
        let rf = camel_to_snake(&b.field);
        let turbofish = if b.rust_args.is_empty() {
            String::new()
        } else {
            format!("::{}", b.rust_args)
        };
        for line in &b.ctor_prelude {
            writeln!(out, "    {line}").unwrap();
        }
        writeln!(
            out,
            "    let {rf} = w.insert({}{turbofish}::new({}));",
            b.class,
            b.ctor_args.join(", ")
        )
        .unwrap();
        fields.push(rf.clone());
        for s in &classes[&b.class].signals {
            deps.push(format!("({rf}.erase(), {})", s.const_name));
        }
    }
    for s in &row_seats(view)? {
        let rf = camel_to_snake(&s.field);
        writeln!(
            out,
            "    let {rf} = w.insert(pixie_kernel::RowSeat::<{}>::new());",
            s.holder
        )
        .unwrap();
        fields.push(rf.clone());
        deps.push(format!("({rf}.erase(), {})", s.sig_const));
    }
    // §8.66: classes the view can only REACH — through a store's
    // class-typed prop, a row's, or a chain of them. There is no
    // handle to name at mount time (the object may not exist yet, and
    // a list's rows change), so the view listens for the SIGNAL on
    // any object that can emit it. Signal ids are unique per (class,
    // property), so this is exactly "any `Tag`'s `n` changed" —
    // deliberately wider than a named target, the same over-
    // subscription the store wiring already chose, and static.
    let mut held: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (_, class) in &p.global_order {
        held.insert(class.clone());
    }
    for b in &state_bindings(view, p)? {
        held.insert(b.class.clone());
    }
    let mut reached: Vec<String> = Vec::new();
    let mut frontier: Vec<String> = held.iter().cloned().collect();
    frontier.sort();
    let mut seen = held.clone();
    while let Some(c) = frontier.pop() {
        let Some(info) = classes.get(&c) else { continue };
        for pr in &info.props {
            for target in handle_classes_of(&pr.ty) {
                if seen.insert(target.clone()) {
                    reached.push(target.clone());
                    frontier.push(target);
                }
            }
        }
    }
    reached.sort();
    writeln!(out, "    let __view = mount(").unwrap();
    writeln!(out, "        &mut w,").unwrap();
    writeln!(out, "        {vname} {{ {} }},", fields.join(", ")).unwrap();
    writeln!(out, "        &[").unwrap();
    for d in &deps {
        writeln!(out, "            {d},").unwrap();
    }
    writeln!(out, "        ],").unwrap();
    writeln!(out, "    );").unwrap();
    if !reached.is_empty() {
        writeln!(out, "    {{").unwrap();
        writeln!(out, "        let __hv = __view.erase();").unwrap();
        for c in &reached {
            let Some(info) = classes.get(c) else { continue };
            for sig in &info.signals {
                writeln!(
                    out,
                    "        w.connect_class({}, Rc::new(move |w| w.mark_view_dirty(__hv))); // {c}",
                    sig.const_name
                )
                .unwrap();
            }
        }
        writeln!(out, "    }}").unwrap();
    }
    // Synthesized startup hook: a store fn literally named
    // `__start` (double-underscore = synthesized, the
    // __pixie_row_seat convention) runs ONCE with the World — after
    // mount so views subscribe first, before the first frame, and
    // CONTAINED so a failing startup prints and the app still opens
    // (the Python tier's exception policy, matched).
    for (i, (_g, class)) in p.global_order.iter().enumerate() {
        let has_start = classes
            .get(class)
            .map(|info| info.methods.iter().any(|m| m.name.name == "__start"))
            .unwrap_or(false);
        if has_start {
            writeln!(
                out,
                "    pixie_kernel::contain(\"startup\", || {{ __g{i}.__start(&mut w); }});"
            )
            .unwrap();
            writeln!(out, "    w.flush();").unwrap();
        }
    }
    // The async tier wraps the World from here on (Runtime::new wires
    // w.async_ctx, so handlers can spawn).
    writeln!(out, "    let __rt = Runtime::new(w);").unwrap();
    if reload {
        writeln!(out, "    __PIXIE_TABLES.with(|t| *t.borrow_mut() = Some(std::rc::Rc::new(__pixie_tables())));").unwrap();
    }
    // Engine mode unless PIXIE_SCRIPT asks for the headless harness.
    writeln!(out, "    if std::env::var(\"PIXIE_SCRIPT\").is_err() {{").unwrap();
    if reload {
        // Rung 2: watch the source; view-slice edits re-interpret in
        // process (validated against the live World before install),
        // anything else defers to the outer `pixie watch` rebuild.
        writeln!(out, "        let __watch = pixie_engine_gpui::ReloadWatch {{").unwrap();
        writeln!(out, "            path: std::path::PathBuf::from(__PIXIE_SRC),").unwrap();
        writeln!(out, "            reload: Box::new(move |w: &mut World| {{").unwrap();
        writeln!(out, "                let Ok(__text) = std::fs::read_to_string(__PIXIE_SRC) else {{ return true; }};").unwrap();
        writeln!(out, "                let __t0 = std::time::Instant::now();").unwrap();
        // The rung question: has anything the COMPILED half owns
        // changed — in the entry or in an import (§8.72)? `false`
        // asks the watcher for a rebuild.
        writeln!(out, "                let Ok(__fp) = pixie_interp::program_fingerprint_of(&__text, &__pixie_foreign_paths()) else {{ return true; }};").unwrap();
        writeln!(out, "                if __fp != __PIXIE_FINGERPRINT {{ return false; }}").unwrap();
        writeln!(out, "                match pixie_interp::reload_from_source_with(&__text, &__pixie_foreign()) {{").unwrap();
        writeln!(out, "                    Ok((_, __lv)) => {{").unwrap();
        writeln!(out, "                        let __lv = std::rc::Rc::new(__lv);").unwrap();
        writeln!(out, "                        let __env = {{").unwrap();
        writeln!(out, "                            let v = w.get(__view);").unwrap();
        writeln!(out, "                            __interp_env(v)").unwrap();
        writeln!(out, "                        }};").unwrap();
        writeln!(out, "                        let __tables = __PIXIE_TABLES.with(|t| t.borrow().clone()).expect(\"tables installed\");").unwrap();
        writeln!(out, "                        match pixie_interp::build_view(&__lv, &__env, &__tables, w) {{").unwrap();
        writeln!(out, "                            Ok(_) => {{").unwrap();
        writeln!(out, "                                __PIXIE_LIVE.with(|l| *l.borrow_mut() = Some(__lv));").unwrap();
        writeln!(out, "                                eprintln!(\"pixie reload: view updated in {{:?}}\", __t0.elapsed());").unwrap();
        writeln!(out, "                            }}").unwrap();
        writeln!(out, "                            Err(e) => eprintln!(\"pixie reload: {{e}}; keeping the last good view\"),").unwrap();
        writeln!(out, "                        }}").unwrap();
        writeln!(out, "                        true").unwrap();
        writeln!(out, "                    }}").unwrap();
        writeln!(out, "                    Err(e) => {{").unwrap();
        writeln!(out, "                        eprintln!(\"pixie reload: parse error: {{e}}; keeping the last good view\");").unwrap();
        writeln!(out, "                        true").unwrap();
        writeln!(out, "                    }}").unwrap();
        writeln!(out, "                }}").unwrap();
        writeln!(out, "            }}),").unwrap();
        writeln!(out, "        }};").unwrap();
        let (title_expr, win_expr) = window_exprs();
        writeln!(
            out,
            "        let __title = {title_expr};\n        pixie_engine_gpui::run_app(__rt, __view, &__title, Some(__watch), {win_expr});"
        )
        .unwrap();
    } else {
        let (title_expr, win_expr) = window_exprs();
        writeln!(
            out,
            "        let __title = {title_expr};\n        pixie_engine_gpui::run_app(__rt, __view, &__title, None, {win_expr});"
        )
        .unwrap();
    }
    writeln!(out, "        return;").unwrap();
    writeln!(out, "    }}").unwrap();
    if reload {
        // The divergence gate (§5.11 / R3): PIXIE_TIER=interp runs the
        // whole headless script through the interpreter — every build
        // delegates to it once the live view is installed — and the
        // printed trees must match the default (compiled) tier's.
        writeln!(out, "    if std::env::var(\"PIXIE_TIER\").as_deref() == Ok(\"interp\") {{").unwrap();
        writeln!(out, "        let __text = std::fs::read_to_string(__PIXIE_SRC)").unwrap();
        writeln!(out, "            .expect(\"PIXIE_TIER=interp needs the source the binary was built from\");").unwrap();
        writeln!(out, "        let __fp = pixie_interp::program_fingerprint_of(&__text, &__pixie_foreign_paths())").unwrap();
        writeln!(out, "            .expect(\"PIXIE_TIER=interp: source must parse\");").unwrap();
        writeln!(out, "        let (_, __lv) = pixie_interp::reload_from_source_with(&__text, &__pixie_foreign())").unwrap();
        writeln!(out, "            .expect(\"PIXIE_TIER=interp: source must parse\");").unwrap();
        writeln!(out, "        assert_eq!(__fp, __PIXIE_FINGERPRINT, \"PIXIE_TIER=interp: source drifted from the built binary\");").unwrap();
        writeln!(out, "        __PIXIE_LIVE.with(|l| *l.borrow_mut() = Some(std::rc::Rc::new(__lv)));").unwrap();
        writeln!(out, "        eprintln!(\"pixie tier: interp\");").unwrap();
        writeln!(out, "    }}").unwrap();
    }
    writeln!(out, "    let _ = __rt.with(|w| w.take_dirty_views());").unwrap();
    // §8.37: the tree carries RESOLVED colors, so the startup palette
    // has to reach the World before the first build — headless too,
    // or `PIXIE_THEME=light` would print a dark tree.
    writeln!(
        out,
        "    __rt.with(|w: &mut World| pixie_kernel::theme::set_light(w, std::env::var(\"PIXIE_THEME\").is_ok_and(|v| v == \"light\")));"
    )
    .unwrap();
    writeln!(out, "    let mut __tree = __rt.with(|w| {{").unwrap();
    writeln!(out, "        pixie_kernel::build_prepared(w, __view)").unwrap();
    writeln!(out, "    }});").unwrap();
    writeln!(out, "    pixie_kernel::script::anim_settle(&__rt, __view, &mut __tree);").unwrap();
    writeln!(out, "    __rt.with(|w| println!(\"{{}}\", __tree.dump(w)));").unwrap();
    // Scripted interaction: the M0 stand-in for an event loop, and the
    // hook the acceptance tests drive the generated reactive path with.
    // Steps: `click:<label>` · `input[@n]:<text>` · `submit[@n]` ·
    // `slide[@n]:<value>`. Every
    // step settles the async tier before the next one runs, so scripted
    // runs stay deterministic.
    writeln!(out, "    if let Ok(__script) = std::env::var(\"PIXIE_SCRIPT\") {{").unwrap();
    writeln!(out, "        println!(\"{{}}\", pixie_kernel::script::run(&__rt, __view, &mut __tree, &__script));").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
    Ok(())
}
