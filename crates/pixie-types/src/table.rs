//! Pre-computed program-wide type table built from `Module + ResolvedProgram`.
//!
//! The `ProgramTable` is the counterpart of HIR's `ResolvedProgram` for the
//! type checker: where HIR holds names and structural shapes, the
//! `ProgramTable` holds the *types* of those entities so `synth`/`check`
//! can dispatch on `MethodCall`, `Member`, `Ident`-of-fn-name, `emit`,
//! and constructor patterns without re-walking the AST every time.
//!
//! Built once at the start of `check_program` by walking `Module.items`
//! and lowering each `TypeExpr` via `crate::ty::lower_type`.

use crate::infer::VarSource;
use crate::ty::{Type, VarId, lower_type};
use pixie_hir::ItemHome;

/// Mirror of cute-codegen's `setter_name`: `text` -> `setText`. Kept
/// in sync because cute-types and cute-codegen are independent crates
/// and this string drives type-check resolution of property setters.
fn setter_name(prop: &str) -> String {
    let mut chars = prop.chars();
    let first = chars.next().unwrap_or('_').to_uppercase().to_string();
    format!("set{first}{}", chars.collect::<String>())
}
use pixie_hir::ResolvedProgram;
use pixie_syntax::ast::{ClassMember, FnDecl, Item, Module};
use std::collections::HashMap;

/// Lower a fn declaration's signature, allocating a `VarId` from the
/// supplied source for each of its generic params and substituting
/// references to those param names with the corresponding `Type::Var`.
/// Decl-time and call-site VarIds share one monotonic source so that
/// instantiation never accidentally binds a var to itself.
fn build_fn_ty(f: &FnDecl, program: &ResolvedProgram, source: &mut VarSource) -> FnTy {
    if f.generics.is_empty() {
        let params = f
            .params
            .iter()
            .map(|p| lower_type(&p.ty, program))
            .collect();
        let ret = f
            .return_ty
            .as_ref()
            .map(|t| lower_type(t, program))
            .unwrap_or(Type::void());
        return FnTy {
            generics: Vec::new(),
            generic_bounds: Vec::new(),
            params,
            ret,
            is_static: f.is_static,
        };
    }
    let generics: Vec<VarId> = (0..f.generics.len()).map(|_| source.fresh()).collect();
    let generic_bounds: Vec<Vec<String>> = f
        .generics
        .iter()
        .map(|g| g.bounds.iter().map(|b| b.name.clone()).collect())
        .collect();
    let name_to_var: HashMap<String, VarId> = f
        .generics
        .iter()
        .zip(generics.iter())
        .map(|(g, v)| (g.name.name.clone(), *v))
        .collect();
    let params = f
        .params
        .iter()
        .map(|p| {
            let raw = lower_type(&p.ty, program);
            substitute_names(&raw, &name_to_var)
        })
        .collect();
    let ret = f
        .return_ty
        .as_ref()
        .map(|t| substitute_names(&lower_type(t, program), &name_to_var))
        .unwrap_or(Type::void());
    FnTy {
        generics,
        generic_bounds,
        params,
        ret,
        is_static: f.is_static,
    }
}

/// Substitute `Type::Var(v)` references with concrete types using a
/// VarId -> Type map. Counterpart to `substitute_names` (which goes
/// the other way: name -> Var). Used by class-generic instantiation
/// at member-lookup sites.
fn substitute_vars(t: &Type, map: &HashMap<VarId, Type>) -> Type {
    match t {
        Type::Var(v) => map.get(v).cloned().unwrap_or_else(|| t.clone()),
        Type::Generic { base, args } => Type::Generic {
            base: base.clone(),
            args: args.iter().map(|a| substitute_vars(a, map)).collect(),
        },
        Type::Nullable(inner) => Type::Nullable(Box::new(substitute_vars(inner, map))),
        Type::ErrorUnion { ok, err } => Type::ErrorUnion {
            ok: Box::new(substitute_vars(ok, map)),
            err: err.clone(),
        },
        Type::Fn { params, ret } => Type::Fn {
            params: params.iter().map(|p| substitute_vars(p, map)).collect(),
            ret: Box::new(substitute_vars(ret, map)),
        },
        other => other.clone(),
    }
}

/// Walk `t`, replacing every occurrence of `Class("Self")` /
/// `External("Self")` with `recv`. Used at trait-method call sites
/// to bind the trait's abstract `Self` to the concrete receiver.
pub fn substitute_self(t: &Type, recv: &Type) -> Type {
    match t {
        Type::Class(name) | Type::External(name) if name == pixie_syntax::ast::SELF_TYPE_NAME => {
            recv.clone()
        }
        Type::Generic { base, args } => Type::Generic {
            base: base.clone(),
            args: args.iter().map(|a| substitute_self(a, recv)).collect(),
        },
        Type::Nullable(inner) => Type::Nullable(Box::new(substitute_self(inner, recv))),
        Type::ErrorUnion { ok, err } => Type::ErrorUnion {
            ok: Box::new(substitute_self(ok, recv)),
            err: err.clone(),
        },
        Type::Fn { params, ret } => Type::Fn {
            params: params.iter().map(|p| substitute_self(p, recv)).collect(),
            ret: Box::new(substitute_self(ret, recv)),
        },
        other => other.clone(),
    }
}

