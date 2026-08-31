//! `Type` representation and the subtype relation.
//!
//! pixie uses nominal subtyping for classes (Qt parent-tree as well as
//! ARC-managed classes), structural equality for value types and
//! primitives, and bottom-style absorption for `Type::Error` so that
//! we don't emit a cascade of follow-on errors after the first one.

use pixie_hir::{ItemKind, ResolvedProgram};
use pixie_syntax::ast::{TypeExpr, TypeKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Type {
    Prim(Prim),
    /// A non-parametric class declared in the current module.
    Class(String),
    /// Parametric type: built-in (`List<T>`, `Map<K,V>`, `Set<T>`,
    /// `Hash<K,V>`, `Future<T>`) or user-defined `class Box<T> {}`.
    /// Built-in bases lower to Qt counterparts via codegen's
    /// `cute_to_cpp`.
    Generic {
        base: String,
        args: Vec<Type>,
    },
    /// A class we have no binding for. Method/property access is opaque
    /// and always succeeds in soft-mode. Subtype questions answer "yes"
    /// against any other External or Class - we can't refute them
    /// without binding files.
    External(String),
    /// A user-defined or extern enum. Variants are looked up via
    /// `Type::Enum("Color").Red` member access; the type itself is
    /// distinct from `Int` (no implicit conversion). The string is
    /// the enum's Cute name; the variant table lives on the program.
    Enum(String),
    /// A `flags X of E` type — QFlags<E> at the C++ level. Allows
    /// `|` / `&` / `^` / `.has(v)` between values; the underlying
    /// enum on its own rejects bitwise ops. The string is the
    /// flags type name (not the underlying enum's).
    Flags(String),
    /// `T?` nullable.
    Nullable(Box<Type>),
    /// `!T` error union with a bound error type name.
    ErrorUnion {
        ok: Box<Type>,
        err: String,
    },
    /// Function type. Includes block / lambda types.
    Fn {
        params: Vec<Type>,
        ret: Box<Type>,
    },
    /// `:foo` symbol literals at expression-position. Compile-time names
    /// for property/signal references.
    Sym,
    /// Reference to a signal on a class, produced by `obj.signal` member
    /// access. The AST stays untyped — this is a type-system value only.
    /// Permits `.connect(handler)` and `.disconnect()`; rejects every
    /// other method. Distinct from `Type::Fn` — signal-refs are not
    /// assignable to function-typed variables.
    ///
    /// `class` is the **declaring** class (the one walked to by
    /// `lookup_signal`'s super-class chain). Receiver-side covariance is
    /// thus stable: `MyReply : QNetworkReply` accessing `.readyRead` and
    /// the parent reading `.readyRead` produce equal `SignalRef`s.
    SignalRef {
        class: String,
        signal: String,
        params: Vec<Type>,
    },
    /// Type variable used by the generic-fn inference pass. Solved into
    /// a `Substitution` during call-site unification (`stage B`).
    Var(VarId),
    /// Sink type: this expression already produced a diagnostic. Anything
    /// involving it short-circuits without further errors.
    Error,
    /// A type we couldn't determine and don't want to flag (e.g. result of
    /// a method call on an External). Compatible with everything.
    Unknown,
}

/// Identifier for a type variable inside a `Substitution`. Allocated by
/// `VarSource::fresh` (one per call-site instantiation of a generic fn).
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct VarId(pub u32);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Prim {
    Bool,
    Int,
    Float,
    String,
    /// COW byte string (kernel `Bytes`, §11.10) — first-class since
    /// §8.19: strict members (`length`), not printable, no
    /// literals yet.
    Bytes,
    Void,
    /// `nil` literal type. Subtype of any Nullable.
    Nil,
}

impl Type {
    pub fn void() -> Self {
        Type::Prim(Prim::Void)
    }
    pub fn bool() -> Self {
        Type::Prim(Prim::Bool)
    }
    pub fn int() -> Self {
        Type::Prim(Prim::Int)
    }
    pub fn float() -> Self {
        Type::Prim(Prim::Float)
    }
    pub fn string() -> Self {
        Type::Prim(Prim::String)
    }
    pub fn nil() -> Self {
        Type::Prim(Prim::Nil)
    }
    pub fn bytes() -> Self {
        Type::Prim(Prim::Bytes)
    }

    /// User-facing pretty rendering.
    pub fn render(&self) -> String {
        match self {
            Type::Prim(Prim::Bool) => "Bool".into(),
            Type::Prim(Prim::Int) => "Int".into(),
            Type::Prim(Prim::Float) => "Float".into(),
            Type::Prim(Prim::String) => "String".into(),
            Type::Prim(Prim::Bytes) => "Bytes".into(),
            Type::Prim(Prim::Void) => "Void".into(),
            Type::Prim(Prim::Nil) => "Nil".into(),
            Type::Class(n) | Type::External(n) | Type::Enum(n) | Type::Flags(n) => n.clone(),
            Type::Generic { base, args } => {
                let a = args
                    .iter()
                    .map(|x| x.render())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{base}<{a}>")
            }
            Type::Nullable(t) => format!("{}?", t.render()),
            Type::ErrorUnion { ok, err } => format!("!{} (err = {err})", ok.render()),
            Type::Fn { params, ret } => {
                let ps = params
                    .iter()
                    .map(|p| p.render())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("fn({ps}) -> {}", ret.render())
            }
            Type::Sym => "Symbol".into(),
            Type::SignalRef {
                class,
                signal,
                params,
            } => {
                let ps = params
                    .iter()
                    .map(|p| p.render())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("signal {class}::{signal}({ps})")
            }
            Type::Var(VarId(n)) => format!("?T{n}"),
            Type::Error => "<error>".into(),
            Type::Unknown => "<unknown>".into(),
        }
    }
}

/// Names of built-in parametric types whose `Type::Generic { base }` is
/// guaranteed to be present and corresponds 1-1 to a Qt type at codegen.
pub const BUILTIN_GENERIC_BASES: &[&str] = &["List", "Map", "Set", "Hash", "Future", "Slice"];

pub fn is_builtin_generic(base: &str) -> bool {
    BUILTIN_GENERIC_BASES.contains(&base)
}

/// Async fns lower to a Qt 6.5+ coroutine whose C++ return type is
/// `QFuture<T>`. Cute lets users declare the return as either bare `T`
/// (codegen wraps) or `Future<T>` (codegen leaves as-is). Either way the
/// fn *body's trailing expression* produces a value of type `T` — the
/// QFuture wrapping happens at codegen, not at the surface. So when
/// type-checking the body, peel a leading `Future<>` if the user spelled
/// it explicitly.
pub fn peel_async_return(ret_ty: Type, is_async: bool) -> Type {
    if !is_async {
        return ret_ty;
    }
    match ret_ty {
        Type::Generic { ref base, ref args } if base == "Future" && args.len() == 1 => {
            args[0].clone()
        }
        other => other,
    }
}

/// Lower a syntactic `TypeExpr` to a semantic `Type`, resolving named
/// type references against the resolved program and binding `!T` to the
/// module's default error type when there is one.
///
/// Parametric type names with type-arg lists (`List<T>`, `Future<T>`,
/// `Box<X>`) lower to `Type::Generic { base, args }`. Built-in bases
/// (`List`/`Map`/`Set`/`Hash`/`Future`) produce a Generic regardless of
/// whether the user has declared a class with that name; user code
/// re-using these names is rejected at the resolver level (TODO).
pub fn lower_type(te: &TypeExpr, program: &ResolvedProgram) -> Type {
    match &te.kind {
        TypeKind::Named { path, args } => {
            let name = path.last().map(|i| i.name.as_str()).unwrap_or("");
            // Bare-prim shortcuts: only when there are no type args.
            if args.is_empty() {
                match name {
                    "String" => return Type::string(),
                    "Bytes" => return Type::bytes(),
                    "Bool" => return Type::bool(),
                    "Int" => return Type::int(),
                    "Float" => return Type::float(),
                    "Void" => return Type::void(),
                    _ => {}
                }
            }
            // With type args -> Generic. Without args, fall through to the
            // class/struct/error/fn lookup or External.
            if !args.is_empty() {
                let lowered_args: Vec<Type> = args.iter().map(|a| lower_type(a, program)).collect();
                return Type::Generic {
                    base: name.to_string(),
                    args: lowered_args,
                };
            }
            // No args, not a primitive: name resolution.
            match program.items.get(name) {
                Some(ItemKind::Class { .. }) => Type::Class(name.to_string()),
                Some(ItemKind::Struct { .. }) => Type::Class(name.to_string()),
                Some(ItemKind::Fn { .. }) => Type::Class(name.to_string()),
                // A trait name in type-annotation position is not
                // currently meaningful (no dynamic dispatch yet).
                // Fall through to External so a future `Box<dyn Foo>`
                // surface stays additive.
                Some(ItemKind::Trait { .. }) => Type::External(name.to_string()),
                // Top-level `let X : T = ...` doesn't introduce a
                // type — `X` is a value, not a type. In type-annotation
                // position the lookup is a syntax-level confusion;
                // fall through to External so the usual diagnostics fire.
                //
                // Exception: same-name singleton (`let Foo : Foo = Foo.new()`,
                // synthesized by `store Foo { ... }`). The Let overwrote
                // the Class entry in `prog.items`, but its declared type
                // points back to itself, so the user-facing name still
                // refers to the (lost) class. Treat as a Class lookup so
                // `Foo.new()` and `Foo.member` type-check correctly.
                Some(ItemKind::Let { ty, .. }) => {
                    if let TypeKind::Named { path, .. } = &ty.kind {
                        if path.last().map(|i| i.name.as_str()) == Some(name) {
                            return Type::Class(name.to_string());
                        }
                    }
                    Type::External(name.to_string())
                }
                Some(ItemKind::Enum { .. }) => Type::Enum(name.to_string()),
                Some(ItemKind::Flags { .. }) => Type::Flags(name.to_string()),
                None => Type::External(name.to_string()),
            }
        }
        TypeKind::Nullable(inner) => Type::Nullable(Box::new(lower_type(inner, program))),
        TypeKind::ErrorUnion(inner) => {
            let err = program
                .default_error_type
                .clone()
                .unwrap_or_else(|| "<unbound>".to_string());
            Type::ErrorUnion {
                ok: Box::new(lower_type(inner, program)),
                err,
            }
        }
        TypeKind::Fn { params, ret } => Type::Fn {
            params: params.iter().map(|p| lower_type(p, program)).collect(),
            ret: Box::new(lower_type(ret, program)),
        },
        TypeKind::SelfType => Type::Unknown,
    }
}

/// Subtype relation: is `sub` assignable to `sup`?
///
/// Soft-fails (returns true) on `Error` / `Unknown` / `External` so that
/// missing bindings do not cascade into a wall of follow-on diagnostics
/// during early-stage development.
pub fn is_subtype(sub: &Type, sup: &Type, program: &ResolvedProgram) -> bool {
    use Type::*;

    // Bottom-style absorption: errors and unknowns are compatible with
    // anything to suppress secondary diagnostics.
    if matches!(sub, Error | Unknown) || matches!(sup, Error | Unknown) {
        return true;
    }

    if sub == sup {
        return true;
    }

    match (sub, sup) {
        // nil <: T? for any T.
        (Prim(self::Prim::Nil), Nullable(_)) => true,
        // T <: T? when T <: U.
        (a, Nullable(b)) => is_subtype(a, b, program),

        // Int widens to Float - standard numeric coercion that
        // matches both Qt (qreal accepts integer literals) and the
        // codegen path (`int -> double` is implicit in C++). Lets
        // `width: 100` pass against a `Float`-typed property.
        (Prim(self::Prim::Int), Prim(self::Prim::Float)) => true,

        // External widens with anything: we don't yet know the binding,
        // be permissive so pre-binding-files code doesn't drown in errors.
        (External(_), _) | (_, External(_)) => true,

        (Class(a), Class(b)) => is_class_chain_subtype(a, b, program),

        (
            ErrorUnion {
                ok: ok_a,
                err: err_a,
            },
            ErrorUnion {
                ok: ok_b,
                err: err_b,
            },
        ) => is_subtype(ok_a, ok_b, program) && err_a == err_b,

        // Value promotion into an error union: a plain `T` flows into
        // `!T` as the ok side, and the union's error enum flows in as
        // the err side (`return MathError.divByZero` raises). Went
        // unexercised in the ancestor because bindings made every
        // error path `External`.
        (Enum(name), ErrorUnion { err, .. }) if name == err => true,
        (a, ErrorUnion { ok, .. }) => is_subtype(a, ok, program),

        // Generic types are invariant in their arguments (safest default).
        // `List<Int>` is NOT a subtype of `List<Number>`.
        (Generic { base: ba, args: aa }, Generic { base: bb, args: ab }) => {
            ba == bb
                && aa.len() == ab.len()
                && aa
                    .iter()
                    .zip(ab.iter())
                    .all(|(a, b)| is_subtype(a, b, program) && is_subtype(b, a, program))
        }

        // Type variables are compatible with anything pre-substitution.
        // Real binding/unification happens in `crate::infer::unify` (Stage B).
        (Var(_), _) | (_, Var(_)) => true,

        (
            Fn {
                params: pa,
                ret: ra,
            },
            Fn {
                params: pb,
                ret: rb,
            },
        ) => {
            // Functions are contravariant in params, covariant in return.
            // For our minimum, demand exact-arity and component subtype.
            pa.len() == pb.len()
                && pa.iter().zip(pb).all(|(a, b)| is_subtype(b, a, program))
                && is_subtype(ra, rb, program)
        }

        _ => false,
    }
}

/// Walk the class chain to decide if `sub_name` extends `sup_name`
/// (transitively). Both sides here are known Cute classes (`Type::Class`);
/// once the walk reaches a class we don't model (e.g. `QObject` itself or
/// any foreign Qt class), we answer false: a known Cute class's ancestry
/// rooted in a foreign class cannot transitively contain another known
/// Cute class, so the only way to be a subtype of a known class is to
/// have it appear before the foreign ceiling.
///
/// Soft-pass for "we don't know this hierarchy" is handled by the
/// `External` arms of `is_subtype`, not here.
fn is_class_chain_subtype(sub_name: &str, sup_name: &str, program: &ResolvedProgram) -> bool {
    if sub_name == sup_name {
        return true;
    }
    let mut current = sub_name;
    loop {
        let Some(ItemKind::Class { super_class, .. }) = program.items.get(current) else {
            return false;
        };
        let Some(parent) = super_class.as_deref() else {
            return false;
        };
        if parent == sup_name {
            return true;
        }
        current = parent;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixie_hir::{ItemKind, resolve};
    use pixie_syntax::{parse, span::FileId};

    fn prog(src: &str) -> ResolvedProgram {
        let m = parse(FileId(0), src).expect("parse");
        resolve(&m, &pixie_hir::ProjectInfo::default()).program
    }

    #[test]
    fn nil_is_subtype_of_any_nullable() {
        let p = prog("");
        assert!(is_subtype(
            &Type::nil(),
            &Type::Nullable(Box::new(Type::int())),
            &p
        ));
        assert!(is_subtype(
            &Type::nil(),
            &Type::Nullable(Box::new(Type::Class("X".into()))),
            &p
        ));
    }

    #[test]
    fn t_is_subtype_of_t_nullable() {
        let p = prog("");
        assert!(is_subtype(
            &Type::int(),
            &Type::Nullable(Box::new(Type::int())),
            &p
        ));
        // But Float? is not a subtype of Int?.
        assert!(!is_subtype(
            &Type::Nullable(Box::new(Type::float())),
            &Type::Nullable(Box::new(Type::int())),
            &p,
        ));
    }

    #[test]
    fn class_chain_is_recognized() {
        let p = prog(
            r#"
class A {}
class B < A {}
class C < B {}
"#,
        );
        assert!(is_subtype(
            &Type::Class("C".into()),
            &Type::Class("B".into()),
            &p
        ));
        assert!(is_subtype(
            &Type::Class("C".into()),
            &Type::Class("A".into()),
            &p
        ));
        assert!(is_subtype(
            &Type::Class("B".into()),
            &Type::Class("A".into()),
            &p
        ));
        assert!(!is_subtype(
            &Type::Class("A".into()),
            &Type::Class("B".into()),
            &p
        ));
    }

    #[test]
    fn external_classes_are_soft_compatible() {
        // QAbstractListModel has no binding yet but should be assignable
        // to QObject (and accepted as-is for opaque calls).
        let p = prog("");
        assert!(is_subtype(
            &Type::External("QAbstractListModel".into()),
            &Type::External("QObject".into()),
            &p,
        ));
        assert!(is_subtype(
            &Type::External("QObject".into()),
            &Type::Class("A".into()),
            &p
        ));
    }

    #[test]
    fn int_widens_to_float_but_not_vice_versa() {
        // Standard numeric coercion: Int -> Float is implicit
        // (matches both Qt's qreal accepting integer literals and
        // C++'s int -> double widening). Float -> Int needs an
        // explicit conversion.
        let p = prog("");
        assert!(is_subtype(&Type::int(), &Type::float(), &p));
        assert!(!is_subtype(&Type::float(), &Type::int(), &p));
        assert!(is_subtype(&Type::int(), &Type::int(), &p));
    }

    #[test]
    fn lower_type_handles_primitives_and_nullable_and_error_union() {
        let m = parse(
            FileId(0),
            r#"
error AppError { boom }

fn f(x: Int?) !Float {
  doIt()
}
"#,
        )
        .unwrap();
        let p = resolve(&m, &pixie_hir::ProjectInfo::default()).program;
        let pixie_syntax::ast::Item::Fn(f) = &m.items[1] else {
            unreachable!()
        };

        let param_ty = lower_type(&f.params[0].ty, &p);
        assert_eq!(param_ty, Type::Nullable(Box::new(Type::int())));

        let ret_ty = lower_type(f.return_ty.as_ref().unwrap(), &p);
        match ret_ty {
            Type::ErrorUnion { ok, err } => {
                assert_eq!(*ok, Type::float());
                assert_eq!(err, "AppError");
            }
            _ => panic!("expected error union"),
        }
    }

    #[test]
    fn unknown_class_resolves_to_external() {
        let p = prog("");
        let m = parse(FileId(0), "fn f(x: QSomeUnboundType) {}").unwrap();
        let pixie_syntax::ast::Item::Fn(f) = &m.items[0] else {
            unreachable!()
        };
        let t = lower_type(&f.params[0].ty, &p);
        assert!(matches!(t, Type::External(ref n) if n == "QSomeUnboundType"));
    }

    #[test]
    fn list_t_lowers_to_generic() {
        let m = parse(FileId(0), "fn f(xs: List<Int>) {}").unwrap();
        let p = resolve(&m, &pixie_hir::ProjectInfo::default()).program;
        let pixie_syntax::ast::Item::Fn(f) = &m.items[0] else {
            unreachable!()
        };
        let t = lower_type(&f.params[0].ty, &p);
        match t {
            Type::Generic { ref base, ref args } => {
                assert_eq!(base, "List");
                assert_eq!(args, &vec![Type::int()]);
            }
            other => panic!("expected Generic, got {other:?}"),
        }
    }

    #[test]
    fn map_k_v_lowers_to_generic_with_two_args() {
        let m = parse(FileId(0), "fn f(m: Map<String, Int>) {}").unwrap();
        let p = resolve(&m, &pixie_hir::ProjectInfo::default()).program;
        let pixie_syntax::ast::Item::Fn(f) = &m.items[0] else {
            unreachable!()
        };
        let t = lower_type(&f.params[0].ty, &p);
        match t {
            Type::Generic { ref base, ref args } => {
                assert_eq!(base, "Map");
                assert_eq!(args, &vec![Type::string(), Type::int()]);
            }
            other => panic!("expected Generic, got {other:?}"),
        }
    }

    #[test]
    fn future_lowers_to_generic_with_t() {
        let m = parse(FileId(0), "fn f(x: Future<Int>) {}").unwrap();
        let p = resolve(&m, &pixie_hir::ProjectInfo::default()).program;
        let pixie_syntax::ast::Item::Fn(f) = &m.items[0] else {
            unreachable!()
        };
        let t = lower_type(&f.params[0].ty, &p);
        match t {
            Type::Generic { ref base, ref args } => {
                assert_eq!(base, "Future");
                assert_eq!(args, &vec![Type::int()]);
            }
            other => panic!("expected Generic, got {other:?}"),
        }
    }

    #[test]
    fn generic_subtype_is_invariant_in_args() {
        let p = prog("class A {} class B < A {}");
        // List<B> is NOT a subtype of List<A>: invariance.
        let lb = Type::Generic {
            base: "List".into(),
            args: vec![Type::Class("B".into())],
        };
        let la = Type::Generic {
            base: "List".into(),
            args: vec![Type::Class("A".into())],
        };
        assert!(
            !is_subtype(&lb, &la, &p),
            "List<B> should not be a subtype of List<A>"
        );
        // Same Generic with same args: yes.
        let li = Type::Generic {
            base: "List".into(),
            args: vec![Type::int()],
        };
        assert!(is_subtype(&li, &li, &p));
        // Different bases: no.
        let si = Type::Generic {
            base: "Set".into(),
            args: vec![Type::int()],
        };
        assert!(!is_subtype(&li, &si, &p));
    }

    #[test]
    fn generic_render_is_human_readable() {
        let g = Type::Generic {
            base: "Map".into(),
            args: vec![
                Type::string(),
                Type::Generic {
                    base: "List".into(),
                    args: vec![Type::int()],
                },
            ],
        };
        assert_eq!(g.render(), "Map<String, List<Int>>");
    }

    #[test]
    fn known_class_resolves_to_class() {
        let m = parse(
            FileId(0),
            "class TodoItem {}\nfn f(x: TodoItem) {}",
        )
        .unwrap();
        let p = resolve(&m, &pixie_hir::ProjectInfo::default()).program;
        let pixie_syntax::ast::Item::Fn(f) = &m.items[1] else {
            unreachable!()
        };
        let t = lower_type(&f.params[0].ty, &p);
        assert_eq!(t, Type::Class("TodoItem".into()));
        // Also: ItemKind for TodoItem is Class.
        assert!(matches!(p.items["TodoItem"], ItemKind::Class { .. }));
    }
}
