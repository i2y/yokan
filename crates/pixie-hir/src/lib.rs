//! pixie HIR (high-level IR): resolved AST plus per-function scope info.
//!
//! Scope: the bare minimum to unlock production codegen quality. Concretely:
//!
//! - Top-level item table (class / struct / error / fn) so `T?` lowering
//!   can tell QObject-derived types from value types, and so `!T` returns
//!   can bind to the (single) error decl in scope.
//! - Per-function name resolution that tracks which `name = expr`
//!   statements are first-occurrence (-> emit `auto name = expr;`) vs
//!   reassignments (-> emit `name = expr;`).
//! - One concrete cross-member type-side check: every `notify: :sig` must
//!   reference an actual signal in the surrounding class.
//!
//! Anything heavier (type inference for arbitrary expressions, generic
//! instantiation, full unification) lives in `pixie-types` and lands later.
//! The current pixie-types crate is still a stub.

use pixie_syntax::ast::*;
use pixie_syntax::diag::Diagnostic;
use pixie_syntax::span::{FileId, Span};
use std::collections::{HashMap, HashSet};

/// Project-level layout passed by the driver: which module each file
/// belongs to, and which other modules each module imports via `use`.
/// Drives the visibility check that lets `pub` mean something across
/// `.cute` files.
#[derive(Default, Debug, Clone)]
pub struct ProjectInfo {
    /// FileId -> module name (file stem of that `.cute` source).
    /// Items declared in a file inherit this as their owning module.
    pub module_for_file: HashMap<FileId, String>,
    /// Module name -> the set of modules it has `use`d (whole-module
    /// imports, including `as`-aliased ones - the alias is keyed in
    /// `module_aliases` separately so this set always reflects the
    /// real module names that are reachable for visibility purposes).
    pub imports_for_module: HashMap<String, HashSet<String>>,
    /// `use foo as bar` aliases: for each importing module,
    /// `local_alias -> real_module_name`. Lets `bar.X` resolve to the
    /// item declared in module `foo`. The `module_for_file` value is
    /// always the real module name (file stem); alias resolution
    /// happens in the visibility check.
    pub module_aliases: HashMap<String, HashMap<String, String>>,
    /// `use foo.{X, Y as A}` selective imports: for each importing
    /// module, `local_name -> (source_module, original_name)`. Bare
    /// references to `local_name` resolve to the named item without
    /// requiring the source module to be in `imports_for_module`.
    pub selective_imports: HashMap<String, HashMap<String, (String, String)>>,
    /// `pub use foo.X` re-exports: for each declaring module,
    /// `exported_name -> (source_module, source_name)`. When another
    /// module imports the declaring module, the re-exported name
    /// looks indistinguishable from one the declaring module wrote
    /// itself - the visibility check treats `M.X` as reachable via
    /// `M`'s re-export, even if `X` actually lives in module `foo`.
    pub re_exports: HashMap<String, HashMap<String, (String, String)>>,
    /// Item names that came from binding files (Qt stdlib `.qpi`
    /// imports + user-supplied bindings). These are unconditionally
    /// visible from every module - the prelude.
    pub prelude_items: HashSet<String>,
}

/// A resolved Cute program: the AST plus name-resolution annotations
/// codegen needs in order to make context-sensitive decisions.
#[derive(Default, Debug, Clone)]
pub struct ResolvedProgram {
    /// Top-level items by simple name. Lossy when the same simple
    /// name appears in multiple modules - the entry "wins" per
    /// insertion order (last-seen). For module-precise lookups,
    /// consult `items_by_module` instead.
    pub items: HashMap<String, ItemKind>,

    /// Top-level items keyed by `(home_module, name)` so multiple
    /// modules can declare the same simple name without losing
    /// information. Module-level namespacing relies on this for cross-
    /// module visibility checks and codegen's emit-name resolution.
    pub items_by_module: HashMap<(String, String), ItemKind>,

    /// Per-function annotations, keyed by the `Block` span of the body.
    pub fn_scopes: HashMap<Span, FnScope>,

    /// If the module declares exactly one `error` decl, all `!T` returns
    /// in that module bind to that error type. Multiple decls or zero
    /// decls leave this `None` (codegen falls back to a placeholder).
    pub default_error_type: Option<String>,

    /// `impl Trait for Type` registry. Keyed by implementing-type
    /// simple name; value is the set of trait names that type
    /// implements. Drives the bound-satisfaction check in
    /// cute-types: when a generic `T: Iterable` call is resolved
    /// with `T = ConcreteType`, we look up `ConcreteType` here and
    /// confirm `Iterable` is in the set.
    pub impls_for: HashMap<String, HashSet<String>>,
}

/// Where an item was declared, for the visibility check. `User(name)`
/// for items declared in a `.cute` file (`name` is that file's
/// module). `Prelude` for items pulled in from binding files (Qt
/// stdlib, manifest-listed `.qpi`s) - those are always visible.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemHome {
    User(String),
    Prelude,
}

#[derive(Clone, Debug)]
pub enum ItemKind {
    Class {
        super_class: Option<String>,
        /// Direct super is `QObject` or another QObject-derived class we
        /// know about (transitively). Heuristic: the super name is
        /// `QObject` itself, or ends in a name we recorded as a Class
        /// whose super is QObject. For Qt classes we don't model
        /// (e.g. `QAbstractListModel`), we accept the convention that
        /// any name starting with `Q` and capitalised is QObject-derived
        /// until binding files say otherwise.
        is_world_class: bool,
        /// `extern value Foo { ... }` — plain C++ value type. Mutually
        /// exclusive with `is_world_class` (forced false) and ARC
        /// (no Object ancestry). Codegen: `T.new(args)` → `T(args)`,
        /// member access via `.`, no Arc, no metaobject.
        is_extern_value: bool,
        property_names: Vec<String>,
        signal_names: Vec<String>,
        /// Names of `static fn` declarations on this class. Used by
        /// codegen to dispatch `ClassName.method(args)` to the
        /// `ClassName::method(args)` C++ static call form (vs the
        /// instance `recv->method(args)` form). Includes both
        /// user-declared statics and binding-side statics.
        static_methods: Vec<String>,
        home: ItemHome,
        is_pub: bool,
    },
    Struct {
        home: ItemHome,
        is_pub: bool,
    },
    Fn {
        home: ItemHome,
        is_pub: bool,
    },
    /// `trait Foo { fn bar -> Baz }` — declared interface. Each
    /// method is kept with enough information to (a) verify that
    /// bound generic calls only access listed methods, (b) check
    /// that impl blocks supply every required method, and (c)
    /// validate call-site arity / arg types against the trait
    /// method's declared signature.
    Trait {
        home: ItemHome,
        is_pub: bool,
        methods: Vec<TraitMethodSig>,
    },
    /// `let X : T = expr` at module scope. Visible to every fn /
    /// class body in the module. The HIR keeps the declared type
    /// (as the source `TypeExpr`) so the type checker can resolve
    /// bare `Ident(X)` references without re-running the parser, and
    /// so codegen can decide between `static const auto` and
    /// `Q_GLOBAL_STATIC` lowering. The `is_world_type` flag is the
    /// codegen hint — true when the declared type resolves to a
    /// QObject-derived class (lowers to `Q_GLOBAL_STATIC`), false for
    /// value types (lowers to `static const auto`).
    Let {
        ty: TypeExpr,
        is_world_type: bool,
        home: ItemHome,
        is_pub: bool,
    },
    /// `enum Color { Red; Green = 7; ... }` or `extern enum Foo
    /// { Bar = 1 }`. Carries the variant list (name + optional
    /// explicit value Expr) for the type checker (lookup of
    /// `Color.Red`) and codegen (emit `enum class Color : qint32`
    /// for user-defined / nothing for extern with a C++ namespace
    /// lookup at call site).
    Enum {
        variants: Vec<EnumVariantSig>,
        is_extern: bool,
        /// True when this enum was declared with the `error` keyword.
        /// Such enums double as the value type of `!T` (Result) returns.
        is_error: bool,
        cpp_namespace: Option<String>,
        home: ItemHome,
        is_pub: bool,
    },
    /// `flags Alignment of AlignmentFlag`. Carries the underlying
    /// enum's name so the type checker can validate that the
    /// referenced enum exists and that `flags X | Y` lifts a pair
    /// of variant values into the flags type.
    Flags {
        of: String,
        is_extern: bool,
        cpp_namespace: Option<String>,
        home: ItemHome,
        is_pub: bool,
    },
}

/// Per-variant info on an `enum` / `extern enum` decl. Used by
/// type-check to resolve `Color.Red` (the variant must exist) and
/// optionally by codegen for the explicit value (when the variant
/// has `= expr`) or for payload pattern matching (when the
/// variant has `fields`).
#[derive(Clone, Debug)]
pub struct EnumVariantSig {
    pub name: String,
    pub value: Option<pixie_syntax::ast::Expr>,
    /// Payload field declarations carried over verbatim from the
    /// AST — name + TypeExpr (no resolved Type at this layer).
    /// Empty for nullary variants (`Red`).
    pub fields: Vec<pixie_syntax::ast::Field>,
    pub is_pub: bool,
}

/// Per-method signature on a `trait` declaration. Used by the type
/// checker to validate generic-bound method calls (name, arity,
/// signature) and by the HIR `impl_completeness_check` pass to know
/// which methods an impl must supply.
#[derive(Clone, Debug)]
pub struct TraitMethodSig {
    pub name: String,
    /// `true` if the trait declaration provided a body (default
    /// implementation). Default-bodied methods are optional in
    /// `impl` blocks; abstract ones are required.
    pub has_default: bool,
    /// Whole `FnDecl` from the trait, kept so the type checker can
    /// substitute `Self` → concrete-T and validate arg-position
    /// types at the call site. Body (when present) is the default
    /// implementation; codegen splices it into impls that omit the
    /// method.
    pub fn_decl: FnDecl,
}

impl ItemKind {
    pub fn home(&self) -> &ItemHome {
        match self {
            ItemKind::Class { home, .. }
            | ItemKind::Struct { home, .. }
            | ItemKind::Fn { home, .. }
            | ItemKind::Trait { home, .. }
            | ItemKind::Let { home, .. }
            | ItemKind::Enum { home, .. }
            | ItemKind::Flags { home, .. } => home,
        }
    }
    pub fn is_pub(&self) -> bool {
        match self {
            ItemKind::Class { is_pub, .. }
            | ItemKind::Struct { is_pub, .. }
            | ItemKind::Fn { is_pub, .. }
            | ItemKind::Trait { is_pub, .. }
            | ItemKind::Let { is_pub, .. }
            | ItemKind::Enum { is_pub, .. }
            | ItemKind::Flags { is_pub, .. } => *is_pub,
        }
    }
}

#[derive(Default, Clone, Debug)]
pub struct FnScope {
    /// For each `Stmt::Assign` with `op == Eq` and `target` a bare
    /// `Ident`, true => the LHS was not previously declared in this
    /// function body, so codegen should emit `auto name = expr;`.
    pub assign_is_decl: HashMap<Span, bool>,
}