/// Walk `t`, replacing `Class(name)` / `External(name)` whose name is a
/// declared generic param with the corresponding `Type::Var`.
pub fn substitute_names(t: &Type, generics: &HashMap<String, VarId>) -> Type {
    match t {
        Type::Class(name) | Type::External(name) => {
            if let Some(&v) = generics.get(name) {
                Type::Var(v)
            } else {
                t.clone()
            }
        }
        Type::Generic { base, args } => Type::Generic {
            base: base.clone(),
            args: args.iter().map(|a| substitute_names(a, generics)).collect(),
        },
        Type::Nullable(inner) => Type::Nullable(Box::new(substitute_names(inner, generics))),
        Type::ErrorUnion { ok, err } => Type::ErrorUnion {
            ok: Box::new(substitute_names(ok, generics)),
            err: err.clone(),
        },
        Type::Fn { params, ret } => Type::Fn {
            params: params
                .iter()
                .map(|p| substitute_names(p, generics))
                .collect(),
            ret: Box::new(substitute_names(ret, generics)),
        },
        other => other.clone(),
    }
}

#[derive(Default, Debug, Clone)]
pub struct ProgramTable {
    pub classes: HashMap<String, ClassEntry>,
    /// Free-fn name -> overload set (one entry per declared signature).
    pub fns: HashMap<String, Vec<FnTy>>,
    pub errors: HashMap<String, ErrorEntry>,
    /// Plain value-type structs (`struct Point { x: Int, y: Int }`).
    /// Distinct from classes — value semantics, no inheritance, no
    /// metaobject. Field access via `p.x` resolves through this map.
    pub structs: HashMap<String, StructEntry>,
}

#[derive(Default, Debug, Clone)]
pub struct ClassEntry {
    pub super_class: Option<String>,
    pub properties: HashMap<String, Type>,
    /// Plain class fields (`let x : T = ...` / `var x : T = ...`).
    /// Distinct from `properties` because plain fields are NOT
    /// Q_PROPERTY: no metaobject registration, no NOTIFY signal, no
    /// synthesized getter/setter in `methods`. Inside class methods the
    /// `@x` syntax binds via this map (parallel to the `@x` for
    /// properties).
    pub fields: HashMap<String, FieldEntry>,
    /// Signal name -> parameter types. Signals are NOT overloaded
    /// (Qt's new pointer-to-member-function `connect` makes overloaded
    /// signals require `qOverload<...>` casts on the user side; Cute
    /// follows Qt 6's direction of single-signature signals).
    pub signals: HashMap<String, Vec<Type>>,
    /// Method name -> overload set. Each inserted entry is one declared
    /// signature; resolution at call sites runs `resolve_overload`.
    pub methods: HashMap<String, Vec<FnTy>>,
    /// Per-member visibility. `pub` opts each name in to access from
    /// outside the declaring class. The simple-name key shares a
    /// namespace across properties / signals / methods because Qt
    /// moc semantics tie property `x` to method `x` (getter) and
    /// signal `xChanged` lives in the same class. The auto-generated
    /// getter / setter for a property inherit the property's pub
    /// state. The check itself runs in `crate::check::synth` /
    /// `synth_method_call` against the receiver's class entry.
    pub member_pub: HashMap<String, bool>,
    /// True when this entry was sourced from a binding `.qpi`. The
    /// member-visibility check skips bindings entirely - users
    /// cannot edit those files to add `pub`, and Qt's API is by
    /// definition the public surface.
    pub from_binding: bool,
    /// Generic parameter `VarId`s for `class Box<T, U> { ... }`.
    /// Empty for non-generic classes. Property / signal / method
    /// types stored above already reference these vars; the
    /// `instantiate_member` helper substitutes call-site type args
    /// against this list when looking up members on a concrete
    /// `Type::Generic { base, args }` receiver.
    pub class_generics: Vec<crate::ty::VarId>,
    /// User-declared `init(params)` signatures. Empty = fall through
    /// to foreign-soft acceptance at `T.new(args)`; otherwise the
    /// call site picks the first matching arity.
    pub inits: Vec<FnTy>,
}

#[derive(Debug, Clone)]
pub struct FieldEntry {
    pub ty: Type,
    /// `let` field → `false` (immutable; writable exactly once in `init`) /
    /// `var` field → `true` (assignable in class method bodies via `x = v`).
    pub is_mut: bool,
    pub is_pub: bool,
}

/// `struct Point { x: Int, y: Int = 0 }` — plain value type. Fields
/// are positional in declaration order (used by `Point.new(x_val,
/// y_val)`). Default values are an AST-level concern (lowered by
/// codegen into the C++ struct's in-class default-initializers); the
/// type table tracks names + types only.
#[derive(Debug, Clone)]
pub struct StructEntry {
    pub fields: Vec<(String, Type)>,
    /// User-declared `fn` methods on the struct body. Same overload-
    /// set shape as `ClassEntry::methods` so the existing dispatch
    /// machinery can resolve `point.method(args)` uniformly.
    pub methods: HashMap<String, Vec<FnTy>>,
}

#[derive(Default, Debug, Clone)]
pub struct ErrorEntry {
    /// Variant name -> field types (empty for nullary variants).
    pub variants: HashMap<String, Vec<Type>>,
}

#[derive(Debug, Clone)]
pub struct FnTy {
    /// Declaration-time `VarId`s standing in for this fn's generic
    /// parameters (`T`, `U`, ...). Empty for non-generic fns. The fn's
    /// `params`/`ret` may reference these vars; call-site instantiation
    /// (see `crate::infer::instantiate`) substitutes them with fresh
    /// vars so each call is independent.
    pub generics: Vec<crate::ty::VarId>,
    /// Per-generic trait bounds, aligned by index with `generics`.
    /// `generic_bounds[i]` is the list of trait simple names that
    /// `generics[i]` is required to implement. Empty inner vec means
    /// the corresponding generic has no bounds (the v1 default for
    /// bare `fn first<T>` form). Populated from the parser's
    /// `GenericParam.bounds`. Read at call-site bound enforcement.
    pub generic_bounds: Vec<Vec<String>>,
    pub params: Vec<Type>,
    pub ret: Type,
    /// True for `static fn name(...)` on a class — receiverless,
    /// callable as `ClassName.name(args)`. False for instance methods
    /// (the common case), prop getters/setters, struct methods, init
    /// signatures, free fns, and trait methods.
    pub is_static: bool,
}

impl FnTy {
    pub fn as_type(&self) -> Type {
        Type::Fn {
            params: self.params.clone(),
            ret: Box::new(self.ret.clone()),
        }
    }
}