#[derive(Debug, Clone)]
pub struct ResolveResult {
    pub program: ResolvedProgram,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn resolve(module: &Module, project: &ProjectInfo) -> ResolveResult {
    let mut prog = ResolvedProgram::default();
    let mut diags = Vec::new();

    // -- pass 0: pixie surface rejections (D9) -----------------------------
    // No user-class inheritance and no `arc`: every plain class is a
    // World-backed handle type. Binding-provided items are exempt.
    for item in &module.items {
        if let Item::Class(c) = item {
            if !matches!(item_home(item, project), ItemHome::User(_)) {
                continue;
            }
            if let Some(sup) = &c.super_class {
                diags.push(Diagnostic::error(
                    sup.span,
                    "pixie classes have no inheritance: compose, or share behavior with a trait",
                ));
            }
            if c.is_arc {
                diags.push(Diagnostic::error(
                    c.span,
                    "`arc` was removed in pixie: every class lives in the World as a handle type; write a plain `class`",
                ));
            }
        }
    }

    // -- pass 1: collect item index ----------------------------------------
    // Each named item lands in two indices:
    //   - `prog.items` keyed by simple name (last-write-wins; lossy
    //     when modules share a simple name but kept for
    //     backward-compat with bare-name lookup paths).
    //   - `prog.items_by_module` keyed by (home_module, name) so
    //     same-name across modules is preserved without loss.
    // Counts/names for default-err-type inference. The rule:
    //
    //   - If exactly one `error E { ... }` decl exists, that wins.
    //   - Otherwise, if exactly one `enum E { ... }` decl exists in the
    //     module, that wins (lets users skip the `error` keyword when
    //     there's no ambiguity).
    //   - Otherwise no default — `!T` codegen falls back to a placeholder.
    let mut error_count = 0usize;
    let mut single_error_name: Option<String> = None;
    let mut enum_count = 0usize;
    let mut single_enum_name: Option<String> = None;
    for item in &module.items {
        let home = item_home(item, project);
        // Module key for the precise-lookup index. Prelude items use
        // an empty module string so binding-only resolves can still
        // find them via items_by_module.
        let module_key = match &home {
            ItemHome::User(m) => m.clone(),
            ItemHome::Prelude => String::new(),
        };
        match item {
            Item::Class(c) => {
                let super_name = class_super_name(c);
                // Every plain class is World-backed (handles + signals).
                // `extern value` / `arc` forms opt out — both are
                // rejected in pixie user code by pass 0, but the flags
                // remain because binding forms may reuse them.
                let is_qobj = !(c.is_extern_value || c.is_arc);
                let property_names = c
                    .members
                    .iter()
                    .filter_map(|m| match m {
                        ClassMember::Property(p) => Some(p.name.name.clone()),
                        _ => None,
                    })
                    .collect();
                // signal_names must include synthesized prop notifies
                // (cute-codegen emits them too); without them
                // `emit_widget_cute_ui` wires zero requestRebuild
                // connects on classes whose only signal source is a
                // bindable prop, freezing the cute_ui Component on
                // its first frame.
                let mut signal_names: Vec<String> = c
                    .members
                    .iter()
                    .filter_map(|m| match m {
                        ClassMember::Signal(s) => Some(s.name.name.clone()),
                        _ => None,
                    })
                    .collect();
                for m in &c.members {
                    if let ClassMember::Property(p) = m {
                        // Every prop carries a NOTIFY by default. Three
                        // resolution rules, picked first-match:
                        //   1. explicit `notify: :foo` — user-supplied
                        //      name (kept for backwards compat with
                        //      hand-written demos).
                        //   2. `, model` — no NOTIFY (the underlying
                        //      ModelList<T>* pointer is stable; row
                        //      changes propagate via the model's own
                        //      QAbstractItemModel signals).
                        //   3. `, constant` — explicit opt-out for
                        //      genuinely immutable storage.
                        //   4. otherwise — synthesize the conventional
                        //      `<propName>Changed` so reactive bindings
                        //      Just Work without ceremony. (Was
                        //      previously gated on `bindable` /
                        //      `bind { }` / `fresh { }`; removing the
                        //      gate is the v1.x boilerplate cut.)
                        let name = if let Some(n) = p.notify.as_ref() {
                            n.name.clone()
                        } else if p.model || p.constant {
                            continue;
                        } else {
                            p.synth_notify_name()
                        };
                        if !signal_names.contains(&name) {
                            signal_names.push(name);
                        }
                    }
                }
                let static_methods: Vec<String> = c
                    .members
                    .iter()
                    .filter_map(|m| match m {
                        ClassMember::Fn(f) if f.is_static => Some(f.name.name.clone()),
                        _ => None,
                    })
                    .collect();
                let kind = ItemKind::Class {
                    super_class: super_name,
                    is_world_class: is_qobj,
                    is_extern_value: c.is_extern_value,
                    property_names,
                    signal_names,
                    static_methods,
                    home: home.clone(),
                    is_pub: c.is_pub,
                };
                prog.items.insert(c.name.name.clone(), kind.clone());
                prog.items_by_module
                    .insert((module_key.clone(), c.name.name.clone()), kind);
            }
            Item::Struct(s) => {
                let kind = ItemKind::Struct {
                    home: home.clone(),
                    is_pub: s.is_pub,
                };
                prog.items.insert(s.name.name.clone(), kind.clone());
                prog.items_by_module
                    .insert((module_key.clone(), s.name.name.clone()), kind);
            }
            Item::Fn(f) => {
                let kind = ItemKind::Fn {
                    home: home.clone(),
                    is_pub: f.is_pub,
                };
                prog.items.insert(f.name.name.clone(), kind.clone());
                prog.items_by_module
                    .insert((module_key.clone(), f.name.name.clone()), kind);
            }
            Item::Use(_) => {}
            Item::UseQml(_) => {} // foreign QML module decl, no Cute item
            // Views / widgets are pure-codegen artifacts: views lower
            // to `.qml` text at build time, widgets to imperative C++.
            // Neither introduces a name in the value namespace.
            // Styles are compile-time-only constants resolved at codegen
            // time; they never produce a runtime symbol either.
            Item::View(_) | Item::Widget(_) | Item::Style(_) => {}
            Item::Let(l) => {
                // Top-level `let` introduces a name in the value
                // namespace. The codegen-relevant flag — is the
                // declared type a QObject-derived class? — is computed
                // from the type's resolved base name against the
                // already-collected `prog.items` registry. (Built-in
                // names like Int / Float / Bool / String resolve to
                // false, since they're not in `prog.items`.)
                let is_qobj = match &l.ty.kind {
                    pixie_syntax::ast::TypeKind::Named { path, .. } => {
                        let name = path.last().map(|i| i.name.as_str()).unwrap_or("");
                        is_world_class(Some(name), &prog.items)
                    }
                    _ => false,
                };
                let kind = ItemKind::Let {
                    ty: l.ty.clone(),
                    is_world_type: is_qobj,
                    home: home.clone(),
                    is_pub: l.is_pub,
                };
                prog.items.insert(l.name.name.clone(), kind.clone());
                prog.items_by_module
                    .insert((module_key.clone(), l.name.name.clone()), kind);
            }
            Item::Trait(t) => {
                let kind = ItemKind::Trait {
                    home: home.clone(),
                    is_pub: t.is_pub,
                    methods: t
                        .methods
                        .iter()
                        .map(|m| TraitMethodSig {
                            name: m.name.name.clone(),
                            has_default: m.body.is_some(),
                            fn_decl: m.clone(),
                        })
                        .collect(),
                };
                prog.items.insert(t.name.name.clone(), kind.clone());
                prog.items_by_module
                    .insert((module_key.clone(), t.name.name.clone()), kind);
            }
            Item::Impl(i) => {
                // Register (target_type, trait_name) so the type
                // checker can confirm bound satisfaction. Codegen
                // consumes the same data to splice impl methods
                // onto the target class.
                //
                // Key by the for-type's simple base name. Both
                // `impl Foo for List<T>` and `impl Foo for List<Int>`
                // share `"List"`, and `impl Foo for QStringList`
                // shares `"QStringList"`. The bound check at call
                // sites matches on base, not on full type-arg shape
                // — coherence (rejecting overlapping impls) is left
                // to a follow-up.
                if let Some(base) = type_expr_base_name(&i.for_type) {
                    prog.impls_for
                        .entry(base)
                        .or_insert_with(HashSet::new)
                        .insert(i.trait_name.name.clone());
                }
            }
            Item::Enum(e) => {
                // Skip extern enums (binding-only, never serve as the
                // module's err type) when counting candidates for
                // default-err-type inference. Also skip prelude-loaded
                // enums/errors (e.g. stdlib `QtBoolError` in
                // `qcore.qpi`) so binding decls don't silently become
                // the program's default `!T` err type — they exist
                // only for the type checker / call-site wrappers.
                let is_prelude_decl = project.prelude_items.contains(&e.name.name);
                if !e.is_extern && !is_prelude_decl {
                    if e.is_error {
                        error_count += 1;
                        single_error_name = Some(e.name.name.clone());
                    } else {
                        enum_count += 1;
                        single_enum_name = Some(e.name.name.clone());
                    }
                }
                let kind = ItemKind::Enum {
                    variants: e
                        .variants
                        .iter()
                        .map(|v| EnumVariantSig {
                            name: v.name.name.clone(),
                            value: v.value.clone(),
                            fields: v.fields.clone(),
                            is_pub: v.is_pub,
                        })
                        .collect(),
                    is_extern: e.is_extern,
                    is_error: e.is_error,
                    cpp_namespace: e.cpp_namespace.clone(),
                    home: home.clone(),
                    is_pub: e.is_pub,
                };
                prog.items.insert(e.name.name.clone(), kind.clone());
                prog.items_by_module
                    .insert((module_key.clone(), e.name.name.clone()), kind);
            }
            Item::Flags(f) => {
                let kind = ItemKind::Flags {
                    of: f.of.name.clone(),
                    is_extern: f.is_extern,
                    cpp_namespace: f.cpp_namespace.clone(),
                    home: home.clone(),
                    is_pub: f.is_pub,
                };
                prog.items.insert(f.name.name.clone(), kind.clone());
                prog.items_by_module
                    .insert((module_key.clone(), f.name.name.clone()), kind);
            }
            Item::Store(_) => unreachable!(
                "Item::Store should be lowered to Item::Class + Item::Let by \
                 cute_codegen::desugar_store::desugar_store before HIR runs",
            ),
            Item::Suite(_) => unreachable!(
                "Item::Suite should be flattened by \
                 cute_codegen::desugar_suite::desugar_suite before HIR runs",
            ),
        }
    }
    if error_count == 1 {
        prog.default_error_type = single_error_name;
    } else if error_count == 0 && enum_count == 1 {
        prog.default_error_type = single_enum_name;
    }

    // -- pass 2: per-class checks (notify-signal existence) ----------------
    for item in &module.items {
        if let Item::Class(c) = item {
            let signal_names: HashSet<&str> = c
                .members
                .iter()
                .filter_map(|m| match m {
                    ClassMember::Signal(s) => Some(s.name.name.as_str()),
                    _ => None,
                })
                .collect();
            for member in &c.members {
                if let ClassMember::Property(p) = member {
                    if let Some(sig_id) = &p.notify {
                        if !signal_names.contains(sig_id.name.as_str()) {
                            diags.push(
                                Diagnostic::error(
                                    sig_id.span,
                                    format!(
                                        "property `{}` notifies signal `{}`, but no such signal is declared in class `{}`",
                                        p.name.name, sig_id.name, c.name.name
                                    ),
                                )
                                .with_note(p.span, "property declared here"),
                            );
                        }
                    }
                }
            }
        }
    }

    // -- pass 3: per-fn scope walk -----------------------------------------
    for item in &module.items {
        match item {
            Item::Fn(f) => collect_fn_scope(f, &mut prog),
            Item::Class(c) => {
                for m in &c.members {
                    match m {
                        ClassMember::Fn(f) | ClassMember::Slot(f) => collect_fn_scope(f, &mut prog),
                        ClassMember::Init(i) => collect_body_scope(&i.params, &i.body, &mut prog),
                        ClassMember::Deinit(d) => collect_body_scope(&[], &d.body, &mut prog),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    // -- pass 4: loop-control validation -----------------------------------
    // `break` / `continue` are only meaningful inside a `for` / `while`
    // loop. The visibility-check pass walks fn bodies but only when the
    // file is registered as a user module — bind/test sources skip it.
    // Run loop-control validation independently so every fn body gets
    // checked, regardless of file home.
    loop_control_check(module, &mut diags);

    // -- pass 4b: case-exhaustiveness warning ------------------------------
    // Pixie codegen closes every `case` with a silent `_ => {}` tail
    // (no UB — unmatched inputs are no-ops), so a missed shape means
    // silently skipped behavior, not a crash. Warn here for the
    // syntactic shapes (ok/err over `!T`, true/false over Bool,
    // some/nil over `T?`, error-enum variants); declared enums get a
    // hard typed error in pixie-types on top of this.
    case_exhaustiveness_check(module, &prog, &mut diags);

    // -- pass 4c: impl-completeness check ----------------------------------
    // For each `impl Trait for Type`, verify the impl supplies every
    // trait method that lacks a default body. Without this check,
    // codegen would silently produce a class missing the method and
    // the user would see a downstream C++ template-instantiation
    // error rather than a clear Cute-source diagnostic.
    impl_completeness_check(module, &mut diags);

    // -- pass 4d: impl-coherence check -------------------------------------
    // Reject duplicate `(trait, for-type)` pairs and overlapping
    // parametric/concrete forms. Without this, the splice path's
    // last-write-wins semantics + namespace-overload duplication
    // surface as confusing build behavior — codegen drops or
    // double-emits methods depending on which form lands second.
    //
    // **Specialization rule (v1)**: parametric vs concrete overlap
    // is allowed when the for-type base is a non-splice target —
    // extern value classes, prelude/binding classes, builtin
    // generics. For these, both impls land as free functions in
    // `cute::trait_impl::<Trait>::` and C++ overload resolution
    // picks the most specific. Splice-target user classes
    // (non-extern_value, home: User) keep the rejection because
    // their class body can only carry one definition per method.
    impl_coherence_check(module, &prog, &mut diags);

    // -- pass 4e: fn-overload coherence check ------------------------------
    // Reject duplicate fn / method / impl-method overloads (same scope,
    // same name, same arity, same param-type list). The type-table now
    // stores overload sets as `Vec<FnTy>`; without this pass, two
    // identical signatures on the same name would both be registered
    // and the resolver's Tier-2 (exact-type) match would be ambiguous.
    // Mirrors `impl_coherence_check`'s structure.
    fn_overload_coherence_check(module, &mut diags);

    // -- pass 4f: iterator-invalidation lint -------------------------------
    // Warn on `for x in coll { coll.append(...)|coll.remove(...)|... }`
    // patterns where the body mutates the same collection being
    // iterated. Qt's QList iterators (and most std containers) are
    // invalidated by structural mutations during iteration; this warning
    // catches the bug at compile time instead of at undefined-behavior
    // runtime.
    iterator_invalidation_check(module, &mut diags);

    // -- pass 4g: class-kind hint ------------------------------------------
    // For each `class X { ... }` (QObject-derived, no super), check
    // whether the class actually uses any Q_OBJECT machinery (signal /
    // slot / Q_PROPERTY-bearing prop). When none of those features
    // are used, emit a hint-level diagnostic suggesting `arc X { ... }`
    // — the lighter ARC variant — to save the user the metaobject
    // overhead.

    // -- pass 4h-j: silent-trap lints --------------------------------------
    // Three small lints that catch patterns that compile cleanly but
    // produce wrong / surprising runtime behaviour:
    //   (h) manual `emit fooChanged` after a prop write to `foo` (the
    //       setter auto-emits, so the manual emit fires twice)
    //   (i) `pub var x : T` on a QObject-derived class (exposed as
    //       getter/setter but not as Q_PROPERTY → QML can't bind to it)
    //   (j) parent-less `T.new()` in a top-level fn body where the
    //       binding is never used (silent QObject leak)
    manual_emit_after_prop_write_check(module, &mut diags);
    parentless_qobject_new_check(module, &prog, &mut diags);

    // -- pass 5: cross-module visibility check -----------------------------
    // For each reference inside a user-declared item, check that the
    // target item is reachable from the reference's module under the
    // `pub` + `use` rules. Bare references resolve to: same module ->
    // any visibility; prelude (Qt bindings) -> always visible; an
    // imported module's pub items -> visible. Qualified refs (`m.X`)
    // require `m` to be the current module or in the import set, and
    // `X` to be pub.
    visibility_check(module, &prog, project, &mut diags);

    ResolveResult {
        program: prog,
        diagnostics: diags,
    }
}

// ---- loop-control validation ------------------------------------------------

/// Reject `break` / `continue` outside of a `for` / `while` loop.
/// Mirrors the C / C++ rule. Without this, a stray top-level `break`
/// would silently lower to a C++ `break;` and surface as a compile
/// error pointing into generated source — moving the check here pins
/// the diagnostic to the user's `.cute` line. Lambdas open a fresh
/// function scope and reset the in-loop context.
/// Warn when a `for x in coll { body }` body mutates the same `coll`
/// being iterated. Detects:
///
/// - `coll.append(...)` / `coll.push_back(...)` / `coll.prepend(...)`
///   / `coll.insert(...)` / `coll.remove(...)` / `coll.removeAt(...)`
///   / `coll.erase(...)` / `coll.clear()` / `coll.resize(...)`
///   / `coll.pop()` / `coll.pop_back()` / `coll.sort()` / `coll.reverse()`
/// - `coll[i] = v` (subscript assignment)
/// - `coll = ...` (re-bind the receiver)
///
/// Detection is lexical: we match by simple binding name. The receiver
/// can be either a bare identifier (`for x in xs`) or `@name`
/// (`for x in @items`); inside the body, references via the same form
/// are flagged. Cross-aliasing (e.g. `let alias = xs; alias.append(...)`)
/// is NOT detected — kept simple to avoid false positives.
fn iterator_invalidation_check(module: &Module, diags: &mut Vec<Diagnostic>) {
    fn collection_name(iter: &Expr) -> Option<String> {
        match &iter.kind {
            ExprKind::Ident(n) => Some(n.clone()),
            ExprKind::AtIdent(n) => Some(format!("@{n}")),
            _ => None,
        }
    }
    fn matches_collection(expr: &Expr, coll: &str) -> bool {
        let coll_at = coll.starts_with('@');
        match &expr.kind {
            ExprKind::Ident(n) => !coll_at && n == coll,
            ExprKind::AtIdent(n) => coll_at && format!("@{n}") == coll,
            _ => false,
        }
    }
    fn is_mutating_method(name: &str) -> bool {
        matches!(
            name,
            "append"
                | "push_back"
                | "prepend"
                | "insert"
                | "remove"
                | "removeAt"
                | "removeFirst"
                | "removeLast"
                | "erase"
                | "clear"
                | "resize"
                | "pop"
                | "pop_back"
                | "pop_front"
                | "sort"
                | "reverse"
                | "swap"
        )
    }
    fn walk_body(b: &Block, coll: &str, diags: &mut Vec<Diagnostic>) {
        for s in &b.stmts {
            walk_stmt(s, coll, diags);
        }
        if let Some(t) = &b.trailing {
            walk_expr(t, coll, diags);
        }
    }
    fn walk_stmt(s: &Stmt, coll: &str, diags: &mut Vec<Diagnostic>) {
        match s {
            Stmt::Expr(e) => walk_expr(e, coll, diags),
            Stmt::Let { value, .. } | Stmt::Var { value, .. } => walk_expr(value, coll, diags),
            Stmt::Return { value: Some(v), .. } => walk_expr(v, coll, diags),
            Stmt::Assign {
                target,
                value,
                span,
                ..
            } => {
                if matches_collection(target, coll) {
                    diags.push(Diagnostic::warning(
                        *span,
                        format!("re-binding `{coll}` while iterating it may invalidate the loop"),
                    ));
                } else if let ExprKind::Index { receiver, .. } = &target.kind {
                    if matches_collection(receiver, coll) {
                        diags.push(Diagnostic::warning(
                            *span,
                            format!(
                                "subscript-assigning into `{coll}` while iterating it may invalidate the loop"
                            ),
                        ));
                    }
                }
                walk_expr(target, coll, diags);
                walk_expr(value, coll, diags);
            }
            Stmt::For { iter, body, .. } => {
                walk_expr(iter, coll, diags);
                walk_body(body, coll, diags);
            }
            Stmt::While { cond, body, .. } => {
                walk_expr(cond, coll, diags);
                walk_body(body, coll, diags);
            }
            Stmt::Batch { body, .. } => walk_body(body, coll, diags),
            Stmt::Emit { args, .. } => {
                for a in args {
                    walk_expr(a, coll, diags);
                }
            }
            Stmt::Break { .. } | Stmt::Continue { .. } | Stmt::Return { value: None, .. } => {}
        }
    }
    fn walk_expr(e: &Expr, coll: &str, diags: &mut Vec<Diagnostic>) {
        use ExprKind as K;
        match &e.kind {
            K::MethodCall {
                receiver,
                method,
                args,
                block,
                ..
            } => {
                if matches_collection(receiver, coll) && is_mutating_method(&method.name) {
                    diags.push(Diagnostic::warning(
                        e.span,
                        format!(
                            "modifying `{coll}` (`.{}`) while iterating it may invalidate the loop",
                            method.name
                        ),
                    ));
                }
                walk_expr(receiver, coll, diags);
                for a in args {
                    walk_expr(a, coll, diags);
                }
                if let Some(b) = block {
                    walk_expr(b, coll, diags);
                }
            }
            K::Call {
                callee,
                args,
                block,
                ..
            } => {
                walk_expr(callee, coll, diags);
                for a in args {
                    walk_expr(a, coll, diags);
                }
                if let Some(b) = block {
                    walk_expr(b, coll, diags);
                }
            }
            K::If {
                cond,
                then_b,
                else_b,
                ..
            } => {
                walk_expr(cond, coll, diags);
                walk_body(then_b, coll, diags);
                if let Some(eb) = else_b {
                    walk_body(eb, coll, diags);
                }
            }
            K::Case { scrutinee, arms } => {
                walk_expr(scrutinee, coll, diags);
                for arm in arms {
                    walk_body(&arm.body, coll, diags);
                }
            }
            K::Block(b) => walk_body(b, coll, diags),
            K::Lambda { body, .. } => walk_body(body, coll, diags),
            K::Member { receiver, .. } | K::SafeMember { receiver, .. } => {
                walk_expr(receiver, coll, diags);
            }
            K::SafeMethodCall {
                receiver,
                args,
                block,
                ..
            } => {
                walk_expr(receiver, coll, diags);
                for a in args {
                    walk_expr(a, coll, diags);
                }
                if let Some(b) = block {
                    walk_expr(b, coll, diags);
                }
            }
            K::Index { receiver, index } => {
                walk_expr(receiver, coll, diags);
                walk_expr(index, coll, diags);
            }
            K::Unary { expr, .. } => walk_expr(expr, coll, diags),
            K::Binary { lhs, rhs, .. } => {
                walk_expr(lhs, coll, diags);
                walk_expr(rhs, coll, diags);
            }
            K::Try(inner) | K::Await(inner) => walk_expr(inner, coll, diags),
            _ => {}
        }
    }
    /// Walk every `for` loop in the module's fn / method bodies and
    /// run the per-loop check.
    fn scan_block(b: &Block, diags: &mut Vec<Diagnostic>) {
        for s in &b.stmts {
            scan_stmt(s, diags);
        }
        if let Some(t) = &b.trailing {
            scan_expr(t, diags);
        }
    }
    fn scan_stmt(s: &Stmt, diags: &mut Vec<Diagnostic>) {
        match s {
            Stmt::For { iter, body, .. } => {
                if let Some(coll) = collection_name(iter) {
                    walk_body(body, &coll, diags);
                }
                scan_block(body, diags);
            }
            Stmt::While { body, .. } => scan_block(body, diags),
            Stmt::Batch { body, .. } => scan_block(body, diags),
            Stmt::Expr(e) => scan_expr(e, diags),
            Stmt::Let { value, .. } | Stmt::Var { value, .. } => scan_expr(value, diags),
            Stmt::Return { value: Some(v), .. } => scan_expr(v, diags),
            Stmt::Assign { value, .. } => scan_expr(value, diags),
            Stmt::Emit { args, .. } => {
                for a in args {
                    scan_expr(a, diags);
                }
            }
            _ => {}
        }
    }
    fn scan_expr(e: &Expr, diags: &mut Vec<Diagnostic>) {
        use ExprKind as K;
        match &e.kind {
            K::MethodCall {
                receiver,
                args,
                block,
                ..
            } => {
                scan_expr(receiver, diags);
                for a in args {
                    scan_expr(a, diags);
                }
                if let Some(b) = block {
                    scan_expr(b, diags);
                }
            }
            K::Call {
                callee,
                args,
                block,
                ..
            } => {
                scan_expr(callee, diags);
                for a in args {
                    scan_expr(a, diags);
                }
                if let Some(b) = block {
                    scan_expr(b, diags);
                }
            }
            K::If {
                cond,
                then_b,
                else_b,
                ..
            } => {
                scan_expr(cond, diags);
                scan_block(then_b, diags);
                if let Some(eb) = else_b {
                    scan_block(eb, diags);
                }
            }
            K::Case { scrutinee, arms } => {
                scan_expr(scrutinee, diags);
                for arm in arms {
                    scan_block(&arm.body, diags);
                }
            }
            K::Block(b) => scan_block(b, diags),
            K::Lambda { body, .. } => scan_block(body, diags),
            _ => {}
        }
    }

    for item in &module.items {
        match item {
            Item::Fn(f) => {
                if let Some(body) = &f.body {
                    scan_block(body, diags);
                }
            }
            Item::Class(c) => {
                for m in &c.members {
                    match m {
                        ClassMember::Fn(f) | ClassMember::Slot(f) => {
                            if let Some(body) = &f.body {
                                scan_block(body, diags);
                            }
                        }
                        ClassMember::Init(i) => scan_block(&i.body, diags),
                        ClassMember::Deinit(d) => scan_block(&d.body, diags),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

// ---- silent-trap lints ----------------------------------------------------
//
// Three small HIR passes catching patterns that compile cleanly but
// produce wrong / surprising runtime behaviour. Each is conservative:
// false-positive cost is much higher than false-negative cost (a noisy
// warning erodes the user's trust in lints), so the rules below err on
// the side of silence when the pattern is ambiguous.

/// Lint: warn when a class method writes to a `prop` (which auto-fires
/// the prop's NOTIFY via the generated setter) AND manually `emit`s the
/// same NOTIFY signal in the same block. The signal fires twice — usually
/// invisible at first but can drive hard-to-trace double-rebuild bugs in
/// QML / QtWidgets observers.
///
/// Conservative: only flags the case where assign + emit live in the
/// **same block** (no walking across nested if / while / case bodies).
/// A `count = 1; if cond { emit countChanged }` is not flagged — the
/// assign auto-fires unconditionally but the manual emit is conditional,
/// so the user might intentionally be re-firing under a guard. Keeping
/// the rule local makes the diagnostic actionable (the user can see both
/// statements in the same code block).
fn manual_emit_after_prop_write_check(module: &Module, diags: &mut Vec<Diagnostic>) {
    fn scan_block(
        b: &Block,
        prop_to_notify: &HashMap<String, String>,
        diags: &mut Vec<Diagnostic>,
    ) {
        // Pair every Stmt::Emit with any earlier Stmt::Assign in the
        // same block whose AtIdent target maps (via prop_to_notify) to
        // the emit's signal name.
        for (i, s) in b.stmts.iter().enumerate() {
            let Stmt::Emit {
                signal,
                span: emit_span,
                ..
            } = s
            else {
                continue;
            };
            let target_notify = signal.name.as_str();
            for prev in &b.stmts[..i] {
                let Stmt::Assign { target, .. } = prev else {
                    continue;
                };
                let ExprKind::AtIdent(prop_name) = &target.kind else {
                    continue;
                };
                let Some(notify) = prop_to_notify.get(prop_name) else {
                    continue;
                };
                if notify.as_str() == target_notify {
                    diags.push(Diagnostic::warning(
                        *emit_span,
                        format!(
                            "`{prop_name} = ...` already fires `{target_notify}` via the auto-generated setter; the manual `emit {target_notify}` will fire it twice. Remove one.",
                        ),
                    ));
                    break;
                }
            }
        }
        // Recurse into nested blocks so the lint catches the same
        // pattern inside `if` / `while` / `case` arms / etc.
        for s in &b.stmts {
            scan_stmt(s, prop_to_notify, diags);
        }
        if let Some(t) = &b.trailing {
            scan_expr(t, prop_to_notify, diags);
        }
    }
    fn scan_stmt(s: &Stmt, prop_to_notify: &HashMap<String, String>, diags: &mut Vec<Diagnostic>) {
        match s {
            Stmt::For { iter, body, .. } => {
                scan_expr(iter, prop_to_notify, diags);
                scan_block(body, prop_to_notify, diags);
            }
            Stmt::While { cond, body, .. } => {
                scan_expr(cond, prop_to_notify, diags);
                scan_block(body, prop_to_notify, diags);
            }
            Stmt::Batch { body, .. } => scan_block(body, prop_to_notify, diags),
            Stmt::Expr(e) => scan_expr(e, prop_to_notify, diags),
            Stmt::Let { value, .. } | Stmt::Var { value, .. } => {
                scan_expr(value, prop_to_notify, diags)
            }
            Stmt::Return { value: Some(v), .. } => scan_expr(v, prop_to_notify, diags),
            Stmt::Assign { value, .. } => scan_expr(value, prop_to_notify, diags),
            Stmt::Emit { args, .. } => {
                for a in args {
                    scan_expr(a, prop_to_notify, diags);
                }
            }
            _ => {}
        }
    }
    fn scan_expr(e: &Expr, prop_to_notify: &HashMap<String, String>, diags: &mut Vec<Diagnostic>) {
        use ExprKind as K;
        match &e.kind {
            K::If {
                cond,
                then_b,
                else_b,
                ..
            } => {
                scan_expr(cond, prop_to_notify, diags);
                scan_block(then_b, prop_to_notify, diags);
                if let Some(eb) = else_b {
                    scan_block(eb, prop_to_notify, diags);
                }
            }
            K::Case { scrutinee, arms } => {
                scan_expr(scrutinee, prop_to_notify, diags);
                for arm in arms {
                    scan_block(&arm.body, prop_to_notify, diags);
                }
            }
            K::Block(b) => scan_block(b, prop_to_notify, diags),
            K::Lambda { body, .. } => scan_block(body, prop_to_notify, diags),
            K::Call {
                callee,
                args,
                block,
                ..
            } => {
                scan_expr(callee, prop_to_notify, diags);
                for a in args {
                    scan_expr(a, prop_to_notify, diags);
                }
                if let Some(b) = block {
                    scan_expr(b, prop_to_notify, diags);
                }
            }
            K::MethodCall {
                receiver,
                args,
                block,
                ..
            }
            | K::SafeMethodCall {
                receiver,
                args,
                block,
                ..
            } => {
                scan_expr(receiver, prop_to_notify, diags);
                for a in args {
                    scan_expr(a, prop_to_notify, diags);
                }
                if let Some(b) = block {
                    scan_expr(b, prop_to_notify, diags);
                }
            }
            _ => {}
        }
    }

    for item in &module.items {
        let Item::Class(c) = item else {
            continue;
        };
        // Build prop-name → notify-signal-name map for this class.
        let mut prop_to_notify: HashMap<String, String> = HashMap::new();
        for m in &c.members {
            if let ClassMember::Property(p) = m {
                if let Some(n) = &p.notify {
                    prop_to_notify.insert(p.name.name.clone(), n.name.clone());
                }
            }
        }
        if prop_to_notify.is_empty() {
            continue;
        }
        for m in &c.members {
            match m {
                ClassMember::Fn(f) | ClassMember::Slot(f) => {
                    if let Some(body) = &f.body {
                        scan_block(body, &prop_to_notify, diags);
                    }
                }
                ClassMember::Init(i) => scan_block(&i.body, &prop_to_notify, diags),
                ClassMember::Deinit(d) => scan_block(&d.body, &prop_to_notify, diags),
                _ => {}
            }
        }
    }
}

/// Lint: warn when a top-level fn body binds a parent-less `T.new()`
/// (where T is QObject-derived) and then never uses the binding. The
/// QObject leaks: Cute doesn't track its lifetime (no auto-RAII for
/// raw QObject*), there's no Qt parent to clean it up, and the user
/// never wired it into a parent tree. Conservative — only fires when
/// the binding is provably unreferenced after declaration. If the user
/// passes the binding to qml_app / widget_app / cli_app / a custom
/// helper, the warning stays silent (we trust the user knows where the
/// lifetime goes).
fn parentless_qobject_new_check(
    module: &Module,
    program: &ResolvedProgram,
    diags: &mut Vec<Diagnostic>,
) {
    // Collect QObject-derived class names from the program registry.
    let qobject_classes: HashSet<String> = program
        .items
        .iter()
        .filter_map(|(name, kind)| match kind {
            ItemKind::Class {
                is_world_class: true,
                ..
            } => Some(name.clone()),
            _ => None,
        })
        .collect();
    if qobject_classes.is_empty() {
        return;
    }

    // Walk all expressions in a block and collect every Ident / AtIdent
    // / Path-leaf name referenced. Used to ask "is `name` referenced
    // anywhere after the let stmt that introduces it?".
    fn collect_names_in_block(b: &Block, names: &mut HashSet<String>) {
        for s in &b.stmts {
            collect_names_in_stmt(s, names);
        }
        if let Some(t) = &b.trailing {
            collect_names_in_expr(t, names);
        }
    }
    fn collect_names_in_stmt(s: &Stmt, names: &mut HashSet<String>) {
        match s {
            Stmt::Let { value, .. } | Stmt::Var { value, .. } => {
                collect_names_in_expr(value, names);
            }
            Stmt::Expr(e) => collect_names_in_expr(e, names),
            Stmt::Return { value: Some(v), .. } => collect_names_in_expr(v, names),
            Stmt::Assign { target, value, .. } => {
                collect_names_in_expr(target, names);
                collect_names_in_expr(value, names);
            }
            Stmt::Emit { args, .. } => {
                for a in args {
                    collect_names_in_expr(a, names);
                }
            }
            Stmt::For { iter, body, .. } => {
                collect_names_in_expr(iter, names);
                collect_names_in_block(body, names);
            }
            Stmt::While { cond, body, .. } => {
                collect_names_in_expr(cond, names);
                collect_names_in_block(body, names);
            }
            Stmt::Batch { body, .. } => collect_names_in_block(body, names),
            _ => {}
        }
    }
    fn collect_names_in_expr(e: &Expr, names: &mut HashSet<String>) {
        use ExprKind as K;
        match &e.kind {
            K::Ident(n) | K::AtIdent(n) => {
                names.insert(n.clone());
            }
            K::Path(p) => {
                if let Some(first) = p.first() {
                    names.insert(first.name.clone());
                }
            }
            K::Call {
                callee,
                args,
                block,
                ..
            } => {
                collect_names_in_expr(callee, names);
                for a in args {
                    collect_names_in_expr(a, names);
                }
                if let Some(b) = block {
                    collect_names_in_expr(b, names);
                }
            }
            K::MethodCall {
                receiver,
                args,
                block,
                ..
            }
            | K::SafeMethodCall {
                receiver,
                args,
                block,
                ..
            } => {
                collect_names_in_expr(receiver, names);
                for a in args {
                    collect_names_in_expr(a, names);
                }
                if let Some(b) = block {
                    collect_names_in_expr(b, names);
                }
            }
            K::Member { receiver, .. } | K::SafeMember { receiver, .. } => {
                collect_names_in_expr(receiver, names);
            }
            K::Index { receiver, index } => {
                collect_names_in_expr(receiver, names);
                collect_names_in_expr(index, names);
            }
            K::If {
                cond,
                then_b,
                else_b,
                ..
            } => {
                collect_names_in_expr(cond, names);
                collect_names_in_block(then_b, names);
                if let Some(eb) = else_b {
                    collect_names_in_block(eb, names);
                }
            }
            K::Case { scrutinee, arms } => {
                collect_names_in_expr(scrutinee, names);
                for arm in arms {
                    collect_names_in_block(&arm.body, names);
                }
            }
            K::Block(b) => collect_names_in_block(b, names),
            K::Lambda { body, .. } => collect_names_in_block(body, names),
            K::Kwarg { value, .. } => collect_names_in_expr(value, names),
            K::Array(items) => {
                for it in items {
                    collect_names_in_expr(it, names);
                }
            }
            K::Map(entries) => {
                for (_, v) in entries {
                    collect_names_in_expr(v, names);
                }
            }
            K::Try(inner) | K::Await(inner) => collect_names_in_expr(inner, names),
            K::Range { start, end, .. } => {
                collect_names_in_expr(start, names);
                collect_names_in_expr(end, names);
            }
            _ => {}
        }
    }

    // Pull a class name out of `T.new()`'s receiver. T may be a bare
    // identifier (`Foo.new()`) or a path leaf (`some.module.Foo.new()`).
    fn class_name_from_new_receiver(recv: &Expr) -> Option<String> {
        match &recv.kind {
            ExprKind::Ident(n) => Some(n.clone()),
            ExprKind::Path(p) => p.last().map(|i| i.name.clone()),
            _ => None,
        }
    }

    /// Pattern-match a `T.new(args)` expression and return T's name
    /// when T is a QObject-derived class declared in this program.
    /// Used by both the dead-let scan and the bare-expression scan
    /// below. Returns `None` for arc / unknown / non-class receivers.
    fn qobject_new_class<'a>(e: &Expr, qobject_classes: &'a HashSet<String>) -> Option<&'a String> {
        let ExprKind::MethodCall {
            receiver, method, ..
        } = &e.kind
        else {
            return None;
        };
        if method.name != "new" {
            return None;
        }
        let class_name = class_name_from_new_receiver(receiver)?;
        qobject_classes.get(&class_name)
    }

    /// Emit the discarded-result warning for a `QFoo.new(...)`
    /// expression whose value is dropped (bare expression-statement
    /// or trailing expr of a fn body that returns void).
    fn warn_discarded_qobject_new(
        e: &Expr,
        qobject_classes: &HashSet<String>,
        diags: &mut Vec<Diagnostic>,
    ) {
        let Some(class_name) = qobject_new_class(e, qobject_classes) else {
            return;
        };
        diags.push(Diagnostic::warning(
            e.span,
            format!(
                "`{class_name}.new()` constructs a World object and discards the handle — nothing can ever reach or remove it. Bind it (`let x = {class_name}.new()`) and hand it to an owner (a view state field or a store).",
            ),
        ));
    }

    fn scan_block(b: &Block, qobject_classes: &HashSet<String>, diags: &mut Vec<Diagnostic>) {
        for (i, s) in b.stmts.iter().enumerate() {
            // (1) Bare expression statement that throws away the
            // result of `QFoo.new(...)`. Outside a class method the
            // implicit `this` parent is gone, so the constructed
            // QObject has no owner and leaks immediately. The
            // refactoring footgun the language-feedback flagged:
            // pasting `Foo.new()` from a class method into a
            // top-level fn / cli_app / server_app body silently
            // changes ownership.
            if let Stmt::Expr(e) = s {
                warn_discarded_qobject_new(e, qobject_classes, diags);
            }

            // (2) Original case: `let foo = QFoo.new()` followed by
            // a block that never references `foo`. Same root cause as
            // (1), but harder to spot at a glance because the leak
            // hides behind a binding that just looks unused.
            let (binding_name, value, span) = match s {
                Stmt::Let {
                    name, value, span, ..
                }
                | Stmt::Var {
                    name, value, span, ..
                } => (&name.name, value, *span),
                _ => continue,
            };
            let Some(class_name) = qobject_new_class(value, qobject_classes) else {
                continue;
            };
            // Walk every later statement / trailing expr in this block
            // (and recurse into their nested blocks) collecting all
            // names. If `binding_name` doesn't appear, the QObject is
            // unused → leak.
            let mut later_names: HashSet<String> = HashSet::new();
            for later in &b.stmts[i + 1..] {
                collect_names_in_stmt(later, &mut later_names);
            }
            if let Some(t) = &b.trailing {
                collect_names_in_expr(t, &mut later_names);
            }
            if !later_names.contains(binding_name) {
                diags.push(Diagnostic::warning(
                    span,
                    format!(
                        "`{class_name}.new()` constructs a World object and `{binding_name}` is never used afterwards — nothing will reach or remove it. Pass `{binding_name}` to whatever should own it.",
                    ),
                ));
            }
        }
        // (1') Same warning if the trailing expression of the block
        // is a bare `T.new()` whose value is dropped — only fires
        // when the surrounding fn returns void; otherwise the value
        // is the fn's return and the lifetime question moves to the
        // caller (handled at that level when the value is bound).
        if let Some(t) = &b.trailing {
            warn_discarded_qobject_new(t, qobject_classes, diags);
        }
        // Recurse into nested blocks too (an inner if / while body
        // has its own "later in block" scope).
        for s in &b.stmts {
            scan_stmt(s, qobject_classes, diags);
        }
        if let Some(t) = &b.trailing {
            scan_expr(t, qobject_classes, diags);
        }
    }
    fn scan_stmt(s: &Stmt, qobject_classes: &HashSet<String>, diags: &mut Vec<Diagnostic>) {
        match s {
            Stmt::For { body, .. } | Stmt::While { body, .. } | Stmt::Batch { body, .. } => {
                scan_block(body, qobject_classes, diags);
            }
            Stmt::Let { value, .. } | Stmt::Var { value, .. } => {
                scan_expr(value, qobject_classes, diags)
            }
            Stmt::Expr(e) | Stmt::Assign { value: e, .. } => scan_expr(e, qobject_classes, diags),
            Stmt::Return { value: Some(v), .. } => scan_expr(v, qobject_classes, diags),
            Stmt::Emit { args, .. } => {
                for a in args {
                    scan_expr(a, qobject_classes, diags);
                }
            }
            _ => {}
        }
    }
    fn scan_expr(e: &Expr, qobject_classes: &HashSet<String>, diags: &mut Vec<Diagnostic>) {
        use ExprKind as K;
        match &e.kind {
            K::If {
                cond,
                then_b,
                else_b,
                ..
            } => {
                scan_expr(cond, qobject_classes, diags);
                scan_block(then_b, qobject_classes, diags);
                if let Some(eb) = else_b {
                    scan_block(eb, qobject_classes, diags);
                }
            }
            K::Case { scrutinee, arms } => {
                scan_expr(scrutinee, qobject_classes, diags);
                for arm in arms {
                    scan_block(&arm.body, qobject_classes, diags);
                }
            }
            K::Block(b) => scan_block(b, qobject_classes, diags),
            K::Lambda { body, .. } => scan_block(body, qobject_classes, diags),
            K::Call { args, block, .. } => {
                for a in args {
                    scan_expr(a, qobject_classes, diags);
                }
                if let Some(b) = block {
                    scan_expr(b, qobject_classes, diags);
                }
            }
            K::MethodCall { args, block, .. } | K::SafeMethodCall { args, block, .. } => {
                for a in args {
                    scan_expr(a, qobject_classes, diags);
                }
                if let Some(b) = block {
                    scan_expr(b, qobject_classes, diags);
                }
            }
            _ => {}
        }
    }

    for item in &module.items {
        let Item::Fn(f) = item else {
            continue;
        };
        let Some(body) = &f.body else {
            continue;
        };
        scan_block(body, &qobject_classes, diags);
    }
}

fn loop_control_check(module: &Module, diags: &mut Vec<Diagnostic>) {
    fn walk_block(b: &Block, in_loop: bool, diags: &mut Vec<Diagnostic>) {
        for s in &b.stmts {
            walk_stmt(s, in_loop, diags);
        }
        if let Some(t) = &b.trailing {
            walk_expr(t, in_loop, diags);
        }
    }
    fn walk_stmt(s: &Stmt, in_loop: bool, diags: &mut Vec<Diagnostic>) {
        match s {
            Stmt::Break { span } => {
                if !in_loop {
                    diags.push(Diagnostic::error(
                        *span,
                        "`break` outside of a loop".to_string(),
                    ));
                }
            }
            Stmt::Continue { span } => {
                if !in_loop {
                    diags.push(Diagnostic::error(
                        *span,
                        "`continue` outside of a loop".to_string(),
                    ));
                }
            }
            Stmt::For { iter, body, .. } => {
                walk_expr(iter, in_loop, diags);
                walk_block(body, true, diags);
            }
            Stmt::While { cond, body, .. } => {
                walk_expr(cond, in_loop, diags);
                walk_block(body, true, diags);
            }
            Stmt::Let { value, .. } | Stmt::Var { value, .. } => {
                walk_expr(value, in_loop, diags);
            }
            Stmt::Expr(e) => walk_expr(e, in_loop, diags),
            Stmt::Return { value: Some(v), .. } => walk_expr(v, in_loop, diags),
            Stmt::Return { value: None, .. } => {}
            Stmt::Emit { args, .. } => {
                for a in args {
                    walk_expr(a, in_loop, diags);
                }
            }
            Stmt::Assign { target, value, .. } => {
                walk_expr(target, in_loop, diags);
                walk_expr(value, in_loop, diags);
            }
            // `batch { ... }` is a transparent scope for break/continue
            // (it doesn't introduce a loop, so an inner break/continue
            // still escapes the enclosing for/while just like in a
            // bare block).
            Stmt::Batch { body, .. } => walk_block(body, in_loop, diags),
        }
    }
    fn walk_expr(e: &Expr, in_loop: bool, diags: &mut Vec<Diagnostic>) {
        use ExprKind as K;
        match &e.kind {
            K::Lambda { body, .. } => walk_block(body, false, diags),
            K::Block(b) => {
                // A value-position Block lowers to an immediately-
                // invoked lambda (`[&]() { ... }()`) so any `break`
                // inside lands inside the synthesized lambda, not the
                // enclosing loop. Reset the in-loop context to match.
                walk_block(b, false, diags)
            }
            K::If {
                cond,
                then_b,
                else_b,
                ..
            } => {
                walk_expr(cond, in_loop, diags);
                walk_block(then_b, in_loop, diags);
                if let Some(eb) = else_b {
                    walk_block(eb, in_loop, diags);
                }
            }
            K::Case { scrutinee, arms } => {
                walk_expr(scrutinee, in_loop, diags);
                for arm in arms {
                    walk_block(&arm.body, in_loop, diags);
                }
            }
            K::Call {
                callee,
                args,
                block,
                ..
            } => {
                walk_expr(callee, in_loop, diags);
                for a in args {
                    walk_expr(a, in_loop, diags);
                }
                if let Some(b) = block {
                    walk_expr(b, in_loop, diags);
                }
            }
            K::MethodCall {
                receiver,
                args,
                block,
                ..
            } => {
                walk_expr(receiver, in_loop, diags);
                for a in args {
                    walk_expr(a, in_loop, diags);
                }
                if let Some(b) = block {
                    walk_expr(b, in_loop, diags);
                }
            }
            K::Member { receiver, .. } | K::SafeMember { receiver, .. } => {
                walk_expr(receiver, in_loop, diags)
            }
            K::SafeMethodCall {
                receiver,
                args,
                block,
                ..
            } => {
                walk_expr(receiver, in_loop, diags);
                for a in args {
                    walk_expr(a, in_loop, diags);
                }
                if let Some(b) = block {
                    walk_expr(b, in_loop, diags);
                }
            }
            K::Index { receiver, index } => {
                walk_expr(receiver, in_loop, diags);
                walk_expr(index, in_loop, diags);
            }
            K::Unary { expr, .. } => walk_expr(expr, in_loop, diags),
            K::Binary { lhs, rhs, .. } => {
                walk_expr(lhs, in_loop, diags);
                walk_expr(rhs, in_loop, diags);
            }
            K::Try(inner) | K::Await(inner) => walk_expr(inner, in_loop, diags),
            K::Kwarg { value, .. } => walk_expr(value, in_loop, diags),
            K::Array(items) => {
                for it in items {
                    walk_expr(it, in_loop, diags);
                }
            }
            K::Map(entries) => {
                for (k, v) in entries {
                    walk_expr(k, in_loop, diags);
                    walk_expr(v, in_loop, diags);
                }
            }
            K::Range { start, end, .. } => {
                walk_expr(start, in_loop, diags);
                walk_expr(end, in_loop, diags);
            }
            K::Str(parts) => {
                for p in parts {
                    match p {
                        StrPart::Interp(inner) => walk_expr(inner, in_loop, diags),
                        StrPart::InterpFmt { expr, .. } => walk_expr(expr, in_loop, diags),
                        StrPart::Text(_) => {}
                    }
                }
            }
            // Element bodies never lower break/continue (no enclosing
            // semantic loop); skip walking element members.
            K::Element(_) => {}
            // Leaves with no sub-expressions.
            K::Int(_)
            | K::Float(_)
            | K::Bool(_)
            | K::Nil
            | K::Sym(_)
            | K::Ident(_)
            | K::AtIdent(_)
            | K::SelfRef
            | K::Path(_) => {}
        }
    }

    fn walk_fn(f: &FnDecl, diags: &mut Vec<Diagnostic>) {
        if let Some(body) = &f.body {
            walk_block(body, false, diags);
        }
    }

    for item in &module.items {
        match item {
            Item::Fn(f) => walk_fn(f, diags),
            Item::Class(c) => {
                for m in &c.members {
                    match m {
                        ClassMember::Fn(f) | ClassMember::Slot(f) => walk_fn(f, diags),
                        ClassMember::Init(i) => walk_block(&i.body, false, diags),
                        ClassMember::Deinit(d) => walk_block(&d.body, false, diags),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

// ---- case-exhaustiveness validation -----------------------------------------

/// Walk every `case <expr> { ... }` in user fns / class methods /
/// view bodies and emit a warning when the arms don't statically
/// cover the scrutinee. Mirrors codegen's own syntactic notion of
/// exhaustiveness (kept in sync deliberately — when codegen says
/// "exhaustive", we stay silent, and vice versa).
///
/// Recognised exhaustive shapes:
/// - any `_` (wildcard) or top-level binding arm
/// - both `ok(...)` and `err(...)` arms (over `!T`)
/// - both `true` and `false` literal arms (over Bool)
/// - every variant of an `error E { ... }` covered (when the
///   scrutinee resolves to a fn returning `!E` or `E`).
fn case_exhaustiveness_check(module: &Module, prog: &ResolvedProgram, diags: &mut Vec<Diagnostic>) {
    fn walk_block(b: &Block, ctx: &Ctx, diags: &mut Vec<Diagnostic>) {
        for s in &b.stmts {
            walk_stmt(s, ctx, diags);
        }
        if let Some(t) = &b.trailing {
            walk_expr(t, ctx, diags);
        }
    }
    fn walk_stmt(s: &Stmt, ctx: &Ctx, diags: &mut Vec<Diagnostic>) {
        match s {
            Stmt::Let { value, .. } | Stmt::Var { value, .. } => walk_expr(value, ctx, diags),
            Stmt::Assign { target, value, .. } => {
                walk_expr(target, ctx, diags);
                walk_expr(value, ctx, diags);
            }
            Stmt::Expr(e) => walk_expr(e, ctx, diags),
            Stmt::Return { value: Some(v), .. } => walk_expr(v, ctx, diags),
            Stmt::Return { value: None, .. } => {}
            Stmt::For { iter, body, .. } => {
                walk_expr(iter, ctx, diags);
                walk_block(body, ctx, diags);
            }
            Stmt::While { cond, body, .. } => {
                walk_expr(cond, ctx, diags);
                walk_block(body, ctx, diags);
            }
            Stmt::Emit { args, .. } => {
                for a in args {
                    walk_expr(a, ctx, diags);
                }
            }
            Stmt::Break { .. } | Stmt::Continue { .. } => {}
            Stmt::Batch { body, .. } => walk_block(body, ctx, diags),
        }
    }
    fn walk_expr(e: &Expr, ctx: &Ctx, diags: &mut Vec<Diagnostic>) {
        use ExprKind as K;
        match &e.kind {
            K::Case { scrutinee, arms } => {
                walk_expr(scrutinee, ctx, diags);
                if !is_case_exhaustive(scrutinee, arms, ctx.prog, ctx.module) {
                    diags.push(Diagnostic::warning(
                        e.span,
                        "non-exhaustive `case` arms; unmatched values fall \
                         through as a silent no-op"
                            .to_string(),
                    ));
                }
                for arm in arms {
                    walk_block(&arm.body, ctx, diags);
                }
            }
            K::Lambda { body, .. } => walk_block(body, ctx, diags),
            K::Block(b) => walk_block(b, ctx, diags),
            K::If {
                cond,
                then_b,
                else_b,
                ..
            } => {
                walk_expr(cond, ctx, diags);
                walk_block(then_b, ctx, diags);
                if let Some(eb) = else_b {
                    walk_block(eb, ctx, diags);
                }
            }
            K::Call {
                callee,
                args,
                block,
                ..
            } => {
                walk_expr(callee, ctx, diags);
                for a in args {
                    walk_expr(a, ctx, diags);
                }
                if let Some(b) = block {
                    walk_expr(b, ctx, diags);
                }
            }
            K::MethodCall {
                receiver,
                args,
                block,
                ..
            } => {
                walk_expr(receiver, ctx, diags);
                for a in args {
                    walk_expr(a, ctx, diags);
                }
                if let Some(b) = block {
                    walk_expr(b, ctx, diags);
                }
            }
            K::Member { receiver, .. } | K::SafeMember { receiver, .. } => {
                walk_expr(receiver, ctx, diags)
            }
            K::SafeMethodCall {
                receiver,
                args,
                block,
                ..
            } => {
                walk_expr(receiver, ctx, diags);
                for a in args {
                    walk_expr(a, ctx, diags);
                }
                if let Some(b) = block {
                    walk_expr(b, ctx, diags);
                }
            }
            K::Index { receiver, index } => {
                walk_expr(receiver, ctx, diags);
                walk_expr(index, ctx, diags);
            }
            K::Unary { expr, .. } => walk_expr(expr, ctx, diags),
            K::Binary { lhs, rhs, .. } => {
                walk_expr(lhs, ctx, diags);
                walk_expr(rhs, ctx, diags);
            }
            K::Try(inner) | K::Await(inner) => walk_expr(inner, ctx, diags),
            K::Kwarg { value, .. } => walk_expr(value, ctx, diags),
            K::Array(items) => {
                for it in items {
                    walk_expr(it, ctx, diags);
                }
            }
            K::Map(entries) => {
                for (k, v) in entries {
                    walk_expr(k, ctx, diags);
                    walk_expr(v, ctx, diags);
                }
            }
            K::Range { start, end, .. } => {
                walk_expr(start, ctx, diags);
                walk_expr(end, ctx, diags);
            }
            K::Str(parts) => {
                for p in parts {
                    match p {
                        StrPart::Interp(inner) => walk_expr(inner, ctx, diags),
                        StrPart::InterpFmt { expr, .. } => walk_expr(expr, ctx, diags),
                        StrPart::Text(_) => {}
                    }
                }
            }
            // View/widget element bodies can carry their own `case`s
            // (the `case_view` demo for example), but the AST shape
            // there sits inside `Element`'s typed property/child
            // tree which isn't easy to walk generically. We only
            // catch top-level cases for now — codegen still elides
            // unreachables correctly there, the diagnostic is just
            // not surfaced. Future improvement.
            K::Element(_) => {}
            K::Int(_)
            | K::Float(_)
            | K::Bool(_)
            | K::Nil
            | K::Sym(_)
            | K::Ident(_)
            | K::AtIdent(_)
            | K::SelfRef
            | K::Path(_) => {}
        }
    }
    fn walk_fn(f: &FnDecl, ctx: &Ctx, diags: &mut Vec<Diagnostic>) {
        if let Some(body) = &f.body {
            walk_block(body, ctx, diags);
        }
    }

    let ctx = Ctx { module, prog };
    for item in &module.items {
        match item {
            Item::Fn(f) => walk_fn(f, &ctx, diags),
            Item::Class(c) => {
                for m in &c.members {
                    match m {
                        ClassMember::Fn(f) | ClassMember::Slot(f) => walk_fn(f, &ctx, diags),
                        ClassMember::Init(i) => walk_block(&i.body, &ctx, diags),
                        ClassMember::Deinit(d) => walk_block(&d.body, &ctx, diags),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

struct Ctx<'a> {
    module: &'a Module,
    prog: &'a ResolvedProgram,
}

/// Decide whether a `case` over `scrutinee` with the given arms
/// covers every possible value of its type. Drives the HIR
/// non-exhaustive lint (codegen always closes with a `_ => {}`
/// no-op tail). Recognized exhaustive shapes: any `Wild` / `Bind`
/// arm, both `ok` + `err` arms over an error union, both `some` +
/// `nil` arms over an Optional / nullable, both `true` + `false`
/// literal arms over a bool, and full coverage of either an
/// `error E { ... }` decl's variants or any `enum E { ... }`'s
/// variants.
pub fn is_case_exhaustive(
    scrutinee: &Expr,
    arms: &[CaseArm],
    prog: &ResolvedProgram,
    module: &Module,
) -> bool {
    use ExprKind as K;
    let any_wild_or_bind = arms
        .iter()
        .any(|arm| matches!(arm.pattern, Pattern::Wild { .. } | Pattern::Bind { .. }));
    if any_wild_or_bind {
        return true;
    }

    let saw_ok = arms
        .iter()
        .any(|arm| matches!(&arm.pattern, Pattern::Ctor { name, .. } if name.name == "ok"));
    let saw_err = arms
        .iter()
        .any(|arm| matches!(&arm.pattern, Pattern::Ctor { name, .. } if name.name == "err"));
    if saw_ok && saw_err {
        return true;
    }

    let saw_some = arms
        .iter()
        .any(|arm| matches!(&arm.pattern, Pattern::Ctor { name, .. } if name.name == "some"));
    let saw_nil = arms.iter().any(|arm| {
        matches!(
            &arm.pattern,
            Pattern::Literal {
                value: Expr { kind: K::Nil, .. },
                ..
            }
        )
    });
    if saw_some && saw_nil {
        return true;
    }

    let saw_bool = |b: bool| {
        arms.iter().any(|arm| {
            matches!(
                &arm.pattern,
                Pattern::Literal {
                    value: Expr { kind: K::Bool(v), .. },
                    ..
                } if *v == b
            )
        })
    };
    if saw_bool(true) && saw_bool(false) {
        return true;
    }

    let arm_variant_names: HashSet<&str> = arms
        .iter()
        .filter_map(|arm| match &arm.pattern {
            Pattern::Ctor { name, .. } => Some(name.name.as_str()),
            _ => None,
        })
        .collect();
    if arm_variant_names.is_empty() {
        return false;
    }

    if let Some(variants) = error_variant_names_for(scrutinee, prog, module) {
        if !variants.is_empty()
            && variants
                .iter()
                .all(|v| arm_variant_names.contains(v.as_str()))
        {
            return true;
        }
    }

    // Enum-variant exhaustiveness: arm names all belong to a
    // single declared enum AND every variant of that enum is
    // covered. Uses prog-level lookup, not type-checker resolution.
    for kind in prog.items.values() {
        if let ItemKind::Enum { variants, .. } = kind {
            let declared: HashSet<&str> = variants.iter().map(|v| v.name.as_str()).collect();
            if arm_variant_names.iter().all(|n| declared.contains(n))
                && declared.iter().all(|v| arm_variant_names.contains(v))
            {
                return true;
            }
        }
    }
    false
}

pub fn error_variant_names_for(
    scrutinee: &Expr,
    prog: &ResolvedProgram,
    module: &Module,
) -> Option<Vec<String>> {
    let class_name = scrutinee_class_name(scrutinee, module)?;
    if !matches!(
        prog.items.get(&class_name),
        Some(ItemKind::Enum { is_error: true, .. })
    ) {
        return None;
    }
    for item in &module.items {
        if let Item::Enum(e) = item {
            if e.is_error && e.name.name == class_name {
                return Some(e.variants.iter().map(|v| v.name.name.clone()).collect());
            }
        }
    }
    None
}

pub fn scrutinee_class_name(e: &Expr, module: &Module) -> Option<String> {
    use ExprKind as K;
    match &e.kind {
        K::Call { callee, .. } => {
            let K::Ident(fn_name) = &callee.kind else {
                return None;
            };
            for item in &module.items {
                if let Item::Fn(f) = item {
                    if f.name.name == *fn_name {
                        let ret = f.return_ty.as_ref()?;
                        let inner = match &ret.kind {
                            TypeKind::ErrorUnion(t) => t,
                            _ => ret,
                        };
                        if let TypeKind::Named { path, .. } = &inner.kind {
                            return path.last().map(|i| i.name.clone());
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
}

// ---- impl-completeness check ------------------------------------------------

/// For every `impl Trait for Type`, confirm the impl supplies every
/// trait method that lacks a default body. Trait methods with a body
/// (default implementations) are optional — codegen splices them in
/// when the impl omits them. Abstract methods (no body) are required;
/// omitting one is a hard error here so the user gets a Cute-source
/// diagnostic instead of a downstream C++ instantiation failure.
///
/// If the trait name doesn't resolve, this pass stays silent — the
/// visibility / name-resolution passes own that diagnostic.
fn impl_completeness_check(module: &Module, diags: &mut Vec<Diagnostic>) {
    use std::collections::{HashMap, HashSet};
    let mut traits: HashMap<&str, &TraitDecl> = HashMap::new();
    for item in &module.items {
        if let Item::Trait(t) = item {
            traits.insert(t.name.name.as_str(), t);
        }
    }
    for item in &module.items {
        let i = match item {
            Item::Impl(i) => i,
            _ => continue,
        };
        let trait_decl = match traits.get(i.trait_name.name.as_str()) {
            Some(t) => *t,
            None => continue,
        };
        // Sig-aware match: a trait method is "supplied" only if the
        // impl provides a method with the same name, arity, AND
        // rendered param-type list. With overloaded trait methods,
        // an impl supplying just one of the overloads still has the
        // others reported as missing.
        let supplied: HashSet<(String, usize, String)> = i
            .methods
            .iter()
            .map(|m| {
                (
                    m.name.name.clone(),
                    m.params.len(),
                    render_param_signature(&m.params),
                )
            })
            .collect();
        // Pre-count trait-side overloads per name so the diagnostic
        // can include the rendered signature only when disambiguation
        // is needed (preserves the simpler prose for non-overloaded
        // traits, which existing tests assert on).
        let mut name_count: HashMap<&str, usize> = HashMap::new();
        for tm in &trait_decl.methods {
            *name_count.entry(tm.name.name.as_str()).or_insert(0) += 1;
        }
        for tm in &trait_decl.methods {
            if tm.body.is_some() {
                continue; // default-bodied; impl may omit it
            }
            let tm_params_render = render_param_signature(&tm.params);
            let key = (
                tm.name.name.clone(),
                tm.params.len(),
                tm_params_render.clone(),
            );
            if supplied.contains(&key) {
                continue;
            }
            let label = if name_count.get(tm.name.name.as_str()).copied().unwrap_or(0) > 1 {
                format!("{}({})", tm.name.name, tm_params_render)
            } else {
                tm.name.name.clone()
            };
            diags.push(
                Diagnostic::error(
                    i.trait_name.span,
                    format!(
                        "impl `{}` for `{}` is missing required method `{}`",
                        i.trait_name.name,
                        type_expr_render(&i.for_type),
                        label
                    ),
                )
                .with_note(tm.name.span, "method declared here"),
            );
        }
    }
}

/// Reject coherence violations among `Item::Impl` declarations.
///
/// Two failure modes are caught here:
///
/// 1. **Exact duplicate.** `impl Trait for Type` declared twice
///    with the same rendered `for_type`. The splice pre-pass'
///    last-write-wins behavior would silently drop one set of
///    methods; the namespace-overload emit would either duplicate
///    or last-write-wins depending on order. Reject at HIR time
///    with a span pointing at the second declaration and a
///    "first declared here" note.
///
/// 2. **Parametric / concrete overlap.** `impl<T> Trait for List<T>`
///    plus `impl Trait for List<Int>` both nominally apply when
///    `T = Int`. v1 doesn't have specialization rules to pick the
///    most-specific, so reject. Approximated by base-name + arity
///    collision when at least one impl is parametric (i.e.
///    `i.generics` is non-empty). Two concrete impls with the
///    same base / arity but different arg shapes
///    (`List<Int>` vs `List<String>`) are clean — they don't
///    overlap.
fn impl_coherence_check(module: &Module, prog: &ResolvedProgram, diags: &mut Vec<Diagnostic>) {
    use std::collections::HashMap;

    // Pass A: exact `(trait, rendered for_type)` dedup. First occurrence
    // wins; subsequent matches emit a duplicate-impl diagnostic.
    let mut exact_keys: HashMap<(String, String), pixie_syntax::span::Span> = HashMap::new();
    // Pass B grouping: `(trait, base_name, arity)` -> list of impls in
    // declaration order. Used after Pass A so duplicates don't
    // double-report.
    let mut groups: HashMap<(String, String, usize), Vec<&ImplDecl>> = HashMap::new();

    for item in &module.items {
        let i = match item {
            Item::Impl(i) => i,
            _ => continue,
        };
        let trait_name = i.trait_name.name.clone();
        let render = type_expr_render(&i.for_type);
        let key = (trait_name.clone(), render.clone());
        if let Some(prev_span) = exact_keys.get(&key) {
            diags.push(
                Diagnostic::error(
                    i.trait_name.span,
                    format!(
                        "impl `{}` for `{}` already declared",
                        i.trait_name.name, render
                    ),
                )
                .with_note(*prev_span, "first declared here"),
            );
            continue;
        }
        exact_keys.insert(key, i.trait_name.span);
        if let Some(base) = type_expr_base_name(&i.for_type) {
            let arity = type_expr_arity(&i.for_type);
            groups.entry((trait_name, base, arity)).or_default().push(i);
        }
    }

    // Pass B: v1 specialization rule.
    //
    // 2+ parametric impls on the same base: ODR-clash at the
    // namespace overload level — reject regardless of base kind.
    //
    // 1 parametric + N concrete: namespace dispatch lets C++ pick
    // the most specific overload, but the splice can only carry
    // one body per method — so the spec rule only fires for
    // splice-target user classes.
    for ((_trait_name, base, _arity), impls) in &groups {
        if impls.len() < 2 {
            continue;
        }
        let parametrics: Vec<&ImplDecl> = impls
            .iter()
            .copied()
            .filter(|i| !i.generics.is_empty())
            .collect();
        if parametrics.is_empty() {
            continue;
        }
        if parametrics.len() >= 2 {
            let first = parametrics[0];
            for &i in &parametrics[1..] {
                diags.push(
                    Diagnostic::error(
                        i.trait_name.span,
                        format!(
                            "ambiguous parametric impl `{}` for `{}`; an earlier parametric \
                             impl for `{}` already covers every instantiation of this base",
                            i.trait_name.name,
                            type_expr_render(&i.for_type),
                            type_expr_render(&first.for_type)
                        ),
                    )
                    .with_note(first.trait_name.span, "first parametric impl declared here"),
                );
            }
            continue;
        }
        if !is_splice_target_base(base, prog) {
            continue;
        }
        let first = impls[0];
        for &i in &impls[1..] {
            diags.push(
                Diagnostic::error(
                    i.trait_name.span,
                    format!(
                        "impl `{}` for `{}` overlaps with earlier impl for `{}` \
                         (specialization on user classes is not yet supported; \
                         the namespace-dispatch path can specialize, but the \
                         class-method splice can carry only one body)",
                        i.trait_name.name,
                        type_expr_render(&i.for_type),
                        type_expr_render(&first.for_type)
                    ),
                )
                .with_note(first.trait_name.span, "first impl declared here"),
            );
        }
    }
}

/// Reject duplicate fn / method overloads in the same scope — same
/// name, same arity, same param-type list. After the overload
/// landing the type table stores `Vec<FnTy>` per name, so genuine
/// duplicates would both register and the resolver's exact-match
/// tier would be ambiguous. Catching them at HIR time gives a
/// clean source-level diagnostic instead.
///
/// Buckets keyed by `(scope, name, arity)`:
///   * scope `"::free"` — top-level fns
///   * scope `<class name>` — instance fns / slots on a Cute class
///
/// Property getter/setter synth: `prop name : T` synthesizes a
/// zero-arg `name` method and a one-arg `setName` method on the
/// class. A user-written `fn name() -> T` would clash; same for
/// `fn setName(x: T)`. Both collisions are caught here as
/// param-type duplicates.
///
/// **Impl methods are intentionally NOT included.** `impl_coherence_check`
/// is the canonical home for impl-level concerns:
///   - Duplicate `impl Trait for X { ... }` blocks → Pass A
///   - Parametric vs concrete impls (`impl<T> Foo for List<T>` +
///     `impl Foo for List<Int>`) on a non-splice base → allowed by
///     Pass B (specialization rule); both register `fn item_count`
///     in different impl blocks, but C++ overload resolution at the
///     namespace dispatch picks the most specific.
/// Including impl methods here would double-flag the parametric+
/// concrete case as duplicate. Class-method vs impl-method overlap
/// (where an impl method shadows a same-signed class method) is left
/// to the splice path's "class method wins" ordering rather than a
/// HIR diagnostic.
///
/// Param-type comparison uses `type_expr_render` (Cute surface
/// printer); this is structurally equivalent to comparing lowered
/// Type values for the cases we care about (named types, generics,
/// nullable, error-union, fn shapes) and avoids pulling in the type
/// checker from HIR.
/// Render an AST param list as a comma-joined Cute-surface string.
/// Shared by `impl_completeness_check` (sig-aware match key) and
/// `fn_overload_coherence_check` (duplicate detection key).
fn render_param_signature(params: &[pixie_syntax::ast::Param]) -> String {
    params
        .iter()
        .map(|p| type_expr_render(&p.ty))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Sentinel scope used in `fn_overload_coherence_check` for free fns
/// (top-level, not attached to a class). The leading `::` keeps it
/// distinct from any user class name.
const FREE_SCOPE: &str = "::free";

/// What kind of declaration occupies a coherence-check bucket entry.
/// Drives the diagnostic prose ("duplicate fn" vs "duplicate method").
#[derive(Debug, Clone, Copy)]
enum DeclKind {
    Fn,
    Method,
}

impl DeclKind {
    fn as_label(self) -> &'static str {
        match self {
            DeclKind::Fn => "fn",
            DeclKind::Method => "method",
        }
    }
}

fn fn_overload_coherence_check(module: &Module, diags: &mut Vec<Diagnostic>) {
    use std::collections::HashMap;
    // Each entry: (decl span, rendered-param-list, is_generic, kind).
    type Decl = (pixie_syntax::span::Span, String, bool, DeclKind);
    let mut buckets: HashMap<(String, String, usize), Vec<Decl>> = HashMap::new();

    for item in &module.items {
        match item {
            Item::Fn(f) => {
                buckets
                    .entry((FREE_SCOPE.into(), f.name.name.clone(), f.params.len()))
                    .or_default()
                    .push((
                        f.name.span,
                        render_param_signature(&f.params),
                        !f.generics.is_empty(),
                        DeclKind::Fn,
                    ));
            }
            Item::Class(c) => {
                let cname = c.name.name.clone();
                for mem in &c.members {
                    match mem {
                        ClassMember::Fn(f) | ClassMember::Slot(f) => {
                            buckets
                                .entry((cname.clone(), f.name.name.clone(), f.params.len()))
                                .or_default()
                                .push((
                                    f.name.span,
                                    render_param_signature(&f.params),
                                    !f.generics.is_empty(),
                                    DeclKind::Method,
                                ));
                        }
                        // Property declarations are deliberately NOT
                        // bucketed. The synth getter `name() -> T` and
                        // synth setter `setName(T)` would collide with
                        // patterns that legitimately co-declare the
                        // prop and the same-signed accessor explicitly
                        // — both binding sources (`stdlib/qt/qtmultimedia.qpi:33`
                        // QAudioOutput, `stdlib/qt/qtcharts.qpi:121` QChart)
                        // AND user code that opts into a custom getter /
                        // setter body. `build_table`'s pre-scan dedupe
                        // (cute-types/src/table.rs) handles these by
                        // suppressing the prop synth whenever a user
                        // fn with the same (name, params) exists, so
                        // the user's explicit fn always wins and
                        // there's no diagnostic to emit here.
                        ClassMember::Property(_) => {}
                        _ => {}
                    }
                }
            }
            // Impl-block methods deliberately skipped — see fn-doc.
            _ => {}
        }
    }

    let scope_label = |scope: &str| -> String {
        if scope == FREE_SCOPE {
            "in this module".to_string()
        } else {
            format!("in `{scope}`")
        }
    };

    for ((scope, name, _arity), decls) in &buckets {
        if decls.len() < 2 {
            continue;
        }
        // Pair-compare by rendered param list. First occurrence wins;
        // subsequent matches emit a diagnostic.
        for (i, (span_i, params_i, _, kind_i)) in decls.iter().enumerate() {
            for (span_j, params_j, _, _) in decls.iter().skip(i + 1) {
                if params_i == params_j {
                    diags.push(
                        Diagnostic::error(
                            *span_j,
                            format!(
                                "duplicate {kind} `{name}` {scope} (same parameter types as \
                                 earlier declaration)",
                                kind = kind_i.as_label(),
                                scope = scope_label(scope),
                            ),
                        )
                        .with_note(*span_i, "first declared here"),
                    );
                }
            }
        }
        // Two or more parametric (generic) overloads with the same
        // arity in the same scope are ambiguous at every call site
        // (the resolver can't distinguish `<T>` from `<U>` when both
        // satisfy any arg). Mirrors the `impl_coherence_check` Pass B
        // parametric-parametric rejection.
        let parametrics: Vec<&Decl> = decls.iter().filter(|d| d.2).collect();
        if parametrics.len() >= 2 {
            let first = parametrics[0];
            for d in &parametrics[1..] {
                diags.push(
                    Diagnostic::error(
                        d.0,
                        format!(
                            "ambiguous parametric {kind} `{name}` {scope}; an earlier \
                             parametric declaration with the same arity already covers any \
                             argument type",
                            kind = d.3.as_label(),
                            scope = scope_label(scope),
                        ),
                    )
                    .with_note(first.0, "first parametric declaration here"),
                );
            }
        }
    }
}

/// True when `base` names a class that the codegen splice
/// (`inline_impls_into_classes` in cute-codegen) writes into. These
/// classes carry one body per method, so parametric vs concrete impls
/// can't both land on them — keep the coherence rejection.
///
/// Non-splice targets (extern value classes, builtin generics like
/// `List<T>`, unknown bases) only emit through the namespace-overload
/// path. C++ overload resolution picks the most specific overload at
/// the call site, so multiple impls on the same base are safe.
///
/// Note: prelude/binding classes (Qt stdlib `class QFoo { ... }`
/// without `extern value`) currently fall through to the splice-target
/// branch since the splice walks every `Item::Class` indiscriminately.
/// Because their C++ definition is supplied externally and not
/// regenerated by codegen, the splice is a no-op for them in practice
/// — but keeping the conservative classification matches the splice's
/// rejection of conflicting impls on those names.
fn is_splice_target_base(base: &str, prog: &ResolvedProgram) -> bool {
    let Some(item) = prog.items.get(base) else {
        return false;
    };
    let ItemKind::Class {
        is_extern_value, ..
    } = item
    else {
        return false;
    };
    !*is_extern_value
}

/// Number of type arguments in a `TypeExpr`. Strips `Nullable` and
/// `ErrorUnion` wrappers to match the rule used by `type_expr_base_name`
/// — `List<Int>?` has the same base as `List<Int>`, both with arity 1.
/// Anonymous shapes (Fn / Self) have no arity (returns 0).
fn type_expr_arity(t: &TypeExpr) -> usize {
    match &t.kind {
        TypeKind::Named { args, .. } => args.len(),
        TypeKind::Nullable(inner) | TypeKind::ErrorUnion(inner) => type_expr_arity(inner),
        TypeKind::Fn { .. } | TypeKind::SelfType => 0,
    }
}

// ---- visibility check -------------------------------------------------------

fn visibility_check(
    module: &Module,
    prog: &ResolvedProgram,
    project: &ProjectInfo,
    diags: &mut Vec<Diagnostic>,
) {
    let ctx = VisCtx { prog, project };
    for item in &module.items {
        // Only walk user items; binding-file items live in the prelude
        // and reference each other freely without `use`.
        if !matches!(ctx.home_of_span(item_span_of(item)), ItemHome::User(_)) {
            continue;
        }
        match item {
            Item::Class(c) => {
                if let Some(s) = &c.super_class {
                    ctx.check_type(s, diags);
                }
                for m in &c.members {
                    match m {
                        ClassMember::Property(p) => ctx.check_type(&p.ty, diags),
                        ClassMember::Field(f) => ctx.check_type(&f.ty, diags),
                        ClassMember::Signal(s) => {
                            for p in &s.params {
                                ctx.check_type(&p.ty, diags);
                            }
                        }
                        ClassMember::Fn(f) | ClassMember::Slot(f) => ctx.check_fn(f, diags),
                        ClassMember::Init(i) => {
                            for p in &i.params {
                                ctx.check_type(&p.ty, diags);
                            }
                            ctx.check_block(&i.body, diags);
                        }
                        ClassMember::Deinit(d) => ctx.check_block(&d.body, diags),
                    }
                }
            }
            Item::Struct(s) => {
                for f in &s.fields {
                    ctx.check_type(&f.ty, diags);
                }
            }
            Item::Fn(f) => ctx.check_fn(f, diags),
            Item::View(v) => {
                for sf in &v.state_fields {
                    ctx.check_expr(&sf.init_expr, diags);
                }
                ctx.check_element(&v.root, diags);
            }
            Item::Widget(w) => {
                // Property-kind state fields are desugared into a
                // synthesized `__<Widget>State` QObject class by
                // `cute_codegen::desugar_state::desugar_widget_state`
                // before HIR runs. Anything still in `Property` form
                // by this point would indicate a desugaring pipeline
                // bug, not user error.
                for sf in &w.state_fields {
                    ctx.check_expr(&sf.init_expr, diags);
                }
                ctx.check_element(&w.root, diags);
            }
            Item::Style(s) => match &s.body {
                StyleBody::Lit(entries) => {
                    for e in entries {
                        ctx.check_expr(&e.value, diags);
                    }
                }
                StyleBody::Alias(rhs) => ctx.check_expr(rhs, diags),
            },
            Item::Use(_) => {}
            Item::UseQml(_) => {}
            Item::Trait(t) => {
                // Walk method signature types (params + return) so a
                // typo in a trait method's return type still surfaces.
                for m in &t.methods {
                    for p in &m.params {
                        ctx.check_type(&p.ty, diags);
                    }
                    if let Some(rt) = &m.return_ty {
                        ctx.check_type(rt, diags);
                    }
                }
            }
            Item::Impl(i) => {
                // Verify the trait + the target type are visible from
                // here, then walk method bodies the same way Fn does.
                for m in &i.methods {
                    ctx.check_fn(m, diags);
                }
            }
            Item::Let(l) => {
                ctx.check_type(&l.ty, diags);
                ctx.check_expr(&l.value, diags);
            }
            Item::Enum(e) => {
                // Variant value expressions (when present) need to be
                // walked — typos / unknown identifiers in
                // `Red = SOMECONST | Other` should surface as
                // diagnostics the same way they would in any other
                // expression position. Also walk any payload field
                // types (used by error-style enums and payload
                // variants) so undeclared field types surface.
                for v in &e.variants {
                    if let Some(val) = &v.value {
                        ctx.check_expr(val, diags);
                    }
                    for f in &v.fields {
                        ctx.check_type(&f.ty, diags);
                    }
                }
            }
            Item::Flags(_) => {
                // Body is just `flags X of E` — the underlying enum
                // existence check happens in cute-types::table.
            }
            Item::Store(_) => unreachable!(
                "Item::Store should be lowered before the visibility pass; \
                 see cute_codegen::desugar_store",
            ),
            Item::Suite(_) => unreachable!(
                "Item::Suite should be flattened before the visibility pass; \
                 see cute_codegen::desugar_suite",
            ),
        }
    }
}

fn item_span_of(item: &Item) -> Span {
    match item {
        Item::Class(c) => c.span,
        Item::Struct(s) => s.span,
        Item::Fn(f) => f.span,
        Item::View(v) => v.span,
        Item::Widget(w) => w.span,
        Item::Style(s) => s.span,
        Item::Use(u) => u.span,
        Item::UseQml(u) => u.span,
        Item::Trait(t) => t.span,
        Item::Impl(i) => i.span,
        Item::Let(l) => l.span,
        Item::Enum(e) => e.span,
        Item::Flags(f) => f.span,
        // Span helpers stay safe for partial pipelines (e.g. tools
        // that walk the AST before desugar runs).
        Item::Store(s) => s.span,
        Item::Suite(s) => s.span,
    }
}

struct VisCtx<'a> {
    prog: &'a ResolvedProgram,
    project: &'a ProjectInfo,
}

impl<'a> VisCtx<'a> {
    fn home_of_span(&self, span: Span) -> ItemHome {
        match self.project.module_for_file.get(&span.file) {
            Some(name) => ItemHome::User(name.clone()),
            None => ItemHome::Prelude,
        }
    }

    fn current_module_of(&self, span: Span) -> Option<&str> {
        self.project
            .module_for_file
            .get(&span.file)
            .map(String::as_str)
    }

    /// Resolve `qualifier` (if any) to its canonical module name,
    /// honoring `as`-aliases declared by `cur_module`. Returns `None`
    /// when the qualifier doesn't match anything visible.
    fn canonical_module(&self, qualifier: &str, cur_module: Option<&str>) -> Option<String> {
        if let Some(cur) = cur_module {
            if let Some(aliases) = self.project.module_aliases.get(cur) {
                if let Some(real) = aliases.get(qualifier) {
                    return Some(real.clone());
                }
            }
        }
        // No alias: the qualifier IS the real module name.
        Some(qualifier.to_string())
    }

    /// Resolve a bare local name through the selective-import map
    /// for `cur_module`. Returns `Some((source_module, original))`
    /// when `local` was rebound via `use foo.{X as local}`. The item
    /// table is keyed by the original simple name (current
    /// flat-table phase), so callers use the second element of the
    /// returned pair to look up the actual `ItemKind`.
    fn selective_target(&self, local: &str, cur_module: Option<&str>) -> Option<(String, String)> {
        let cur = cur_module?;
        self.project.selective_imports.get(cur)?.get(local).cloned()
    }

    /// Look up a re-export entry: for `(through_module, name)`,
    /// returns `Some((source_module, source_name))` when `through`
    /// exposes `name` via `pub use source.X`. Used so a downstream
    /// module that imports `through` can reach `name` even though it
    /// physically lives in `source`.
    fn re_export_target(&self, through_module: &str, name: &str) -> Option<(String, String)> {
        self.project
            .re_exports
            .get(through_module)?
            .get(name)
            .cloned()
    }

    /// True when `cur_module` can reach `decl_module` through any
    /// re-export chain. Used by the bare-reference path: even when
    /// the item lives in `decl_module` and `cur_module` doesn't
    /// directly `use` it, an imported module that re-exports the
    /// item makes the reference legal.
    fn re_export_path_visible(&self, cur_module: &str, decl_module: &str, item_name: &str) -> bool {
        let Some(imports) = self.project.imports_for_module.get(cur_module) else {
            return false;
        };
        for through in imports {
            if let Some((src, _)) = self.re_export_target(through, item_name) {
                if src == decl_module {
                    return true;
                }
            }
        }
        false
    }

    /// The core check: `name` is referenced from `ref_span`, optionally
    /// qualified by `qualifier`. Push a diagnostic if it's a
    /// cross-module reference to a non-pub item or to an unimported
    /// module. Bare references to unresolved names are silently
    /// skipped - the type checker handles unknowns separately.
    fn check_name(
        &self,
        name: &str,
        qualifier: Option<&str>,
        ref_span: Span,
        diags: &mut Vec<Diagnostic>,
    ) {
        let cur_module = self.current_module_of(ref_span);

        // -- selective-import shortcut --------------------------------
        // For unqualified refs, a selective-import binding rewires
        // `local` to `(source_module, original)`. Resolve through it
        // first so renamed imports `use foo.{X as A}` work: the user
        // wrote `A`, but the actual item lives at name `X` in module
        // `foo`. The rewrite skips the "is the source module
        // imported?" check below since the selective import IS the
        // import - it just doesn't enter `imports_for_module`.
        let mut from_selective = false;
        let (lookup_name, effective_qualifier) = if qualifier.is_none() {
            if let Some((src_mod, original)) = self.selective_target(name, cur_module) {
                from_selective = true;
                (original, Some(src_mod))
            } else {
                (name.to_string(), None)
            }
        } else {
            (name.to_string(), qualifier.map(str::to_string))
        };

        // Module-precise lookup when a qualifier is in play (either
        // user-typed or selective-import-rewritten). Same-name
        // collisions across modules are handled here: the flat
        // `items` table is lossy for collisions (last-write-wins),
        // so qualified refs MUST consult `items_by_module` to find
        // the right one.
        let item_owned: Option<ItemKind> = if let Some(q) = effective_qualifier.as_deref() {
            // Resolve aliases first so `use foo as bar` followed by
            // `bar.X` looks up under foo.
            let canonical = self
                .canonical_module(q, cur_module)
                .unwrap_or_else(|| q.to_string());
            self.prog
                .items_by_module
                .get(&(canonical, lookup_name.clone()))
                .cloned()
                .or_else(|| self.prog.items.get(&lookup_name).cloned())
        } else {
            // Unqualified: prefer the current-module entry, then fall
            // back to the flat table (which surfaces prelude /
            // imported items).
            cur_module
                .and_then(|m| {
                    self.prog
                        .items_by_module
                        .get(&(m.to_string(), lookup_name.clone()))
                        .cloned()
                })
                .or_else(|| self.prog.items.get(&lookup_name).cloned())
        };
        let item = match item_owned {
            Some(it) => it,
            None => return, // unknown name; not our problem
        };
        let item_home = item.home().clone();

        // The qualified case is checked first because qualified refs
        // demand stricter validation: the named module must be in the
        // import set (or be the current module). The qualifier
        // visible to the user might be an `as`-alias, so canonicalize
        // before comparing against the item's home.
        if let Some(q_user) = effective_qualifier.as_deref() {
            if self.project.prelude_items.contains(&lookup_name) {
                return; // prelude items are always reachable
            }
            let q_canonical = match self.canonical_module(q_user, cur_module) {
                Some(c) => c,
                None => {
                    diags.push(Diagnostic::error(
                        ref_span,
                        format!("module `{q_user}` is not in scope (no `use` statement bound it)"),
                    ));
                    return;
                }
            };
            // Verify the (possibly aliased) prefix matches the item's
            // home, OR that the prefix re-exports the item via
            // `pub use src.X`. Either path makes `M.X` legal.
            let target_module: String = match &item_home {
                ItemHome::User(m) => m.clone(),
                ItemHome::Prelude => return,
            };
            let via_reexport_through_qualifier = self
                .re_export_target(&q_canonical, &lookup_name)
                .map(|(src, _)| src == target_module)
                .unwrap_or(false);
            if q_canonical != target_module && !via_reexport_through_qualifier {
                diags.push(Diagnostic::error(
                    ref_span,
                    format!(
                        "module qualifier `{q_user}` does not match where `{lookup_name}` is declared (module `{target_module}`)"
                    ),
                ));
                return;
            }
            if Some(target_module.as_str()) == cur_module {
                return; // own module, any visibility
            }
            // Selective imports satisfy the import-set check on their
            // own (the user wrote `use foo.{X}`, that IS the import).
            // Whole-module imports require an entry in
            // `imports_for_module`. Re-exports through any imported
            // module also count: `M.X` is reachable when M did
            // `pub use src.X`, even if cur_module doesn't `use src`.
            if !from_selective {
                if let Some(cur) = cur_module {
                    let imports = self.project.imports_for_module.get(cur);
                    let is_imported = imports.map_or(false, |s| s.contains(&target_module));
                    let via_reexport =
                        self.re_export_path_visible(cur, &target_module, &lookup_name);
                    if !is_imported && !via_reexport {
                        diags.push(Diagnostic::error(
                            ref_span,
                            format!(
                                "module `{target_module}` is not imported in this file - add `use {target_module}` to access `{target_module}.{lookup_name}`"
                            ),
                        ));
                        return;
                    }
                }
            }
            if !item.is_pub() {
                diags.push(Diagnostic::error(
                    ref_span,
                    format!(
                        "`{lookup_name}` is not exported from module `{target_module}` - declare it as `pub` to make it visible"
                    ),
                ));
            }
            return;
        }

        // Bare reference: same-module, prelude, selective-import (handled
        // above as a rewrite), or the pub items of an imported module.
        if self.project.prelude_items.contains(&lookup_name) {
            return;
        }
        match (item_home, cur_module) {
            (ItemHome::Prelude, _) => {}
            (ItemHome::User(decl_mod), Some(cur)) if decl_mod == cur => {}
            (ItemHome::User(decl_mod), Some(cur)) => {
                let imports = self.project.imports_for_module.get(cur);
                let is_imported = imports.map_or(false, |s| s.contains(&decl_mod));
                let via_reexport = self.re_export_path_visible(cur, &decl_mod, &lookup_name);
                if !is_imported && !via_reexport {
                    diags.push(Diagnostic::error(
                        ref_span,
                        format!(
                            "`{lookup_name}` is declared in module `{decl_mod}` but this file does not `use` it"
                        ),
                    ));
                } else if !item.is_pub() && !via_reexport {
                    diags.push(Diagnostic::error(
                        ref_span,
                        format!(
                            "`{lookup_name}` is not exported from module `{decl_mod}` - declare it as `pub` to make it visible"
                        ),
                    ));
                }
            }
            (ItemHome::User(_), None) => {} // ref from prelude/binding; skip
        }
    }

    fn check_type(&self, t: &TypeExpr, diags: &mut Vec<Diagnostic>) {
        match &t.kind {
            TypeKind::Named { path, args } => {
                if let Some(last) = path.last() {
                    let qualifier = if path.len() >= 2 {
                        Some(path[path.len() - 2].name.as_str())
                    } else {
                        None
                    };
                    // Skip trivial primitives - they're always available.
                    if !matches!(
                        last.name.as_str(),
                        "String" | "Bool" | "Int" | "Float" | "Void"
                    ) {
                        self.check_name(&last.name, qualifier, t.span, diags);
                    }
                }
                for a in args {
                    self.check_type(a, diags);
                }
            }
            TypeKind::Nullable(inner) | TypeKind::ErrorUnion(inner) => {
                self.check_type(inner, diags)
            }
            TypeKind::Fn { params, ret } => {
                for p in params {
                    self.check_type(p, diags);
                }
                self.check_type(ret, diags);
            }
            TypeKind::SelfType => {}
        }
    }

    fn check_fn(&self, f: &FnDecl, diags: &mut Vec<Diagnostic>) {
        for p in &f.params {
            self.check_type(&p.ty, diags);
        }
        if let Some(rt) = &f.return_ty {
            self.check_type(rt, diags);
        }
        if let Some(body) = &f.body {
            self.check_block(body, diags);
        }
    }

    fn check_block(&self, b: &Block, diags: &mut Vec<Diagnostic>) {
        for s in &b.stmts {
            self.check_stmt(s, diags);
        }
        if let Some(t) = &b.trailing {
            self.check_expr(t, diags);
        }
    }

    fn check_stmt(&self, s: &Stmt, diags: &mut Vec<Diagnostic>) {
        match s {
            Stmt::Let { ty, value, .. } | Stmt::Var { ty, value, .. } => {
                if let Some(t) = ty {
                    self.check_type(t, diags);
                }
                self.check_expr(value, diags);
            }
            Stmt::Expr(e) => self.check_expr(e, diags),
            Stmt::Return { value: Some(v), .. } => self.check_expr(v, diags),
            Stmt::Return { value: None, .. } => {}
            Stmt::Emit { args, .. } => {
                for a in args {
                    self.check_expr(a, diags);
                }
            }
            Stmt::Assign { target, value, .. } => {
                self.check_expr(target, diags);
                self.check_expr(value, diags);
            }
            Stmt::For { iter, body, .. } => {
                self.check_expr(iter, diags);
                self.check_block(body, diags);
            }
            Stmt::While { cond, body, .. } => {
                self.check_expr(cond, diags);
                self.check_block(body, diags);
            }
            Stmt::Break { .. } | Stmt::Continue { .. } => {}
            Stmt::Batch { body, .. } => self.check_block(body, diags),
        }
    }

    fn check_expr(&self, e: &Expr, diags: &mut Vec<Diagnostic>) {
        use ExprKind as K;
        match &e.kind {
            K::Ident(name) => {
                self.check_name(name, None, e.span, diags);
            }
            // `M.X` qualified access - if `M` looks like a module name
            // we recognize, treat this as a qualified item ref. The
            // ordinary value-receiver case (instance.field) just walks
            // the receiver and stops.
            K::Member { receiver, name } => {
                if let K::Ident(rcv_name) = &receiver.kind {
                    // Could be a module path (`model.Counter` /
                    // `bar.Counter` where `bar` is an alias) or an
                    // ordinary value-member access (`obj.field`).
                    // Disambiguator: is `rcv_name` reachable as a
                    // module name from the current file?
                    let cur = self.current_module_of(receiver.span);
                    let alias_target = cur
                        .and_then(|c| self.project.module_aliases.get(c))
                        .and_then(|a| a.get(rcv_name));
                    let qualifier_is_module = alias_target.is_some()
                        || self
                            .project
                            .imports_for_module
                            .values()
                            .any(|s| s.contains(rcv_name))
                        || cur == Some(rcv_name)
                        || self.project.module_for_file.values().any(|m| m == rcv_name);
                    let item = self.prog.items.get(&name.name);
                    if qualifier_is_module && item.is_some() {
                        self.check_name(&name.name, Some(rcv_name), e.span, diags);
                        return;
                    }
                }
                self.check_expr(receiver, diags);
            }
            K::Call {
                callee,
                args,
                block,
                ..
            } => {
                self.check_expr(callee, diags);
                for a in args {
                    self.check_expr(a, diags);
                }
                if let Some(b) = block {
                    self.check_expr(b, diags);
                }
            }
            K::MethodCall {
                receiver,
                args,
                block,
                ..
            }
            | K::SafeMethodCall {
                receiver,
                args,
                block,
                ..
            } => {
                self.check_expr(receiver, diags);
                for a in args {
                    self.check_expr(a, diags);
                }
                if let Some(b) = block {
                    self.check_expr(b, diags);
                }
            }
            // `?.` skips the module-qualified-name shortcut: a safe
            // member access only makes sense on a runtime value, not a
            // module path.
            K::SafeMember { receiver, .. } => self.check_expr(receiver, diags),
            K::Index { receiver, index } => {
                self.check_expr(receiver, diags);
                self.check_expr(index, diags);
            }
            K::Block(b) => self.check_block(b, diags),
            K::Lambda { body, .. } => self.check_block(body, diags),
            K::Unary { expr, .. } => self.check_expr(expr, diags),
            K::Binary { lhs, rhs, .. } => {
                self.check_expr(lhs, diags);
                self.check_expr(rhs, diags);
            }
            K::Try(inner) | K::Await(inner) => self.check_expr(inner, diags),
            K::If {
                cond,
                then_b,
                else_b,
                ..
            } => {
                self.check_expr(cond, diags);
                self.check_block(then_b, diags);
                if let Some(eb) = else_b {
                    self.check_block(eb, diags);
                }
            }
            K::Case { scrutinee, arms } => {
                self.check_expr(scrutinee, diags);
                for arm in arms {
                    self.check_block(&arm.body, diags);
                }
            }
            K::Kwarg { value, .. } => self.check_expr(value, diags),
            K::Element(el) => self.check_element(el, diags),
            K::Array(items) => {
                for it in items {
                    self.check_expr(it, diags);
                }
            }
            K::Range { start, end, .. } => {
                self.check_expr(start, diags);
                self.check_expr(end, diags);
            }
            K::Map(entries) => {
                for (k, v) in entries {
                    self.check_expr(k, diags);
                    self.check_expr(v, diags);
                }
            }
            K::Str(parts) => {
                for p in parts {
                    match p {
                        StrPart::Interp(inner) => self.check_expr(inner, diags),
                        StrPart::InterpFmt { expr, .. } => self.check_expr(expr, diags),
                        StrPart::Text(_) => {}
                    }
                }
            }
            K::Path(segments) => {
                if let Some(last) = segments.last() {
                    let qualifier = if segments.len() >= 2 {
                        Some(segments[segments.len() - 2].name.as_str())
                    } else {
                        None
                    };
                    self.check_name(&last.name, qualifier, e.span, diags);
                }
            }
            K::Int(_)
            | K::Float(_)
            | K::Bool(_)
            | K::Nil
            | K::Sym(_)
            | K::AtIdent(_)
            | K::SelfRef => {}
        }
    }

    fn check_element(&self, el: &Element, diags: &mut Vec<Diagnostic>) {
        let qualifier = el.module_path.last().map(|i| i.name.as_str());
        self.check_name(&el.name.name, qualifier, el.span, diags);
        for m in &el.members {
            match m {
                ElementMember::Property { value, .. } => self.check_expr(value, diags),
                ElementMember::Child(c) => self.check_element(c, diags),
                ElementMember::Stmt(s) => self.check_stmt(s, diags),
            }
        }
    }
}

/// Determine where an item was declared. Items in user `.cute` files
/// inherit the file's module name from `ProjectInfo`; everything else
/// (binding files, anything not registered) falls into the prelude.
fn item_home(item: &Item, project: &ProjectInfo) -> ItemHome {
    let span = match item {
        Item::Class(c) => c.span,
        Item::Struct(s) => s.span,
        Item::Fn(f) => f.span,
        Item::View(v) => v.span,
        Item::Widget(w) => w.span,
        Item::Style(s) => s.span,
        Item::Use(u) => u.span,
        Item::UseQml(u) => u.span,
        Item::Trait(t) => t.span,
        Item::Impl(i) => i.span,
        Item::Let(l) => l.span,
        Item::Enum(e) => e.span,
        Item::Flags(f) => f.span,
        Item::Store(s) => s.span,
        Item::Suite(s) => s.span,
    };
    match project.module_for_file.get(&span.file) {
        Some(name) => ItemHome::User(name.clone()),
        None => ItemHome::Prelude,
    }
}

fn class_super_name(c: &ClassDecl) -> Option<String> {
    let t = c.super_class.as_ref()?;
    if let TypeKind::Named { path, .. } = &t.kind {
        path.last().map(|i| i.name.clone())
    } else {
        None
    }
}

/// Is `name` a World-backed class in the collected registry? There is no
/// implicit super chain to chase and no name heuristic — a class is a
/// world class iff its own declaration says so (no inheritance, D9).
fn is_world_class(name: Option<&str>, items: &HashMap<String, ItemKind>) -> bool {
    let Some(name) = name else { return false };
    matches!(
        items.get(name),
        Some(ItemKind::Class {
            is_world_class: true,
            ..
        })
    )
}

/// Per-fn scope walk. Keyed by the body's span; consumers (loop-control
/// validator, codegen lowerer) treat fn / slot / init / deinit
/// uniformly through `prog.fn_scopes`. The optional `FnDecl` shape is
/// shared with the `init` / `deinit` callers via `collect_body_scope`.
fn collect_fn_scope(f: &FnDecl, prog: &mut ResolvedProgram) {
    if let Some(body) = &f.body {
        collect_body_scope(&f.params, body, prog);
    }
}

fn collect_body_scope(params: &[Param], body: &Block, prog: &mut ResolvedProgram) {
    let mut scope = FnScope::default();
    let mut declared: HashSet<String> = params.iter().map(|p| p.name.name.clone()).collect();
    walk_block(body, &mut declared, &mut scope);
    prog.fn_scopes.insert(body.span, scope);
}

fn walk_block(b: &Block, declared: &mut HashSet<String>, scope: &mut FnScope) {
    for stmt in &b.stmts {
        walk_stmt(stmt, declared, scope);
    }
    if let Some(t) = &b.trailing {
        walk_expr(t, declared, scope);
    }
}

fn walk_stmt(s: &Stmt, declared: &mut HashSet<String>, scope: &mut FnScope) {
    match s {
        Stmt::Let { name, value, .. } | Stmt::Var { name, value, .. } => {
            walk_expr(value, declared, scope);
            declared.insert(name.name.clone());
        }
        Stmt::Assign {
            target,
            op,
            value,
            span,
        } => {
            walk_expr(value, declared, scope);
            if matches!(op, AssignOp::Eq) {
                if let ExprKind::Ident(name) = &target.kind {
                    let is_new = !declared.contains(name);
                    if is_new {
                        declared.insert(name.clone());
                    }
                    scope.assign_is_decl.insert(*span, is_new);
                }
            }
            walk_expr(target, declared, scope);
        }
        Stmt::Expr(e) => walk_expr(e, declared, scope),
        Stmt::Return { value: Some(v), .. } => walk_expr(v, declared, scope),
        Stmt::Return { value: None, .. } => {}
        Stmt::Emit { args, .. } => {
            for a in args {
                walk_expr(a, declared, scope);
            }
        }
        Stmt::For {
            binding,
            iter,
            body,
            ..
        } => {
            walk_expr(iter, declared, scope);
            // The for-binding shadows any outer name only inside the
            // body, so use a fresh declared-set scoped to the loop.
            let mut sub = declared.clone();
            sub.insert(binding.name.clone());
            walk_block(body, &mut sub, scope);
        }
        Stmt::While { cond, body, .. } => {
            walk_expr(cond, declared, scope);
            let mut sub = declared.clone();
            walk_block(body, &mut sub, scope);
        }
        Stmt::Break { .. } | Stmt::Continue { .. } => {}
        // `batch { ... }` opens a fresh declaration scope (the C++
        // braces it lowers to enforce the same), so locals declared
        // inside don't leak into the surrounding fn — match the
        // for/while convention.
        Stmt::Batch { body, .. } => {
            let mut sub = declared.clone();
            walk_block(body, &mut sub, scope);
        }
    }
}

fn walk_expr(e: &Expr, declared: &mut HashSet<String>, scope: &mut FnScope) {
    use pixie_syntax::ast::ExprKind as K;
    match &e.kind {
        K::Call {
            callee,
            args,
            block,
            ..
        } => {
            walk_expr(callee, declared, scope);
            for a in args {
                walk_expr(a, declared, scope);
            }
            if let Some(b) = block {
                walk_expr(b, declared, scope);
            }
        }
        K::MethodCall {
            receiver,
            args,
            block,
            ..
        }
        | K::SafeMethodCall {
            receiver,
            args,
            block,
            ..
        } => {
            walk_expr(receiver, declared, scope);
            for a in args {
                walk_expr(a, declared, scope);
            }
            if let Some(b) = block {
                walk_expr(b, declared, scope);
            }
        }
        K::Member { receiver, .. } | K::SafeMember { receiver, .. } => {
            walk_expr(receiver, declared, scope)
        }
        K::Index { receiver, index } => {
            walk_expr(receiver, declared, scope);
            walk_expr(index, declared, scope);
        }
        K::Block(b) => walk_block(b, declared, scope),
        K::Lambda { params, body } => {
            // Lambda params shadow into a sub-scope.
            let mut sub = declared.clone();
            for p in params {
                sub.insert(p.name.name.clone());
            }
            walk_block(body, &mut sub, scope);
        }
        K::Unary { expr, .. } => walk_expr(expr, declared, scope),
        K::Binary { lhs, rhs, .. } => {
            walk_expr(lhs, declared, scope);
            walk_expr(rhs, declared, scope);
        }
        K::Try(inner) => walk_expr(inner, declared, scope),
        K::If {
            cond,
            then_b,
            else_b,
            ..
        } => {
            walk_expr(cond, declared, scope);
            walk_block(then_b, declared, scope);
            if let Some(eb) = else_b {
                walk_block(eb, declared, scope);
            }
        }
        K::Case { scrutinee, arms } => {
            walk_expr(scrutinee, declared, scope);
            for arm in arms {
                // Arm body may pull in `Bind` patterns at codegen time; for
                // now we conservatively treat the arm body as a fresh scope
                // so an `name = expr` inside an arm cannot accidentally
                // shadow an outer binding.
                let mut sub = declared.clone();
                walk_pattern(&arm.pattern, &mut sub);
                walk_block(&arm.body, &mut sub, scope);
            }
        }
        K::Await(e) => walk_expr(e, declared, scope),
        K::Str(parts) => {
            for p in parts {
                match p {
                    StrPart::Interp(inner) => walk_expr(inner, declared, scope),
                    StrPart::InterpFmt { expr, .. } => walk_expr(expr, declared, scope),
                    StrPart::Text(_) => {}
                }
            }
        }
        K::Kwarg { value, .. } => walk_expr(value, declared, scope),
        K::Array(items) => {
            for item in items {
                walk_expr(item, declared, scope);
            }
        }
        K::Map(entries) => {
            for (k, v) in entries {
                walk_expr(k, declared, scope);
                walk_expr(v, declared, scope);
            }
        }
        K::Range { start, end, .. } => {
            walk_expr(start, declared, scope);
            walk_expr(end, declared, scope);
        }
        K::Element(_) => {
            // Element bodies hang off `Item::View` / `Item::Widget`
            // and are walked from there at module-resolution time -
            // not from inside an expression's Element wrapper.
        }
        K::Int(_)
        | K::Float(_)
        | K::Bool(_)
        | K::Nil
        | K::Sym(_)
        | K::Ident(_)
        | K::AtIdent(_)
        | K::SelfRef
        | K::Path(_) => {}
    }
}

fn walk_pattern(p: &Pattern, declared: &mut HashSet<String>) {
    match p {
        Pattern::Bind { name, .. } => {
            declared.insert(name.name.clone());
        }
        Pattern::Ctor { args, .. } => {
            for a in args {
                walk_pattern(a, declared);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixie_syntax::{parse, span::FileId};

    fn resolve_str(src: &str) -> ResolveResult {
        let m = parse(FileId(0), src).expect("parse");
        resolve(&m, &ProjectInfo::default())
    }

    #[test]
    fn collects_class_signals_and_properties() {
        let src = r#"
class TodoItem {
  prop text : String, default: ""
  prop done : Bool, notify: :stateChanged
  signal stateChanged
}
"#;
        let r = resolve_str(src);
        assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
        let ItemKind::Class {
            ref property_names,
            ref signal_names,
            is_world_class,
            ..
        } = r.program.items["TodoItem"]
        else {
            panic!("expected Class");
        };
        assert_eq!(
            property_names,
            &vec!["text".to_string(), "done".to_string()]
        );
        // `text` (no explicit notify) auto-derives `textChanged`;
        // `done` keeps the user-specified `stateChanged`. Order
        // mirrors the source (explicit signal first, then per-prop
        // synth in declaration order).
        assert_eq!(
            signal_names,
            &vec!["stateChanged".to_string(), "textChanged".to_string()]
        );
        assert!(is_world_class);
    }

    #[test]
    fn missing_notify_signal_emits_diagnostic() {
        let src = r#"
class X {
  prop done : Bool, notify: :noSuchSignal
  signal otherSignal
}
"#;
        let r = resolve_str(src);
        assert_eq!(r.diagnostics.len(), 1, "{:?}", r.diagnostics);
        let msg = &r.diagnostics[0].message;
        assert!(msg.contains("notifies signal `noSuchSignal`"), "msg: {msg}");
        assert!(msg.contains("class `X`"), "msg: {msg}");
    }

    #[test]
    fn fn_scope_marks_first_assignment_as_decl() {
        let src = r#"
fn run {
  text = compute()
  text = compute()
}
"#;
        let r = resolve_str(src);
        let body_span = match &parse(FileId(0), src).unwrap().items[0] {
            Item::Fn(f) => f.body.as_ref().unwrap().span,
            _ => unreachable!(),
        };
        let scope = &r.program.fn_scopes[&body_span];
        let kinds: Vec<bool> = scope.assign_is_decl.values().copied().collect();
        // Two assignments: first should be a decl (true), second a reassignment (false).
        assert_eq!(kinds.iter().filter(|b| **b).count(), 1);
        assert_eq!(kinds.iter().filter(|b| !**b).count(), 1);
    }

    #[test]
    fn fn_param_does_not_become_decl_on_assignment() {
        let src = r#"
fn run(path: String) {
  path = compute()
}
"#;
        let r = resolve_str(src);
        let scope = r.program.fn_scopes.values().next().unwrap();
        let only = scope.assign_is_decl.values().next().copied().unwrap();
        assert!(!only, "param should already be in scope, so reassignment");
    }

    #[test]
    fn single_error_decl_becomes_default_error_type() {
        let src = r#"
error FileError {
  notFound
}

fn open(path: String) !File {
  doIt()
}
"#;
        let r = resolve_str(src);
        assert_eq!(r.program.default_error_type.as_deref(), Some("FileError"));
    }

    #[test]
    fn multiple_error_decls_clears_default() {
        let src = r#"
error FileError { notFound }
error NetError { offline }
"#;
        let r = resolve_str(src);
        assert!(r.program.default_error_type.is_none());
    }

    #[test]
    fn case_arm_bind_does_not_pollute_outer_scope() {
        let src = r#"
fn run {
  case x {
    when ok(cfg) { text = cfg }
  }
  text = "outer"
}
"#;
        let r = resolve_str(src);
        let scope = r.program.fn_scopes.values().next().unwrap();
        // The outer `text = "outer"` should be a fresh decl. The inner
        // `text = cfg` is in a sub-scope and may or may not be decl - we
        // care only that the outer one is.
        assert!(
            scope.assign_is_decl.values().any(|b| *b),
            "expected at least one decl: {:?}",
            scope.assign_is_decl
        );
    }

    #[test]
    fn break_outside_loop_is_diagnostic() {
        let src = r#"
fn run {
  break
}
"#;
        let r = resolve_str(src);
        assert_eq!(r.diagnostics.len(), 1, "{:?}", r.diagnostics);
        assert!(
            r.diagnostics[0].message.contains("`break` outside"),
            "msg: {}",
            r.diagnostics[0].message
        );
    }

    #[test]
    fn continue_outside_loop_is_diagnostic() {
        let src = r#"
fn run {
  continue
}
"#;
        let r = resolve_str(src);
        assert_eq!(r.diagnostics.len(), 1, "{:?}", r.diagnostics);
        assert!(
            r.diagnostics[0].message.contains("`continue` outside"),
            "msg: {}",
            r.diagnostics[0].message
        );
    }

    #[test]
    fn break_inside_for_is_ok() {
        let src = r#"
fn run {
  for i in 0..10 {
    if i == 5 { break }
  }
}
"#;
        let r = resolve_str(src);
        assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
    }

    #[test]
    fn continue_inside_while_is_ok() {
        let src = r#"
fn run {
  var i = 0
  while i < 10 {
    if i == 5 { continue }
    i = i + 1
  }
}
"#;
        let r = resolve_str(src);
        assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
    }

    #[test]
    fn break_inside_lambda_does_not_inherit_outer_loop() {
        // `for { f({ |x| break }) }` — the lambda body is a fresh
        // function scope; the outer for does not flow in. The
        // `break` inside the lambda is invalid even though there's
        // an enclosing for.
        let src = r#"
fn run {
  for i in 0..10 {
    each({ |x| break })
  }
}
"#;
        let r = resolve_str(src);
        let break_diags: Vec<_> = r
            .diagnostics
            .iter()
            .filter(|d| d.message.contains("`break` outside"))
            .collect();
        assert_eq!(
            break_diags.len(),
            1,
            "expected one break-outside diagnostic, got: {:?}",
            r.diagnostics
        );
    }

    fn case_warning_count(src: &str) -> usize {
        resolve_str(src)
            .diagnostics
            .iter()
            .filter(|d| {
                matches!(d.severity, pixie_syntax::diag::Severity::Warning)
                    && d.message.contains("non-exhaustive `case`")
            })
            .count()
    }

    /// `case` over `!T` is exhaustive when both `ok(...)` and
    /// `err(...)` arms are present.
    #[test]
    fn case_with_ok_and_err_emits_no_warning() {
        let src = r#"
error E { foo }
fn produce !Int {
  return E.foo()
}
fn run {
  case produce() {
    when ok(n)  { let _ = n }
    when err(e) { let _ = e }
  }
}
"#;
        assert_eq!(case_warning_count(src), 0);
    }

    /// `case` with a `_` wildcard is always exhaustive.
    #[test]
    fn case_with_wildcard_emits_no_warning() {
        let src = r#"
fn run {
  case 7 {
    when 1 { let _ = 1 }
    when _ { let _ = 0 }
  }
}
"#;
        assert_eq!(case_warning_count(src), 0);
    }

    /// `case` over a literal scrutinee with no wildcard and no
    /// recognised total cover should warn.
    #[test]
    fn case_without_wildcard_emits_warning() {
        let src = r#"
fn run {
  case 7 {
    when 1 { let _ = 1 }
    when 2 { let _ = 2 }
  }
}
"#;
        let r = resolve_str(src);
        let msg = r
            .diagnostics
            .iter()
            .find(|d| d.message.contains("non-exhaustive `case`"))
            .expect("expected non-exhaustive warning");
        assert!(matches!(msg.severity, pixie_syntax::diag::Severity::Warning));
    }

    /// `case` over `!E` covering only `ok(...)` is non-exhaustive
    /// (the `err` arm is missing) — should warn.
    #[test]
    fn case_over_error_union_missing_err_arm_warns() {
        let src = r#"
error E { foo }
fn produce !Int { return E.foo() }
fn run {
  case produce() {
    when ok(n) { let _ = n }
  }
}
"#;
        assert_eq!(case_warning_count(src), 1);
    }

    /// `case` over an `error E { ... }` value covering every variant
    /// (with an `err(...)` outer pattern shape) is exhaustive at the
    /// codegen level. The HIR check only matches direct variant
    /// patterns (no `err(...)` inner cover, since variant-set walk
    /// looks for ctor names). Pure variant arms should not warn.
    #[test]
    fn case_covering_all_error_variants_emits_no_warning() {
        let src = r#"
error E { alpha, beta }
fn produce E { return E.alpha() }
fn run {
  case produce() {
    when alpha { let _ = 1 }
    when beta  { let _ = 2 }
  }
}
"#;
        assert_eq!(case_warning_count(src), 0);
    }

    /// `case` over a Bool scrutinee covering both `true` and `false`
    /// arms is exhaustive — codegen elides the unreachable, HIR
    /// stays silent.
    #[test]
    fn case_over_bool_with_both_arms_emits_no_warning() {
        let src = r#"
fn run {
  case true {
    when true  { let _ = 1 }
    when false { let _ = 0 }
  }
}
"#;
        assert_eq!(case_warning_count(src), 0);
    }

    /// `case` over an Optional / nullable scrutinee covering both
    /// `some(x)` and `nil` arms is exhaustive — `Parent?` reads on a
    /// `weak` field yield this exact shape.
    #[test]
    fn case_over_optional_with_some_and_nil_emits_no_warning() {
        let src = r#"
class Parent { pub var label : Int = 0 }
class Child {
  weak let parent : Parent?
  init(p: Parent) { parent = p }
  pub fn describe {
    case parent {
      when some(p) { let _ = p }
      when nil     { let _ = 0 }
    }
  }
}
"#;
        assert_eq!(case_warning_count(src), 0);
    }

    /// `trait Foo { ... }` registers a Trait ItemKind in the
    /// resolved program with the method names listed.
    #[test]
    fn trait_decl_lands_in_items_with_method_names() {
        let src = r#"
trait Iterable {
  fn iter Iter
  fn size Int
}
"#;
        let r = resolve_str(src);
        assert!(
            r.diagnostics.is_empty(),
            "unexpected diags: {:?}",
            r.diagnostics
        );
        let kind = r
            .program
            .items
            .get("Iterable")
            .expect("Iterable should be registered");
        let ItemKind::Trait { methods, .. } = kind else {
            panic!("expected Trait, got {kind:?}");
        };
        let names: Vec<&str> = methods.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, vec!["iter", "size"]);
        assert!(methods.iter().all(|m| !m.has_default));
    }

    /// `impl Foo for Bar { ... }` lands in the `impls_for` map
    /// keyed by the implementing-type name.
    #[test]
    fn impl_decl_registers_in_impls_for_map() {
        let src = r#"
trait Iterable { fn iter Iter }
class MyList { prop n : Int, default: 0 }
impl Iterable for MyList {
  fn iter Iter { nil }
}
"#;
        let r = resolve_str(src);
        let traits = r
            .program
            .impls_for
            .get("MyList")
            .expect("MyList should have impl entry");
        assert!(traits.contains("Iterable"));
    }

    /// Two impls of distinct traits on the same class accumulate
    /// in the same set.
    #[test]
    fn multiple_impls_on_same_class_accumulate() {
        let src = r#"
trait A { fn a Int }
trait B { fn b Int }
class C { prop x : Int, default: 0 }
impl A for C { fn a Int { 1 } }
impl B for C { fn b Int { 2 } }
"#;
        let r = resolve_str(src);
        let traits = r.program.impls_for.get("C").expect("C should have impls");
        assert!(traits.contains("A"));
        assert!(traits.contains("B"));
    }

    /// `trait Foo { fn x -> String { "default" } fn y -> Int }` records
    /// `has_default = true` for `x`, `false` for `y`. Drives the
    /// impl-completeness pass.
    #[test]
    fn trait_method_default_body_marks_has_default_true() {
        let src = r#"
trait Greeter {
  fn greet String { "hi" }
  fn level Int
}
"#;
        let r = resolve_str(src);
        let kind = r.program.items.get("Greeter").expect("Greeter present");
        let ItemKind::Trait { methods, .. } = kind else {
            panic!("expected Trait, got {kind:?}");
        };
        let by_name: std::collections::HashMap<&str, bool> = methods
            .iter()
            .map(|m| (m.name.as_str(), m.has_default))
            .collect();
        assert_eq!(by_name.get("greet").copied(), Some(true));
        assert_eq!(by_name.get("level").copied(), Some(false));
    }

    /// `impl` that omits a non-default trait method is a hard error
    /// at HIR time. The diagnostic names the trait, the impl target,
    /// and the missing method.
    #[test]
    fn impl_missing_required_method_is_diagnostic() {
        let src = r#"
trait Pair {
  fn first Int
  fn second Int
}
class P { prop x : Int, default: 0 }
impl Pair for P {
  fn first Int { 1 }
}
"#;
        let r = resolve_str(src);
        let errs: Vec<&str> = r
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, pixie_syntax::diag::Severity::Error))
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            errs.iter()
                .any(|m| m.contains("missing required method `second`")),
            "expected missing-method diagnostic, got: {errs:?}"
        );
    }

    /// `impl` that supplies every required method is clean — even
    /// when the trait has a default-bodied method that the impl
    /// chooses to omit.
    #[test]
    fn impl_supplying_all_required_methods_is_clean() {
        let src = r#"
trait Greeter {
  fn level Int
  fn greet String { "hi" }
}
class G { prop x : Int, default: 0 }
impl Greeter for G {
  fn level Int { 7 }
}
"#;
        let r = resolve_str(src);
        let errs: Vec<&str> = r
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, pixie_syntax::diag::Severity::Error))
            .map(|d| d.message.as_str())
            .collect();
        assert!(errs.is_empty(), "unexpected errors: {errs:?}");
    }

    /// Multiple missing required methods produce one diagnostic
    /// per missing method (not a single "missing methods" rollup).
    #[test]
    fn impl_missing_multiple_required_methods_emits_per_method_diagnostics() {
        let src = r#"
trait Triple {
  fn a Int
  fn b Int
  fn c Int
}
class T { prop n : Int, default: 0 }
impl Triple for T {
  fn a Int { 1 }
}
"#;
        let r = resolve_str(src);
        let errs: Vec<&str> = r
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, pixie_syntax::diag::Severity::Error))
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            errs.iter()
                .any(|m| m.contains("missing required method `b`"))
        );
        assert!(
            errs.iter()
                .any(|m| m.contains("missing required method `c`"))
        );
    }

    /// An `impl` of an unknown trait stays silent in this pass —
    /// the visibility / name-resolution layers own that diagnostic.
    /// (We only emit when we can verify which methods are required.)
    #[test]
    fn impl_of_unknown_trait_does_not_emit_completeness_error() {
        let src = r#"
class X { prop n : Int, default: 0 }
impl Bogus for X {
  fn anything Int { 0 }
}
"#;
        let r = resolve_str(src);
        let errs: Vec<&str> = r
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, pixie_syntax::diag::Severity::Error))
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            errs.iter().all(|m| !m.contains("missing required method")),
            "completeness check should stay silent on unknown trait, got: {errs:?}"
        );
    }

    /// `impl<T> Foo for List<T> { ... }` registers under the
    /// for-type's simple base ("List"). Bound check at call sites
    /// then looks up "List", not "List<Int>" or "List<T>".
    #[test]
    fn parametric_impl_registers_under_for_type_base_name() {
        let src = r#"
trait Foo { fn x Int }
impl<T> Foo for List<T> {
  fn x Int { 0 }
}
"#;
        let r = resolve_str(src);
        let traits = r
            .program
            .impls_for
            .get("List")
            .expect("List should have impl entry");
        assert!(traits.contains("Foo"));
    }

    /// `impl Iterable for QStringList { ... }` registers under
    /// "QStringList" — the impls_for map keys by simple base name
    /// regardless of whether the for-type is a user class or an
    /// extern (binding-resolved) type.
    #[test]
    fn impl_for_extern_type_registers_in_impls_for() {
        let src = r#"
trait Iterable { fn first Int }
impl Iterable for QStringList {
  fn first Int { 0 }
}
"#;
        let r = resolve_str(src);
        let traits = r
            .program
            .impls_for
            .get("QStringList")
            .expect("QStringList impl entry should be registered");
        assert!(traits.contains("Iterable"));
    }

    /// Required-method completeness check still fires for
    /// parametric impls — the check keys off the for-type's base
    /// name and the missing-method diagnostic uses the rendered
    /// for-type expression so the user sees what they wrote.
    #[test]
    fn parametric_impl_missing_required_method_renders_for_type_expr() {
        let src = r#"
trait Pair {
  fn a Int
  fn b Int
}
impl<T> Pair for Box<T> {
  fn a Int { 1 }
}
"#;
        let r = resolve_str(src);
        let errs: Vec<&str> = r
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, pixie_syntax::diag::Severity::Error))
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            errs.iter()
                .any(|m| m.contains("missing required method `b`") && m.contains("`Box<T>`")),
            "expected diagnostic naming the rendered for-type, got: {errs:?}"
        );
    }

    /// `impl Foo for X` written twice with the same exact for-type
    /// is a hard error — the splice path's last-write-wins semantics
    /// would silently drop one set of methods.
    #[test]
    fn duplicate_impl_for_same_type_is_diagnostic() {
        let src = r#"
trait Foo { fn x Int }
class P { prop n : Int, default: 0 }
impl Foo for P { fn x Int { 1 } }
impl Foo for P { fn x Int { 2 } }
"#;
        let r = resolve_str(src);
        let errs: Vec<&str> = r
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, pixie_syntax::diag::Severity::Error))
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            errs.iter().any(|m| m.contains("already declared")
                && m.contains("`Foo`")
                && m.contains("`P`")),
            "expected duplicate-impl diagnostic, got: {errs:?}"
        );
    }

    /// Two impls of the same trait on different concrete types are
    /// independent — both register under different keys, no overlap.
    #[test]
    fn impls_of_same_trait_on_different_types_are_clean() {
        let src = r#"
trait Foo { fn x Int }
class A { prop n : Int, default: 0 }
class B { prop n : Int, default: 0 }
impl Foo for A { fn x Int { 1 } }
impl Foo for B { fn x Int { 2 } }
"#;
        let r = resolve_str(src);
        let errs: Vec<&str> = r
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, pixie_syntax::diag::Severity::Error))
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            !errs
                .iter()
                .any(|m| m.contains("already declared") || m.contains("overlaps")),
            "expected clean coherence, got: {errs:?}"
        );
    }

    /// Two impls of *different* traits on the same type are
    /// independent (the same class can implement many traits).
    #[test]
    fn impls_of_different_traits_on_same_type_are_clean() {
        let src = r#"
trait Foo { fn x Int }
trait Bar { fn y Int }
class P { prop n : Int, default: 0 }
impl Foo for P { fn x Int { 1 } }
impl Bar for P { fn y Int { 2 } }
"#;
        let r = resolve_str(src);
        let errs: Vec<&str> = r
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, pixie_syntax::diag::Severity::Error))
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            !errs
                .iter()
                .any(|m| m.contains("already declared") || m.contains("overlaps")),
            "expected clean coherence, got: {errs:?}"
        );
    }

    /// Parametric `impl<T> Foo for List<T>` plus a concrete
    /// `impl Foo for List<Int>` — the v1 specialization rule allows
    /// this on non-splice bases (here, the builtin `List<T>`). Both
    /// impls land as namespace-overload free functions and C++
    /// overload resolution picks the most specific at the call site.
    #[test]
    fn parametric_and_concrete_impls_on_builtin_base_specialize_cleanly() {
        let src = r#"
trait Foo { fn x Int }
impl<T> Foo for List<T> { fn x Int { 0 } }
impl Foo for List<Int> { fn x Int { 1 } }
"#;
        let r = resolve_str(src);
        let errs: Vec<&str> = r
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, pixie_syntax::diag::Severity::Error))
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            !errs
                .iter()
                .any(|m| m.contains("overlaps") || m.contains("already declared")),
            "expected clean coherence under specialization rule, got: {errs:?}"
        );
    }

    /// **Specialization on a user generic class is rejected** in v1.
    /// Splicing the impl method into the class body can only carry
    /// one definition per method, so `impl<T> Foo for Bag<T>` plus
    /// `impl Foo for Bag<Int>` would silently last-write-wins on
    /// the splice path. Reject with a clear message.
    #[test]
    fn parametric_and_concrete_on_user_class_is_diagnostic() {
        let src = r#"
class Bag<T> {
  let Item : T
}
trait Foo { fn x Int }
impl<T> Foo for Bag<T> { fn x Int { 0 } }
impl Foo for Bag<Int> { fn x Int { 1 } }
"#;
        let r = resolve_str(src);
        let errs: Vec<&str> = r
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, pixie_syntax::diag::Severity::Error))
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            errs.iter().any(|m| m.contains("overlaps")
                && m.contains("specialization on user classes is not yet supported")),
            "expected splice-target rejection diagnostic, got: {errs:?}"
        );
    }

    /// Different concrete instantiations on a user generic class are
    /// disjoint — neither covers the other and both can splice with
    /// non-overlapping signatures (different T binding per impl).
    /// Stays clean.
    #[test]
    fn different_concrete_impls_on_user_class_are_clean() {
        let src = r#"
class Bag<T> {
  let Item : T
}
trait Foo { fn x Int }
impl Foo for Bag<Int> { fn x Int { 1 } }
impl Foo for Bag<String> { fn x Int { 2 } }
"#;
        let r = resolve_str(src);
        let errs: Vec<&str> = r
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, pixie_syntax::diag::Severity::Error))
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            !errs
                .iter()
                .any(|m| m.contains("overlaps") || m.contains("already declared")),
            "expected clean coherence for disjoint concrete impls, got: {errs:?}"
        );
    }

    /// Two concrete impls with the same base + arity but different
    /// arg shapes (`List<Int>` vs `List<String>`) don't overlap —
    /// they cover disjoint concrete types. Stays clean.
    #[test]
    fn concrete_impls_with_different_arg_shapes_are_clean() {
        let src = r#"
trait Foo { fn x Int }
impl Foo for List<Int> { fn x Int { 1 } }
impl Foo for List<String> { fn x Int { 2 } }
"#;
        let r = resolve_str(src);
        let errs: Vec<&str> = r
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, pixie_syntax::diag::Severity::Error))
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            !errs
                .iter()
                .any(|m| m.contains("already declared") || m.contains("overlaps")),
            "expected clean coherence, got: {errs:?}"
        );
    }

    /// Two parametric impls on the same base + arity (regardless of
    /// type-var name) have identical specificity — even with the v1
    /// specialization rule, neither is more specific than the other.
    /// At C++ namespace-overload time this would be an ODR violation
    /// (two `template<typename T>` definitions with the same
    /// signature). Reject with an "ambiguous parametric" diagnostic.
    #[test]
    fn two_parametric_impls_on_same_base_are_ambiguous() {
        let src = r#"
trait Foo { fn x Int }
impl<T> Foo for List<T> { fn x Int { 0 } }
impl<U> Foo for List<U> { fn x Int { 1 } }
"#;
        let r = resolve_str(src);
        let errs: Vec<&str> = r
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, pixie_syntax::diag::Severity::Error))
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            errs.iter().any(|m| m.contains("ambiguous parametric impl")),
            "expected ambiguous-parametric diagnostic, got: {errs:?}"
        );
    }

    /// Different bases never overlap (sanity).
    #[test]
    fn impls_on_different_bases_never_overlap() {
        let src = r#"
trait Foo { fn x Int }
impl Foo for List<Int> { fn x Int { 1 } }
impl Foo for Box<Int> { fn x Int { 2 } }
"#;
        let r = resolve_str(src);
        let errs: Vec<&str> = r
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, pixie_syntax::diag::Severity::Error))
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            !errs
                .iter()
                .any(|m| m.contains("already declared") || m.contains("overlaps")),
            "expected clean coherence, got: {errs:?}"
        );
    }

    // ---- fn-overload coherence -------------------------------------

    /// Two methods with identical signatures (same name + arity +
    /// param types) on the same class are a hard error — the
    /// resolver's exact-match tier would be ambiguous and codegen
    /// would emit two C++ definitions with the same signature.
    #[test]
    fn duplicate_class_method_signature_is_diagnostic() {
        let src = r#"
class Greeter {
  fn greet(name: String) String { name }
  fn greet(name: String) String { name }
}
"#;
        let r = resolve_str(src);
        let errs: Vec<&str> = r
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, pixie_syntax::diag::Severity::Error))
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            errs.iter()
                .any(|m| m.contains("duplicate method `greet`") && m.contains("`Greeter`")),
            "expected duplicate-method diagnostic, got: {errs:?}"
        );
    }

    /// Same shape at the free-fn level: two `fn fmt(x: Int)` declared
    /// in the same module are duplicates.
    #[test]
    fn duplicate_top_level_fn_signature_is_diagnostic() {
        let src = r#"
fn fmt(x: Int) String { "a" }
fn fmt(x: Int) String { "b" }
"#;
        let r = resolve_str(src);
        let errs: Vec<&str> = r
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, pixie_syntax::diag::Severity::Error))
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            errs.iter()
                .any(|m| m.contains("duplicate fn `fmt`") && m.contains("in this module")),
            "expected duplicate-fn diagnostic, got: {errs:?}"
        );
    }

    /// Different param types under the same name + arity are NOT
    /// duplicates — the resolver picks via Tier 2 exact match.
    #[test]
    fn overload_with_different_arg_types_is_clean() {
        let src = r#"
class Tag {
  fn matches(other: String) Bool { true }
  fn matches(other: Int) Bool { true }
}
"#;
        let r = resolve_str(src);
        let errs: Vec<&str> = r
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, pixie_syntax::diag::Severity::Error))
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            errs.is_empty(),
            "expected clean overload, got errors: {errs:?}"
        );
    }

    /// Different arities under the same name are not duplicates.
    #[test]
    fn overload_with_different_arities_is_clean() {
        let src = r#"
fn fmt String { "" }
fn fmt(x: Int) String { "" }
fn fmt(x: Int, y: Int) String { "" }
"#;
        let r = resolve_str(src);
        let errs: Vec<&str> = r
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, pixie_syntax::diag::Severity::Error))
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            errs.is_empty(),
            "expected clean overload, got errors: {errs:?}"
        );
    }

    /// Two parametric (`<T>`) overloads with the same arity in the
    /// same scope are ambiguous at every call site — neither is more
    /// specific than the other.
    #[test]
    fn two_generic_overloads_same_arity_is_diagnostic() {
        let src = r#"
fn pick<T>(x: T) T { x }
fn pick<U>(x: U) U { x }
"#;
        let r = resolve_str(src);
        let errs: Vec<&str> = r
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, pixie_syntax::diag::Severity::Error))
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            errs.iter()
                .any(|m| m.contains("ambiguous parametric") && m.contains("`pick`")),
            "expected ambiguous-parametric diagnostic, got: {errs:?}"
        );
    }

    /// One parametric `<T>` plus one concrete overload of the same
    /// name + arity is clean — concrete wins via the resolver's
    /// most-specific tiebreak.
    #[test]
    fn generic_plus_concrete_overload_is_clean() {
        let src = r#"
fn pick<T>(x: T) T { x }
fn pick(x: Int) Int { x }
"#;
        let r = resolve_str(src);
        let errs: Vec<&str> = r
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, pixie_syntax::diag::Severity::Error))
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            errs.is_empty(),
            "expected clean parametric+concrete, got: {errs:?}"
        );
    }

    // ---- silent-trap lints ---------------------------------------------

    /// Helper: collect diagnostic messages whose text contains `substr`.
    fn diags_containing<'a>(r: &'a ResolveResult, substr: &str) -> Vec<&'a str> {
        r.diagnostics
            .iter()
            .map(|d| d.message.as_str())
            .filter(|m| m.contains(substr))
            .collect()
    }

    // -- Lint 1: manual emit after prop write --

    #[test]
    fn manual_emit_after_prop_write_in_same_block_warns() {
        let src = r#"
class Counter {
  prop count : Int, notify: :countChanged, default: 0
  signal countChanged
  fn incr {
    count = count + 1
    emit countChanged
  }
}
"#;
        let r = resolve_str(src);
        let hits = diags_containing(&r, "fire it twice");
        assert_eq!(
            hits.len(),
            1,
            "expected one double-emit warning, got: {:?}",
            r.diagnostics
        );
        assert!(hits[0].contains("`count = ...`"), "msg: {}", hits[0]);
        assert!(hits[0].contains("countChanged"), "msg: {}", hits[0]);
    }

    #[test]
    fn manual_emit_for_unrelated_signal_does_not_warn() {
        // Writing `count` and emitting `displayChanged` is unrelated —
        // no auto-emit overlap.
        let src = r#"
class Counter {
  prop count : Int, notify: :countChanged, default: 0
  signal countChanged
  signal displayChanged
  fn incr {
    count = count + 1
    emit displayChanged
  }
}
"#;
        let r = resolve_str(src);
        let hits = diags_containing(&r, "fire it twice");
        assert!(hits.is_empty(), "should not warn: {:?}", r.diagnostics);
    }

    #[test]
    fn manual_emit_without_prior_assign_does_not_warn() {
        // Emitting countChanged without writing count first — user is
        // probably notifying observers of an external state change. Fine.
        let src = r#"
class Counter {
  prop count : Int, notify: :countChanged, default: 0
  signal countChanged
  fn touch { emit countChanged }
}
"#;
        let r = resolve_str(src);
        let hits = diags_containing(&r, "fire it twice");
        assert!(hits.is_empty(), "should not warn: {:?}", r.diagnostics);
    }

    #[test]
    fn assign_to_plain_field_then_emit_does_not_warn() {
        // `var` is not a Q_PROPERTY — its setter does not auto-emit, so
        // a manual emit afterwards is the user's only emit, not a double.
        let src = r#"
class Counter {
  var label : String = ""
  signal labelChanged
  fn rename {
    label = "x"
    emit labelChanged
  }
}
"#;
        let r = resolve_str(src);
        let hits = diags_containing(&r, "fire it twice");
        assert!(hits.is_empty(), "should not warn: {:?}", r.diagnostics);
    }

    #[test]
    fn manual_emit_inside_nested_block_is_caught() {
        // Same-block scope is the rule; the inner `if` body counts as
        // its own block, but our recursion walks into it and pairs the
        // assign+emit there too.
        let src = r#"
class Counter {
  prop count : Int, notify: :countChanged, default: 0
  signal countChanged
  fn incr {
    if true {
      count = count + 1
      emit countChanged
    }
  }
}
"#;
        let r = resolve_str(src);
        let hits = diags_containing(&r, "fire it twice");
        assert_eq!(
            hits.len(),
            1,
            "expected nested-block double-emit warning, got: {:?}",
            r.diagnostics
        );
    }

    // The two `arc` lint tests went with the keyword (§8.71).

    // Lint 2 (pub var on a world class) was removed in §8.58: a
    // plain field carries a change signal now, so "views cannot react
    // to it" stopped being true.

    #[test]
    fn parentless_qobject_new_unused_warns() {
        let src = r#"
class Foo {
  prop x : Int, notify: :xChanged, default: 0
}
fn main {
  let foo = Foo.new()
}
"#;
        let r = resolve_str(src);
        let hits = diags_containing(&r, "constructs a World object");
        assert_eq!(
            hits.len(),
            1,
            "expected parent-less warning, got: {:?}",
            r.diagnostics
        );
        assert!(hits[0].contains("`Foo.new()`"), "msg: {}", hits[0]);
        assert!(hits[0].contains("`foo` is never used"), "msg: {}", hits[0]);
    }

    #[test]
    fn parentless_qobject_new_bare_expr_stmt_warns() {
        // The bare-expression-statement form: `Foo.new()` whose
        // result is thrown away. This is the cut-and-paste-from-a-
        // class-method footgun the language feedback flagged — inside
        // a class method `Foo.new()` would auto-parent to `this`, but
        // pasted into a top-level fn or cli_app body it leaks
        // immediately. Lint should warn, distinct from the dead-let
        // case so the diagnostic explains the parent-injection
        // difference clearly.
        let src = r#"
class Foo {
  prop x : Int, default: 0
}
fn main {
  Foo.new()
}
"#;
        let r = resolve_str(src);
        let hits = diags_containing(&r, "discards the handle");
        assert_eq!(
            hits.len(),
            1,
            "expected discarded-handle warning, got: {:?}",
            r.diagnostics
        );
        assert!(hits[0].contains("`Foo.new()`"), "msg: {}", hits[0]);
        assert!(
            hits[0].contains("hand it to an owner"),
            "diagnostic should point at ownership: {}",
            hits[0]
        );
    }


    #[test]
    fn parentless_qobject_new_bare_expr_stmt_inside_class_method_does_not_warn() {
        // Inside a class method `Foo.new()` auto-injects `this` as
        // parent — the QObject is owned by the surrounding class
        // instance. No leak, no warning.
        let src = r#"
class Foo {
  prop x : Int, default: 0
}
class Owner {
  fn make { Foo.new() }
}
"#;
        let r = resolve_str(src);
        let hits = diags_containing(&r, "result is discarded");
        assert!(
            hits.is_empty(),
            "class-method context should not trigger: {:?}",
            r.diagnostics
        );
    }

    #[test]
    fn parentless_qobject_new_with_handoff_does_not_warn() {
        // Passing the binding to a follow-up call (qml_app, anything)
        // means the user knows where it goes. Suppress.
        let src = r#"
class Foo {
  prop x : Int, notify: :xChanged, default: 0
}
fn main {
  let foo = Foo.new()
  qml_app(view: foo)
}
"#;
        let r = resolve_str(src);
        let hits = diags_containing(&r, "constructs a World object");
        assert!(
            hits.is_empty(),
            "should not warn (binding handed off): {:?}",
            r.diagnostics
        );
    }


    #[test]
    fn parentless_qobject_new_inside_class_method_does_not_warn() {
        // Inside a class method, `this` provides an implicit parent
        // for `T.new()` — the lint deliberately skips class methods.
        let src = r#"
class Child { }
class Parent {
  fn spawn {
    let c = Child.new()
  }
}
"#;
        let r = resolve_str(src);
        let hits = diags_containing(&r, "constructs a World object");
        assert!(
            hits.is_empty(),
            "should not warn (class method): {:?}",
            r.diagnostics
        );
    }

    #[test]
    fn parentless_qobject_new_used_in_subsequent_method_call_does_not_warn() {
        // The binding is referenced (foo.x) → user is using it. Trust
        // them; even if leak-prone, the lint stays silent.
        let src = r#"
class Foo {
  prop x : Int, notify: :xChanged, default: 0
}
fn main {
  let foo = Foo.new()
  println(foo.x)
}
"#;
        let r = resolve_str(src);
        let hits = diags_containing(&r, "constructs a World object");
        assert!(
            hits.is_empty(),
            "should not warn (binding used): {:?}",
            r.diagnostics
        );
    }
}