pub fn build(module: &Module, program: &ResolvedProgram, source: &mut VarSource) -> ProgramTable {
    let mut t = ProgramTable::default();
    for item in &module.items {
        match item {
            Item::Class(c) => {
                let mut entry = ClassEntry::default();
                // Generic class: allocate one fresh VarId per type
                // param and store them on the entry. Member-type
                // lowering below substitutes the param names through
                // these vars so a property typed `T` becomes
                // `Type::Var(VarId)`. Lookups on `Type::Generic {
                // base, args }` then substitute args into those vars.
                let class_generic_vars: Vec<crate::ty::VarId> =
                    (0..c.generics.len()).map(|_| source.fresh()).collect();
                let class_name_to_var: HashMap<String, crate::ty::VarId> = c
                    .generics
                    .iter()
                    .zip(class_generic_vars.iter())
                    .map(|(g, v)| (g.name.name.clone(), *v))
                    .collect();
                entry.class_generics = class_generic_vars.clone();
                // pixie: no implicit super. The chain is exactly what
                // the source declares — and for user code that is
                // nothing, because HIR's pass 0 rejects `< Super` (D9).
                let super_name = c.super_class.as_ref().and_then(|t| {
                    if let pixie_syntax::ast::TypeKind::Named { path, .. } = &t.kind {
                        path.last().map(|i| i.name.clone())
                    } else {
                        None
                    }
                });
                entry.super_class = super_name;
                // Tag the entry as binding-sourced when its home is
                // Prelude (Qt stdlib `.qpi` files etc.). The
                // visibility check uses this flag to skip member-pub
                // enforcement on Qt classes - their entire surface is
                // by definition public, and users can't edit binding
                // files to add `pub`. `extern value` classes get the
                // same treatment regardless of where they're declared
                // — they exclusively describe foreign C++ surfaces, so
                // there's no internal Cute implementation to hide.
                entry.from_binding = c.is_extern_value
                    || matches!(
                        program.items.get(&c.name.name).map(|it| it.home().clone()),
                        Some(ItemHome::Prelude)
                    );
                // Pre-scan user-declared method (name, lowered-param-types)
                // pairs so the prop-getter / prop-setter synth below can
                // skip emission whenever the class also declares the same
                // accessor explicitly. Common in `.qpi` bindings that
                // co-declare `prop title : String, default: \"\"` and
                // `fn setTitle(title: String)`; without the dedupe both
                // candidates land in the methods bucket and the resolver
                // calls every `setTitle("x")` ambiguous.
                let mut user_declared_sigs: HashMap<String, Vec<Vec<Type>>> = HashMap::new();
                for member in &c.members {
                    if let ClassMember::Fn(f) | ClassMember::Slot(f) = member {
                        let params: Vec<Type> = f
                            .params
                            .iter()
                            .map(|p| {
                                substitute_names(&lower_type(&p.ty, program), &class_name_to_var)
                            })
                            .collect();
                        user_declared_sigs
                            .entry(f.name.name.clone())
                            .or_default()
                            .push(params);
                    }
                }
                for member in &c.members {
                    match member {
                        ClassMember::Property(p) => {
                            // Lower the declared type, then substitute
                            // any generic-param-name reference with its
                            // VarId so `property item: T` on
                            // `class Box<T>` stores `Type::Var(...)`
                            // instead of `Type::External("T")`.
                            // `, model` props are special: at the
                            // surface the user writes `List<T>`, but
                            // the actual runtime type is
                            // `cute::ModelList<T*>*` — a QObject-derived
                            // adapter. For Cute-side type-checking we
                            // expose it as External("QAbstractItemModel")
                            // so it satisfies `model: QAbstractItemModel*`
                            // assignments in QML view bodies, and
                            // method calls on it (`xs.append(b)`,
                            // `xs.size()`, …) soft-pass — the C++
                            // compiler validates against the real
                            // ModelList surface at build time.
                            let pty = if p.model {
                                Type::External("QAbstractItemModel".to_string())
                            } else {
                                let raw = lower_type(&p.ty, program);
                                substitute_names(&raw, &class_name_to_var)
                            };
                            entry.properties.insert(p.name.name.clone(), pty.clone());
                            // Synth getter `name() -> T`. Skip if the
                            // class also declares an explicit `fn name`
                            // with no params (binding pattern).
                            let getter_dup = user_declared_sigs
                                .get(&p.name.name)
                                .map(|sigs| sigs.iter().any(|s| s.is_empty()))
                                .unwrap_or(false);
                            if !getter_dup {
                                entry
                                    .methods
                                    .entry(p.name.name.clone())
                                    .or_default()
                                    .push(FnTy {
                                        generics: Vec::new(),
                                        generic_bounds: Vec::new(),
                                        params: vec![],
                                        ret: pty.clone(),
                                        is_static: false,
                                    });
                            }
                            // Synth setter `setName(T)`. Skip if the
                            // class also declares an explicit
                            // `fn setName(_: T)` with the same param.
                            let setter_n = setter_name(&p.name.name);
                            let setter_dup = user_declared_sigs
                                .get(&setter_n)
                                .map(|sigs| sigs.iter().any(|s| s.len() == 1 && s[0] == pty))
                                .unwrap_or(false);
                            if !setter_dup {
                                entry.methods.entry(setter_n).or_default().push(FnTy {
                                    generics: Vec::new(),
                                    generic_bounds: Vec::new(),
                                    params: vec![pty],
                                    ret: Type::void(),
                                    is_static: false,
                                });
                            }
                            entry.member_pub.insert(p.name.name.clone(), p.is_pub);
                            entry.member_pub.insert(setter_name(&p.name.name), p.is_pub);
                            // `, model` props were previously synthesised
                            // as a sibling `<propName>Model` accessor.
                            // Dropped 2026-05-04 evening when the wrapper-
                            // type design landed: the prop's value type
                            // itself becomes `cute::ModelList<T*>*`,
                            // exposing append / removeAt / clear / replace
                            // / size / etc. as ordinary methods. Cute-
                            // side type-checking treats `library.Books`
                            // as `List<Book>` (existing builtin generic
                            // shape); per-method validation still soft-
                            // passes for now, with the C++ compile catching
                            // typos against the real ModelList surface.
                        }
                        ClassMember::Signal(s) => {
                            let params = s
                                .params
                                .iter()
                                .map(|p| {
                                    substitute_names(
                                        &lower_type(&p.ty, program),
                                        &class_name_to_var,
                                    )
                                })
                                .collect();
                            entry.signals.insert(s.name.name.clone(), params);
                            entry.member_pub.insert(s.name.name.clone(), s.is_pub);
                        }
                        ClassMember::Fn(f) | ClassMember::Slot(f) => {
                            // Method signatures: lower the fn's own
                            // generics on top of the class's generics.
                            // The class's name->var map applies first
                            // so a method like `fn put(x: T)` on
                            // `class Box<T>` sees T as the class's
                            // VarId.
                            let mut fnty = build_fn_ty(f, program, source);
                            fnty.params = fnty
                                .params
                                .iter()
                                .map(|p| substitute_names(p, &class_name_to_var))
                                .collect();
                            fnty.ret = substitute_names(&fnty.ret, &class_name_to_var);
                            entry
                                .methods
                                .entry(f.name.name.clone())
                                .or_default()
                                .push(fnty);
                            entry.member_pub.insert(f.name.name.clone(), f.is_pub);
                        }
                        ClassMember::Field(f) => {
                            // Plain class field (`let x : T = ...` /
                            // `var x : T = ...`). Distinct from
                            // `prop` — no Q_PROPERTY, no NOTIFY. The
                            // `fields` map binds `@x` for method body
                            // resolution. For `pub` fields, the codegen
                            // emits a public getter (and setter for
                            // `var`); to make external `obj.x` /
                            // `obj.setX(v)` typecheck, we also enter
                            // synth method entries — same shape as the
                            // prop getter/setter synth path.
                            let raw = lower_type(&f.ty, program);
                            let pty = substitute_names(&raw, &class_name_to_var);
                            entry.fields.insert(
                                f.name.name.clone(),
                                FieldEntry {
                                    ty: pty.clone(),
                                    is_mut: f.is_mut,
                                    is_pub: f.is_pub,
                                },
                            );
                            entry.member_pub.insert(f.name.name.clone(), f.is_pub);
                            // For `pub` fields, also enter the field type into
                            // `properties` so `obj.x` resolves to the field
                            // type (T) directly. The checker treats `properties`
                            // as the canonical "this is the type of obj.x"
                            // table; without this entry, member access falls
                            // through to the `methods` table and returns the
                            // getter's `fn() -> T` instead of `T`.
                            if f.is_pub {
                                entry.properties.insert(f.name.name.clone(), pty.clone());
                            }
                            if f.is_pub {
                                let getter_dup = user_declared_sigs
                                    .get(&f.name.name)
                                    .map(|sigs| sigs.iter().any(|s| s.is_empty()))
                                    .unwrap_or(false);
                                if !getter_dup {
                                    entry.methods.entry(f.name.name.clone()).or_default().push(
                                        FnTy {
                                            generics: Vec::new(),
                                            generic_bounds: Vec::new(),
                                            params: vec![],
                                            ret: pty.clone(),
                                            is_static: false,
                                        },
                                    );
                                }
                                if f.is_mut {
                                    let setter_n = setter_name(&f.name.name);
                                    let setter_dup = user_declared_sigs
                                        .get(&setter_n)
                                        .map(|sigs| {
                                            sigs.iter().any(|s| s.len() == 1 && s[0] == pty)
                                        })
                                        .unwrap_or(false);
                                    if !setter_dup {
                                        entry.methods.entry(setter_n.clone()).or_default().push(
                                            FnTy {
                                                generics: Vec::new(),
                                                generic_bounds: Vec::new(),
                                                params: vec![pty],
                                                ret: Type::void(),
                                                is_static: false,
                                            },
                                        );
                                    }
                                    entry.member_pub.insert(setter_n, true);
                                }
                            }
                        }
                        ClassMember::Init(i) => {
                            // Build a FnTy from the init's params; ret
                            // is the class itself (T.new(args) -> T).
                            let params = i
                                .params
                                .iter()
                                .map(|p| {
                                    substitute_names(
                                        &lower_type(&p.ty, program),
                                        &class_name_to_var,
                                    )
                                })
                                .collect();
                            let ret = if class_generic_vars.is_empty() {
                                Type::Class(c.name.name.clone())
                            } else {
                                Type::Generic {
                                    base: c.name.name.clone(),
                                    args: class_generic_vars
                                        .iter()
                                        .map(|v| Type::Var(*v))
                                        .collect(),
                                }
                            };
                            entry.inits.push(FnTy {
                                generics: Vec::new(),
                                generic_bounds: Vec::new(),
                                params,
                                ret,
                                is_static: false,
                            });
                        }
                        ClassMember::Deinit(_) => {
                            // No signature to register; existence is
                            // enough — codegen looks the body up
                            // directly off the AST.
                        }
                    }
                }
                t.classes.insert(c.name.name.clone(), entry);
            }
            Item::Fn(f) => {
                t.fns
                    .entry(f.name.name.clone())
                    .or_default()
                    .push(build_fn_ty(f, program, source));
            }
            Item::Impl(i) => {
                // Splice impl methods onto the target class so a
                // call like `myList.iter()` resolves through the
                // existing class-method lookup path. Method-name
                // collisions (impl supplies a method already on the
                // class) keep whichever was inserted first; the
                // checker pass should diagnose this separately.
                //
                // Look up by the for-type's simple base name. Impls
                // on parametric instantiations (`impl<T> Foo for
                // List<T>`) resolve to the base ("List"); user
                // classes are looked up directly. Impls on extern /
                // built-in bases that have no class entry (e.g.
                // `QStringList`) are skipped here — they're still
                // registered in `program.impls_for` for bound
                // satisfaction, but methods don't get spliced
                // because there's no class table row to write to.
                let base = match pixie_syntax::ast::type_expr_base_name(&i.for_type) {
                    Some(b) => b,
                    None => continue,
                };
                if let Some(class_entry) = t.classes.get_mut(&base) {
                    for m in &i.methods {
                        let fnty = build_fn_ty(m, program, source);
                        // Push as an additional overload of `m.name`.
                        // True duplicates (same arity + same param types)
                        // are caught by the HIR `fn_overload_coherence_check`
                        // pass; here we just register the candidate.
                        class_entry
                            .methods
                            .entry(m.name.name.clone())
                            .or_default()
                            .push(fnty);
                        // Mirror the per-member visibility info so
                        // `recv.method()` from outside the class
                        // respects the `pub` marker on the impl
                        // method's declaration. Without this the
                        // visibility check rejects every impl method
                        // as private.
                        class_entry
                            .member_pub
                            .entry(m.name.name.clone())
                            .or_insert(m.is_pub);
                    }
                }
            }
            Item::Struct(s) => {
                let fields = s
                    .fields
                    .iter()
                    .map(|f| (f.name.name.clone(), lower_type(&f.ty, program)))
                    .collect();
                let mut methods: HashMap<String, Vec<FnTy>> = HashMap::new();
                for m in &s.methods {
                    let fnty = build_fn_ty(m, program, source);
                    methods.entry(m.name.name.clone()).or_default().push(fnty);
                }
                t.structs
                    .insert(s.name.name.clone(), StructEntry { fields, methods });
            }
            Item::Enum(e) => {
                // Every non-extern enum is also registered as an
                // "error" entry so its variant constructors look up
                // the same way through the type checker
                // (`E.variant(args)`) regardless of whether it was
                // declared with `error` or `enum`. Extern enums skip
                // this — they have no Cute-side bodies and lower to
                // bare C++ enum names, not std::variant tagged unions.
                if !e.is_extern {
                    let mut entry = ErrorEntry::default();
                    for v in &e.variants {
                        let fields = v
                            .fields
                            .iter()
                            .map(|f| lower_type(&f.ty, program))
                            .collect();
                        entry.variants.insert(v.name.name.clone(), fields);
                    }
                    t.errors.insert(e.name.name.clone(), entry);
                }
            }
            Item::Trait(_)
            | Item::Use(_)
            | Item::UseQml(_)
            | Item::View(_)
            | Item::Widget(_)
            | Item::Style(_)
            | Item::Let(_)
            | Item::Flags(_) => {
                // Top-level lets / enums / flags are tracked in
                // `prog.items` (ItemKind::Let / Enum / Flags). The
                // type checker resolves bare `Ident(X)` lookups
                // through that registry — ProgramTable doesn't need
                // its own entry. Variant resolution (`Color.Red`)
                // also goes through the program-side enum table.
            }
            Item::Store(_) => unreachable!(
                "Item::Store should be lowered before the type table builds; \
                 see cute_codegen::desugar_store",
            ),
            Item::Suite(_) => unreachable!(
                "Item::Suite should be flattened before the type table builds; \
                 see cute_codegen::desugar_suite",
            ),
        }
    }
    t
}

impl ProgramTable {
    /// Walk the class chain to find a method's overload set. Returns the
    /// `Vec<FnTy>` for the first ancestor that declares any overload of
    /// `method`. Falls through to struct methods when the receiver
    /// name resolves to a `struct` rather than a `class`. Empty slice
    /// = not found at all (callers should treat as soft-pass through
    /// the foreign chain).
    pub fn lookup_method_overloads(&self, class_name: &str, method: &str) -> &[FnTy] {
        let mut current = class_name.to_string();
        loop {
            if let Some(entry) = self.classes.get(&current) {
                if let Some(v) = entry.methods.get(method) {
                    if !v.is_empty() {
                        return v.as_slice();
                    }
                }
                let Some(parent) = entry.super_class.as_ref() else {
                    return &[];
                };
                current = parent.clone();
                continue;
            }
            // Not a class — try as a struct.
            if let Some(s) = self.structs.get(&current) {
                if let Some(v) = s.methods.get(method) {
                    if !v.is_empty() {
                        return v.as_slice();
                    }
                }
            }
            return &[];
        }
    }

    /// Backward-compatible single-result lookup: returns the **first**
    /// overload found by `lookup_method_overloads`. Useful for best-effort
    /// inference paths (`synth_no_check`'s receiver-typing) and for code
    /// that only consumes the return type. Real call-site dispatch goes
    /// through `lookup_method_overloads` + `resolve_overload`.
    pub fn lookup_method(&self, class_name: &str, method: &str) -> Option<&FnTy> {
        self.lookup_method_overloads(class_name, method).first()
    }

    /// Same chain walk for properties. Falls through to the structs
    /// map when the receiver name isn't a class — `Point.x` on a
    /// `struct Point { x: Int }` resolves through the field list.
    pub fn lookup_property(&self, class_name: &str, prop: &str) -> Option<&Type> {
        let mut current = class_name.to_string();
        loop {
            if let Some(entry) = self.classes.get(&current) {
                if let Some(t) = entry.properties.get(prop) {
                    return Some(t);
                }
                current = entry.super_class.as_ref()?.clone();
                continue;
            }
            // Not a class — try as a struct.
            if let Some(s) = self.structs.get(&current) {
                return s.fields.iter().find(|(n, _)| n == prop).map(|(_, t)| t);
            }
            return None;
        }
    }

    /// Look up a property on a generic-instantiated class. The class
    /// must be in the table; its generic params get substituted with
    /// `args` before the property type is returned. For
    /// `class Box<T> { property item: T }` and a receiver of type
    /// `Box<Int>`, this returns `Int` instead of the unsubstituted
    /// `Type::Var(...)`.
    pub fn lookup_property_instantiated(
        &self,
        base: &str,
        args: &[Type],
        prop: &str,
    ) -> Option<Type> {
        let entry = self.classes.get(base)?;
        let raw = entry.properties.get(prop)?.clone();
        Some(self.instantiate_in(entry, args, &raw))
    }

    /// Like `lookup_property_instantiated` but for methods. Returns
    /// every overload of `method` on `base` with class generics
    /// substituted out by `args`. Empty vec = method not declared.
    pub fn lookup_method_instantiated_overloads(
        &self,
        base: &str,
        args: &[Type],
        method: &str,
    ) -> Vec<FnTy> {
        let Some(entry) = self.classes.get(base) else {
            return Vec::new();
        };
        let Some(raws) = entry.methods.get(method) else {
            return Vec::new();
        };
        raws.iter()
            .map(|raw| {
                let params = raw
                    .params
                    .iter()
                    .map(|p| self.instantiate_in(entry, args, p))
                    .collect();
                let ret = self.instantiate_in(entry, args, &raw.ret);
                FnTy {
                    generics: raw.generics.clone(),
                    generic_bounds: raw.generic_bounds.clone(),
                    params,
                    ret,
                    is_static: raw.is_static,
                }
            })
            .collect()
    }

    /// Backward-compatible single-result instantiated lookup. Returns
    /// the first overload (or None) — same role as `lookup_method` for
    /// best-effort inference. Real dispatch uses
    /// `lookup_method_instantiated_overloads` + `resolve_overload`.
    pub fn lookup_method_instantiated(
        &self,
        base: &str,
        args: &[Type],
        method: &str,
    ) -> Option<FnTy> {
        self.lookup_method_instantiated_overloads(base, args, method)
            .into_iter()
            .next()
    }

    /// Substitute the class's generic-param `VarId`s with the given
    /// concrete `args` inside `t`. When `args.len() != entry.class_generics.len()`
    /// (mismatch), substitution is skipped and the raw type is
    /// returned as-is - the diagnostic surfaces from elsewhere.
    fn instantiate_in(&self, entry: &ClassEntry, args: &[Type], t: &Type) -> Type {
        if entry.class_generics.len() != args.len() {
            return t.clone();
        }
        let map: HashMap<crate::ty::VarId, Type> = entry
            .class_generics
            .iter()
            .zip(args.iter())
            .map(|(v, a)| (*v, a.clone()))
            .collect();
        substitute_vars(t, &map)
    }

    /// Same chain walk for signals. Returns `(declaring_class, params)`
    /// so callers can carry the **declaring** class name (the one whose
    /// `signals` table actually held the entry) instead of the receiver
    /// class — keeps `Type::SignalRef` equality stable across subclass
    /// receivers.
    pub fn lookup_signal(&self, class_name: &str, signal: &str) -> Option<(String, &Vec<Type>)> {
        let mut current = class_name.to_string();
        loop {
            let entry = self.classes.get(&current)?;
            if let Some(v) = entry.signals.get(signal) {
                return Some((current, v));
            }
            current = entry.super_class.as_ref()?.clone();
        }
    }
}

/// Outcome of `resolve_overload` over an overload set + actual arg types.
/// `Unique` is the happy path; the other variants carry enough context
/// for callers to render a candidate-set diagnostic.
#[derive(Debug)]
pub enum OverloadResolution<'a> {
    Unique(&'a FnTy),
    NoArityMatch { arities: Vec<usize> },
    NoTypeMatch { tier3_winners: Vec<&'a FnTy> },
    Ambiguous { winners: Vec<&'a FnTy> },
    Empty,
}

impl<'a> OverloadResolution<'a> {
    /// Candidates to render in a "no overload matches" / "ambiguous"
    /// diagnostic. Returns the slice of viable FnTys for `NoTypeMatch`
    /// and `Ambiguous`; empty for the other variants. Lets call sites
    /// share the same rendering helper without per-variant
    /// `iter().copied().collect()` reshapes.
    pub fn candidates(&self) -> &[&'a FnTy] {
        match self {
            OverloadResolution::NoTypeMatch { tier3_winners } => tier3_winners.as_slice(),
            OverloadResolution::Ambiguous { winners } => winners.as_slice(),
            _ => &[],
        }
    }
}

/// Resolve an overload set against actual argument types.
///
/// Four-tier search:
/// 1. **Arity** — `c.params.len() == args.len()` OR (`block_present` AND
///    `c.params.len() == args.len() + 1` AND last param is `Type::Fn`).
///    The trailing-block convention (`f(a) { lambda }` desugars to
///    `f(a, lambda)`) is per-candidate.
/// 2. **Exact type** — pair-wise structural equality on the *positional*
///    args (block slot is accepted softly — its type is refined by
///    downstream bidirectional checking).
/// 3. **Subtype** — `is_subtype(arg, param)` on positional args.
/// 4. **Most-specific** — strict-min on `specificity_score`. Concrete
///    beats generic-var; exact beats widened.
///
/// **External soft-pass tiebreak:** `is_subtype` returns true for any
/// pair involving `External(_)`, so a foreign-typed arg matches every
/// Tier 3 candidate. When Tier 4 ties on External-driven scores, we
/// pick **first-declared** for determinism — without this, every Qt
/// binding overload like `KLocalizedString.toString` would be
/// flagged ambiguous on calls from Cute code.
pub fn resolve_overload<'a>(
    candidates: &'a [FnTy],
    args: &[Type],
    block_present: bool,
    program: &ResolvedProgram,
) -> OverloadResolution<'a> {
    use crate::ty::is_subtype;
    if candidates.is_empty() {
        return OverloadResolution::Empty;
    }

    // Tier 1: arity. Per-candidate: positional-only OR positional + block.
    let by_arity: Vec<&FnTy> = candidates
        .iter()
        .filter(|c| candidate_arity_matches(c, args.len(), block_present))
        .collect();
    if by_arity.is_empty() {
        let arities: Vec<usize> = candidates.iter().map(|c| c.params.len()).collect();
        return OverloadResolution::NoArityMatch { arities };
    }
    if by_arity.len() == 1 {
        return OverloadResolution::Unique(by_arity[0]);
    }

    // Tier 2: exact type match on positional args.
    let exact: Vec<&FnTy> = by_arity
        .iter()
        .copied()
        .filter(|c| {
            positional_params(c, args.len())
                .iter()
                .zip(args)
                .all(|(p, a)| p == a)
        })
        .collect();
    if exact.len() == 1 {
        return OverloadResolution::Unique(exact[0]);
    }
    if exact.len() > 1 {
        // Coherence pass should reject this at HIR time. Surfaces here
        // only if coherence missed it (e.g. binding-author bug).
        return OverloadResolution::Ambiguous { winners: exact };
    }

    // Tier 3: subtype-compatible on positional args.
    let compatible: Vec<&FnTy> = by_arity
        .iter()
        .copied()
        .filter(|c| {
            positional_params(c, args.len())
                .iter()
                .zip(args)
                .all(|(p, a)| is_subtype(a, p, program))
        })
        .collect();
    if compatible.is_empty() {
        return OverloadResolution::NoTypeMatch {
            tier3_winners: by_arity,
        };
    }
    if compatible.len() == 1 {
        return OverloadResolution::Unique(compatible[0]);
    }

    // Tier 4: most-specific (strict-min on specificity_score).
    let scores: Vec<(usize, &FnTy)> = compatible
        .iter()
        .map(|c| {
            (
                specificity_score(positional_params(c, args.len()), args),
                *c,
            )
        })
        .collect();
    let min = scores.iter().map(|(s, _)| *s).min().unwrap();
    let winners: Vec<&FnTy> = scores
        .iter()
        .filter(|(s, _)| *s == min)
        .map(|(_, c)| *c)
        .collect();
    if winners.len() == 1 {
        return OverloadResolution::Unique(winners[0]);
    }
    // External soft-pass fallback: if all tied winners reached this tier
    // via External-touching params, pick the first-declared one.
    let all_external_touched = winners.iter().all(|c| {
        positional_params(c, args.len())
            .iter()
            .zip(args)
            .any(|(p, a)| matches!(p, Type::External(_)) || matches!(a, Type::External(_)))
    });
    if all_external_touched {
        if let Some(first) = candidates
            .iter()
            .find(|c| winners.iter().any(|w| std::ptr::eq(*w, *c)))
        {
            return OverloadResolution::Unique(first);
        }
    }
    OverloadResolution::Ambiguous { winners }
}

fn candidate_arity_matches(c: &FnTy, n_args: usize, block_present: bool) -> bool {
    if c.params.len() == n_args {
        return true;
    }
    // Trailing-block convention: `f(args) { lambda }` counts the block
    // as the (n+1)th positional arg, regardless of the last param's
    // type. Bindings that type the slot as `External("Handler")` (or
    // any non-Fn alias — e.g. `stdlib/qt/qhttpserver.qpi`'s
    // `route(path: String, handler: Handler)`) still accept the block.
    if block_present && c.params.len() == n_args + 1 {
        return true;
    }
    false
}

/// Slice of candidate's params restricted to the positional-arg slots
/// (drops the trailing block slot when block-arity expansion fired).
fn positional_params(c: &FnTy, n_args: usize) -> &[Type] {
    if c.params.len() > n_args {
        &c.params[..n_args]
    } else {
        &c.params[..]
    }
}

/// Per-candidate specificity score. Lower = more specific. Sum across all
/// argument positions, then strict-min picks the winner.
///
/// - `0` exact match
/// - `1` Int→Float widening
/// - `2` Nullable lift (`T <: T?`)
/// - `3` Generic-var match (least specific concrete)
/// - `4` External soft-pass (catches everything; lowest priority)
fn specificity_score(params: &[Type], args: &[Type]) -> usize {
    use Type::*;
    params
        .iter()
        .zip(args)
        .map(|(p, a)| {
            if p == a {
                0
            } else {
                match (p, a) {
                    (Prim(crate::ty::Prim::Float), Prim(crate::ty::Prim::Int)) => 1,
                    (Nullable(_), _) => 2,
                    (Var(_), _) | (_, Var(_)) => 3,
                    (External(_), _) | (_, External(_)) => 4,
                    _ => 0,
                }
            }
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixie_hir::resolve;
    use pixie_syntax::{parse, span::FileId};

    fn build_table(src: &str) -> ProgramTable {
        let m = parse(FileId(0), src).expect("parse");
        let p = resolve(&m, &pixie_hir::ProjectInfo::default()).program;
        let mut source = VarSource::default();
        build(&m, &p, &mut source)
    }

    #[test]
    fn collects_class_members_with_resolved_types() {
        let t = build_table(
            r#"
class TodoItem {
  prop text : String, default: ""
  prop done : Bool, default: false

  signal pinged(message: String, count: Int)

  fn toggle {
    doIt()
  }

  fn rename(name: String) Bool {
    true
  }
}
"#,
        );
        let c = &t.classes["TodoItem"];
        assert_eq!(c.properties["text"], Type::string());
        assert_eq!(c.properties["done"], Type::bool());
        assert_eq!(c.signals["pinged"], vec![Type::string(), Type::int()]);
        // Methods are stored as overload sets; the user wrote one
        // `toggle` and one `rename`, so each bucket has a single entry.
        assert_eq!(c.methods["toggle"].len(), 1);
        assert_eq!(c.methods["toggle"][0].params, vec![]);
        assert_eq!(c.methods["toggle"][0].ret, Type::void());
        assert_eq!(c.methods["rename"].len(), 1);
        assert_eq!(c.methods["rename"][0].params, vec![Type::string()]);
        assert_eq!(c.methods["rename"][0].ret, Type::bool());
    }

    #[test]
    fn collects_top_level_fn_signatures() {
        let t = build_table(
            r#"
fn add(a: Int, b: Int) Int {
  a + b
}
"#,
        );
        // Free fns also stored as overload sets (one entry per
        // declared signature); a single `add` here means a 1-element vec.
        assert_eq!(t.fns["add"].len(), 1);
        let f = &t.fns["add"][0];
        assert_eq!(f.params, vec![Type::int(), Type::int()]);
        assert_eq!(f.ret, Type::int());
    }

    #[test]
    fn collects_error_variants_with_field_types() {
        let t = build_table(
            r#"
error FileError {
  notFound
  ioError(message: String, code: Int)
}
"#,
        );
        let e = &t.errors["FileError"];
        assert_eq!(e.variants["notFound"], Vec::<Type>::new());
        assert_eq!(e.variants["ioError"], vec![Type::string(), Type::int()]);
    }

    #[test]
    fn method_lookup_walks_class_chain() {
        let t = build_table(
            r#"
class A {
  fn parentOnly() Bool {
    true
  }
}

class B < A {
  fn childOnly() Int {
    42
  }
}
"#,
        );
        // Direct lookup.
        let sig = t.lookup_method("B", "childOnly").unwrap();
        assert_eq!(sig.ret, Type::int());

        // Inherited lookup: B doesn't have it, walk up to A.
        let sig = t.lookup_method("B", "parentOnly").unwrap();
        assert_eq!(sig.ret, Type::bool());

        // Not found at all.
        assert!(t.lookup_method("B", "nope").is_none());
    }
}
