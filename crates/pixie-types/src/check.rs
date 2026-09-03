//! Bidirectional type checker.
//!
//! Two modes:
//!  - `synth(env, expr) -> Type`: infer the type of an expression bottom-up.
//!  - `check(env, expr, expected)`: verify the expression has a subtype of
//!    the expected type, and report a diagnostic if not.
//!
//! Statements use `synth` for RHS values, `check` for declared bindings,
//! return statements (against the surrounding fn return), property writes,
//! signal emissions, and pattern arms. Bodies are walked top-down with a
//! mutable scope chain (`TypeEnv`).
//!
//! Soft-fail policy: anything that lowers to `Type::External` (a class
//! we have no binding for) accepts arbitrary method calls, member access,
//! and arg lists. This keeps pre-binding-files programs (like our spec
//! samples that call `File.open(...)`) from drowning in errors. Once
//! `.qpi` binding files land we tighten this.

use crate::env::TypeEnv;
use crate::infer::{VarSource, instantiate, unify_or_subtype};
use crate::table::{FnTy, ProgramTable, build as build_table, substitute_names, substitute_self};
use crate::ty::{Prim, Type, VarId, is_subtype, lower_type};
use pixie_hir::{ItemKind, ResolvedProgram};
use pixie_syntax::ast::*;
use pixie_syntax::diag::Diagnostic;
use pixie_syntax::span::Span;

pub struct CheckResult {
    pub diagnostics: Vec<Diagnostic>,
    /// Side-band: when `T.new()` is type-checked against an expected
    /// `Generic{base, args}` (let / var annotation, fn parameter,
    /// method parameter, return type), record `call.span -> args`.
    /// Codegen consumes this map to emit the instantiated form
    /// `cute::Arc<T<args>>(new T<args>(...))` even when the call has
    /// no explicit `type_args` attached at the syntax layer.
    pub generic_instantiations: std::collections::HashMap<Span, Vec<crate::ty::Type>>,
    /// Side-band: operand spans the checker WIDENED from `Int` to
    /// `Float` in a mixed arithmetic expression (§8.55). The checker
    /// has always allowed `30.0 * n`, and the emitter has no types,
    /// so without this it emitted `30f64 * n` and rustc rejected the
    /// generated crate — a D10 violation. Codegen emits `as f64` at
    /// each recorded span.
    pub int_to_float: std::collections::HashSet<Span>,
}

/// Compose `module_path . name` into the namespace-mangled lookup
/// key used by binding-loaded classes (e.g. Kirigami: source has
/// `Kirigami.PageRow { ... }`, the type table has it under
/// `Kirigami_PageRow` because `apply_namespace_mangle` rewrote it
/// at load time). When `module_path` is empty, returns the leaf.
fn qualified_element_name(e: &pixie_syntax::ast::Element) -> String {
    if e.module_path.is_empty() {
        e.name.name.clone()
    } else {
        let prefix = e
            .module_path
            .iter()
            .map(|i| i.name.as_str())
            .collect::<Vec<_>>()
            .join("_");
        format!("{prefix}_{}", e.name.name)
    }
}

/// Lift `t` to `Type::Nullable(t)` unless it already is a Nullable
/// (avoids `Nullable<Nullable<T>>` for chained safe access where the
/// inner member itself returns a nullable). `Type::Error` and
/// `Type::Unknown` pass through unchanged so we don't manufacture a
/// nullable around a propagated type-checker failure.
pub(crate) fn lift_to_nullable(t: Type) -> Type {
    match t {
        Type::Nullable(_) | Type::Error | Type::Unknown => t,
        other => Type::Nullable(Box::new(other)),
    }
}

/// Extract the `id: someName` property value from a QML element if
/// present. Returns the local name `someName` for the visibility-
/// walker's env so sibling element bodies see `someName: <ClassOf>`.
fn qml_id_of(e: &pixie_syntax::ast::Element) -> Option<String> {
    for m in &e.members {
        if let pixie_syntax::ast::ElementMember::Property { key, value, .. } = m {
            if key == "id" {
                if let pixie_syntax::ast::ExprKind::Ident(n) = &value.kind {
                    return Some(n.clone());
                }
            }
        }
    }
    None
}

pub fn check_program(module: &Module, program: &ResolvedProgram) -> CheckResult {
    let mut var_source = VarSource::default();
    let table = build_table(module, program, &mut var_source);
    let mut c = Checker {
        program,
        table,
        diags: Vec::new(),
        var_source,
        current_class: None,
        generic_instantiations: std::collections::HashMap::new(),
        int_to_float: std::collections::HashSet::new(),
        current_fn_bounds: std::collections::HashMap::new(),
    };
    c.check_module(module);
    CheckResult {
        diagnostics: c.diags,
        generic_instantiations: c.generic_instantiations,
        int_to_float: c.int_to_float,
    }
}

struct Checker<'a> {
    program: &'a ResolvedProgram,
    table: ProgramTable,
    diags: Vec<Diagnostic>,
    /// Source of fresh `VarId`s for call-site generic instantiation.
    var_source: VarSource,
    /// Name of the class whose method body we're currently checking.
    /// Set by `check_method` and cleared on exit. Used by the
    /// member-visibility check: a non-pub member is freely accessible
    /// from inside its declaring class but not from anywhere else.
    current_class: Option<String>,
    /// `T.new()` calls that the checker accepted against an expected
    /// `Generic{base, args}` shape — see `CheckResult` for the rationale.
    generic_instantiations: std::collections::HashMap<Span, Vec<crate::ty::Type>>,
    /// Operand spans widened `Int` → `Float` — see `CheckResult`.
    int_to_float: std::collections::HashSet<Span>,
    /// Active generic bounds for the surrounding fn body. Maps each
    /// declaration-time `VarId` (i.e. `T` in `fn use_it<T: Foo>`) to
    /// the trait names listed as its bounds. Set on fn-body entry,
    /// restored on exit. Drives the body-side method-name check at
    /// `synth_method_call`: a `recv.method()` whose `recv: Type::Var(v)`
    /// must reference a method present on at least one of the bound
    /// traits, otherwise the call is rejected at the Cute source line
    /// instead of waiting for a C++ template instantiation error.
    current_fn_bounds: std::collections::HashMap<crate::ty::VarId, Vec<String>>,
}

impl<'a> Checker<'a> {
    // ---- module / item walks --------------------------------------------

    fn check_module(&mut self, module: &Module) {
        for item in &module.items {
            match item {
                Item::Use(_) => {}
                Item::UseQml(_) => {} // foreign QML decl, no Cute type info
                Item::Class(c) => self.check_class(c),
                Item::Struct(s) => self.check_struct(s),
                Item::Fn(f) => self.check_top_level_fn(f),
                Item::Style(s) => self.check_style(s),
                // View / widget bodies are partially walked: we synth
                // each property value and stmt so that member access
                // (`counter.count`) triggers the visibility check.
                // Element shape itself is still primarily codegen's
                // concern; this walk is just enough to make `pub` /
                // private on class members observable in views.
                Item::View(v) => self.check_view(v),
                Item::Widget(w) => self.check_widget(w),
                Item::Trait(_) => {
                    // Trait method signatures don't have bodies to type-
                    // check; argument/return types are vetted at the
                    // visibility-check pass in cute-hir.
                }
                Item::Impl(i) => {
                    // An impl method's body runs ON the for-type: when
                    // that is a user class in this module, bare prop
                    // reads must type against the class (checking them
                    // as top-level fns left `r * 2.0` unresolvable —
                    // string interpolation masked the gap because it
                    // renders anything). Extern values, builtins and
                    // generic bases keep the top-level path.
                    let for_class = match &i.for_type.kind {
                        TypeKind::Named { path, args }
                            if path.len() == 1 && args.is_empty() =>
                        {
                            module.items.iter().find_map(|it| match it {
                                Item::Class(c) if c.name.name == path[0].name => {
                                    Some(c)
                                }
                                _ => None,
                            })
                        }
                        _ => None,
                    };
                    for m in &i.methods {
                        match for_class {
                            Some(c) => self.check_method(c, m),
                            None => self.check_top_level_fn(m),
                        }
                    }
                }
                Item::Let(l) => self.check_top_level_let(l),
                Item::Enum(_) | Item::Flags(_) => {
                    // Enum / flags decl bodies don't have runtime
                    // expressions to walk in the type-check pass —
                    // variant value Exprs (when present) are
                    // resolved by the table builder, not here.
                }
                Item::Store(_) => unreachable!(
                    "Item::Store should be lowered before type-check; see \
                     cute_codegen::desugar_store",
                ),
                Item::Suite(_) => unreachable!(
                    "Item::Suite should be flattened before type-check; see \
                     cute_codegen::desugar_suite",
                ),
            }
        }
    }

    fn check_top_level_let(&mut self, l: &pixie_syntax::ast::LetDecl) {
        // Verify the value's inferred type is compatible with the
        // declared type. Top-level lets always carry an explicit type
        // annotation (the parser requires it), so we have a `want`
        // immediately.
        let want = lower_type(&l.ty, self.program);
        let mut env = TypeEnv::root();
        self.check(&mut env, &l.value, &want);
    }

    fn check_class(&mut self, class: &ClassDecl) {
        // A method sharing a name with a generated property accessor
        // (the getter `x`, `set_x`, `push_x` on a list, `insert_x` on
        // a map) would land in the emitted Ref trait twice — a rustc
        // error inside generated code, which D10 forbids. Name the
        // collision here instead.
        let mut reserved: Vec<(String, String)> = Vec::new();
        for member in &class.members {
            let (name, ty, assignable) = match member {
                ClassMember::Property(p) => (
                    p.name.name.clone(),
                    &p.ty,
                    p.binding.is_none() && p.fresh.is_none(),
                ),
                ClassMember::Field(f) => (f.name.name.clone(), &f.ty, f.is_mut),
                _ => continue,
            };
            reserved.push((name.clone(), name.clone()));
            if assignable {
                reserved.push((format!("set_{name}"), name.clone()));
                if let TypeKind::Named { path, .. } = &ty.kind {
                    if path.len() == 1 {
                        match path[0].name.as_str() {
                            "List" => reserved.push((format!("push_{name}"), name.clone())),
                            "Map" => reserved.push((format!("insert_{name}"), name.clone())),
                            _ => {}
                        }
                    }
                }
            }
        }
        for member in &class.members {
            if let ClassMember::Fn(f) | ClassMember::Slot(f) = member {
                if let Some((_, prop)) = reserved.iter().find(|(r, _)| *r == f.name.name) {
                    self.diags.push(Diagnostic::error(
                        f.name.span,
                        format!(
                            "method `{}` collides with an accessor generated for property `{prop}` — rename the method (`{prop}`, `set_{prop}`, and `push_`/`insert_{prop}` on list/map properties are reserved)",
                            f.name.name
                        ),
                    ));
                }
            }
        }
        // Type each property/field default (if any) against the declared type.
        for member in &class.members {
            match member {
                ClassMember::Property(p) => {
                    if let Some(default) = &p.default {
                        let want = lower_type(&p.ty, self.program);
                        let mut env = TypeEnv::root();
                        self.check(&mut env, default, &want);
                    }
                    // A derived property's body (§8.61) is checked the
                    // way a method body is: it resolves the class's own
                    // members, and its value has to be the declared
                    // type. Leaving it unchecked would send a type
                    // error straight to rustc (D10) — and the Int-to-
                    // Float widening is recorded during `check`, so an
                    // unvisited body would also miss its casts.
                    if let Some(b) = &p.binding {
                        let want = lower_type(&p.ty, self.program);
                        let block = Block {
                            stmts: Vec::new(),
                            trailing: Some(Box::new(b.clone())),
                            span: b.span,
                        };
                        self.check_class_body(class, &[], &block, &want);
                    }
                }
                ClassMember::Field(f) => {
                    if let Some(default) = &f.default {
                        let want = lower_type(&f.ty, self.program);
                        let mut env = TypeEnv::root();
                        self.check(&mut env, default, &want);
                    }
                    self.check_weak_unowned(
                        class,
                        &f.ty,
                        f.weak,
                        f.unowned,
                        f.span,
                        if f.is_mut { "var" } else { "let" },
                        &f.name.name,
                    );
                }
                ClassMember::Fn(f) | ClassMember::Slot(f) => {
                    self.check_method(class, f);
                }
                ClassMember::Signal(_) => {}
                ClassMember::Init(i) => self.check_init(class, i),
                ClassMember::Deinit(d) => self.check_deinit(class, d),
            }
        }
    }

    /// pixie removed `weak` / `unowned` along with the Qt ownership
    /// model: class handles are World-backed and generation-checked, so
    /// `x : T?` already reads as nil once the target is removed, and a
    /// plain handle cannot dangle (DESIGN section 3).
    fn check_weak_unowned(
        &mut self,
        _class: &ClassDecl,
        _ty: &TypeExpr,
        weak: bool,
        unowned: bool,
        span: Span,
        kw: &str,
        name: &str,
    ) {
        if !weak && !unowned {
            return;
        }
        let modifier = if weak { "weak" } else { "unowned" };
        self.diags.push(Diagnostic::error(
            span,
            format!(
                "`{modifier}` was removed in pixie: handles are stale-aware, so `{kw} {name} : T?` already reads as nil once its target is removed, and a plain handle cannot dangle"
            ),
        ));
    }

    fn check_init(&mut self, class: &ClassDecl, init: &pixie_syntax::ast::InitDecl) {
        self.check_class_body(class, &init.params, &init.body, &Type::void());
    }

    fn check_deinit(&mut self, class: &ClassDecl, d: &pixie_syntax::ast::DeinitDecl) {
        self.check_class_body(class, &[], &d.body, &Type::void());
    }

    /// Shared method-body checker for fn / slot / init / deinit. Builds
    /// the `self` + `@field` + param env (with generic-name → VarId
    /// substitution against the class's stored generics, so `T`
    /// references resolve to the same `Type::Var` instances the
    /// `ProgramTable` uses for member storage), swaps in
    /// `current_class` for visibility checks, then walks the body.
    fn check_class_body(
        &mut self,
        class: &ClassDecl,
        params: &[pixie_syntax::ast::Param],
        body: &pixie_syntax::ast::Block,
        ret_ty: &Type,
    ) {
        let class_var_ids: Vec<crate::ty::VarId> = self
            .table
            .classes
            .get(&class.name.name)
            .map(|e| e.class_generics.clone())
            .unwrap_or_default();
        let name_to_var: std::collections::HashMap<String, crate::ty::VarId> = class
            .generics
            .iter()
            .zip(class_var_ids.iter())
            .map(|(g, v)| (g.name.name.clone(), *v))
            .collect();
        let self_ty = if class_var_ids.is_empty() {
            Type::Class(class.name.name.clone())
        } else {
            Type::Generic {
                base: class.name.name.clone(),
                args: class_var_ids.iter().map(|v| Type::Var(*v)).collect(),
            }
        };
        let mut env = TypeEnv::root();
        env.bind("self", self_ty);
        for member in &class.members {
            match member {
                ClassMember::Property(p) => {
                    let raw = lower_type(&p.ty, self.program);
                    let pty = crate::table::substitute_names(&raw, &name_to_var);
                    env.bind(format!("@{}", p.name.name), pty);
                }
                ClassMember::Field(f) => {
                    let raw = lower_type(&f.ty, self.program);
                    let pty = crate::table::substitute_names(&raw, &name_to_var);
                    env.bind(format!("@{}", f.name.name), pty);
                }
                _ => {}
            }
        }
        for p in params {
            let raw = lower_type(&p.ty, self.program);
            let pty = crate::table::substitute_names(&raw, &name_to_var);
            env.bind(p.name.name.clone(), pty);
        }
        let prev = self.current_class.replace(class.name.name.clone());
        self.check_block(&mut env, body, ret_ty);
        self.current_class = prev;
    }

    /// Walk a `style X { ... }` (or `style X = A + B`) declaration
    /// and synth every entry value so that internal type errors
    /// (e.g. `font.bold: 1 + "x"`) surface here. We do NOT check
    /// each value against the eventual target's property type -
    /// styles are reusable across multiple classes, so the target
    /// type is only known at the element site (where the desugar
    /// pass inlines the entry as a regular property and the
    /// element-property check kicks in).
    fn check_style(&mut self, s: &pixie_syntax::ast::StyleDecl) {
        use pixie_syntax::ast::StyleBody;
        // Alias bodies (`style X = A + B`) are codegen-time merges,
        // not runtime Adds. The `+` operator there is overloaded as
        // a style-table operator handled by `cute_codegen::style`,
        // so we deliberately skip type-checking them. Cycle and
        // unknown-reference detection happen there too.
        if let StyleBody::Lit(entries) = &s.body {
            let mut env = TypeEnv::root();
            for e in entries {
                let synth_ty = self.synth(&mut env, &e.value);
                self.check_qss_shorthand_value(&e.key, &synth_ty, e.value.span);
            }
        }
    }

    /// If `key` is part of the QSS shorthand vocabulary (per
    /// `pixie_types::qss::shape_for`), validate that the value's
    /// inferred type fits the shape Cute is going to lower it as.
    /// Color / Str / Align want a `String`; Length / Numeric accept
    /// `Int`, `Float`, or `String`. Mismatches emit a typed error
    /// at the value's span so `cute check` rejects them up-front
    /// instead of silently emitting `border-radius: abc;` which Qt
    /// would just discard at runtime.
    fn check_qss_shorthand_value(&mut self, key: &str, value_ty: &Type, span: Span) {
        use crate::qss::{QssValueShape, shape_for};
        let Some(shape) = shape_for(key) else {
            return;
        };
        // Bail if synth already errored or we have no information —
        // piling on diagnostics with `Type::Error` is just noise.
        if matches!(value_ty, Type::Error | Type::Unknown) {
            return;
        }
        let ok = match shape {
            QssValueShape::Length | QssValueShape::Numeric => matches!(
                value_ty,
                Type::Prim(Prim::Int) | Type::Prim(Prim::Float) | Type::Prim(Prim::String)
            ),
            QssValueShape::Color | QssValueShape::Str | QssValueShape::Align => {
                matches!(value_ty, Type::Prim(Prim::String))
            }
        };
        if !ok {
            self.diags.push(Diagnostic::error(
                span,
                format!(
                    "QSS shorthand `{}` expects {} (got `{}`)",
                    key,
                    shape.render_expected(),
                    value_ty.render(),
                ),
            ));
        }
    }

    fn check_view(&mut self, v: &ViewDecl) {
        let mut env = TypeEnv::root();
        for p in &v.params {
            env.bind(p.name.name.clone(), lower_type(&p.ty, self.program));
        }
        for sf in &v.state_fields {
            // Full synth on state-field initializers - the relaxed
            // Add rule (String + any-printable -> String) means
            // common patterns like `let label = "count: " + n` no
            // longer trip the strict same-type rule, and member
            // access through the binding gets accurate types.
            let ty = self.synth(&mut env, &sf.init_expr);
            env.bind(sf.name.name.clone(), ty);
        }
        self.walk_element(&mut env, &v.root);
    }

    fn check_widget(&mut self, w: &WidgetDecl) {
        let mut env = TypeEnv::root();
        for p in &w.params {
            env.bind(p.name.name.clone(), lower_type(&p.ty, self.program));
        }
        for sf in &w.state_fields {
            let ty = self.synth(&mut env, &sf.init_expr);
            env.bind(sf.name.name.clone(), ty);
        }
        self.walk_element(&mut env, &w.root);
    }

    /// Walk a view / widget element tree, type-checking property
    /// value expressions and recursing into children. Property
    /// values go through the full `synth` path so member-access
    /// visibility, generic-fn instantiation, and class-method
    /// resolution all fire; the relaxed `Add` rule means common
    /// patterns like `text: "x: " + count` type-check cleanly.
    /// The `id: someName` property binds `someName: <Class>` into
    /// the env on a first pass so sibling elements can reference
    /// the id without forward-declaration concerns.
    fn walk_element(&mut self, env: &mut TypeEnv<'_>, e: &pixie_syntax::ast::Element) {
        // First pass: register every child element's id binding so
        // sibling element bodies can resolve `id.X`.
        for m in &e.members {
            if let pixie_syntax::ast::ElementMember::Child(child) = m {
                if let Some(id_name) = qml_id_of(child) {
                    if let Some(class_ty) = self.element_class_type(child) {
                        env.bind(id_name, class_ty);
                    }
                }
            }
        }
        // Second pass: type-check each member. For Property, we
        // synth the value and (when the parent's class declares the
        // property's type) check it against that target type so
        // `Label { text: 42 }` errors instead of silently coercing.
        // Look up the qualified name first so `Kirigami.Page { ... }`
        // resolves to `Kirigami_Page` rather than the bare `Page`.
        let qualified = qualified_element_name(e);
        let parent_class = if self.table.classes.contains_key(&qualified) {
            Some(qualified)
        } else if self.table.classes.contains_key(&e.name.name) {
            Some(e.name.name.clone())
        } else {
            None
        };
        for m in &e.members {
            match m {
                pixie_syntax::ast::ElementMember::Property { key, value, .. } => {
                    self.check_element_property(
                        env,
                        parent_class.as_deref(),
                        &e.name.name,
                        key,
                        value,
                    );
                }
                pixie_syntax::ast::ElementMember::Child(c) => {
                    self.walk_element(env, c);
                }
                pixie_syntax::ast::ElementMember::Stmt(s) => {
                    self.check_stmt(env, s);
                }
            }
        }
    }

    /// Type-check a single `key: value` element member. When `key`
    /// matches a declared property of `parent_class`, check the
    /// value against that target type (so a property typed as
    /// `String` rejects an `Int` literal). Special-cased keys:
    ///   - `id` is a QML-only binding, value must be an identifier.
    ///   - `style` is consumed by the codegen desugar pass; values
    ///     are reduced against the project's style table, so we
    ///     skip arithmetic-style synth here.
    /// Unknown keys (or keys on external/unknown parent classes)
    /// fall back to `synth` so member access still fires the
    /// visibility check, but don't enforce a target type.
    fn check_element_property(
        &mut self,
        env: &mut TypeEnv<'_>,
        parent_class: Option<&str>,
        // The element's own name, for the handful of handler keys
        // whose implicit argument depends on WHICH widget carries
        // them (`onChange` is a Float on Slider / NumberField and an
        // Int on IntField).
        element: &str,
        key: &str,
        value: &Expr,
    ) {
        if key == "id" || key == "style" {
            // Don't synth - id is a QML-side binding (already
            // captured in the env-pre-pass), style is a codegen
            // desugar input.
            return;
        }
        // Text-widget handlers bind an implicit `text : String`
        // argument (the new / current content) in the handler body.
        if key == "onTextChanged" || key == "onSubmitted" {
            let mut child = env.child();
            child.bind("text", Type::Prim(Prim::String));
            let _ = self.synth(&mut child, value);
            return;
        }
        // Toggle handlers (Checkbox / Switch `onToggle:`) bind the
        // NEW value as an implicit `checked : Bool` the same way.
        if key == "onToggle" {
            let mut child = env.child();
            child.bind("checked", Type::Prim(Prim::Bool));
            let _ = self.synth(&mut child, value);
            return;
        }
        // The value controls' `onChange:` binds an implicit `value`
        // argument (the new value) the same way — a Float on the
        // Slider and the NumberField, an Int on the IntField, which
        // is the whole point of having two number fields.
        if key == "onChange" {
            let mut child = env.child();
            let prim = if element == "IntField" {
                Prim::Int
            } else {
                Prim::Float
            };
            child.bind("value", Type::Prim(prim));
            let _ = self.synth(&mut child, value);
            return;
        }
        // Chooser handlers (Select / RadioGroup / TabBar `onSelect:`)
        // bind an implicit `index : Int` — the chosen 0-based index.
        if key == "onSelect" {
            let mut child = env.child();
            child.bind("index", Type::Prim(Prim::Int));
            let _ = self.synth(&mut child, value);
            return;
        }
        // Signal handler keys (`onClicked: <expr>`) carry a void-
        // returning lambda body; we just synth and ignore the
        // result type.
        if key.starts_with("on") && key.len() > 2 {
            let _ = self.synth(env, value);
            return;
        }
        // QSS shorthand keys (`color`, `borderRadius`, `hover.X`,
        // ...) target Qt classes that aren't in the project type
        // table, so the parent-class lookup below would always miss.
        // Validate the value's shape directly against the shorthand
        // vocabulary so wrong-typed literals are caught up-front
        // instead of flowing into a malformed QSS string.
        if crate::qss::shape_for(key).is_some() {
            let synth_ty = self.synth(env, value);
            self.check_qss_shorthand_value(key, &synth_ty, value.span);
            return;
        }
        // Resolve the property type via the parent's ClassEntry.
        // Dotted keys like `font.bold` skip the check - they target
        // a sub-object Qt knows about but we don't model.
        let target_ty = if !key.contains('.') {
            parent_class.and_then(|cn| self.table.lookup_property(cn, key).cloned())
        } else {
            None
        };
        match target_ty {
            Some(want) => {
                self.check(env, value, &want);
            }
            None => {
                // No target type - just synth so member access fires.
                let _ = self.synth(env, value);
            }
        }
    }

    /// Build a `Type::Class` for an element's head, if its name
    /// matches a class declared in this project's table. Used to
    /// register QML `id:` bindings into the local env so `id.X`
    /// member access through the view body type-checks against the
    /// declaring class.
    fn element_class_type(&self, e: &pixie_syntax::ast::Element) -> Option<Type> {
        let qualified = qualified_element_name(e);
        if self.table.classes.contains_key(&qualified) {
            return Some(Type::Class(qualified));
        }
        let name = &e.name.name;
        if self.table.classes.contains_key(name) {
            Some(Type::Class(name.clone()))
        } else {
            None
        }
    }

    #[allow(dead_code)]
    /// Like a pared-down `synth`: traverse the expression tree, and
    /// at every member-access site (`obj.member` and `obj.method(...)`),
    /// resolve the receiver's type and run `check_member_pub`.
    /// Other expression shapes recurse into their sub-expressions
    /// but do NOT trigger type rules. NOTE: superseded by full
    /// `synth` in view bodies once `Add` was relaxed; kept around
    /// in case future codegen-only lowerings need a non-strict
    /// walk.
    fn walk_expr_for_visibility(&mut self, env: &TypeEnv<'_>, e: &Expr) {
        use ExprKind as K;
        match &e.kind {
            K::Member { receiver, name } | K::SafeMember { receiver, name } => {
                let recv_ty = self.synth_no_check(env, receiver);
                let unwrapped = match recv_ty {
                    Type::Nullable(inner) => *inner,
                    other => other,
                };
                if let Type::Class(class_name) = unwrapped {
                    self.check_member_pub(&class_name, &name.name, name.span);
                }
                self.walk_expr_for_visibility(env, receiver);
            }
            K::MethodCall {
                receiver,
                method,
                args,
                block,
                ..
            }
            | K::SafeMethodCall {
                receiver,
                method,
                args,
                block,
                ..
            } => {
                let recv_ty = self.synth_no_check(env, receiver);
                let unwrapped = match recv_ty {
                    Type::Nullable(inner) => *inner,
                    other => other,
                };
                if let Type::Class(class_name) = unwrapped {
                    if method.name != "new" {
                        self.check_member_pub(&class_name, &method.name, method.span);
                    }
                }
                self.walk_expr_for_visibility(env, receiver);
                for a in args {
                    self.walk_expr_for_visibility(env, a);
                }
                if let Some(b) = block {
                    self.walk_expr_for_visibility(env, b);
                }
            }
            K::Call {
                callee,
                args,
                block,
                ..
            } => {
                self.walk_expr_for_visibility(env, callee);
                for a in args {
                    self.walk_expr_for_visibility(env, a);
                }
                if let Some(b) = block {
                    self.walk_expr_for_visibility(env, b);
                }
            }
            K::Index { receiver, index } => {
                self.walk_expr_for_visibility(env, receiver);
                self.walk_expr_for_visibility(env, index);
            }
            K::Binary { lhs, rhs, .. } => {
                self.walk_expr_for_visibility(env, lhs);
                self.walk_expr_for_visibility(env, rhs);
            }
            K::Unary { expr, .. } | K::Try(expr) | K::Await(expr) => {
                self.walk_expr_for_visibility(env, expr)
            }
            K::If {
                cond,
                then_b,
                else_b,
                ..
            } => {
                self.walk_expr_for_visibility(env, cond);
                self.walk_block_for_visibility(env, then_b);
                if let Some(eb) = else_b {
                    self.walk_block_for_visibility(env, eb);
                }
            }
            K::Case { scrutinee, arms } => {
                self.walk_expr_for_visibility(env, scrutinee);
                for arm in arms {
                    self.walk_block_for_visibility(env, &arm.body);
                }
            }
            K::Block(b) => self.walk_block_for_visibility(env, b),
            K::Lambda { body, .. } => self.walk_block_for_visibility(env, body),
            K::Element(el) => self.walk_element(&mut env.child(), el),
            K::Array(items) => {
                for it in items {
                    self.walk_expr_for_visibility(env, it);
                }
            }
            K::Map(entries) => {
                for (k, v) in entries {
                    self.walk_expr_for_visibility(env, k);
                    self.walk_expr_for_visibility(env, v);
                }
            }
            K::Range { start, end, .. } => {
                self.walk_expr_for_visibility(env, start);
                self.walk_expr_for_visibility(env, end);
            }
            K::Kwarg { value, .. } => self.walk_expr_for_visibility(env, value),
            K::Str(parts) => {
                for p in parts {
                    match p {
                        pixie_syntax::ast::StrPart::Interp(inner) => {
                            self.walk_expr_for_visibility(env, inner);
                        }
                        pixie_syntax::ast::StrPart::InterpFmt { expr, .. } => {
                            self.walk_expr_for_visibility(env, expr);
                        }
                        pixie_syntax::ast::StrPart::Text(_) => {}
                    }
                }
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

    fn walk_block_for_visibility(&mut self, env: &TypeEnv<'_>, b: &Block) {
        let mut sub = env.child();
        for s in &b.stmts {
            self.walk_stmt_for_visibility(&mut sub, s);
        }
        if let Some(t) = &b.trailing {
            self.walk_expr_for_visibility(&sub, t);
        }
    }

    fn walk_stmt_for_visibility(&mut self, env: &mut TypeEnv<'_>, s: &Stmt) {
        match s {
            Stmt::Let { name, value, .. } | Stmt::Var { name, value, .. } => {
                self.walk_expr_for_visibility(env, value);
                let ty = self.synth_no_check(env, value);
                env.bind(name.name.clone(), ty);
            }
            Stmt::Expr(e) => self.walk_expr_for_visibility(env, e),
            Stmt::Return { value: Some(v), .. } => self.walk_expr_for_visibility(env, v),
            Stmt::Return { value: None, .. } => {}
            Stmt::Emit { args, .. } => {
                for a in args {
                    self.walk_expr_for_visibility(env, a);
                }
            }
            Stmt::Assign { target, value, .. } => {
                self.walk_expr_for_visibility(env, target);
                self.walk_expr_for_visibility(env, value);
            }
            Stmt::For {
                binding,
                iter,
                body,
                ..
            } => {
                self.walk_expr_for_visibility(env, iter);
                let mut sub = env.child();
                sub.bind(binding.name.clone(), Type::Unknown);
                self.walk_block_for_visibility(&sub, body);
            }
            Stmt::While { cond, body, .. } => {
                self.walk_expr_for_visibility(env, cond);
                let sub = env.child();
                self.walk_block_for_visibility(&sub, body);
            }
            Stmt::Break { .. } | Stmt::Continue { .. } => {}
            Stmt::Batch { body, .. } => {
                let sub = env.child();
                self.walk_block_for_visibility(&sub, body);
            }
        }
    }

    /// Lightweight type synthesis for the visibility walker - enough
    /// to identify class types of receivers, without firing strict
    /// type rules (Add operand match, etc.) that the heavyweight
    /// `synth` would. Falls back to `Type::Unknown` for anything
    /// non-trivial; the visibility check tolerates that.
    fn synth_no_check(&self, env: &TypeEnv<'_>, e: &Expr) -> Type {
        use ExprKind as K;
        match &e.kind {
            // Literals — needed by overload resolution, which calls
            // synth_no_check on each arg before picking the right
            // overload. Without these, `fmt(1)` against `fmt(Int)` /
            // `fmt(String)` would synth as `Unknown` and the resolver
            // would call it ambiguous.
            K::Int(_) => Type::int(),
            K::Float(_) => Type::float(),
            K::Bool(_) => Type::bool(),
            K::Str(_) => Type::string(),
            K::Nil => Type::nil(),
            K::Ident(name) => env.lookup(name).cloned().unwrap_or(Type::Unknown),
            // Bare class-member reference. The least-sigil desugar rewrites
            // a bare member name to `@name`, and `check_class_body` binds
            // class members into the method env under that `@`-prefixed
            // key. Overload resolution types a call's arguments through
            // synth_no_check, so without this arm a bare member argument
            // (`writeEntry("k", someProp)`) synths as Unknown and the
            // resolver reports a spurious ambiguity across every same-arity
            // candidate. Mirrors `synth`'s own K::AtIdent handling.
            K::AtIdent(name) => env
                .lookup(&format!("@{name}"))
                .cloned()
                .unwrap_or(Type::Unknown),
            K::SelfRef => env.lookup("self").cloned().unwrap_or(Type::Unknown),
            K::Member { receiver, name } => {
                let recv = self.synth_no_check(env, receiver);
                if let Type::Class(class_name) = recv {
                    if let Some(t) = self.table.lookup_property(&class_name, &name.name) {
                        return t.clone();
                    }
                    if let Some(m) = self.table.lookup_method(&class_name, &name.name) {
                        return m.as_type();
                    }
                }
                Type::Unknown
            }
            K::SafeMember { receiver, name } => {
                let recv = self.synth_no_check(env, receiver);
                let inner = match recv {
                    Type::Nullable(t) => *t,
                    other => other,
                };
                if let Type::Class(class_name) = inner {
                    if let Some(t) = self.table.lookup_property(&class_name, &name.name) {
                        return lift_to_nullable(t.clone());
                    }
                    if let Some(m) = self.table.lookup_method(&class_name, &name.name) {
                        return lift_to_nullable(m.as_type());
                    }
                }
                Type::Unknown
            }
            K::MethodCall {
                receiver, method, ..
            } => {
                // `Type.new(...)` / `Type.staticMethod(...)` — the
                // receiver is a bare class name (a type, not a value),
                // so synth_no_check on it yields Unknown. Resolve it
                // against the class table directly, mirroring the
                // `Foo()` constructor handling in the K::Call arm.
                if let K::Ident(name) = &receiver.kind {
                    if self.table.classes.contains_key(name) {
                        if method.name == "new" {
                            return Type::Class(name.clone());
                        }
                        if let Some(m) = self.table.lookup_method(name, &method.name) {
                            return m.ret.clone();
                        }
                    }
                }
                let recv = self.synth_no_check(env, receiver);
                if let Type::Class(class_name) = recv {
                    if method.name == "new" {
                        return Type::Class(class_name.clone());
                    }
                    if let Some(m) = self.table.lookup_method(&class_name, &method.name) {
                        return m.ret.clone();
                    }
                }
                Type::Unknown
            }
            K::SafeMethodCall {
                receiver, method, ..
            } => {
                let recv = self.synth_no_check(env, receiver);
                let inner = match recv {
                    Type::Nullable(t) => *t,
                    other => other,
                };
                if let Type::Class(class_name) = inner {
                    if let Some(m) = self.table.lookup_method(&class_name, &method.name) {
                        return lift_to_nullable(m.ret.clone());
                    }
                }
                Type::Unknown
            }
            K::Call { callee, type_args, .. } => {
                // `Foo()` constructor: classes used as bare callables
                // produce an instance of that class; `Stack<Int>()`
                // instantiates a generic one (§8.25). State-field
                // bindings (`let counter = Counter()`) flow through
                // here, so the visibility walker can attach the
                // class type to the local name.
                if let K::Ident(name) = &callee.kind {
                    if let Some(entry) = self.table.classes.get(name) {
                        if !entry.class_generics.is_empty() && !type_args.is_empty() {
                            let lowered: Vec<Type> = type_args
                                .iter()
                                .map(|t| lower_type(t, self.program))
                                .collect();
                            return Type::Generic {
                                base: name.clone(),
                                args: lowered,
                            };
                        }
                        return Type::Class(name.clone());
                    }
                    // Best-effort inference path: pick the first overload's
                    // return type. Real call-site dispatch (with arg types
                    // available) goes through `synth`'s K::Call branch.
                    if let Some(fnty) = self.table.fns.get(name).and_then(|v| v.first()) {
                        return fnty.ret.clone();
                    }
                }
                Type::Unknown
            }
            _ => Type::Unknown,
        }
    }

    fn check_struct(&mut self, s: &StructDecl) {
        for f in &s.fields {
            if let Some(default) = &f.default {
                let want = lower_type(&f.ty, self.program);
                let mut env = TypeEnv::root();
                self.check(&mut env, default, &want);
            }
        }
        for m in &s.methods {
            self.check_struct_method(s, m);
        }
    }

    /// Type-check a struct method body. Mirrors `check_class_body` but
    /// with `self` bound to the struct's named type and no `@field`
    /// auto-binding (struct fields are accessed via `self.x`, not
    /// `@x`).
    fn check_struct_method(&mut self, s: &StructDecl, m: &pixie_syntax::ast::FnDecl) {
        let Some(body) = &m.body else { return };
        let ret_ty = m
            .return_ty
            .as_ref()
            .map(|t| lower_type(t, self.program))
            .unwrap_or(Type::void());
        let ret_ty = crate::ty::peel_async_return(ret_ty, m.is_async);
        let mut env = TypeEnv::root();
        env.bind("self", Type::Class(s.name.name.clone()));
        for p in &m.params {
            env.bind(p.name.name.clone(), lower_type(&p.ty, self.program));
        }
        let prev = self.current_class.replace(s.name.name.clone());
        self.check_block(&mut env, body, &ret_ty);
        self.current_class = prev;
    }

    fn check_top_level_fn(&mut self, f: &FnDecl) {
        let Some(body) = &f.body else { return };
        // Mirror `build_fn_ty`'s substitution: a fn-param typed `T`
        // (where T is one of the surrounding generics) is bound as
        // `Type::Var(decl_var_id)` rather than `External("T")`.
        // Without this, `thing.method()` inside a generic fn body
        // sees the receiver as External, which falls through to
        // soft-pass — bypassing the bound-driven method-name check.
        //
        // Overload-aware lookup: free fns can have multiple signatures
        // sharing a name; pick the overload whose arity matches this
        // FnDecl. Same-name same-arity duplicates are a coherence error
        // caught by HIR's `fn_overload_coherence_check`, so falling back
        // to the first-arity-match here is safe (any program where it
        // would mispick is also rejected upstream).
        let fnty_decl = self
            .table
            .fns
            .get(&f.name.name)
            .and_then(|v| v.iter().find(|t| t.params.len() == f.params.len()))
            .cloned();
        let name_to_var: std::collections::HashMap<String, crate::ty::VarId> = match &fnty_decl {
            Some(t) => f
                .generics
                .iter()
                .zip(t.generics.iter())
                .map(|(g, v)| (g.name.name.clone(), *v))
                .collect(),
            None => std::collections::HashMap::new(),
        };
        let ret_ty = f
            .return_ty
            .as_ref()
            .map(|t| {
                let raw = lower_type(t, self.program);
                crate::table::substitute_names(&raw, &name_to_var)
            })
            .unwrap_or(Type::void());
        let ret_ty = crate::ty::peel_async_return(ret_ty, f.is_async);
        let mut env = TypeEnv::root();
        for p in &f.params {
            let raw = lower_type(&p.ty, self.program);
            let pty = crate::table::substitute_names(&raw, &name_to_var);
            env.bind(p.name.name.clone(), pty);
        }
        // Activate the fn's bounds for the body. The bounds map from
        // FnTy is aligned by index with `generics`; lift each to a
        // VarId -> Vec<String> entry. Save the prior state so nested
        // checks (lambdas, etc.) don't accidentally inherit.
        let saved_bounds = std::mem::replace(
            &mut self.current_fn_bounds,
            std::collections::HashMap::new(),
        );
        if let Some(t) = &fnty_decl {
            for (i, var_id) in t.generics.iter().enumerate() {
                if let Some(b) = t.generic_bounds.get(i) {
                    if !b.is_empty() {
                        self.current_fn_bounds.insert(*var_id, b.clone());
                    }
                }
            }
        }
        self.check_block(&mut env, body, &ret_ty);
        self.current_fn_bounds = saved_bounds;
    }

    fn check_method(&mut self, class: &ClassDecl, f: &FnDecl) {
        let Some(body) = &f.body else { return };
        // Lower the return type up here because it can reference the
        // class's generic params by name; check_class_body rebuilds
        // the same name→VarId map for the body walk.
        let class_var_ids: Vec<crate::ty::VarId> = self
            .table
            .classes
            .get(&class.name.name)
            .map(|e| e.class_generics.clone())
            .unwrap_or_default();
        let name_to_var: std::collections::HashMap<String, crate::ty::VarId> = class
            .generics
            .iter()
            .zip(class_var_ids.iter())
            .map(|(g, v)| (g.name.name.clone(), *v))
            .collect();
        let ret_ty = f
            .return_ty
            .as_ref()
            .map(|t| {
                let raw = lower_type(t, self.program);
                crate::table::substitute_names(&raw, &name_to_var)
            })
            .unwrap_or(Type::void());
        let ret_ty = crate::ty::peel_async_return(ret_ty, f.is_async);
        self.check_class_body(class, &f.params, body, &ret_ty);
    }

    /// Verify that `name` on class `class_name` is reachable from the
    /// current context. Inside a method of `class_name` itself, any
    /// member is fair game. Outside (any other class's method, fn at
    /// module level, view / widget body), only members declared
    /// `pub` are visible. Binding-sourced classes (Qt stdlib) skip
    /// the check entirely - users can't add `pub` to those, and the
    /// whole binding surface is treated as the public API.
    fn check_member_pub(&mut self, class_name: &str, name: &str, ref_span: Span) {
        let Some(entry) = self.table.classes.get(class_name) else {
            return;
        };
        if entry.from_binding {
            return;
        }
        // Inside the class's own methods, visibility doesn't apply.
        if self.current_class.as_deref() == Some(class_name) {
            return;
        }
        let is_pub = entry.member_pub.get(name).copied().unwrap_or(false);
        if !is_pub {
            self.diags.push(Diagnostic::error(
                ref_span,
                format!("`{name}` is private to class `{class_name}` - declare it as `pub` to access from outside"),
            ));
        }
    }

    // ---- statement / block walks ----------------------------------------

    fn check_block(&mut self, env: &mut TypeEnv<'_>, b: &Block, expected: &Type) {
        for stmt in &b.stmts {
            self.check_stmt(env, stmt);
        }
        if let Some(t) = &b.trailing {
            // Trailing expression of a fn body: must match the fn's
            // return type. For other blocks (case arms etc.) we still
            // synth so type errors inside surface, but don't check the
            // overall arm value type (fold to Void).
            if !matches!(expected, Type::Prim(Prim::Void)) {
                self.check(env, t, expected);
            } else {
                let _ = self.synth(env, t);
            }
        }
    }

    /// Synthesize a block's value type: the trailing expression's
    /// type, or `Void` when none. Used by `if` / `case` synthesis
    /// to propagate the branch value outward (so `let s = if p {
    /// "x" } else { "y" }` synthesizes as `String`, not `Void`).
    fn synth_block(&mut self, env: &mut TypeEnv<'_>, b: &Block) -> Type {
        for stmt in &b.stmts {
            self.check_stmt(env, stmt);
        }
        match &b.trailing {
            Some(t) => self.synth(env, t),
            None => Type::void(),
        }
    }

    fn check_stmt(&mut self, env: &mut TypeEnv<'_>, s: &Stmt) {
        match s {
            Stmt::Let {
                name, ty, value, ..
            }
            | Stmt::Var {
                name, ty, value, ..
            } => {
                let bound = if let Some(annotated) = ty {
                    // `check` handles all "expected type drives the
                    // value's check" rules, including generic-class
                    // instantiation (`let b: Box<Int> = Box.new()`),
                    // lambda-param binding, and nil-against-nullable.
                    let want = lower_type(annotated, self.program);
                    self.check(env, value, &want);
                    want
                } else {
                    self.synth(env, value)
                };
                env.bind(name.name.clone(), bound);
            }
            Stmt::Assign {
                target,
                op,
                value,
                span,
            } => {
                // Bare-Ident `=` with no prior binding = first-occurrence
                // declaration (HIR's resolver flagged this in `assign_is_decl`).
                // Treat the synth'd RHS type as the new binding's type.
                let lhs_ty = self.synth(env, target);
                if matches!(op, AssignOp::Eq) {
                    if let ExprKind::Ident(name) = &target.kind {
                        if env.lookup(name).is_none() {
                            if let Some(class) = self.current_class.as_ref() {
                                if let Some(entry) = self.table.classes.get(class) {
                                    // Levenshtein only kicks in for
                                    // candidates within edit-distance
                                    // 2 (closest_within's threshold);
                                    // length-prefilter the candidate
                                    // set so we skip the per-call
                                    // `Vec<char>` allocs when no prop
                                    // is even close to the target name.
                                    let nlen = name.len();
                                    let plausible = entry
                                        .properties
                                        .keys()
                                        .filter(|k| k.len().abs_diff(nlen) <= 2)
                                        .map(|k| k.as_str());
                                    if let Some(s) = closest_within(name, plausible) {
                                        self.diags.push(Diagnostic::error(
                                            target.span,
                                            format!(
                                                "no property `{name}` in this class (did you mean `{s}`?)"
                                            ),
                                        ));
                                        let _ = self.synth(env, value);
                                        let _ = span;
                                        return;
                                    }
                                }
                            }
                            // First occurrence => decl. Type is RHS synth.
                            let rhs_ty = self.synth(env, value);
                            env.bind(name.clone(), rhs_ty);
                            // Also remember we annotated this assignment;
                            // codegen consults HIR's assign_is_decl, not our
                            // env, so we don't need to thread anything back.
                            let _ = span;
                            return;
                        }
                    }
                }
                self.check(env, value, &lhs_ty);
            }
            Stmt::Expr(e) => {
                let _ = self.synth(env, e);
            }
            Stmt::Return { value: Some(v), .. } => {
                // We don't have the surrounding fn's return type here
                // (check_block carries it as its `expected`). Conservatively
                // synth and let block-level check catch trailing mismatch.
                let _ = self.synth(env, v);
            }
            Stmt::Return { value: None, .. } => {}
            Stmt::Emit { signal, args, span } => {
                self.check_emit(env, signal, args, *span);
            }
            Stmt::For {
                binding,
                iter,
                body,
                ..
            } => {
                // The type system doesn't have a real iterable element-
                // type derivation yet (no trait machinery / Iterable
                // type class). Synth the iter expression for side
                // effects; bind the for-name to Type::Error so it
                // soft-passes in the body without false positives.
                let _ = self.synth(env, iter);
                let mut sub = env.clone();
                sub.bind(binding.name.clone(), crate::ty::Type::Error);
                for stmt in &body.stmts {
                    self.check_stmt(&mut sub, stmt);
                }
                if let Some(t) = &body.trailing {
                    let _ = self.synth(&mut sub, t);
                }
            }
            Stmt::While { cond, body, .. } => {
                let _ = self.synth(env, cond);
                let mut sub = env.clone();
                for stmt in &body.stmts {
                    self.check_stmt(&mut sub, stmt);
                }
                if let Some(t) = &body.trailing {
                    let _ = self.synth(&mut sub, t);
                }
            }
            Stmt::Break { .. } | Stmt::Continue { .. } => {
                // Loop-context validation lives in the parser/HIR; the
                // type checker has nothing to synthesize for these.
            }
            Stmt::Batch { body, .. } => {
                let mut sub = env.clone();
                for stmt in &body.stmts {
                    self.check_stmt(&mut sub, stmt);
                }
                if let Some(t) = &body.trailing {
                    let _ = self.synth(&mut sub, t);
                }
            }
        }
    }

    fn check_emit(&mut self, env: &mut TypeEnv<'_>, signal: &Ident, args: &[Expr], span: Span) {
        // `emit` only makes sense inside a class method. Find the class
        // context from `self`'s binding type.
        let Some(self_ty) = env.lookup("self").cloned() else {
            self.diags.push(Diagnostic::error(
                span,
                "`emit` is only valid inside a class method",
            ));
            return;
        };
        let class_name = match self_ty {
            Type::Class(n) => n,
            _ => {
                // Soft-fail (External self etc.).
                for a in args {
                    let _ = self.synth(env, a);
                }
                return;
            }
        };
        let Some(ItemKind::Class { signal_names, .. }) = self.program.items.get(&class_name) else {
            for a in args {
                let _ = self.synth(env, a);
            }
            return;
        };
        if !signal_names.contains(&signal.name) {
            let suggestion = closest_within(&signal.name, signal_names.iter().map(|s| s.as_str()));
            let mut msg = format!(
                "no signal `{}` declared in class `{}`",
                signal.name, class_name
            );
            if let Some(s) = suggestion {
                msg.push_str(&format!(" (did you mean `{s}`?)"));
            }
            self.diags.push(Diagnostic::error(signal.span, msg));
        }
        // We don't yet record signal parameter types in HIR, so just
        // synth the args for type-error surfacing.
        for a in args {
            let _ = self.synth(env, a);
        }
    }

    // ---- expression synthesis -------------------------------------------

    fn synth(&mut self, env: &mut TypeEnv<'_>, e: &Expr) -> Type {
        use ExprKind as K;
        match &e.kind {
            K::Int(_) => Type::int(),
            K::Float(_) => Type::float(),
            K::Bool(_) => Type::bool(),
            K::Str(parts) => {
                // Walk interp segments for type errors but the whole expr is String.
                for p in parts {
                    match p {
                        StrPart::Interp(inner) => {
                            let _ = self.synth(env, inner);
                        }
                        StrPart::InterpFmt { expr, .. } => {
                            let _ = self.synth(env, expr);
                        }
                        StrPart::Text(_) => {}
                    }
                }
                Type::string()
            }
            K::Sym(_) => Type::Sym,
            K::Nil => Type::nil(),

            // `this` is the receiver (§8.63) — the object whose
            // method is running. It has a type only inside one.
            K::Ident(name) if name == "this" => match &self.current_class {
                Some(c) => Type::Class(c.clone()),
                None => {
                    self.diags.push(Diagnostic::error(
                        e.span,
                        "`this` is the object a method is running on, and there is no \
                         object here"
                            .to_string(),
                    ));
                    Type::Error
                }
            },
            // Bare member reads inside a class body: the env binds
            // members as `@name`, and without this fallback a bare
            // `r` fell through to Type::External — unchecked, and
            // masked for years by `Add`'s leniency until an impl
            // body multiplied a prop.
            K::Ident(name) => match env
                .lookup(name)
                .or_else(|| env.lookup(&format!("@{name}")))
            {
                Some(t) => t.clone(),
                None => {
                    // Could be a class name used as a value (e.g. `File.open(...)`),
                    // or a top-level fn whose signature we have. Soft-resolve.
                    // Bare-value uses of an overloaded fn surface only the
                    // first overload's type — first-class fn refs that
                    // discriminate on overload need explicit annotation.
                    if let Some(fnty) = self.table.fns.get(name).and_then(|v| v.first()) {
                        return fnty.as_type();
                    }
                    if let Some(item) = self.program.items.get(name) {
                        match item {
                            ItemKind::Class { .. } => Type::Class(name.clone()),
                            ItemKind::Struct { .. } => Type::Class(name.clone()),
                            ItemKind::Fn { .. } => Type::Unknown, // unreachable: in self.table.fns
                            // A bare trait name in expression position
                            // is meaningless — traits aren't values.
                            // Treat it as an unknown so the usual
                            // "unresolved name" diagnostics still fire.
                            ItemKind::Trait { .. } => Type::Unknown,
                            // Top-level `let X : T = ...` — the bare
                            // name `X` resolves to the declared type T.
                            ItemKind::Let { ty, .. } => lower_type(ty, self.program),
                            // Bare enum / flags name in expression
                            // position acts as a namespace handle for
                            // the K::Member access path below — return
                            // Type::Enum so `Color.Red` resolves.
                            ItemKind::Enum { .. } => Type::Enum(name.clone()),
                            ItemKind::Flags { .. } => Type::Flags(name.clone()),
                        }
                    } else {
                        // Unknown - probably an external/Qt name. Don't error.
                        Type::External(name.clone())
                    }
                }
            },

            K::AtIdent(name) => match env.lookup(&format!("@{name}")) {
                Some(t) => t.clone(),
                None => {
                    let suggestion = self
                        .current_class
                        .as_ref()
                        .and_then(|c| self.table.classes.get(c))
                        .and_then(|e| {
                            closest_within(name, e.properties.keys().map(|k| k.as_str()))
                        });
                    let mut msg = format!("no property `{name}` in this class");
                    if let Some(s) = suggestion {
                        msg.push_str(&format!(" (did you mean `{s}`?)"));
                    }
                    self.diags.push(Diagnostic::error(e.span, msg));
                    Type::Error
                }
            },

            K::SelfRef => env.lookup("self").cloned().unwrap_or(Type::Unknown),

            K::Path(parts) => {
                // `File.open` style: each `.` segment is treated as resolution.
                // We only know about top-level item names; deeper paths are
                // External.
                let head = parts.first().map(|i| i.name.as_str()).unwrap_or("");
                match self.program.items.get(head) {
                    Some(_) => Type::Class(head.into()),
                    None => Type::External(head.into()),
                }
            }

            K::Call {
                callee,
                args,
                block,
                type_args,
                ..
            } => {
                // Free-fn dispatch (uniform overload-resolver path).
                //
                // The resolver knows about the trailing-block convention
                // (`f(a) { lambda }` → `f(a, lambda)`); it walks each
                // candidate and tries `arity == args.len()` first, then
                // `arity == args.len() + 1` if a block is present. Block
                // slot type-check is deferred to `check_fn_args` /
                // `synth_generic_call` so bidirectional lambda inference
                // still fires.
                //
                // Diagnostic shape: a single-candidate set renders as
                // "function expects N args, got M" + signature note;
                // multi-candidate sets render the candidate set explicitly.
                if let K::Ident(name) = &callee.kind {
                    // Class construction (`Tag("x")`, `Stack<Int>()`)
                    // — args check against the declared `init`
                    // (§8.25; one init in v1), and explicit type args
                    // instantiate generic classes.
                    if let Some(entry) = self.table.classes.get(name).cloned() {
                        let ret = if !entry.class_generics.is_empty() && !type_args.is_empty() {
                            Type::Generic {
                                base: name.clone(),
                                args: type_args
                                    .iter()
                                    .map(|t| lower_type(t, self.program))
                                    .collect(),
                            }
                        } else {
                            Type::Class(name.clone())
                        };
                        if let Some(init) = entry.inits.first() {
                            if args.len() != init.params.len() {
                                self.diags.push(Diagnostic::error(
                                    e.span,
                                    format!(
                                        "`{name}` takes {} constructor argument(s), got {}",
                                        init.params.len(),
                                        args.len()
                                    ),
                                ));
                            }
                            for (a, want) in args.iter().zip(init.params.iter()) {
                                let got = self.synth(env, a);
                                if !crate::ty::is_subtype(&got, want, self.program) {
                                    self.diags.push(Diagnostic::error(
                                        a.span,
                                        format!(
                                            "constructor argument expects `{}`, got `{}`",
                                            want.render(),
                                            got.render()
                                        ),
                                    ));
                                }
                            }
                        } else {
                            if !args.is_empty() {
                                self.diags.push(Diagnostic::error(
                                    e.span,
                                    format!("`{name}` has no `init` — construct with `{name}()`"),
                                ));
                            }
                            for a in args {
                                let _ = self.synth(env, a);
                            }
                        }
                        return ret;
                    }
                    let overloads = self.table.fns.get(name).cloned().unwrap_or_default();
                    if !overloads.is_empty() {
                        let arg_tys: Vec<Type> =
                            args.iter().map(|a| self.synth_no_check(env, a)).collect();
                        let chosen = match crate::table::resolve_overload(
                            &overloads,
                            &arg_tys,
                            block.is_some(),
                            self.program,
                        ) {
                            crate::table::OverloadResolution::Unique(f) => Some(f.clone()),
                            crate::table::OverloadResolution::NoArityMatch { arities } => {
                                if overloads.len() == 1 {
                                    // Single-candidate case: emit the
                                    // legacy-shape diag with signature
                                    // note (preserves test fidelity).
                                    let only = &overloads[0];
                                    let mut diag = Diagnostic::error(
                                        e.span,
                                        format!(
                                            "function expects {} argument(s), got {}",
                                            only.params.len(),
                                            args.len(),
                                        ),
                                    );
                                    diag.notes.push((
                                        e.span,
                                        format!(
                                            "signature: {}",
                                            render_call_signature(&only.params, Some(&only.ret))
                                        ),
                                    ));
                                    self.diags.push(diag);
                                } else {
                                    self.diags.push(Diagnostic::error(
                                        e.span,
                                        format!(
                                            "no overload of `{}` accepts {} argument(s) (declared arities: {})",
                                            name,
                                            args.len(),
                                            render_arities(&arities),
                                        ),
                                    ));
                                }
                                self.walk_args_for_diagnostics(env, args, block.as_deref());
                                return Type::Error;
                            }
                            crate::table::OverloadResolution::NoTypeMatch { tier3_winners } => {
                                self.diags.push(Diagnostic::error(
                                    e.span,
                                    format!(
                                        "no overload of `{}` matches argument types {} (candidates: {})",
                                        name,
                                        render_arg_types(&arg_tys),
                                        render_overload_candidates(&tier3_winners, name),
                                    ),
                                ));
                                self.walk_args_for_diagnostics(env, args, block.as_deref());
                                return Type::Error;
                            }
                            crate::table::OverloadResolution::Ambiguous { winners } => {
                                self.diags.push(Diagnostic::error(
                                    e.span,
                                    format!(
                                        "ambiguous call to `{}` — multiple overloads match (candidates: {})",
                                        name,
                                        render_overload_candidates(&winners, name),
                                    ),
                                ));
                                self.walk_args_for_diagnostics(env, args, block.as_deref());
                                return Type::Error;
                            }
                            crate::table::OverloadResolution::Empty => None,
                        };
                        if let Some(fnty) = chosen {
                            if !fnty.generics.is_empty() {
                                return self.synth_generic_call(
                                    env,
                                    &fnty,
                                    args,
                                    block.as_deref(),
                                    e.span,
                                );
                            }
                            // Bidirectional check on chosen non-generic
                            // overload — handles lambda param inference,
                            // numeric widening, type-mismatch diags.
                            self.check_fn_args(
                                env,
                                &fnty.params,
                                Some(&fnty.ret),
                                args,
                                block.as_deref(),
                                e.span,
                            );
                            return fnty.ret.clone();
                        }
                    }
                }
                // `recv.method { ... }` (trailing-block call) parses as
                // `Call { callee: Member { recv, method }, ... }`
                // rather than `MethodCall`. Route through the method-
                // call path so method-level generics get the same
                // instantiation+unification treatment as the explicit-
                // parens form.
                if let K::Member { receiver, name } = &callee.kind {
                    let receiver_is_type_ident = self.receiver_is_type_ident(env, receiver);
                    let recv_ty = self.synth(env, receiver);
                    return self.synth_method_call(
                        env,
                        &recv_ty,
                        name,
                        args,
                        block.as_deref(),
                        e.span,
                        receiver_is_type_ident,
                    );
                }
                let callee_ty = self.synth(env, callee);
                self.synth_call(env, &callee_ty, args, block.as_deref(), e.span)
            }

            K::MethodCall {
                receiver,
                method,
                args,
                block,
                type_args,
            } => {
                // Form-(b) generic-class instantiation: when the call
                // is `T<TypeArgs>.new(...)`, lift the receiver from
                // `Class(T)` to `Generic { base: T, args: <TypeArgs> }`
                // BEFORE method-call checking. The same instantiation
                // path that handles form (a) (let-annotated) then
                // accepts the call uniformly.
                if !type_args.is_empty() && method.name == "new" {
                    if let ExprKind::Ident(class_name) = &receiver.kind {
                        if self
                            .table
                            .classes
                            .get(class_name)
                            .map(|e| !e.class_generics.is_empty())
                            .unwrap_or(false)
                        {
                            let lowered_args: Vec<Type> = type_args
                                .iter()
                                .map(|t| lower_type(t, self.program))
                                .collect();
                            // Walk constructor args / block for nested
                            // type errors (constructor params are still
                            // soft-passed, matching the form-(a) path).
                            for a in args {
                                let _ = self.synth(env, a);
                            }
                            if let Some(b) = block {
                                let _ = self.synth(env, b);
                            }
                            return Type::Generic {
                                base: class_name.clone(),
                                args: lowered_args,
                            };
                        }
                    }
                }
                let recv_ty = self.synth(env, receiver);
                // Enum variant constructor call: `Tree.Leaf(7)` —
                // receiver is the enum type, method is a known
                // variant. Type-check args against the variant's
                // declared field types and return the enum type.
                // Nullary variants are also accepted via parens
                // (`Color.Red()` ≡ `Color.Red`).
                if let Type::Enum(enum_name) = &recv_ty {
                    if let Some(ItemKind::Enum { variants, .. }) = self.program.items.get(enum_name)
                    {
                        if let Some(variant) =
                            variants.iter().find(|v| v.name == method.name).cloned()
                        {
                            if args.len() != variant.fields.len() {
                                self.diags.push(Diagnostic::error(
                                    method.span,
                                    format!(
                                        "enum variant `{}::{}` takes {} field(s), got {} arg(s)",
                                        enum_name,
                                        method.name,
                                        variant.fields.len(),
                                        args.len()
                                    ),
                                ));
                            }
                            for (a, f) in args.iter().zip(variant.fields.iter()) {
                                let expected = lower_type(&f.ty, self.program);
                                let _ = self.check(env, a, &expected);
                            }
                            return Type::Enum(enum_name.clone());
                        }
                    }
                }
                let receiver_is_type_ident = self.receiver_is_type_ident(env, receiver);
                self.synth_method_call(
                    env,
                    &recv_ty,
                    method,
                    args,
                    block.as_deref(),
                    e.span,
                    receiver_is_type_ident,
                )
            }

            K::Member { receiver, name } => {
                let recv_ty = self.synth(env, receiver);
                // `e.rawValue` (no parens) on an enum / flags
                // value — built-in extractor that returns the
                // underlying integer. Same handling regardless of
                // which enum the value is from.
                if matches!(recv_ty, Type::Enum(_) | Type::Flags(_)) && name.name == "rawValue" {
                    return Type::Prim(Prim::Int);
                }
                // `Color.Red` — variant access on an enum. The
                // synth above returned `Type::Enum("Color")`; look
                // up the variant in `prog.items` and return the
                // enum type itself (the value's type IS the enum).
                if let Type::Enum(enum_name) = &recv_ty {
                    if let Some(ItemKind::Enum { variants, .. }) = self.program.items.get(enum_name)
                    {
                        if variants.iter().any(|v| v.name == name.name) {
                            return Type::Enum(enum_name.clone());
                        }
                        let suggestion =
                            closest_within(&name.name, variants.iter().map(|v| v.name.as_str()));
                        let mut msg = format!("no variant `{}` on enum `{}`", name.name, enum_name);
                        if let Some(s) = suggestion {
                            msg.push_str(&format!(" (did you mean `{s}`?)"));
                        }
                        self.diags.push(Diagnostic::error(name.span, msg));
                        return Type::Error;
                    }
                }
                match &recv_ty {
                    Type::Class(class_name) => {
                        // Clone the borrowed values before invoking
                        // `check_member_pub` (which takes `&mut self`)
                        // to satisfy the borrow checker.
                        let prop = self.table.lookup_property(class_name, &name.name).cloned();
                        if let Some(t) = prop {
                            self.check_member_pub(class_name, &name.name, name.span);
                            return t;
                        }
                        let method = self.table.lookup_method(class_name, &name.name).cloned();
                        if let Some(m) = method {
                            self.check_member_pub(class_name, &name.name, name.span);
                            return m.as_type();
                        }
                        // Signals form their own member axis (sender +
                        // signal name → connect target). Lift to a typed
                        // `Type::SignalRef` so `.connect` / `.disconnect`
                        // can be checked against the signal's parameter
                        // list and IDE/LSP hover renders something
                        // useful. Codegen still pattern-matches the AST
                        // shape and emits Qt's modern function-pointer
                        // `QObject::connect(...)` — the typed value here
                        // is invisible to it.
                        if let Some((decl_class, params)) =
                            self.table.lookup_signal(class_name, &name.name)
                        {
                            let params = params.clone();
                            return Type::SignalRef {
                                class: decl_class,
                                signal: name.name.clone(),
                                params,
                            };
                        }
                        let suggestion = self.table.classes.get(class_name).and_then(|e| {
                            // Properties + methods + signals all share the
                            // member name space at the call site.
                            let names = e
                                .properties
                                .keys()
                                .chain(e.methods.keys())
                                .chain(e.signals.keys());
                            closest_within(&name.name, names.map(|k| k.as_str()))
                        });
                        let mut msg =
                            format!("no member `{}` on class `{}`", name.name, class_name);
                        if let Some(s) = suggestion {
                            msg.push_str(&format!(" (did you mean `{s}`?)"));
                        }
                        self.diags.push(Diagnostic::error(name.span, msg));
                        Type::Error
                    }
                    // Generic instantiation: `Box<Int>` receiver. Look
                    // up the property / method on the base class and
                    // substitute the call-site args for the class's
                    // generic params before returning.
                    Type::Generic { base, args } => {
                        if let Some(t) = self
                            .table
                            .lookup_property_instantiated(base, args, &name.name)
                        {
                            self.check_member_pub(base, &name.name, name.span);
                            return t;
                        }
                        if let Some(m) = self
                            .table
                            .lookup_method_instantiated(base, args, &name.name)
                        {
                            self.check_member_pub(base, &name.name, name.span);
                            return m.as_type();
                        }
                        // Built-in generic bases (List, Map, ...) have
                        // no class entry; soft-pass.
                        Type::Unknown
                    }
                    // `Bytes` is a closed prim: exactly one member.
                    // Cheap strictness the soft-pass externals can't
                    // give (§8.19).
                    Type::Prim(Prim::Bytes) => {
                        if name.name == "length" {
                            return Type::int();
                        }
                        self.diags.push(Diagnostic::error(
                            name.span,
                            format!("no member `{}` on `Bytes` (only `length`)", name.name),
                        ));
                        Type::Error
                    }
                    // External / Unknown / Error: soft-pass.
                    _ => Type::Unknown,
                }
            }

            K::SafeMember { receiver, name } => {
                // `recv?.name` — receiver should be Nullable; we
                // unwrap, then look up `.name` on the inner type, and
                // lift the result back to Nullable. A non-nullable
                // receiver is accepted (no-op) so refactoring `T?` to
                // `T` doesn't immediately break call sites.
                let recv_ty = self.synth(env, receiver);
                let inner = match recv_ty {
                    Type::Nullable(t) => *t,
                    other => other,
                };
                let result = match &inner {
                    Type::Class(class_name) => {
                        let prop = self.table.lookup_property(class_name, &name.name).cloned();
                        if let Some(t) = prop {
                            self.check_member_pub(class_name, &name.name, name.span);
                            t
                        } else if let Some(m) =
                            self.table.lookup_method(class_name, &name.name).cloned()
                        {
                            self.check_member_pub(class_name, &name.name, name.span);
                            m.as_type()
                        } else {
                            self.diags.push(Diagnostic::error(
                                name.span,
                                format!("no member `{}` on class `{}`", name.name, class_name),
                            ));
                            return Type::Error;
                        }
                    }
                    Type::Generic { base, args } => {
                        if let Some(t) = self
                            .table
                            .lookup_property_instantiated(base, args, &name.name)
                        {
                            self.check_member_pub(base, &name.name, name.span);
                            t
                        } else if let Some(m) = self
                            .table
                            .lookup_method_instantiated(base, args, &name.name)
                        {
                            self.check_member_pub(base, &name.name, name.span);
                            m.as_type()
                        } else {
                            return Type::Unknown;
                        }
                    }
                    _ => return Type::Unknown,
                };
                lift_to_nullable(result)
            }

            K::SafeMethodCall {
                receiver,
                method,
                args,
                block,
                ..
            } => {
                let recv_ty = self.synth(env, receiver);
                let inner = match recv_ty {
                    Type::Nullable(t) => *t,
                    other => other,
                };
                let inner_result = self.synth_method_call(
                    env,
                    &inner,
                    method,
                    args,
                    block.as_deref(),
                    e.span,
                    false, // `recv?.method` is always an instance call
                );
                lift_to_nullable(inner_result)
            }

            // `xs[i]` on the built-in containers. A list subscript is
            // the TRAPPING accessor and yields `T` — the safe lookup
            // is `xs.get(i)`, which answers `T?` like `first()`. A map
            // subscript answers `V?` because absence is ordinary there,
            // not a program error. Anything else keeps the soft pass.
            K::Index { receiver, index } => {
                let recv_ty = self.synth(env, receiver);
                let idx_ty = self.synth(env, index);
                match &recv_ty {
                    Type::Generic { base, args } if base == "List" && args.len() == 1 => {
                        if !matches!(idx_ty, Type::Prim(Prim::Int) | Type::Unknown) {
                            self.diags.push(Diagnostic::error(
                                index.span,
                                format!("a list index is an Int, found `{}`", idx_ty.render()),
                            ));
                        }
                        args[0].clone()
                    }
                    Type::Generic { base, args } if base == "Map" && args.len() == 2 => {
                        if !matches!(idx_ty, Type::Unknown)
                            && !crate::ty::is_subtype(&idx_ty, &args[0], self.program)
                        {
                            self.diags.push(Diagnostic::error(
                                index.span,
                                format!(
                                    "this map is keyed by `{}`, found `{}`",
                                    args[0].render(),
                                    idx_ty.render()
                                ),
                            ));
                        }
                        Type::Nullable(Box::new(args[1].clone()))
                    }
                    _ => Type::Unknown,
                }
            }

            K::Block(b) => {
                let mut sub = env.child();
                self.check_block(&mut sub, b, &Type::void());
                Type::void()
            }

            K::Lambda { params, body } => {
                let mut sub = env.child();
                let mut param_tys = Vec::with_capacity(params.len());
                for p in params {
                    let pt = lower_type(&p.ty, self.program);
                    sub.bind(p.name.name.clone(), pt.clone());
                    param_tys.push(pt);
                }
                // Walk the body's statements (so internal type errors
                // surface) and infer the lambda's return type from the
                // trailing expression. A trailing-less body is void.
                // Param types come from explicit annotations only -
                // unannotated `|x|` keeps the placeholder, which the
                // call-site `check` path can refine when the expected
                // function type is known (see `check` at the bottom of
                // this file).
                for stmt in &body.stmts {
                    self.check_stmt(&mut sub, stmt);
                }
                let ret_ty = if let Some(t) = &body.trailing {
                    self.synth(&mut sub, t)
                } else {
                    Type::void()
                };
                Type::Fn {
                    params: param_tys,
                    ret: Box::new(ret_ty),
                }
            }

            K::Unary { op, expr } => {
                let inner = self.synth(env, expr);
                match op {
                    UnaryOp::Neg => self.expect_numeric(&inner, expr.span),
                    UnaryOp::Not => self.expect(&inner, &Type::bool(), expr.span),
                }
                inner
            }

            K::Binary { op, lhs, rhs } => {
                let l = self.synth(env, lhs);
                let r = self.synth(env, rhs);
                self.synth_binary(*op, &l, &r, lhs.span, rhs.span)
            }

            K::Try(inner) => {
                let inner_ty = self.synth(env, inner);
                match inner_ty {
                    Type::ErrorUnion { ok, .. } => *ok,
                    Type::External(_) | Type::Unknown | Type::Error => Type::Unknown,
                    other => {
                        self.diags.push(Diagnostic::error(
                            e.span,
                            format!(
                                "`?` requires an `!T` (error-union) operand, got `{}`",
                                other.render()
                            ),
                        ));
                        Type::Error
                    }
                }
            }

            K::If {
                cond,
                then_b,
                else_b,
                let_binding,
            } => {
                // The if/else value is the synth'd type of the
                // branches' trailing expressions when both branches
                // exist. With no `else`, the result is Void
                // (statement-form). When the branches disagree we
                // pick the then-type and let `check` (the caller)
                // produce the actual mismatch diagnostic.
                let then_ty = if let Some((pat, init)) = let_binding {
                    // `if let pat = init { ... } else { ... }` —
                    // bind the pattern's variables in the then-branch
                    // env. The init's type drives bind_pattern.
                    let init_ty = self.synth(env, init);
                    let mut sub = env.child();
                    self.bind_pattern(&mut sub, pat, &init_ty);
                    self.synth_block(&mut sub, then_b)
                } else {
                    let _ = self.check(env, cond, &Type::bool());
                    let mut sub = env.child();
                    self.synth_block(&mut sub, then_b)
                };
                match else_b {
                    Some(eb) => {
                        let mut sub = env.child();
                        let _else_ty = self.synth_block(&mut sub, eb);
                        then_ty
                    }
                    None => Type::void(),
                }
            }

            K::Case { scrutinee, arms } => {
                let scrutinee_ty = self.synth(env, scrutinee);
                let mut arm_ty: Option<Type> = None;
                for arm in arms {
                    let mut sub = env.child();
                    self.bind_pattern(&mut sub, &arm.pattern, &scrutinee_ty);
                    let t = self.synth_block(&mut sub, &arm.body);
                    if arm_ty.is_none() {
                        arm_ty = Some(t);
                    }
                }
                self.check_case_exhaustiveness(&scrutinee_ty, arms, e.span);
                arm_ty.unwrap_or(Type::void())
            }

            K::Await(inner) => {
                // pixie: `await` is an execution-mode marker (the call
                // runs on a worker and re-enters the main loop), not a
                // Future unwrap — the operand keeps its type. A real
                // `Future<T>` (task handles, M2+) still unwraps.
                let inner_ty = self.synth(env, inner);
                match inner_ty {
                    Type::Generic { base, args } if base == "Future" && args.len() == 1 => {
                        args.into_iter().next().unwrap()
                    }
                    other => other,
                }
            }

            K::Kwarg { value, .. } => self.synth(env, value),
            // Array / map literals: the type system doesn't have real
            // collection-element inference yet, so the literal synths as
            // `Type::Error` (= unknown, don't trust). Unlike the Qt
            // ancestor there is no untyped runtime fallback behind this:
            // pixie's emitter only lowers literals in typed positions
            // (prop defaults, annotated lets) and hard-errors otherwise,
            // so nothing silently becomes a variant type. Real literal
            // inference is on the M1 ledger.
            K::Array(items) => {
                for it in items {
                    let _ = self.synth(env, it);
                }
                Type::Error
            }
            K::Map(entries) => {
                for (k, v) in entries {
                    let _ = self.synth(env, k);
                    let _ = self.synth(env, v);
                }
                Type::Error
            }
            K::Range { start, end, .. } => {
                let _ = self.synth(env, start);
                let _ = self.synth(env, end);
                // Range exists today only as the iter of a for-loop;
                // codegen detects it there and emits a C-style for, so
                // leaking the type out as Error is fine.
                Type::Error
            }
            K::Element(_) => {
                // An Element used as an expression value only makes
                // sense inside view/widget body context where codegen
                // pulls it back out as the trailing expression of a
                // conditional / repeated branch. The type system has
                // no `Element` type today; report as `Unknown` so the
                // expression doesn't pollute surrounding inference.
                Type::Unknown
            }
        }
    }

    fn synth_call(
        &mut self,
        env: &mut TypeEnv<'_>,
        callee: &Type,
        args: &[Expr],
        block: Option<&Expr>,
        call_span: Span,
    ) -> Type {
        match callee {
            Type::Fn { params, ret } => {
                self.check_fn_args(env, params, Some(ret), args, block, call_span);
                (**ret).clone()
            }
            // `ClassName(args)` is the bare-callable form of `ClassName.new(args)`:
            // both produce an instance. Recognizing it here lets state-field
            // bindings like `let counter = Counter()` flow the correct
            // class type out, which the visibility check downstream needs
            // to identify member accesses.
            Type::Class(class_name) => {
                for a in args {
                    let _ = self.synth(env, a);
                }
                if let Some(b) = block {
                    let _ = self.synth(env, b);
                }
                Type::Class(class_name.clone())
            }
            // External / Unknown / etc: soft-pass on positional args, but
            // walk them so internal type errors still surface.
            _ => {
                for a in args {
                    let _ = self.synth(env, a);
                }
                if let Some(b) = block {
                    let _ = self.synth(env, b);
                }
                Type::Unknown
            }
        }
    }

    /// Type-check a call to a generic top-level fn. Allocates fresh
    /// `VarId`s for each generic param, substitutes them through the
    /// stored signature, then unifies each argument's synthesized type
    /// against the (substituted) expected param. The return type is
    /// `Substitution::apply`'d at the end so any inferred bindings
    /// propagate outward.
    fn synth_generic_call(
        &mut self,
        env: &mut TypeEnv<'_>,
        fnty: &FnTy,
        args: &[Expr],
        block: Option<&Expr>,
        call_span: Span,
    ) -> Type {
        let (mut subst, params, ret) = instantiate(fnty, &mut self.var_source);
        // Synth each arg, then unify against expected.
        let positional = args.len();
        let block_count = if block.is_some() { 1 } else { 0 };
        let provided = positional + block_count;
        if provided != params.len() {
            let diag = Diagnostic::error(
                call_span,
                format!(
                    "function expects {} argument(s), got {}",
                    params.len(),
                    provided
                ),
            )
            .with_note(
                call_span,
                format!("signature: {}", render_call_signature(&params, Some(&ret))),
            );
            self.diags.push(diag);
        }
        for (i, a) in args.iter().enumerate() {
            let actual = self.synth(env, a);
            if let Some(expected) = params.get(i) {
                let actual_resolved = subst.apply(&actual);
                let expected_resolved = subst.apply(expected);
                if let Err(m) = unify_or_subtype(
                    &actual_resolved,
                    &expected_resolved,
                    &mut subst,
                    self.program,
                ) {
                    self.diags.push(Diagnostic::error(
                        a.span,
                        format!(
                            "expected `{}`, found `{}`",
                            m.expected.render(),
                            m.actual.render()
                        ),
                    ));
                }
            }
        }
        if let Some(b) = block {
            if let Some(expected) = params.get(positional) {
                // Lambdas: thread the expected (post-subst) Fn type into
                // `check` so its bidirectional rule sees concrete types
                // for the lambda's params (rather than unsolved Vars).
                let expected_resolved = subst.apply(expected);
                let actual = self.check(env, b, &expected_resolved);
                // Unify the actual lambda type back against the
                // expected so any vars referenced in the expected type
                // (e.g. `U` in `fn(T) -> U`) get bound to the lambda's
                // return type. Without this, U stays unsolved when
                // inference relies on the lambda body's trailing
                // expression to constrain it.
                let actual_resolved = subst.apply(&actual);
                let _ = unify_or_subtype(
                    &actual_resolved,
                    &expected_resolved,
                    &mut subst,
                    self.program,
                );
            }
        }
        // Record the inferred type args at this call's span so codegen
        // can emit `make<X>(args)` / `obj->transform<X>(args)` even
        // though the user wrote `make(args)` / `obj.transform(args)`.
        // C++ template deduction handles the simple cases on its own,
        // but lambdas-as-`std::function<U(T)>` won't deduce U without
        // an explicit type argument, so emitting the inferred args
        // unconditionally avoids the failure mode.
        let inferred_args: Vec<Type> = fnty
            .generics
            .iter()
            .map(|orig| subst.apply(&Type::Var(*orig)))
            .collect();
        // Bound enforcement. For each generic param `T: Iterable + Comparable`,
        // verify the inferred concrete type implements every listed
        // trait.
        //
        // Strict-check eligible: Class, Generic{base}, and primitives
        // (Int / Float / Bool / String) — Cute has visibility into
        // their declarations and the user can write an `impl` for them.
        //
        // External types are usually opaque (Qt classes pulled in
        // via bindings), so they soft-pass — *unless* the user has
        // written an `impl Trait for ExternType` somewhere. Once that
        // impl exists we have positive visibility into the type's
        // impl set and can check strictly. The lookup key is the
        // External's simple name (`QStringList`), matching the HIR
        // `impls_for` registration.
        //
        // Unresolved Vars / Unknown / Fn / ErrorUnion / Sym soft-pass
        // unconditionally.
        for (i, inferred) in inferred_args.iter().enumerate() {
            let bound_names = match fnty.generic_bounds.get(i) {
                Some(bs) if !bs.is_empty() => bs,
                _ => continue,
            };
            let strict_name = bound_check_target_name(inferred);
            let external_with_impl = match inferred {
                Type::External(name) if self.program.impls_for.contains_key(name) => {
                    Some(name.clone())
                }
                _ => None,
            };
            let target_name = match strict_name.or(external_with_impl) {
                Some(n) => n,
                None => continue,
            };
            let empty: std::collections::HashSet<String> = std::collections::HashSet::new();
            let impls = self.program.impls_for.get(&target_name).unwrap_or(&empty);
            for bound in bound_names {
                if !impls.contains(bound) {
                    self.diags.push(Diagnostic::error(
                        call_span,
                        format!(
                            "type `{}` does not implement trait `{}`",
                            inferred.render(),
                            bound
                        ),
                    ));
                }
            }
        }
        if !inferred_args.is_empty() {
            self.generic_instantiations.insert(call_span, inferred_args);
        }
        subst.apply(&ret)
    }

    /// True when `receiver` is a bare class name used in expression
    /// position — i.e. `QTimer.singleShot(...)` rather than
    /// `someTimer.start()`. The two produce the same `recv_ty`
    /// (`Type::Class("QTimer")`) but only the former is the correct
    /// call form for `static fn` methods. `synth_method_call` uses
    /// this to filter overloads by `FnTy.is_static`.
    ///
    /// A local binding shadows the class name (`let QTimer = ...`),
    /// so `env.lookup` is consulted first.
    fn receiver_is_type_ident(&self, env: &TypeEnv<'_>, receiver: &Expr) -> bool {
        let pixie_syntax::ast::ExprKind::Ident(name) = &receiver.kind else {
            return false;
        };
        if env.lookup(name).is_some() {
            return false;
        }
        matches!(
            self.program.items.get(name),
            Some(pixie_hir::ItemKind::Class { .. })
        )
    }

    fn synth_method_call(
        &mut self,
        env: &mut TypeEnv<'_>,
        recv_ty: &Type,
        method: &Ident,
        args: &[Expr],
        block: Option<&Expr>,
        call_span: Span,
        receiver_is_type_ident: bool,
    ) -> Type {
        // Built-in methods on enum / flags values:
        //
        //   `e.rawValue` → Int — extract the underlying integer the
        //   enum lowers to. Useful for serialisation, interop with
        //   int-typed APIs, and explicit casts. (Cute enums don't
        //   implicitly convert to Int.)
        //
        // The pattern is restrictive: only `rawValue` with no args
        // and no block; anything else falls through to the regular
        // class / external lookup so the user gets the normal
        // diagnostics.
        if matches!(recv_ty, Type::Enum(_) | Type::Flags(_))
            && method.name == "rawValue"
            && args.is_empty()
            && block.is_none()
        {
            return Type::Prim(Prim::Int);
        }
        // `obj.signal.connect { ... }` and `.disconnect()`: the typed
        // dispatch built on `Type::SignalRef`. Routes through a small
        // helper so the surrounding generic / class / external logic
        // doesn't have to learn about this special-cased value type.
        if let Type::SignalRef {
            class,
            signal,
            params,
        } = recv_ty
        {
            return self.synth_signal_method_call(
                env, class, signal, params, method, args, block, call_span,
            );
        }
        // Body-side method-name validation against the surrounding
        // fn's generic bounds. Receiver type is `Type::Var(v)` when
        // it refers to a generic-typed param (`thing` in
        // `fn use_it<T: Foo>(thing: T)`). For each trait the var is
        // bound by, look up the method on its declared surface; if
        // none of them list `method.name`, reject the call here
        // rather than waiting for a C++ template instantiation
        // error. Var with no bounds (bare `<T>`) soft-passes — the
        // user opted out of the constraint check.
        if let Type::Var(v) = recv_ty {
            if let Some(bounds) = self.current_fn_bounds.get(v).cloned() {
                // First-bound-wins for cross-trait same-name methods
                // (existing semantics: `<T: Reader + Writer>` with
                // `Reader::read -> Int` and `Writer::read -> String`
                // picks Reader). Walk bound traits in order; the first
                // trait declaring ANY method named `method.name` owns
                // the call. Within THAT trait, all overloads of the
                // name go through the resolver so `trait Foo { fn x;
                // fn x(Int) }` picks the right one by arg type.
                let mut candidate_decls: Vec<pixie_syntax::ast::FnDecl> = Vec::new();
                for trait_name in &bounds {
                    if let Some(pixie_hir::ItemKind::Trait { methods, .. }) =
                        self.program.items.get(trait_name)
                    {
                        for m in methods {
                            if m.name == method.name {
                                candidate_decls.push(m.fn_decl.clone());
                            }
                        }
                        if !candidate_decls.is_empty() {
                            break;
                        }
                    }
                }
                if candidate_decls.is_empty() {
                    let bound_list = bounds.join(" + ");
                    self.diags.push(Diagnostic::error(
                        method.span,
                        format!(
                            "no method `{}` on generic type bound by `{}`",
                            method.name, bound_list
                        ),
                    ));
                    // Walk args defensively so internal type errors
                    // (e.g. an unbound name in an arg) still surface.
                    for a in args {
                        let _ = self.synth(env, a);
                    }
                    if let Some(b) = block {
                        let _ = self.synth(env, b);
                    }
                    let _ = call_span;
                    return Type::Error;
                }
                // Single candidate (the common case): bypass the
                // resolver and route directly. Preserves existing
                // diagnostic shapes (synth_generic_call's arity / type
                // checks) for trait surfaces with no overload.
                if candidate_decls.len() == 1 {
                    let fnty = self.instantiate_trait_method_fnty(&candidate_decls[0], recv_ty);
                    return self.synth_generic_call(env, &fnty, args, block, call_span);
                }
                // Multiple overloads of the same name on bound traits:
                // build FnTys for each and resolve by arg type.
                let candidate_fntys: Vec<crate::table::FnTy> = candidate_decls
                    .iter()
                    .map(|d| self.instantiate_trait_method_fnty(d, recv_ty))
                    .collect();
                let arg_tys: Vec<Type> = args.iter().map(|a| self.synth_no_check(env, a)).collect();
                let chosen = match crate::table::resolve_overload(
                    &candidate_fntys,
                    &arg_tys,
                    block.is_some(),
                    self.program,
                ) {
                    crate::table::OverloadResolution::Unique(f) => Some(f.clone()),
                    crate::table::OverloadResolution::NoArityMatch { arities } => {
                        self.diags.push(Diagnostic::error(
                            call_span,
                            format!(
                                "no overload of trait method `{}` accepts {} argument(s) (declared arities: {})",
                                method.name,
                                args.len(),
                                render_arities(&arities),
                            ),
                        ));
                        self.walk_args_for_diagnostics(env, args, block);
                        return Type::Error;
                    }
                    crate::table::OverloadResolution::NoTypeMatch { tier3_winners } => {
                        self.diags.push(Diagnostic::error(
                            call_span,
                            format!(
                                "no overload of trait method `{}` matches argument types {} (candidates: {})",
                                method.name,
                                render_arg_types(&arg_tys),
                                render_overload_candidates(&tier3_winners, &method.name),
                            ),
                        ));
                        self.walk_args_for_diagnostics(env, args, block);
                        return Type::Error;
                    }
                    crate::table::OverloadResolution::Ambiguous { winners } => {
                        self.diags.push(Diagnostic::error(
                            call_span,
                            format!(
                                "ambiguous call to trait method `{}` (candidates: {})",
                                method.name,
                                render_overload_candidates(&winners, &method.name),
                            ),
                        ));
                        self.walk_args_for_diagnostics(env, args, block);
                        return Type::Error;
                    }
                    crate::table::OverloadResolution::Empty => None,
                };
                if let Some(fnty) = chosen {
                    return self.synth_generic_call(env, &fnty, args, block, call_span);
                }
                return Type::Error;
            }
        }
        if let Type::Class(class_name) = recv_ty {
            // Struct constructor: `Point.new(x, y)` checks args
            // positionally against the declared field types. No
            // overloads — a struct has exactly one constructor whose
            // arity matches the field count. Zero-arg `Point.new()`
            // is accepted as default-construction.
            if method.name == "new" {
                if let Some(struct_entry) = self.table.structs.get(class_name).cloned() {
                    if !args.is_empty() && args.len() != struct_entry.fields.len() {
                        self.diags.push(Diagnostic::error(
                            call_span,
                            format!(
                                "`struct {0}` has {1} field(s); `{0}.new(...)` needs {1} argument(s) or zero (got {2})",
                                class_name,
                                struct_entry.fields.len(),
                                args.len(),
                            ),
                        ));
                        for a in args {
                            let _ = self.synth(env, a);
                        }
                        return Type::Class(class_name.clone());
                    }
                    if !args.is_empty() {
                        for (a, (fname, fty)) in args.iter().zip(struct_entry.fields.iter()) {
                            let got = self.synth(env, a);
                            if !crate::ty::is_subtype(&got, fty, self.program) {
                                self.diags.push(Diagnostic::error(
                                    a.span,
                                    format!(
                                        "field `{0}.{1}` expects `{2}`, got `{3}`",
                                        class_name,
                                        fname,
                                        fty.render(),
                                        got.render(),
                                    ),
                                ));
                            }
                        }
                    } else {
                        // Zero-arg call: accept as default-construction.
                    }
                    return Type::Class(class_name.clone());
                }
            }
            // `T.new(args)` constructs an instance. With one or more
            // user `init`s, args are checked against the declared
            // signatures (arity-overload, first match wins). Without
            // any init, args are accepted unchecked — codegen still
            // emits a synthetic `T(QObject*)` ctor and the call site
            // relies on parent auto-injection.
            if method.name == "new" {
                let inits = self
                    .table
                    .classes
                    .get(class_name)
                    .map(|e| e.inits.clone())
                    .unwrap_or_default();
                if inits.is_empty() {
                    // No user init declared — foreign-soft accept (the
                    // synthetic `T(QObject*)` ctor is enough at codegen
                    // time).
                    for a in args {
                        let _ = self.synth(env, a);
                    }
                    if let Some(b) = block {
                        let _ = self.synth(env, b);
                    }
                    return Type::Class(class_name.clone());
                }
                // Overload-resolve init by arg-type so e.g.
                // `init(name: String)` and `init(id: Int)` disambiguate
                // on `Tag.new(42)` / `Tag.new("foo")`.
                let arg_tys: Vec<Type> = args.iter().map(|a| self.synth_no_check(env, a)).collect();
                let chosen = match crate::table::resolve_overload(
                    &inits,
                    &arg_tys,
                    block.is_some(),
                    self.program,
                ) {
                    crate::table::OverloadResolution::Unique(f) => Some(f.clone()),
                    crate::table::OverloadResolution::NoArityMatch { arities } => {
                        self.diags.push(Diagnostic::error(
                            call_span,
                            format!(
                                "no `init` on class `{}` accepts {} argument(s) (declared inits take: {})",
                                class_name,
                                args.len(),
                                render_arities(&arities),
                            ),
                        ));
                        for a in args {
                            let _ = self.synth(env, a);
                        }
                        if let Some(b) = block {
                            let _ = self.synth(env, b);
                        }
                        return Type::Class(class_name.clone());
                    }
                    crate::table::OverloadResolution::NoTypeMatch { tier3_winners } => {
                        self.diags.push(Diagnostic::error(
                            call_span,
                            format!(
                                "no `init` on class `{}` matches argument types {} (candidates: {})",
                                class_name,
                                render_arg_types(&arg_tys),
                                render_overload_candidates(&tier3_winners, "init"),
                            ),
                        ));
                        for a in args {
                            let _ = self.synth(env, a);
                        }
                        if let Some(b) = block {
                            let _ = self.synth(env, b);
                        }
                        return Type::Class(class_name.clone());
                    }
                    crate::table::OverloadResolution::Ambiguous { winners } => {
                        self.diags.push(Diagnostic::error(
                            call_span,
                            format!(
                                "ambiguous `init` call on class `{}` (candidates: {})",
                                class_name,
                                render_overload_candidates(&winners, "init"),
                            ),
                        ));
                        for a in args {
                            let _ = self.synth(env, a);
                        }
                        if let Some(b) = block {
                            let _ = self.synth(env, b);
                        }
                        return Type::Class(class_name.clone());
                    }
                    crate::table::OverloadResolution::Empty => None,
                };
                if let Some(fnty) = chosen {
                    self.check_fn_args(env, &fnty.params, None, args, block, call_span);
                }
                if let Some(b) = block {
                    let _ = self.synth(env, b);
                }
                return Type::Class(class_name.clone());
            }
            // `ErrorType.<variant>(args)` constructs an error value.
            // The error decl emits a `static T variant(...)` factory
            // (see `emit_error_decl`); resolve against the variants
            // table so type-check accepts these calls instead of
            // confusing them with class methods. The result type is
            // the error class itself.
            if let Some(err_entry) = self.table.errors.get(class_name) {
                if let Some(field_tys) = err_entry.variants.get(&method.name).cloned() {
                    // Error-variant constructors don't return through the
                    // usual path — the result type is the error class
                    // itself, not the variant's "ret". No signature note.
                    self.check_fn_args(env, &field_tys, None, args, block, call_span);
                    return Type::Class(class_name.clone());
                }
                // Variant lookup failed: fall through to class-method
                // diagnostic so users get a clear "no variant `X` on
                // error `T`" message.
                let suggestion =
                    closest_within(&method.name, err_entry.variants.keys().map(|k| k.as_str()));
                let mut msg = format!("no variant `{}` on error `{}`", method.name, class_name);
                if let Some(s) = suggestion {
                    msg.push_str(&format!(" (did you mean `{s}`?)"));
                }
                self.diags.push(Diagnostic::error(method.span, msg));
                for a in args {
                    let _ = self.synth(env, a);
                }
                if let Some(b) = block {
                    let _ = self.synth(env, b);
                }
                return Type::Error;
            }
            // Class method dispatch with overload resolution. Clone the
            // overload Vec out of self.table first to release the
            // borrow (resolver / check_fn_args mutably borrow self).
            let all_overloads = self
                .table
                .lookup_method_overloads(class_name, &method.name)
                .to_vec();
            if all_overloads.is_empty() {
                // No method by that name — but the receiver may carry a
                // closure-typed field of that name. `recv.run(args)`
                // then invokes the stored closure; the lowered C++ field
                // is a `std::function` member, and the very same
                // `recv.run(args)` syntax calls it. This is how a
                // closure-bearing struct (e.g. a tool registry —
                // `struct Tool { var run : fn(String) -> String }`)
                // dispatches without traits or virtual methods.
                let closure_field = self
                    .table
                    .structs
                    .get(class_name)
                    .and_then(|s| {
                        s.fields
                            .iter()
                            .find(|(n, _)| n == &method.name)
                            .map(|(_, t)| t.clone())
                    })
                    .or_else(|| {
                        self.table
                            .classes
                            .get(class_name)
                            .and_then(|c| c.fields.get(&method.name).map(|f| f.ty.clone()))
                    });
                if let Some(Type::Fn { params, ret }) = closure_field {
                    self.check_fn_args(env, &params, Some(ret.as_ref()), args, block, call_span);
                    return *ret;
                }
                let suggestion = self.table.classes.get(class_name).and_then(|e| {
                    closest_within(&method.name, e.methods.keys().map(|k| k.as_str()))
                });
                let mut msg = format!("no method `{}` on class `{}`", method.name, class_name);
                if let Some(s) = suggestion {
                    msg.push_str(&format!(" (did you mean `{s}`?)"));
                }
                self.diags.push(Diagnostic::error(method.span, msg));
                for a in args {
                    let _ = self.synth(env, a);
                }
                if let Some(b) = block {
                    let _ = self.synth(env, b);
                }
                return Type::Error;
            }
            // Static-vs-instance call form filter, driven by
            // `FnTy.is_static`. `T.method(...)` keeps only static
            // overloads; `instance.method(...)` keeps only instance
            // ones. When the chosen direction has no candidates but
            // the other does, that's a category error (e.g.
            // `someTimer.singleShot(...)` calling a static method on
            // an instance, or `QTimer.start()` calling an instance
            // method via the type). The one exception is extern-value
            // classes called as `T.method(...)`: their non-`static fn`
            // factory/utility methods predate the explicit annotation
            // and the codegen has long lowered them as `T::method(...)`,
            // so fall back to the full overload set rather than
            // breaking existing bindings.
            let is_extern_value = matches!(
                self.program.items.get(class_name),
                Some(pixie_hir::ItemKind::Class {
                    is_extern_value: true,
                    ..
                })
            );
            let overloads: Vec<crate::table::FnTy> = if receiver_is_type_ident {
                let static_only: Vec<_> = all_overloads
                    .iter()
                    .filter(|f| f.is_static)
                    .cloned()
                    .collect();
                if !static_only.is_empty() {
                    static_only
                } else if is_extern_value {
                    all_overloads.clone()
                } else {
                    self.diags.push(Diagnostic::error(
                        method.span,
                        format!(
                            "method `{}` on class `{}` is an instance method; call it on an instance, not the type",
                            method.name, class_name
                        ),
                    ));
                    for a in args {
                        let _ = self.synth(env, a);
                    }
                    if let Some(b) = block {
                        let _ = self.synth(env, b);
                    }
                    return Type::Error;
                }
            } else {
                let instance_only: Vec<_> = all_overloads
                    .iter()
                    .filter(|f| !f.is_static)
                    .cloned()
                    .collect();
                if !instance_only.is_empty() {
                    instance_only
                } else {
                    self.diags.push(Diagnostic::error(
                        method.span,
                        format!(
                            "method `{}` on class `{}` is static; call it as `{}.{}(...)` rather than on an instance",
                            method.name, class_name, class_name, method.name
                        ),
                    ));
                    for a in args {
                        let _ = self.synth(env, a);
                    }
                    if let Some(b) = block {
                        let _ = self.synth(env, b);
                    }
                    return Type::Error;
                }
            };
            self.check_member_pub(class_name, &method.name, method.span);
            let arg_tys: Vec<Type> = args.iter().map(|a| self.synth_no_check(env, a)).collect();
            let chosen = match crate::table::resolve_overload(
                &overloads,
                &arg_tys,
                block.is_some(),
                self.program,
            ) {
                crate::table::OverloadResolution::Unique(f) => Some(f.clone()),
                crate::table::OverloadResolution::NoArityMatch { arities } => {
                    if overloads.len() == 1 {
                        let only = &overloads[0];
                        let mut diag = Diagnostic::error(
                            call_span,
                            format!(
                                "method `{}` on class `{}` expects {} argument(s), got {}",
                                method.name,
                                class_name,
                                only.params.len(),
                                args.len(),
                            ),
                        );
                        diag.notes.push((
                            call_span,
                            format!(
                                "signature: {}",
                                render_call_signature(&only.params, Some(&only.ret))
                            ),
                        ));
                        self.diags.push(diag);
                    } else {
                        self.diags.push(Diagnostic::error(
                            call_span,
                            format!(
                                "no overload of method `{}` on class `{}` accepts {} argument(s) (declared arities: {})",
                                method.name,
                                class_name,
                                args.len(),
                                render_arities(&arities),
                            ),
                        ));
                    }
                    for a in args {
                        let _ = self.synth(env, a);
                    }
                    if let Some(b) = block {
                        let _ = self.synth(env, b);
                    }
                    return Type::Error;
                }
                crate::table::OverloadResolution::NoTypeMatch { tier3_winners } => {
                    self.diags.push(Diagnostic::error(
                        call_span,
                        format!(
                            "no overload of method `{}` on class `{}` matches argument types {} (candidates: {})",
                            method.name,
                            class_name,
                            render_arg_types(&arg_tys),
                            render_overload_candidates(&tier3_winners, &method.name),
                        ),
                    ));
                    for a in args {
                        let _ = self.synth(env, a);
                    }
                    if let Some(b) = block {
                        let _ = self.synth(env, b);
                    }
                    return Type::Error;
                }
                crate::table::OverloadResolution::Ambiguous { winners } => {
                    self.diags.push(Diagnostic::error(
                        call_span,
                        format!(
                            "ambiguous call to method `{}` on class `{}` (candidates: {})",
                            method.name,
                            class_name,
                            render_overload_candidates(&winners, &method.name),
                        ),
                    ));
                    for a in args {
                        let _ = self.synth(env, a);
                    }
                    if let Some(b) = block {
                        let _ = self.synth(env, b);
                    }
                    return Type::Error;
                }
                crate::table::OverloadResolution::Empty => None,
            };
            if let Some(m) = chosen {
                if !m.generics.is_empty() {
                    return self.synth_generic_call(env, &m, args, block, call_span);
                }
                self.check_fn_args(env, &m.params, Some(&m.ret), args, block, call_span);
                return m.ret;
            }
            return Type::Error;
        }
        // Generic instantiated receiver: `box.put(x)` where box is
        // `Box<Int>`. Look up the overload set with class-generic
        // substitution applied so the param/ret types are concrete,
        // then run the resolver.
        if let Type::Generic { base, args: t_args } = recv_ty {
            let overloads =
                self.table
                    .lookup_method_instantiated_overloads(base, t_args, &method.name);
            if !overloads.is_empty() {
                self.check_member_pub(base, &method.name, method.span);
                let arg_tys: Vec<Type> = args.iter().map(|a| self.synth_no_check(env, a)).collect();
                let chosen = match crate::table::resolve_overload(
                    &overloads,
                    &arg_tys,
                    block.is_some(),
                    self.program,
                ) {
                    crate::table::OverloadResolution::Unique(f) => Some(f.clone()),
                    crate::table::OverloadResolution::NoArityMatch { arities } => {
                        if overloads.len() == 1 {
                            let only = &overloads[0];
                            let mut diag = Diagnostic::error(
                                call_span,
                                format!(
                                    "method `{}` on `{}` expects {} argument(s), got {}",
                                    method.name,
                                    base,
                                    only.params.len(),
                                    args.len(),
                                ),
                            );
                            diag.notes.push((
                                call_span,
                                format!(
                                    "signature: {}",
                                    render_call_signature(&only.params, Some(&only.ret))
                                ),
                            ));
                            self.diags.push(diag);
                        } else {
                            self.diags.push(Diagnostic::error(
                                call_span,
                                format!(
                                    "no overload of method `{}` on `{}` accepts {} argument(s) (declared arities: {})",
                                    method.name,
                                    base,
                                    args.len(),
                                    render_arities(&arities),
                                ),
                            ));
                        }
                        for a in args {
                            let _ = self.synth(env, a);
                        }
                        if let Some(b) = block {
                            let _ = self.synth(env, b);
                        }
                        return Type::Error;
                    }
                    crate::table::OverloadResolution::NoTypeMatch { tier3_winners } => {
                        self.diags.push(Diagnostic::error(
                            call_span,
                            format!(
                                "no overload of method `{}` on `{}` matches argument types {} (candidates: {})",
                                method.name,
                                base,
                                render_arg_types(&arg_tys),
                                render_overload_candidates(&tier3_winners, &method.name),
                            ),
                        ));
                        for a in args {
                            let _ = self.synth(env, a);
                        }
                        if let Some(b) = block {
                            let _ = self.synth(env, b);
                        }
                        return Type::Error;
                    }
                    crate::table::OverloadResolution::Ambiguous { winners } => {
                        self.diags.push(Diagnostic::error(
                            call_span,
                            format!(
                                "ambiguous call to method `{}` on `{}` (candidates: {})",
                                method.name,
                                base,
                                render_overload_candidates(&winners, &method.name),
                            ),
                        ));
                        for a in args {
                            let _ = self.synth(env, a);
                        }
                        if let Some(b) = block {
                            let _ = self.synth(env, b);
                        }
                        return Type::Error;
                    }
                    crate::table::OverloadResolution::Empty => None,
                };
                if let Some(m) = chosen {
                    if !m.generics.is_empty() {
                        return self.synth_generic_call(env, &m, args, block, call_span);
                    }
                    self.check_fn_args(env, &m.params, Some(&m.ret), args, block, call_span);
                    return m.ret;
                }
                return Type::Error;
            }
            // Built-in generic with no class entry (List, Map, ...).
            // Before soft-passing, check whether `<base>` has a
            // registered impl declaring `method.name` — if so, route
            // through `synth_generic_call` so method-level generics
            // get inferred and recorded for codegen's
            // `inferred_type_args_at`. Mirrors the trait-bound path
            // for direct calls in concrete contexts.
            if let Some(ret) =
                self.try_dispatch_via_impl(env, recv_ty, method, args, block, call_span)
            {
                return ret;
            }
            for a in args {
                let _ = self.synth(env, a);
            }
            if let Some(b) = block {
                let _ = self.synth(env, b);
            }
            return Type::Unknown;
        }
        // External receiver with a registered impl: route through
        // the trait method's signature so method-level generics get
        // inferred. `let p : QPoint = ...; p.map_to(lambda)` records
        // `U → <lambda's ret>` into `generic_instantiations` so
        // codegen emits `::cute::trait_impl::Mapper::map_to<X>(p, lambda)`.
        if let Some(ret) = self.try_dispatch_via_impl(env, recv_ty, method, args, block, call_span)
        {
            return ret;
        }
        // External / Unknown / Future / etc: soft-pass, but try to
        // catch typos. If `method.name` doesn't exactly match any
        // known method across bound classes AND there's a close
        // match (Levenshtein <= 2), emit a warning that points to
        // the likely intended method. Distance > 2 is treated as a
        // genuinely unknown method on a foreign class - silent
        // pass so users don't get spammed for legitimate calls into
        // unbound libraries.
        if let Some((suggest_class, suggest_method)) = self.suggest_close_method(&method.name) {
            self.diags.push(Diagnostic::warning(
                method.span,
                format!(
                    "no method `{}` on the receiver's type; did you mean `{}` (from `{}`)?",
                    method.name, suggest_method, suggest_class
                ),
            ));
        }
        for a in args {
            let _ = self.synth(env, a);
        }
        if let Some(b) = block {
            let _ = self.synth(env, b);
        }
        Type::Unknown
    }

    /// Type-check `.connect(handler)` / `.disconnect()` on a
    /// `Type::SignalRef`. Anything else on a signal is a hard error —
    /// signals aren't ordinary callables.
    ///
    /// `connect` accepts the handler in three shapes (mirroring what
    /// codegen lowers to a callable):
    ///   - `connect { |a, b| ... }` — explicit-param lambda; bidirectional
    ///     inference fills in unannotated param types.
    ///   - `connect { ... }` — bare block; treated as a no-arg handler
    ///     (codegen wraps it in `[]() { ... }`). Only valid when the
    ///     signal has zero params.
    ///   - `connect(fn_ref)` — single positional arg of any callable type.
    ///
    /// Handler return is checked against `Type::Unknown` rather than Void
    /// so handlers may end with any expression — Qt discards signal-
    /// handler returns anyway.
    fn synth_signal_method_call(
        &mut self,
        env: &mut TypeEnv<'_>,
        class: &str,
        signal: &str,
        params: &[Type],
        method: &Ident,
        args: &[Expr],
        block: Option<&Expr>,
        call_span: Span,
    ) -> Type {
        match method.name.as_str() {
            "connect" => {
                self.check_signal_connect_handler(
                    env, class, signal, params, args, block, call_span,
                );
                Type::void()
            }
            "disconnect" => {
                if !args.is_empty() || block.is_some() {
                    self.diags.push(Diagnostic::error(
                        call_span,
                        format!("`disconnect` on signal `{class}::{signal}` takes no arguments"),
                    ));
                    self.walk_args_for_diagnostics(env, args, block);
                }
                Type::void()
            }
            other => {
                self.diags.push(Diagnostic::error(
                    method.span,
                    format!(
                        "no method `{other}` on signal `{class}::{signal}`; only `connect` and `disconnect` are supported"
                    ),
                ));
                self.walk_args_for_diagnostics(env, args, block);
                Type::Error
            }
        }
    }

    /// Validate the handler shape passed to `obj.signal.connect(...)`.
    /// Splits on the three accepted forms (lambda / bare block / other
    /// callable expression) so each gets the right diagnostic. Internal
    /// to `synth_signal_method_call`.
    fn check_signal_connect_handler(
        &mut self,
        env: &mut TypeEnv<'_>,
        class: &str,
        signal: &str,
        params: &[Type],
        args: &[Expr],
        block: Option<&Expr>,
        call_span: Span,
    ) {
        // Resolve to exactly one handler: trailing block wins, else the
        // single positional arg.
        let handler: &Expr = match (args.len(), block) {
            (0, Some(b)) => b,
            (1, None) => &args[0],
            _ => {
                let provided = args.len() + if block.is_some() { 1 } else { 0 };
                self.diags.push(Diagnostic::error(
                    call_span,
                    format!(
                        "`connect` on signal `{class}::{signal}` expects exactly one handler, got {provided}"
                    ),
                ));
                self.walk_args_for_diagnostics(env, args, block);
                return;
            }
        };
        // Bare block (no `|...|`): implicit no-arg callable. Codegen
        // wraps as `[]() { ... }`. Only valid when the signal has zero
        // params; otherwise the user needs `{ |a, b| ... }`.
        if let ExprKind::Block(b) = &handler.kind {
            if !params.is_empty() {
                self.diags.push(Diagnostic::error(
                    handler.span,
                    format!(
                        "signal `{class}::{signal}` handler expects {} param(s); use `{{ |...| ... }}` to bind them",
                        params.len()
                    ),
                ));
            }
            let mut sub = env.child();
            self.check_block(&mut sub, b, &Type::void());
            return;
        }
        // Lambda with mismatched arity: emit a signal-specific
        // diagnostic before falling through to body synth, so users
        // see "signal X::name handler expects N param(s)" rather than
        // the bare Fn-vs-Fn render the generic check would produce.
        if let ExprKind::Lambda {
            params: lam_params, ..
        } = &handler.kind
        {
            if lam_params.len() != params.len() {
                self.diags.push(Diagnostic::error(
                    handler.span,
                    format!(
                        "signal `{class}::{signal}` handler expects {} param(s), got {}",
                        params.len(),
                        lam_params.len()
                    ),
                ));
                let _ = self.synth(env, handler);
                return;
            }
        }
        // Lambda (arity-matched) or any other callable expression:
        // delegate to bidirectional `check` against the synthesized
        // handler shape. The (Lambda, Fn) arm at the bottom of this
        // file handles param-type binding + body walking; the
        // fallthrough handles non-Lambda callables (fn refs etc.).
        // Unknown return soft-passes whatever the body produces — Qt
        // discards signal-handler returns.
        let expected = Type::Fn {
            params: params.to_vec(),
            ret: Box::new(Type::Unknown),
        };
        let _ = self.check(env, handler, &expected);
    }

    /// Direct-call dispatch routing for trait-impl methods on value-typed
    /// receivers (extern types like `QPoint`, builtin generics like
    /// `List<T>`). Mirrors the trait-bound (`Type::Var`) branch in
    /// `synth_method_call`: when the receiver's base name has a
    /// registered impl declaring `method.name`, build a `FnTy` from
    /// the trait method's `FnDecl` and route through `synth_generic_call`
    /// so method-level generics get unified against call-site args and
    /// recorded into `generic_instantiations`. Codegen reads them via
    /// `inferred_type_args_at` and emits the explicit `<X>` template arg
    /// at the namespace dispatch.
    ///
    /// Returns `None` when no matching impl exists — caller falls back
    /// to its normal soft-pass.
    fn try_dispatch_via_impl(
        &mut self,
        env: &mut TypeEnv<'_>,
        recv_ty: &Type,
        method: &Ident,
        args: &[Expr],
        block: Option<&Expr>,
        call_span: Span,
    ) -> Option<Type> {
        let base = match recv_ty {
            Type::External(name) => name.clone(),
            Type::Generic { base, .. } => base.clone(),
            _ => return None,
        };
        let traits = self.program.impls_for.get(&base)?;
        // Collect every trait method matching name across all impl'd
        // traits — overloaded trait method support (e.g. trait declares
        // `fn x` and `fn x(y: Int)`, impl supplies both, call site picks
        // the right overload by arg type).
        let mut candidate_decls: Vec<pixie_syntax::ast::FnDecl> = Vec::new();
        for trait_name in traits {
            let Some(pixie_hir::ItemKind::Trait { methods, .. }) =
                self.program.items.get(trait_name)
            else {
                continue;
            };
            for m in methods {
                if m.name == method.name {
                    candidate_decls.push(m.fn_decl.clone());
                }
            }
        }
        if candidate_decls.is_empty() {
            return None;
        }
        // Single candidate: bypass the resolver and route directly.
        if candidate_decls.len() == 1 {
            let fnty = self.instantiate_trait_method_fnty(&candidate_decls[0], recv_ty);
            return Some(self.synth_generic_call(env, &fnty, args, block, call_span));
        }
        // Multi-overload: resolve by arg type.
        let candidate_fntys: Vec<crate::table::FnTy> = candidate_decls
            .iter()
            .map(|d| self.instantiate_trait_method_fnty(d, recv_ty))
            .collect();
        let arg_tys: Vec<Type> = args.iter().map(|a| self.synth_no_check(env, a)).collect();
        let chosen = match crate::table::resolve_overload(
            &candidate_fntys,
            &arg_tys,
            block.is_some(),
            self.program,
        ) {
            crate::table::OverloadResolution::Unique(f) => Some(f.clone()),
            crate::table::OverloadResolution::NoArityMatch { arities } => {
                self.diags.push(Diagnostic::error(
                    call_span,
                    format!(
                        "no overload of trait method `{}` accepts {} argument(s) (declared arities: {})",
                        method.name,
                        args.len(),
                        render_arities(&arities),
                    ),
                ));
                self.walk_args_for_diagnostics(env, args, block);
                return Some(Type::Error);
            }
            crate::table::OverloadResolution::NoTypeMatch { tier3_winners } => {
                self.diags.push(Diagnostic::error(
                    call_span,
                    format!(
                        "no overload of trait method `{}` matches argument types {} (candidates: {})",
                        method.name,
                        render_arg_types(&arg_tys),
                        render_overload_candidates(&tier3_winners, &method.name),
                    ),
                ));
                self.walk_args_for_diagnostics(env, args, block);
                return Some(Type::Error);
            }
            crate::table::OverloadResolution::Ambiguous { winners } => {
                self.diags.push(Diagnostic::error(
                    call_span,
                    format!(
                        "ambiguous call to trait method `{}` (candidates: {})",
                        method.name,
                        render_overload_candidates(&winners, &method.name),
                    ),
                ));
                self.walk_args_for_diagnostics(env, args, block);
                return Some(Type::Error);
            }
            crate::table::OverloadResolution::Empty => None,
        };
        chosen.map(|fnty| self.synth_generic_call(env, &fnty, args, block, call_span))
    }

    /// Build a `FnTy` for one trait method at a use site. Allocates
    /// fresh `VarId`s for each method-level generic, then walks the
    /// declared param + return types substituting first the
    /// generic-name → Var mapping and then `Self → recv_ty`. Shared
    /// between the trait-bound branch (`recv_ty = Type::Var(v)`) and
    /// the direct-call branch (`recv_ty` = the concrete external /
    /// generic type) of `synth_method_call`.
    ///
    /// Trait methods don't carry an explicit `self` parameter, so the
    /// returned FnTy's `params` line up with the call's positional
    /// args 1:1.
    fn instantiate_trait_method_fnty(
        &mut self,
        fn_decl: &pixie_syntax::ast::FnDecl,
        recv_ty: &Type,
    ) -> FnTy {
        let generics: Vec<VarId> = (0..fn_decl.generics.len())
            .map(|_| self.var_source.fresh())
            .collect();
        let generic_bounds: Vec<Vec<String>> = fn_decl
            .generics
            .iter()
            .map(|g| g.bounds.iter().map(|b| b.name.clone()).collect())
            .collect();
        let name_to_var: std::collections::HashMap<String, VarId> = fn_decl
            .generics
            .iter()
            .zip(generics.iter())
            .map(|(g, v)| (g.name.name.clone(), *v))
            .collect();
        let lower_param = |t: &pixie_syntax::ast::TypeExpr| {
            substitute_self(
                &substitute_names(&lower_type(t, self.program), &name_to_var),
                recv_ty,
            )
        };
        let params: Vec<Type> = fn_decl.params.iter().map(|p| lower_param(&p.ty)).collect();
        let ret = fn_decl
            .return_ty
            .as_ref()
            .map(|t| lower_param(t))
            .unwrap_or(Type::void());
        FnTy {
            generics,
            generic_bounds,
            params,
            ret,
            is_static: false,
        }
    }

    /// Walk every method known to `ProgramTable.classes` looking
    /// for a name within edit distance 2 of `name`. Returns
    /// `(class_name, method_name)` of the best match, or None when
    /// no close candidate exists. Exact matches are skipped (those
    /// are the legitimate-call-on-unbound case, not a typo).
    fn suggest_close_method(&self, name: &str) -> Option<(String, String)> {
        let mut best: Option<(usize, String, String)> = None;
        for (class_name, entry) in &self.table.classes {
            for method_name in entry.methods.keys() {
                if method_name == name {
                    return None; // exact match exists somewhere - not a typo
                }
                let d = levenshtein(name, method_name);
                if d == 0 || d > 2 {
                    continue;
                }
                let candidate = (d, class_name.clone(), method_name.clone());
                best = match best {
                    Some((bd, _, _)) if bd <= d => best,
                    _ => Some(candidate),
                };
            }
        }
        best.map(|(_, c, m)| (c, m))
    }

    /// Common arity + per-arg check for both top-level fn calls and
    /// class method calls. The trailing `block`, when present, becomes
    /// Walk args + an optional trailing block for side-effect diagnostics
    /// after a parent error has already been pushed (e.g. resolver
    /// `NoArityMatch` / `NoTypeMatch` / `Ambiguous`). Return values are
    /// dropped because the call as a whole already failed; the synth is
    /// only there to surface nested errors like an unbound name in an
    /// arg expression.
    fn walk_args_for_diagnostics(
        &mut self,
        env: &mut TypeEnv<'_>,
        args: &[Expr],
        block: Option<&Expr>,
    ) {
        for a in args {
            let _ = self.synth(env, a);
        }
        if let Some(b) = block {
            let _ = self.synth(env, b);
        }
    }

    /// the (n+1)th argument if the callee accepts one more parameter
    /// than the positional list - this is how `f(args) { |x| ... }` is
    /// typed against a fn whose last param is `Fn(...)`.
    /// `ret` is threaded in only for the signature note attached to
    /// the arity-mismatch diagnostic; pass `None` when the call form
    /// has no meaningful return (error-variant constructors).
    fn check_fn_args(
        &mut self,
        env: &mut TypeEnv<'_>,
        params: &[Type],
        ret: Option<&Type>,
        args: &[Expr],
        block: Option<&Expr>,
        call_span: Span,
    ) {
        let positional = args.len();
        let block_count = if block.is_some() { 1 } else { 0 };
        let provided = positional + block_count;
        if provided != params.len() {
            let diag = Diagnostic::error(
                call_span,
                format!(
                    "function expects {} argument(s), got {}",
                    params.len(),
                    provided
                ),
            )
            .with_note(
                call_span,
                format!("signature: {}", render_call_signature(params, ret)),
            );
            self.diags.push(diag);
        }
        for (i, a) in args.iter().enumerate() {
            if let Some(p) = params.get(i) {
                self.check(env, a, p);
            } else {
                let _ = self.synth(env, a);
            }
        }
        if let Some(b) = block {
            if let Some(p) = params.get(positional) {
                self.check(env, b, p);
            } else {
                let _ = self.synth(env, b);
            }
        }
    }

    fn synth_binary(&mut self, op: BinOp, l: &Type, r: &Type, l_sp: Span, r_sp: Span) -> Type {
        use BinOp::*;
        match op {
            Add => {
                // String + any-printable lowers to QString concat (the
                // codegen path emits QString::number / arg() / similar
                // depending on the operand). Accept it on either side
                // - QML and the C++ runtime both Just Work.
                if matches!(l, Type::Prim(Prim::String)) && self.is_printable(r) {
                    return Type::string();
                }
                if matches!(r, Type::Prim(Prim::String)) && self.is_printable(l) {
                    return Type::string();
                }
                self.expect_numeric_or_string(l, l_sp);
                self.expect_numeric_or_string(r, r_sp);
                if l == r {
                    l.clone()
                } else if matches!(l, Type::Error | Type::Unknown) {
                    r.clone()
                } else if matches!(r, Type::Error | Type::Unknown) {
                    l.clone()
                } else if matches!(
                    (l, r),
                    (Type::Prim(Prim::Int), Type::Prim(Prim::Float))
                        | (Type::Prim(Prim::Float), Type::Prim(Prim::Int))
                ) {
                    // Int + Float promotes to Float; matches the
                    // is_subtype Int→Float widening already used
                    // in fn-arg / let-annotation contexts. Record
                    // WHICH side widened so codegen can emit the
                    // cast (§8.55) — it has no types of its own.
                    if matches!(l, Type::Prim(Prim::Int)) {
                        self.int_to_float.insert(l_sp);
                    } else {
                        self.int_to_float.insert(r_sp);
                    }
                    Type::float()
                } else {
                    self.diags.push(Diagnostic::error(
                        r_sp,
                        format!(
                            "operands of `Add` must have the same type, got `{}` and `{}`",
                            l.render(),
                            r.render()
                        ),
                    ));
                    Type::Error
                }
            }
            Sub | Mul | Div | Mod => {
                self.expect_numeric_or_string(l, l_sp);
                self.expect_numeric_or_string(r, r_sp);
                if l == r {
                    l.clone()
                } else if matches!(l, Type::Error | Type::Unknown) {
                    r.clone()
                } else if matches!(r, Type::Error | Type::Unknown) {
                    l.clone()
                } else if matches!(
                    (l, r),
                    (Type::Prim(Prim::Int), Type::Prim(Prim::Float))
                        | (Type::Prim(Prim::Float), Type::Prim(Prim::Int))
                ) {
                    // Int <-> Float numeric promotion: matches the
                    // `is_subtype` rule that already widens Int to
                    // Float in passes-as-arg position (§8.55 records
                    // the widened side for codegen).
                    if matches!(l, Type::Prim(Prim::Int)) {
                        self.int_to_float.insert(l_sp);
                    } else {
                        self.int_to_float.insert(r_sp);
                    }
                    Type::float()
                } else {
                    self.diags.push(Diagnostic::error(
                        r_sp,
                        format!(
                            "operands of `{:?}` must have the same type, got `{}` and `{}`",
                            op,
                            l.render(),
                            r.render()
                        ),
                    ));
                    Type::Error
                }
            }
            Lt | LtEq | Gt | GtEq => {
                self.expect_numeric(l, l_sp);
                self.expect_numeric(r, r_sp);
                Type::bool()
            }
            Eq | NotEq => {
                if !is_subtype(l, r, self.program) && !is_subtype(r, l, self.program) {
                    self.diags.push(Diagnostic::error(
                        r_sp,
                        format!("cannot compare `{}` with `{}`", l.render(), r.render()),
                    ));
                }
                Type::bool()
            }
            And | Or => {
                self.expect(l, &Type::bool(), l_sp);
                self.expect(r, &Type::bool(), r_sp);
                Type::bool()
            }
            BitOr | BitAnd | BitXor => {
                // Bitwise ops on enum / flags / Int values. Three
                // accepted operand-pair shapes:
                //   - flags op flags          → flags  (combine)
                //   - flags op enum           → flags  (Q_DECLARE_OPERATORS_FOR_FLAGS gives us this)
                //   - enum  op enum           → flags  (when both belong to the same `flags X of E` decl, lifts to that flags type — checked below)
                //   - Int   op Int            → Int
                // Cute doesn't accept other operand types — `String
                // | Bool` is a type error rather than a shrug-pass.
                let bitwise_int =
                    matches!(l, Type::Prim(Prim::Int)) && matches!(r, Type::Prim(Prim::Int));
                let bitwise_flags = matches!(l, Type::Flags(_) | Type::Enum(_))
                    && matches!(r, Type::Flags(_) | Type::Enum(_));
                if bitwise_int {
                    return Type::int();
                }
                if bitwise_flags {
                    // Pick the flags side as the result; if both
                    // are enum (no flags decl), promote to the
                    // first operand's enum (Cute treats it as a
                    // flags-bag at the C++ level since
                    // Q_DECLARE_OPERATORS_FOR_FLAGS makes the
                    // result a QFlags<E>).
                    if let Type::Flags(_) = l {
                        return l.clone();
                    }
                    if let Type::Flags(_) = r {
                        return r.clone();
                    }
                    return l.clone();
                }
                if matches!(l, Type::Error | Type::Unknown)
                    || matches!(r, Type::Error | Type::Unknown)
                {
                    return Type::int();
                }
                let op_sym = match op {
                    BinOp::BitOr => "|",
                    BinOp::BitAnd => "&",
                    BinOp::BitXor => "^",
                    _ => "?",
                };
                self.diags.push(Diagnostic::error(
                    r_sp,
                    format!(
                        "bitwise `{op_sym}` requires two Int / enum / flags operands, got `{}` and `{}`",
                        l.render(),
                        r.render()
                    ),
                ));
                Type::Error
            }
        }
    }

    // ---- check (against expected) ---------------------------------------

    fn check(&mut self, env: &mut TypeEnv<'_>, e: &Expr, expected: &Type) -> Type {
        // Narrowing rules where check beats synth:
        // - `nil` literal: accept against any Nullable directly.
        // - Lambda against an `Fn { params, ret }` expected: bind each
        //   lambda param to the corresponding expected param when the
        //   lambda left it untyped (parser fills `_` placeholder), so
        //   the body can be checked with the real types from the call
        //   site rather than `External("_")`.
        if matches!(&e.kind, ExprKind::Nil) && matches!(expected, Type::Nullable(_)) {
            return expected.clone();
        }
        // Collection literals: propagate the expected element types so
        // `let xs : List<Int> = [1, "a"]` reports on the "a", instead of
        // the literal soft-passing as `Type::Error`.
        if let (ExprKind::Array(items), Type::Generic { base, args }) = (&e.kind, expected) {
            if (base == "List" || base == "Set") && args.len() == 1 {
                for it in items {
                    self.check(env, it, &args[0]);
                }
                return expected.clone();
            }
        }
        if let (ExprKind::Map(entries), Type::Generic { base, args }) = (&e.kind, expected) {
            if (base == "Map" || base == "Hash") && args.len() == 2 {
                for (k, v) in entries {
                    // Identifier keys are string-key sugar; only check
                    // explicit key expressions against K.
                    if !matches!(&k.kind, ExprKind::Ident(_)) {
                        self.check(env, k, &args[0]);
                    }
                    self.check(env, v, &args[1]);
                }
                return expected.clone();
            }
        }
        // Positional construction against a generic expectation:
        // `fn swapped Pair<T> { Pair(b, a) }` — the constructor call
        // adopts the expected instantiation (args synth soft; rustc
        // verifies the emitted literal, §8.25).
        if let (ExprKind::Call { callee, args, .. }, Type::Generic { base, .. }) =
            (&e.kind, expected)
        {
            if let ExprKind::Ident(n) = &callee.kind {
                if n == base {
                    for a in args {
                        let _ = self.synth(env, a);
                    }
                    return expected.clone();
                }
            }
        }
        // Generic-class instantiation propagation: when expected is
        // `Generic{base, args}` and the value is `T.new(args)` whose
        // receiver matches `base` AND `T` is declared as a generic
        // class, accept the call as instantiated to the expected
        // args. Records the binding in `generic_instantiations` so
        // codegen can emit the typed-template form even though no
        // type args appear in the syntax.
        //
        // Covers: `let b: Box<Int> = Box.new()`, `var b: Box<Int> = ...`,
        // `put(Box.new())` where `fn put(b: Box<Int>)`,
        // `obj.set(Box.new())` against a generic-typed param,
        // `return Box.new()` from a `-> Box<Int>` fn — all reach this
        // path through `check`.
        if let Type::Generic { base, args } = expected {
            if let ExprKind::MethodCall {
                receiver,
                method,
                args: call_args,
                block,
                ..
            } = &e.kind
            {
                if method.name == "new" {
                    if let ExprKind::Ident(class_name) = &receiver.kind {
                        if class_name == base
                            && self
                                .table
                                .classes
                                .get(class_name)
                                .map(|entry| !entry.class_generics.is_empty())
                                .unwrap_or(false)
                        {
                            // Walk constructor args / block for nested
                            // type errors so the rest of the checker
                            // still runs (constructor params remain
                            // soft-passed, mirroring the standard
                            // `T.new(...)` synth path).
                            for a in call_args {
                                let _ = self.synth(env, a);
                            }
                            if let Some(b) = block {
                                let _ = self.synth(env, b);
                            }
                            // Record the call's span so codegen can
                            // emit the instantiated-template form.
                            self.generic_instantiations.insert(e.span, args.clone());
                            return expected.clone();
                        }
                    }
                }
            }
        }
        if let (
            ExprKind::Lambda { params, body },
            Type::Fn {
                params: ep,
                ret: er,
            },
        ) = (&e.kind, expected)
        {
            if params.len() == ep.len() {
                let mut sub = env.child();
                let mut bound_params = Vec::with_capacity(params.len());
                for (lp, et) in params.iter().zip(ep.iter()) {
                    let declared = lower_type(&lp.ty, self.program);
                    let bind = if is_placeholder(&declared) {
                        et.clone()
                    } else {
                        if !is_subtype(et, &declared, self.program) {
                            self.diags.push(Diagnostic::error(
                                lp.span,
                                format!(
                                    "lambda parameter declared `{}` but call site expects `{}`",
                                    declared.render(),
                                    et.render()
                                ),
                            ));
                        }
                        declared
                    };
                    sub.bind(lp.name.name.clone(), bind.clone());
                    bound_params.push(bind);
                }
                // Walk the body's stmts for type errors, then synth the
                // trailing expression so its actual return type comes
                // back to the call site (where unification can bind
                // generic vars in the expected return). When the
                // expected return is concrete, also do an explicit
                // subtype check so non-Fn-related expectations report
                // a clean diagnostic.
                for stmt in &body.stmts {
                    self.check_stmt(&mut sub, stmt);
                }
                let actual_ret = if let Some(t) = &body.trailing {
                    self.synth(&mut sub, t)
                } else {
                    Type::void()
                };
                if !is_subtype(&actual_ret, er, self.program) {
                    if let Some(t) = &body.trailing {
                        self.diags.push(Diagnostic::error(
                            t.span,
                            format!(
                                "lambda body returns `{}` but expected `{}`",
                                actual_ret.render(),
                                er.render()
                            ),
                        ));
                    }
                }
                return Type::Fn {
                    params: bound_params,
                    ret: Box::new(actual_ret),
                };
            }
        }
        let actual = self.synth(env, e);
        // §8.55: `is_subtype` widens Int to Float everywhere a value
        // flows into a checked position — a Float prop, a Float
        // parameter, an annotated `let`, a `+=` onto a Float. The
        // emitter has no types, so record the span here (the ONE place
        // the widening is decided) and let it emit the `as f64`.
        if matches!(actual, Type::Prim(Prim::Int)) && matches!(expected, Type::Prim(Prim::Float)) {
            self.int_to_float.insert(e.span);
        }
        if !is_subtype(&actual, expected, self.program) {
            self.diags.push(Diagnostic::error(
                e.span,
                format!(
                    "expected `{}`, found `{}`",
                    expected.render(),
                    actual.render()
                ),
            ));
        }
        actual
    }

    // ---- pattern bindings -----------------------------------------------

    /// Error when a `case` over a `Type::Enum` doesn't cover every
    /// declared variant and lacks a wildcard / bind catch-all.
    /// Mirrors Swift / Rust's exhaustiveness check. A hard error
    /// (§8.27): codegen's unmatched-input tail is a silent no-op, so
    /// letting a missing variant through would swallow behavior.
    fn check_case_exhaustiveness(
        &mut self,
        scrutinee_ty: &Type,
        arms: &[pixie_syntax::ast::CaseArm],
        case_span: Span,
    ) {
        let Type::Enum(enum_name) = scrutinee_ty else {
            return;
        };
        let Some(ItemKind::Enum { variants, .. }) = self.program.items.get(enum_name) else {
            return;
        };
        // Wildcard / bind / literal arms are exhaustive on their
        // own — no further variant-set check needed.
        if arms.iter().any(|a| {
            matches!(
                a.pattern,
                Pattern::Wild { .. } | Pattern::Bind { .. } | Pattern::Literal { .. }
            )
        }) {
            return;
        }
        let covered: std::collections::HashSet<&str> = arms
            .iter()
            .filter_map(|a| match &a.pattern {
                Pattern::Ctor { name, .. } => Some(name.name.as_str()),
                _ => None,
            })
            .collect();
        let missing: Vec<&str> = variants
            .iter()
            .map(|v| v.name.as_str())
            .filter(|n| !covered.contains(n))
            .collect();
        if missing.is_empty() {
            return;
        }
        let head = missing
            .iter()
            .take(3)
            .copied()
            .collect::<Vec<_>>()
            .join(", ");
        let suffix = if missing.len() > 3 {
            format!(" ({} more)", missing.len() - 3)
        } else {
            String::new()
        };
        self.diags.push(Diagnostic::error(
            case_span,
            format!(
                "non-exhaustive `case` on `{enum_name}`: missing {head}{suffix}. Add the missing arms or a `when _` catch-all."
            ),
        ));
    }

    /// Attempt to bind a `when VariantName(args)` pattern against
    /// the named enum's variants. Returns `true` when the variant
    /// resolved (success or arity / no-payload error pushed),
    /// `false` when the named enum isn't actually a declared enum
    /// (caller falls through to the error-variant path).
    fn bind_enum_variant_pattern(
        &mut self,
        env: &mut TypeEnv<'_>,
        enum_name: &str,
        name: &Ident,
        args: &[Pattern],
    ) -> bool {
        let Some(ItemKind::Enum { variants, .. }) = self.program.items.get(enum_name) else {
            return false;
        };
        let Some(variant) = variants.iter().find(|v| v.name == name.name).cloned() else {
            let suggestion = closest_within(&name.name, variants.iter().map(|v| v.name.as_str()));
            let mut msg = format!("no variant `{}` on enum `{}`", name.name, enum_name);
            if let Some(s) = suggestion {
                msg.push_str(&format!(" (did you mean `{s}`?)"));
            }
            self.diags.push(Diagnostic::error(name.span, msg));
            return true;
        };
        if variant.fields.is_empty() {
            if !args.is_empty() {
                self.diags.push(Diagnostic::error(
                    name.span,
                    format!(
                        "enum variant `{}::{}` takes no payload",
                        enum_name, name.name
                    ),
                ));
            }
            return true;
        }
        if args.len() != variant.fields.len() {
            self.diags.push(Diagnostic::error(
                name.span,
                format!(
                    "enum variant `{}::{}` takes {} field(s), got {} pattern(s)",
                    enum_name,
                    name.name,
                    variant.fields.len(),
                    args.len()
                ),
            ));
        }
        for (a, f) in args.iter().zip(variant.fields.iter()) {
            let f_ty = lower_type(&f.ty, self.program);
            self.bind_pattern(env, a, &f_ty);
        }
        true
    }

    fn bind_pattern(&mut self, env: &mut TypeEnv<'_>, p: &Pattern, scrutinee: &Type) {
        match p {
            Pattern::Wild { .. } => {}
            Pattern::Bind { name, .. } => {
                env.bind(name.name.clone(), scrutinee.clone());
            }
            Pattern::Literal { .. } => {}
            Pattern::Ctor { name, args, .. } => {
                let head = name.name.as_str();
                match head {
                    "ok" => {
                        if let Type::ErrorUnion { ok, .. } = scrutinee {
                            for a in args {
                                self.bind_pattern(env, a, ok);
                            }
                        } else if !matches!(
                            scrutinee,
                            Type::External(_) | Type::Unknown | Type::Error
                        ) {
                            self.diags.push(Diagnostic::error(
                                name.span,
                                format!(
                                    "`when ok(...)` requires `!T` scrutinee, got `{}`",
                                    scrutinee.render()
                                ),
                            ));
                        }
                    }
                    "err" => {
                        if let Type::ErrorUnion { err, .. } = scrutinee {
                            // Bind the err to `Type::Enum` when it
                            // resolves to an enum decl (`error E` or
                            // plain `enum E`), so the recursive
                            // `Pattern::Ctor` arm routes into the
                            // existing enum-variant handling and
                            // nested `when err(VariantName(payload))`
                            // typing comes for free.
                            let err_ty = match self.program.items.get(err) {
                                Some(ItemKind::Enum { .. }) => Type::Enum(err.clone()),
                                Some(_) => Type::Class(err.clone()),
                                None => Type::External(err.clone()),
                            };
                            for a in args {
                                self.bind_pattern(env, a, &err_ty);
                            }
                        } else if !matches!(
                            scrutinee,
                            Type::External(_) | Type::Unknown | Type::Error
                        ) {
                            self.diags.push(Diagnostic::error(
                                name.span,
                                format!(
                                    "`when err(...)` requires `!T` scrutinee, got `{}`",
                                    scrutinee.render()
                                ),
                            ));
                        }
                    }
                    "some" => {
                        // `when some(v)` / `if let some(v) = ...` —
                        // requires Nullable scrutinee. Bind the inner
                        // type for `v`.
                        if let Type::Nullable(inner) = scrutinee {
                            for a in args {
                                self.bind_pattern(env, a, inner);
                            }
                        } else if !matches!(
                            scrutinee,
                            Type::External(_) | Type::Unknown | Type::Error
                        ) {
                            self.diags.push(Diagnostic::error(
                                name.span,
                                format!(
                                    "`some(...)` pattern requires `T?` scrutinee, got `{}`",
                                    scrutinee.render()
                                ),
                            ));
                        }
                    }
                    "nil" => {
                        // `when nil` — no bindings, requires Nullable
                        // (or Unknown / External soft-pass).
                        if !matches!(
                            scrutinee,
                            Type::Nullable(_) | Type::External(_) | Type::Unknown | Type::Error
                        ) {
                            self.diags.push(Diagnostic::error(
                                name.span,
                                format!(
                                    "`nil` pattern requires `T?` scrutinee, got `{}`",
                                    scrutinee.render()
                                ),
                            ));
                        }
                    }
                    _ => {
                        // Enum variant discriminator: `case c { when Red
                        // { ... } }` on a Type::Enum scrutinee. Verify
                        // the variant exists; reject anything else as a
                        // Cute-side typo.
                        if let Type::Enum(enum_name) = scrutinee {
                            if self.bind_enum_variant_pattern(env, enum_name, name, args) {
                                return;
                            }
                        }
                        // Error-variant discriminator: bindings inside take
                        // External (we don't model variant payloads yet).
                        for a in args {
                            self.bind_pattern(env, a, &Type::Unknown);
                        }
                    }
                }
            }
        }
    }

    // ---- small expectation helpers --------------------------------------

    // ---- helpers --------------------------------------------------------

    fn expect(&mut self, actual: &Type, expected: &Type, span: Span) {
        if !is_subtype(actual, expected, self.program) {
            self.diags.push(Diagnostic::error(
                span,
                format!(
                    "expected `{}`, found `{}`",
                    expected.render(),
                    actual.render()
                ),
            ));
        }
    }

    fn expect_numeric(&mut self, t: &Type, span: Span) {
        match t {
            Type::Prim(Prim::Int | Prim::Float)
            | Type::Error
            | Type::Unknown
            | Type::External(_) => {}
            other => {
                self.diags.push(Diagnostic::error(
                    span,
                    format!("expected numeric type, found `{}`", other.render()),
                ));
            }
        }
    }

    fn expect_numeric_or_string(&mut self, t: &Type, span: Span) {
        match t {
            Type::Prim(Prim::Int | Prim::Float | Prim::String)
            | Type::Error
            | Type::Unknown
            | Type::External(_) => {}
            other => {
                self.diags.push(Diagnostic::error(
                    span,
                    format!(
                        "expected numeric or string operand, found `{}`",
                        other.render()
                    ),
                ));
            }
        }
    }

    /// True when `t` is a type that the codegen can render as a
    /// `QString` for concatenation: primitives map to `QString::number`
    /// / `QString::fromStdString` / `QString::asprintf` and friends;
    /// QObject-derived classes get `obj->toString()` if defined,
    /// otherwise fall through. Used by `Add` to allow `"x: " + value`
    /// when value is non-String. External and Unknown are accepted
    /// soft - they could be anything.
    fn is_printable(&self, t: &Type) -> bool {
        match t {
            Type::Prim(_) => true,
            Type::External(_) | Type::Unknown | Type::Error => true,
            Type::Class(_) | Type::Generic { .. } => true,
            // Enums and flags lower to int at the C++ level; format
            // via `QString::number(static_cast<int>(value))` either
            // way. Emitting the symbolic variant name would be
            // nicer but needs an enum-aware `toString` overload to
            // come along; print as int for now.
            Type::Enum(_) | Type::Flags(_) => true,
            Type::Nullable(inner) => self.is_printable(inner),
            Type::ErrorUnion { ok, .. } => self.is_printable(ok),
            Type::Fn { .. } | Type::Sym | Type::SignalRef { .. } | Type::Var(_) => false,
        }
    }
}

/// True when `t` is the placeholder type the parser assigns to untyped
/// block parameters (`|x|` rather than `|x: T|`). cute-syntax models
/// this as a Named type with the literal name `_`, which `lower_type`
/// produces as `Type::External("_")`.
fn is_placeholder(t: &Type) -> bool {
    matches!(t, Type::External(s) if s == "_")
}

/// Format a callable's parameter+return types for the signature note
/// attached to arity-mismatch diagnostics. Void returns and `None`
/// returns (constructor-style calls) drop the `-> ...` suffix.
/// Map a resolved `Type` to the simple name the `impls_for` map is
/// keyed by, for bound enforcement at generic-call sites. Returns
/// `None` for types that should soft-pass the check: an unresolved
/// `Var`, an `External` (no visibility into its impl set), `Unknown`,
/// or `Error`.
fn bound_check_target_name(t: &Type) -> Option<String> {
    match t {
        Type::Class(name) => Some(name.clone()),
        Type::Generic { base, .. } => Some(base.clone()),
        Type::Prim(Prim::Int) => Some("Int".to_string()),
        Type::Prim(Prim::Float) => Some("Float".to_string()),
        Type::Prim(Prim::Bool) => Some("Bool".to_string()),
        Type::Prim(Prim::String) => Some("String".to_string()),
        // Void / Nil — meaningless to bound-check; skip.
        Type::Prim(Prim::Void) | Type::Prim(Prim::Nil) => None,
        Type::Nullable(inner) => bound_check_target_name(inner),
        // External / Var / Unknown / Error / Fn / Sym / ErrorUnion all
        // soft-pass: we can't verify and don't want to falsely accuse.
        _ => None,
    }
}

fn render_call_signature(params: &[Type], ret: Option<&Type>) -> String {
    let p = params
        .iter()
        .map(|t| t.render())
        .collect::<Vec<_>>()
        .join(", ");
    match ret {
        Some(r) if !matches!(r, Type::Prim(Prim::Void)) => format!("({p}) -> {}", r.render()),
        _ => format!("({p})"),
    }
}

/// Render a list of arities like "0, 1, 3" for "no overload accepting K
/// args" diagnostics.
fn render_arities(arities: &[usize]) -> String {
    arities
        .iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Render the actual arg types tuple at a call site, e.g. `(String, Int)`.
fn render_arg_types(args: &[Type]) -> String {
    let inner = args
        .iter()
        .map(|t| t.render())
        .collect::<Vec<_>>()
        .join(", ");
    format!("({inner})")
}

/// Render an overload candidate set for diagnostics:
/// "foo(Int), foo(Int, Int)". Each FnTy shows up as `name(params) -> ret`
/// (or `name(params)` for void returns).
fn render_overload_candidates(candidates: &[&crate::table::FnTy], name: &str) -> String {
    candidates
        .iter()
        .map(|c| format!("{}{}", name, render_call_signature(&c.params, Some(&c.ret))))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Pick the closest candidate to `target` within Levenshtein distance
/// 2, excluding exact matches. Used by typo-suggestion at error sites
/// where we already know the legal name set (signals on a class,
/// methods on a class entry, error variants, etc.). Returns `None`
/// when an exact match exists in the candidate set (the call site
/// already failed for another reason — visibility, arity, …) or no
/// candidate is close enough to be useful.
fn closest_within<'a>(
    target: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Option<String> {
    let mut best: Option<(usize, String)> = None;
    for cand in candidates {
        if cand == target {
            return None;
        }
        let d = levenshtein(target, cand);
        if d == 0 || d > 2 {
            continue;
        }
        best = match best {
            Some((bd, _)) if bd <= d => best,
            _ => Some((d, cand.to_string())),
        };
    }
    best.map(|(_, n)| n)
}

/// Tiny Levenshtein distance used by the typo-suggestion helper.
/// Two-row DP, O(min(a, b)) memory. We only ever care about
/// distances <= 2, but the full computation is cheap enough that
/// the early-exit isn't worth complicating the code.
fn levenshtein(a: &str, b: &str) -> usize {
    let av: Vec<char> = a.chars().collect();
    let bv: Vec<char> = b.chars().collect();
    if av.is_empty() {
        return bv.len();
    }
    if bv.is_empty() {
        return av.len();
    }
    let mut prev: Vec<usize> = (0..=bv.len()).collect();
    let mut curr = vec![0usize; bv.len() + 1];
    for (i, ca) in av.iter().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in bv.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (curr[j] + 1).min(prev[j + 1] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[bv.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixie_hir::resolve;
    use pixie_syntax::{parse, span::FileId};

    fn check_str(src: &str) -> Vec<Diagnostic> {
        let m = parse(FileId(0), src).expect("parse");
        // Tag the test source as user module "test" so the
        // visibility check sees its items as User-home rather than
        // Prelude. Without this, the default empty ProjectInfo
        // would lump every item into the prelude and bypass member-
        // visibility rules entirely.
        let mut info = pixie_hir::ProjectInfo::default();
        info.module_for_file.insert(FileId(0), "test".into());
        let prog = resolve(&m, &info).program;
        let r = check_program(&m, &prog);
        r.diagnostics
    }

    /// Same as `check_str` but pre-loads an explicit `.rpi` binding so a
    /// foreign class surface is visible to the type checker (matches what
    /// the driver does with crate bindings in real builds).
    fn check_with_binding(binding_src: &str, src: &str) -> Vec<Diagnostic> {
        // Mirror the driver's setup: register the bindings first so they
        // get their own FileIds, then parse the user source against a
        // fresh FileId allocated through the same SourceMap. Without this
        // the user file would alias to the first binding's FileId and the
        // visibility check would mis-attribute every binding to the user
        // "test" module.
        let mut sm = pixie_syntax::SourceMap::default();
        let mut bindings = pixie_binding::load_stdlib(&mut sm).expect("stdlib loads");
        bindings.push(
            pixie_binding::parse_rpi(&mut sm, "test.rpi", binding_src).expect("binding parses"),
        );
        let user_fid = sm.add("test.cute".into(), src.to_string());
        let user_src = sm.source(user_fid).to_string();
        let m = parse(user_fid, &user_src).expect("parse");
        let mut items: Vec<pixie_syntax::ast::Item> = bindings
            .iter()
            .flat_map(|b| b.items.iter().cloned())
            .collect();
        items.extend(m.items.iter().cloned());
        let combined = pixie_syntax::ast::Module {
            items,
            span: m.span,
        };
        let mut info = pixie_hir::ProjectInfo::default();
        info.module_for_file.insert(user_fid, "test".into());
        let prog = resolve(&combined, &info).program;
        let r = check_program(&combined, &prog);
        r.diagnostics
    }

    /// Like `check_str` but keeps the resolve-phase diagnostics too —
    /// pixie's surface rejections (inheritance, `arc`) live in HIR's
    /// pass 0, which `check_str` alone would drop.
    fn check_str_full(src: &str) -> Vec<Diagnostic> {
        let mut sm = pixie_syntax::SourceMap::default();
        let fid = sm.add("test.pix".into(), src.to_string());
        let stored = sm.source(fid).to_string();
        let m = parse(fid, &stored).expect("parse");
        let mut info = pixie_hir::ProjectInfo::default();
        info.module_for_file.insert(fid, "test".into());
        let rr = resolve(&m, &info);
        let mut diags = rr.diagnostics;
        diags.extend(check_program(&m, &rr.program).diagnostics);
        diags
    }

    #[test]
    fn class_inheritance_is_rejected() {
        let d = check_str_full("class Base { }\nclass Derived < Base { }\n");
        assert!(
            d.iter().any(|x| x.message.contains("no inheritance")),
            "expected the D9 rejection, got: {:?}",
            d
        );
    }


    fn assert_clean(src: &str) {
        let d = check_str(src);
        assert!(d.is_empty(), "expected clean, got: {:?}", d);
    }

    #[test]
    fn list_literal_elements_check_against_annotation() {
        let d = check_str("fn f {\n  let xs : List<Int> = [1, \"a\"]\n}\n");
        assert!(
            d.iter().any(|x| x.message.contains("expected `Int`")),
            "expected element mismatch, got: {:?}",
            d
        );
    }

    #[test]
    fn typed_list_literal_is_clean() {
        assert_clean("fn f {\n  let xs : List<Int> = [1, 2, 3]\n}\n");
    }

    #[test]
    fn typed_map_literal_values_check() {
        let d = check_str("fn f {\n  let m : Map<String, Int> = { a: 1, b: \"x\" }\n}\n");
        assert!(
            d.iter().any(|x| x.message.contains("expected `Int`")),
            "expected value mismatch, got: {:?}",
            d
        );
    }

    fn assert_one_error_containing(src: &str, needle: &str) {
        let d = check_str(src);
        assert_eq!(d.len(), 1, "expected one error, got: {:?}", d);
        assert!(
            d[0].message.contains(needle),
            "message did not contain `{needle}`: {}",
            d[0].message
        );
    }

    #[test]
    fn private_member_blocked_from_outside_class() {
        let src = r#"
class Counter {
  prop count : Int, default: 0
}
fn run(c: Counter) {
  c.setCount(5)
}
"#;
        let d = check_str(src);
        assert!(
            d.iter()
                .any(|x| x.message.contains("`setCount` is private")),
            "expected setCount-private diagnostic, got: {:?}",
            d
        );
    }

    #[test]
    fn pub_member_visible_from_outside_class() {
        let src = r#"
class Counter {
  pub prop count : Int, default: 0
}
fn main {
  let c = Counter.new()
  c.setCount(5)
  let n = c.count
}
"#;
        assert_clean(src);
    }

    #[test]
    fn private_member_freely_visible_inside_own_class() {
        // Within a method of Counter, accessing self's own private
        // members is fine - the check only fires across classes.
        let src = r#"
class Counter {
  prop count : Int, default: 0
  signal changed
  fn bump {
    self.setCount(self.count + 1)
    emit changed
  }
}
"#;
        assert_clean(src);
    }

    #[test]
    fn binding_class_members_skip_visibility_check() {
        // Binding-declared members are exempt from the visibility check
        // regardless of how `pub` was declared in the .rpi file.
        let d = check_with_binding(
            "class Logger {\n  fn flush\n}\n",
            r#"
fn cleanup(l: Logger) {
  l.flush()
}
"#,
        );
        assert!(
            d.iter().all(|x| !x.message.contains("is private")),
            "binding member should not trigger visibility error: {:?}",
            d
        );
    }

    #[test]
    fn spec_todo_item_is_clean() {
        assert_clean(
            r#"
class TodoItem {
  prop text : String, default: ""
  prop done : Bool, notify: :stateChanged

  signal stateChanged

  fn toggle {
    done = !done
    emit stateChanged
  }
}
"#,
        );
    }

    #[test]
    fn spec_error_handling_is_clean() {
        assert_clean(
            r#"
error FileError {
  notFound
  permissionDenied
  ioError(message: String)
}

fn open(path: String) !File {
  try File.open(path)
}

fn loadConfig(path: String) !Config {
  let file = try File.open(path)
  let text = try file.readAll
  try parse(text)
}

fn run {
  case loadConfig("/etc/cute.conf") {
    when ok(cfg)  { apply(cfg) }
    when err(e)   { log(e) }
  }
}
"#,
        );
    }

    #[test]
    fn assigning_string_to_bool_property_errors() {
        assert_one_error_containing(
            r#"
class X {
  prop done : Bool, default: false

  fn setDone {
    done = "yes"
  }
}
"#,
            "expected `Bool`, found `String`",
        );
    }

    #[test]
    fn try_on_non_error_union_errors() {
        assert_one_error_containing(
            r#"
fn run {
  let x = 42
  let _ = try x
}
"#,
            "`?` requires an `!T`",
        );
    }

    #[test]
    fn await_is_type_transparent() {
        // pixie: `await` marks execution mode (worker dispatch); the
        // operand keeps its type — what may actually be awaited is the
        // emitter's call. Transparency shows up as a plain type error
        // when the awaited value is bound against the wrong type.
        let d = check_str(
            r#"
fn run {
  let x = 42
  let _ = await x
}
"#,
        );
        assert!(d.is_empty(), "{d:?}");
        assert_one_error_containing(
            r#"
fn run {
  let x = 42
  let s : String = await x
}
"#,
            "expected `String`",
        );
    }

    #[test]
    fn unary_not_on_int_errors() {
        let d = check_str(
            r#"
fn run {
  let x = 1
  let _ = !x
}
"#,
        );
        assert!(!d.is_empty());
        assert!(d[0].message.contains("expected `Bool`"), "{}", d[0].message);
    }

    #[test]
    fn binary_arith_on_string_int_errors() {
        // String + Int is now a deliberate convenience: codegen
        // lowers it to QString concat. The type checker accepts it
        // and produces `String`. Other type mismatches in arith
        // (e.g. `Int * String`) are still errors.
        let d = check_str(
            r#"
fn run {
  let s = "hi"
  let n = 1
  let _ = s + n
}
"#,
        );
        assert!(d.is_empty(), "String + Int should be allowed: {:?}", d);

        let d = check_str(
            r#"
fn run {
  let s = "hi"
  let n = 1
  let _ = n * s
}
"#,
        );
        assert!(!d.is_empty(), "expected error for Int * String");
        assert!(d[0].message.contains("same type"), "got: {}", d[0].message);
    }

    #[test]
    fn class_chain_subtype_assignment_is_clean() {
        // B is a subclass of A; passing B where A is expected is fine.
        assert_clean(
            r#"
class A {}
class B < A {}

fn take(x: A) {}

fn run(b: B) {
  take(b)
}
"#,
        );
    }

    #[test]
    fn nullable_property_accepts_nil() {
        assert_clean(
            r#"
class X {
  let maybe : String?

  fn clear {
    maybe = nil
  }
}
"#,
        );
    }

    /// `nope = true` inside a class with no member named `nope`
    /// silently introduces a local declaration (Cute's
    /// declaration-on-first-assign rule). This is a deliberate
    /// post-`@`-retirement behaviour: bare names default to local
    /// scope when nothing else in scope binds them. Member typos that
    /// are *close* to a real member still get the typo-suggest
    /// diagnostic — see `typo_in_at_property_suggests_close_match`.
    #[test]
    fn unknown_bare_assign_target_in_class_creates_local() {
        let d = check_str(
            r#"
class X {
  fn run {
    nope = true
  }
}
"#,
        );
        assert!(
            d.iter().all(|d| !d.message.contains("no property")),
            "should not flag `nope` as a property — there are no members to suggest from: {:?}",
            d
        );
    }

    #[test]
    fn emit_unknown_signal_errors() {
        let d = check_str(
            r#"
class X {
  signal known
  fn run {
    emit unknown
  }
}
"#,
        );
        assert!(d.iter().any(|d| d.message.contains("no signal `unknown`")));
    }

    // ---- ProgramTable / dispatch ----------------------------------------

    #[test]
    fn calling_unknown_method_on_own_class_errors() {
        let d = check_str(
            r#"
class X {
  fn known {}
}

fn run(x: X) {
  x.unknown()
}
"#,
        );
        assert!(
            d.iter().any(|d| d.message.contains("no method `unknown`")),
            "{:?}",
            d
        );
    }

    #[test]
    fn arity_mismatch_attaches_signature_note() {
        let d = check_str(
            r#"
fn compute(a: Float, op: String, b: Float) Float { a }

fn run {
  let r = compute(1.0, "+")
}
"#,
        );
        assert!(
            d.iter()
                .any(|d| d.message.contains("function expects 3 argument(s), got 2")),
            "missing arity error: {:?}",
            d
        );
        assert!(
            d.iter().any(|d| d
                .notes
                .iter()
                .any(|(_, n)| n.contains("signature: (Float, String, Float) -> Float"))),
            "missing signature note: {:?}",
            d
        );
    }

    #[test]
    fn typo_in_method_name_suggests_close_match() {
        let d = check_str(
            r#"
class X {
  fn increment {}
}

fn run(x: X) {
  x.incremnt()
}
"#,
        );
        assert!(
            d.iter()
                .any(|d| d.message.contains("did you mean `increment`")),
            "expected suggestion in: {:?}",
            d
        );
    }

    #[test]
    fn typo_in_at_property_suggests_close_match() {
        let d = check_str(
            r#"
class X {
  prop count : Int, default: 0
  fn run {
    cont = 1
  }
}
"#,
        );
        assert!(
            d.iter().any(|d| d.message.contains("did you mean `count`")),
            "{:?}",
            d
        );
    }

    #[test]
    fn typo_in_signal_name_suggests_close_match() {
        let d = check_str(
            r#"
class X {
  signal valueChanged
  fn run {
    emit valueChnged
  }
}
"#,
        );
        assert!(
            d.iter()
                .any(|d| d.message.contains("did you mean `valueChanged`")),
            "{:?}",
            d
        );
    }

    #[test]
    fn method_arg_type_is_checked() {
        let d = check_str(
            r#"
class X {
  fn rename(name: String) {}
}

fn run(x: X) {
  x.rename(42)
}
"#,
        );
        assert!(
            d.iter()
                .any(|d| d.message.contains("expected `String`, found `Int`")),
            "{:?}",
            d
        );
    }

    #[test]
    fn method_return_type_propagates() {
        // square()'s ret = Int, and we assign it to a property of type Bool.
        // Should error: "expected Bool, found Int".
        let d = check_str(
            r#"
class X {
  prop done : Bool, default: false
  fn square(n: Int) Int { n }
  fn run {
    done = self.square(3)
  }
}
"#,
        );
        assert!(
            d.iter()
                .any(|d| d.message.contains("expected `Bool`, found `Int`")),
            "{:?}",
            d
        );
    }

    #[test]
    fn member_access_returns_property_type() {
        // x.text: String -> assigning to a Bool field errors.
        let d = check_str(
            r#"
class X {
  prop text : String, default: ""
  prop flag : Bool, default: false
  fn run(other: X) {
    flag = other.text
  }
}
"#,
        );
        assert!(
            d.iter()
                .any(|d| d.message.contains("expected `Bool`, found `String`")),
            "{:?}",
            d
        );
    }

    #[test]
    fn unknown_property_on_own_class_errors() {
        let d = check_str(
            r#"
class X {
  prop text : String, default: ""
  fn run(other: X) {
    let _ = other.nope
  }
}
"#,
        );
        assert!(
            d.iter().any(|d| d.message.contains("no member `nope`")),
            "{:?}",
            d
        );
    }

    #[test]
    fn top_level_fn_arg_count_is_checked() {
        let d = check_str(
            r#"
fn add(a: Int, b: Int) Int { a }

fn run {
  let _ = add(1)
}
"#,
        );
        assert!(
            d.iter()
                .any(|d| d.message.contains("expects 2 argument(s), got 1")),
            "{:?}",
            d
        );
    }

    #[test]
    fn top_level_fn_arg_types_are_checked() {
        let d = check_str(
            r#"
fn greet(name: String) {}

fn run {
  greet(42)
}
"#,
        );
        assert!(
            d.iter()
                .any(|d| d.message.contains("expected `String`, found `Int`")),
            "{:?}",
            d
        );
    }

    // ---- Lambda inference ----------------------------------------------

    #[test]
    fn untyped_lambda_param_infers_from_callee() {
        // The closure's `x` should be inferred as Int from the fn
        // signature. Since `Add` accepts `Int + String` (lowering to
        // QString concat), we test the inference using a strict op
        // instead: `x * "no"` is `Int * String`, which does NOT have
        // the lenient String escape - it must error.
        let d = check_str(
            r#"
fn forEach(xs: Int, f: fn(Int) -> Bool) {}

fn run {
  forEach(0) { |x|
    x * "no"
  }
}
"#,
        );
        assert!(
            d.iter()
                .any(|d| d.message.contains("must have the same type")),
            "expected arith mismatch from inferred Int*String, got: {:?}",
            d
        );
    }

    // ---- View body type checking --------------------------------------

    #[test]
    fn view_body_string_concat_with_int_is_clean() {
        // The relaxed `Add` rule lets view bodies write
        // `text: "count: " + n` without tripping the strict
        // same-type rule. Codegen lowers this to QString concat.
        assert_clean(
            r#"
class Counter {
  pub prop count : Int, default: 0
}

view Main {
  let counter = Counter()
  ApplicationWindow {
    Label { text: "Count: " + counter.count }
  }
}
"#,
        );
    }

    #[test]
    fn style_entry_value_is_type_checked() {
        // `font.bold: 1 + "x"` arithmetic mismatch surfaces from the
        // style body now that the type checker walks it. The
        // `style: X` element-site desugar runs in codegen so the
        // ELEMENT-site target check happens via `check_element_property`
        // after the style is inlined.
        let d = check_str(
            r#"
style Card {
  padding: 1 * "twelve"
}
"#,
        );
        assert!(
            d.iter()
                .any(|e| e.message.contains("must have the same type")),
            "expected arith mismatch in style body: {:?}",
            d
        );
    }

    #[test]
    fn user_generic_class_substitutes_t_on_member_access() {
        // `class Box<T> { property item: T }` plus a typed binding
        // `var b: Box<Int>` should let `b.item` synth as `Int` (not
        // External("T")). The setter `setItem` accepts the same Int.
        let d = check_str(
            r#"
class Box<T> {
  let Item : T
}

fn run(b: Box<Int>) Int {
  b.Item
}
"#,
        );
        assert!(
            d.is_empty(),
            "Box<Int>.Item should type-check as Int: {:?}",
            d
        );
    }

    #[test]
    fn user_generic_class_setter_rejects_wrong_type() {
        // `setItem` was registered as taking T, which is the class's
        // generic VarId. A `Box<Int>` receiver substitutes T = Int,
        // so passing a String literal should error.
        let d = check_str(
            r#"
class Box<T> {
  pub var item : T
}

fn run(b: Box<Int>) {
  b.setItem("nope")
}
"#,
        );
        assert!(
            d.iter().any(|e| e.message.contains("expected")),
            "expected type mismatch on setItem(String) when Box<Int>: {:?}",
            d
        );
    }

    #[test]
    fn generic_class_let_annotation_accepts_bare_new() {
        // Form (a) generic instantiation: `let b: Box<Int> = Box.new()`
        // type-checks even though `Box.new()` synths as the bare
        // `Class("Box")`. The let arm of check_stmt special-cases this
        // shape so users don't have to spell `Box<Int>.new()`.
        let d = check_str(
            r#"
class Box<T> {
  let Item : T
}

fn run {
  let i: Box<Int> = Box.new()
  let s: Box<String> = Box.new()
  i.setItem(42)
  s.setItem("hello")
}
"#,
        );
        assert!(
            d.is_empty(),
            "Box<T>.new() with let annotation should type-check: {:?}",
            d
        );
    }

    #[test]
    fn generic_class_fn_arg_propagation_accepts_bare_new() {
        // Form (c) — fn arg propagation: when the called fn's
        // parameter is `Box<Int>`, the bare `Box.new()` argument is
        // accepted and instantiated to match.
        let d = check_str(
            r#"
class Box<T> {
  let Item : T
}

fn put(b: Box<Int>) {
  b.setItem(99)
}

fn run {
  put(Box.new())
}
"#,
        );
        assert!(
            d.is_empty(),
            "fn-arg propagation should accept Box.new(): {:?}",
            d
        );
    }

    #[test]
    fn generic_class_return_type_propagation_accepts_bare_new() {
        // Return-type propagation: when the fn's return type is
        // `Box<Int>`, the trailing `Box.new()` is accepted.
        let d = check_str(
            r#"
class Box<T> {
  let Item : T
}

fn make Box<Int> {
  let b: Box<Int> = Box.new()
  b.setItem(0)
  b
}
"#,
        );
        assert!(
            d.is_empty(),
            "return-type propagation should type-check: {:?}",
            d
        );
    }

    #[test]
    fn method_level_generic_records_inferred_arg() {
        // `class Container<T> { fn transform<U>(f: fn(T) -> U) -> U }`
        // — calling `c.transform { |x| ... }` should both type-check
        // and record the inferred U at the call's span so codegen
        // can emit the explicit template arg.
        let module = pixie_syntax::parse(
            pixie_syntax::span::FileId(0),
            r#"
class Container<T> {
  let Item : T
  pub fn transform<U>(f: fn(T) -> U) U {
    f(Item)
  }
}

fn run {
  let c: Container<Int> = Container.new()
  let s = c.transform { |x: Int| "got" }
}
"#,
        )
        .unwrap();
        let resolved = pixie_hir::resolve(&module, &pixie_hir::ProjectInfo::default());
        let result = check_program(&module, &resolved.program);
        assert!(
            result.diagnostics.is_empty(),
            "expected clean type check, got {:?}",
            result.diagnostics
        );
        // Two instantiations: the Container.new() and the transform call.
        assert!(
            result.generic_instantiations.len() >= 2,
            "expected at least 2 instantiations, got {}: {:?}",
            result.generic_instantiations.len(),
            result.generic_instantiations
        );
        // One of them should have args=[String] (the U from the
        // lambda body's String return).
        let saw_string = result
            .generic_instantiations
            .values()
            .any(|args| args.iter().any(|t| matches!(t, Type::Prim(Prim::String))));
        assert!(
            saw_string,
            "expected to record String for U, got {:?}",
            result.generic_instantiations
        );
    }

    #[test]
    fn generic_class_records_instantiation_for_codegen() {
        // The check pass should populate `generic_instantiations`
        // for every accepted `T.new()` against a generic-typed
        // expectation, so codegen can emit the typed-template form.
        let module = pixie_syntax::parse(
            pixie_syntax::span::FileId(0),
            r#"
class Box<T> {
  let Item : T
}

fn put(b: Box<Int>) { b.setItem(1) }

fn run {
  let i: Box<String> = Box.new()
  put(Box.new())
}
"#,
        )
        .unwrap();
        let resolved = pixie_hir::resolve(&module, &pixie_hir::ProjectInfo::default());
        let result = check_program(&module, &resolved.program);
        assert!(
            result.diagnostics.is_empty(),
            "expected clean type check, got {:?}",
            result.diagnostics
        );
        // Two instantiations expected: one for the let-annotated
        // String box, one for the fn-arg propagated Int box.
        assert_eq!(
            result.generic_instantiations.len(),
            2,
            "expected 2 instantiations, got {}: {:?}",
            result.generic_instantiations.len(),
            result.generic_instantiations
        );
    }

    #[test]
    fn generic_class_let_annotation_still_checks_method_arg_types() {
        // Form (a) shouldn't bypass downstream type checks: after
        // `let i: Box<Int> = Box.new()`, calling `i.setItem("nope")`
        // must still fail because T = Int.
        let d = check_str(
            r#"
class Box<T> {
  pub var item : T
}

fn run {
  let i: Box<Int> = Box.new()
  i.setItem("nope")
}
"#,
        );
        assert!(
            d.iter().any(|e| e.message.contains("expected")),
            "expected type mismatch on setItem(String) when Box<Int>: {:?}",
            d
        );
    }

    #[test]
    fn lambda_return_type_inferred_from_trailing_expression() {
        // The body's trailing expr is `n + 1` -> Int, so the lambda
        // synthesizes as `fn(Int) -> Int`. Calling .map(...) on a
        // List<Int> with this lambda type-checks against the generic
        // signature (`map<T, U>(f: fn(T) -> U) -> List<U>`).
        let d = check_str(
            r#"
fn map<T, U>(xs: List<T>, f: fn(T) -> U) List<U> { xs }

fn run(xs: List<Int>) List<Int> {
  map(xs) { |n: Int| n + 1 }
}
"#,
        );
        // We just want: no Add operand mismatch and no return-type
        // mismatch. Soft-pass on remaining details is fine.
        assert!(
            !d.iter()
                .any(|e| e.message.contains("must have the same type")),
            "lambda body should type-check cleanly: {:?}",
            d
        );
        assert!(
            !d.iter().any(|e| e.message.contains("return")),
            "lambda return type should match List<Int>: {:?}",
            d
        );
    }

    #[test]
    fn view_body_member_visibility_fires() {
        // `count` is private; accessing it through a state field
        // from a view body must error - the visibility check now
        // runs on view bodies too.
        let d = check_str(
            r#"
class Counter {
  prop count : Int, default: 0
}

view Main {
  let counter = Counter()
  ApplicationWindow {
    Label { text: "x" + counter.count }
  }
}
"#,
        );
        assert!(
            d.iter().any(|d| d.message.contains("`count` is private")),
            "expected count-private diagnostic, got: {:?}",
            d
        );
    }

    #[test]
    fn typed_lambda_param_overrides_callee() {
        // Explicit annotation `|x: String|` takes precedence; callee
        // expects Int. We currently flag the disagreement.
        let d = check_str(
            r#"
fn forEach(xs: Int, f: fn(Int) -> Bool) {}

fn run {
  forEach(0) { |x: String|
    true
  }
}
"#,
        );
        assert!(
            d.iter().any(|d| d.message.contains("call site expects")),
            "{:?}",
            d
        );
    }

    // ---- stdlib bindings (.qpi) ----------------------------------------

    #[test]
    fn bound_class_method_is_callable() {
        // A class declared only in an `.rpi` binding: calls resolve
        // against the bound method table instead of soft-passing.
        let d = check_with_binding(
            "class Logger {\n  fn flush\n  fn log(msg: String)\n}\n",
            r#"
fn run(l: Logger) {
  l.flush()
}
"#,
        );
        assert!(d.is_empty(), "expected clean: {:?}", d);
    }

    #[test]
    fn typo_on_bound_class_method_is_caught() {
        // A bound class has a real method table, so a typo is caught
        // instead of soft-passing as External.
        let d = check_with_binding(
            "class Logger {\n  fn flush\n}\n",
            r#"
fn run(l: Logger) {
  l.flus()
}
"#,
        );
        assert!(
            d.iter().any(|d| d.message.contains("no method `flus`")),
            "expected typo to be caught, got: {:?}",
            d
        );
    }

    #[test]
    fn bound_class_method_is_typed() {
        // log(msg: String) - passing a Bool should error.
        let d = check_with_binding(
            "class Logger {\n  fn log(msg: String)\n}\n",
            r#"
fn run(l: Logger) {
  l.log(true)
}
"#,
        );
        assert!(
            d.iter()
                .any(|d| d.message.contains("expected `String`, found `Bool`")),
            "expected setter type-mismatch, got: {:?}",
            d
        );
    }

    // ---- Stage B: generic fn inference ---------------------------------

    #[test]
    fn generic_identity_fn_infers_t_from_arg() {
        // `id(42)` should return Int (T inferred from argument).
        // We surface the inferred type by checking against a property's
        // declared type: assigning `id(42)` to a Bool property must error
        // with "expected Bool, found Int".
        let d = check_str(
            r#"
fn id<T>(x: T) T { x }

class X {
  prop flag : Bool, default: false
  fn run {
    flag = id(42)
  }
}
"#,
        );
        assert!(
            d.iter()
                .any(|d| d.message.contains("expected `Bool`, found `Int`")),
            "expected Bool/Int mismatch from inferred T=Int, got: {:?}",
            d
        );
    }

    #[test]
    fn generic_fn_arg_type_drives_t_inference() {
        // `head(xs)` where xs: List<String> should return String, so
        // assigning to a Bool property errors.
        let d = check_str(
            r#"
fn head<T>(xs: List<T>) T { xs }

class X {
  prop flag : Bool, default: false
  fn run(xs: List<String>) {
    flag = head(xs)
  }
}
"#,
        );
        assert!(
            d.iter()
                .any(|d| d.message.contains("expected `Bool`, found `String`")),
            "expected Bool/String mismatch, got: {:?}",
            d
        );
    }

    #[test]
    fn generic_fn_two_params_unify_consistently() {
        // `pair(1, 2)` is fine: both args are Int, T = Int.
        // `pair(1, "x")` should fail at the second arg.
        let d_ok = check_str(
            r#"
fn pair<T>(a: T, b: T) T { a }

fn run {
  let _ = pair(1, 2)
}
"#,
        );
        assert!(d_ok.is_empty(), "expected clean: {:?}", d_ok);

        let d_fail = check_str(
            r#"
fn pair<T>(a: T, b: T) T { a }

fn run {
  let _ = pair(1, "x")
}
"#,
        );
        assert!(
            !d_fail.is_empty(),
            "expected mismatch when T can't be both Int and String"
        );
    }

    #[test]
    fn generic_fn_nested_in_generic_arg() {
        // `wrap(xs)` where xs: List<Int> -> List<List<Int>>.
        // Then assigning to List<List<String>> must fail.
        let d = check_str(
            r#"
fn wrap<T>(xs: List<T>) List<List<T>> { xs }

class X {
  let bag : List<List<String>>
  fn run(xs: List<Int>) {
    bag = wrap(xs)
  }
}
"#,
        );
        assert!(
            !d.is_empty(),
            "expected error: List<List<Int>> vs List<List<String>>"
        );
    }

    #[test]
    fn generic_fn_passes_when_fully_consistent() {
        let d = check_str(
            r#"
fn head<T>(xs: List<T>) T { xs }

class X {
  prop name : String, default: ""
  fn run(xs: List<String>) {
    name = head(xs)
  }
}
"#,
        );
        assert!(d.is_empty(), "expected clean: {:?}", d);
    }

    #[test]
    fn external_method_call_does_not_error() {
        // File and read_all aren't bound; should soft-pass without diagnostics.
        assert_clean(
            r#"
fn run {
  let x = File.open("p")
  let _ = x.readAll()
}
"#,
        );
    }

    #[test]
    fn safe_member_lifts_result_to_nullable() {
        // `p?.name` on a `Person?` returns `String?`. The `let n :
        // String? = ...` annotation forces the checker to verify
        // assignability — if the safe access didn't lift to nullable
        // it would synth `String` and the annotation check would
        // either accept (unsound) or reject. Either outcome is
        // observable so the assert pins the right answer.
        assert_clean(
            r#"
class Person {
  pub prop name : String, default: ""
}

fn greet(p: Person?) {
  let n : String? = p?.name
}
"#,
        );
    }

    #[test]
    fn safe_method_call_lifts_result_to_nullable() {
        assert_clean(
            r#"
class Greeter {
  pub fn shout(who: String) String { "" }
}

fn run(g: Greeter?) {
  let s : String? = g?.shout("world")
}
"#,
        );
    }

    #[test]
    fn safe_member_does_not_double_wrap_already_nullable_member() {
        // `?.maybe` on a `Person?` where `maybe : String?` should
        // produce `String?`, not `String??`. The annotation pins the
        // expected shape — if the lifter ever double-wraps, the
        // assignment would mismatch.
        assert_clean(
            r#"
class Person {
  pub let maybe : String?
}

fn run(p: Person?) {
  let n : String? = p?.maybe
}
"#,
        );
    }

    /// `fn first<T: Iterable>(xs: T)` called with a class `MyList`
    /// that has `impl Iterable for MyList { ... }` should type-check.
    #[test]
    fn generic_call_with_satisfied_bound_is_clean() {
        assert_clean(
            r#"
trait Iterable { fn iter Int }
class MyList {
  prop n : Int, default: 0
}
impl Iterable for MyList {
  fn iter Int { 0 }
}
fn first<T: Iterable>(xs: T) T { xs }
fn run {
  let xs = MyList.new()
  let _ = first(xs)
}
"#,
        );
    }

    /// `fn first<T: Iterable>` called with a class that does NOT
    /// implement `Iterable` should error at the call site.
    #[test]
    fn generic_call_with_missing_bound_errors() {
        assert_one_error_containing(
            r#"
trait Iterable { fn iter Int }
class NotIter {
  prop n : Int, default: 0
}
fn first<T: Iterable>(xs: T) T { xs }
fn run {
  let xs = NotIter.new()
  let _ = first(xs)
}
"#,
            "does not implement trait `Iterable`",
        );
    }

    /// Multiple bounds on the same generic param: each must be
    /// independently satisfied. Missing one of two should error.
    #[test]
    fn generic_call_with_partial_multi_bound_errors() {
        let src = r#"
trait A { fn a Int }
trait B { fn b Int }
class HasA { prop n : Int, default: 0 }
impl A for HasA { fn a Int { 1 } }
fn useBoth<T: A + B>(xs: T) T { xs }
fn run {
  let xs = HasA.new()
  let _ = useBoth(xs)
}
"#;
        let d = check_str(src);
        assert_eq!(d.len(), 1, "expected one error, got: {:?}", d);
        assert!(
            d[0].message.contains("does not implement trait `B`"),
            "should call out missing trait B, got: {}",
            d[0].message
        );
    }

    /// Bare generic (no bounds) should accept any concrete type
    /// without bound errors — backward compat.
    #[test]
    fn bare_generic_accepts_any_type() {
        assert_clean(
            r#"
class Foo { prop n : Int, default: 0 }
fn id<T>(xs: T) T { xs }
fn run {
  let xs = Foo.new()
  let _ = id(xs)
}
"#,
        );
    }

    /// `xs.method()` inside `fn use_it<T: Foo>(xs: T)` is accepted
    /// when the trait declares the method. Body-side lookup uses
    /// the trait's declared surface; the receiver's static type is
    /// `Type::Var` (a generic), so the existing class-method path
    /// doesn't run.
    #[test]
    fn body_call_to_trait_method_is_clean() {
        assert_clean(
            r#"
trait Foo { fn x Int }
fn useIt<T: Foo>(thing: T) Int {
  thing.x()
}
"#,
        );
    }

    /// `xs.bogus()` inside a bounded body — `bogus` isn't on the
    /// trait surface, so the call is rejected at the Cute source
    /// rather than waiting for a C++ template instantiation
    /// failure. Diagnostic names the offending method + bound list.
    #[test]
    fn body_call_to_unknown_method_on_bound_generic_errors() {
        let src = r#"
trait Foo { fn x Int }
fn useIt<T: Foo>(thing: T) Int {
  thing.bogus()
}
"#;
        let d = check_str(src);
        let msg = d
            .iter()
            .find(|d| d.message.contains("no method `bogus`"))
            .expect("expected lookup-failure diagnostic");
        assert!(
            msg.message.contains("`Foo`"),
            "should name the bound trait, got: {}",
            msg.message
        );
    }

    /// Multiple bounds: any trait that declares the method is
    /// enough to accept the call. The check unions the surfaces.
    #[test]
    fn body_call_resolves_via_any_of_multiple_bounds() {
        assert_clean(
            r#"
trait Reader { fn read Int }
trait Writer { fn write Int }
fn useIt<T: Reader + Writer>(thing: T) Int {
  thing.read()
}
"#,
        );
    }

    /// Bare generic (no bounds) keeps the existing soft-pass:
    /// without a bound, we have no surface to check against, so
    /// `xs.anything()` is accepted (the C++ template instantiation
    /// is the backstop).
    #[test]
    fn body_call_on_bare_generic_soft_passes() {
        assert_clean(
            r#"
fn id<T>(thing: T) T {
  thing
}
"#,
        );
        // And calling something on the bare-generic param falls
        // back to the External / Var soft-pass.
        assert_clean(
            r#"
fn touch<T>(thing: T) T {
  let _ = thing.foo()
  thing
}
"#,
        );
    }

    /// External types (not declared in the user module) soft-pass
    /// the bound check — we don't have visibility into their impl
    /// set. C++ template instantiation is the backstop.
    #[test]
    fn external_type_satisfies_any_bound_softly() {
        // `QStringList` isn't declared anywhere in the source; the
        // checker treats it as External and skips bound enforcement.
        assert_clean(
            r#"
trait Iterable { fn iter Int }
fn first<T: Iterable>(xs: T) T { xs }
fn run(xs: QStringList) {
  let _ = first(xs)
}
"#,
        );
    }

    /// Body-side calls inherit the trait method's declared return
    /// type. `xs.x()` returns `Int` here, so passing the result to
    /// a `String` slot is a type error — this is the "signature
    /// flows through" smoke test.
    #[test]
    fn body_call_to_trait_method_returns_declared_type() {
        let src = r#"
trait Foo { fn x Int }
fn useIt<T: Foo>(thing: T) String {
  thing.x()
}
"#;
        let d = check_str(src);
        assert!(
            d.iter().any(|d| d.message.contains("expected `String`")
                || d.message.contains("`String`") && d.message.contains("`Int`")),
            "expected a return-type mismatch diagnostic, got: {:?}",
            d
        );
    }

    /// Trait method declares no params; calling it with one is an
    /// arity error pinned to the call span.
    #[test]
    fn body_call_with_extra_arg_is_arity_error() {
        let src = r#"
trait Foo { fn x Int }
fn useIt<T: Foo>(thing: T) Int {
  thing.x(42)
}
"#;
        let d = check_str(src);
        let arity = d
            .iter()
            .find(|d| d.message.contains("expects 0 argument") && d.message.contains("got 1"))
            .expect(&format!("expected arity diagnostic, got: {:?}", d));
        assert!(arity.message.contains("0 argument"));
    }

    /// Trait method declares one Int param; calling it with a
    /// String value triggers the type-mismatch path.
    #[test]
    fn body_call_with_wrong_arg_type_errors() {
        let src = r#"
trait Foo { fn put(n: Int) Int }
fn useIt<T: Foo>(thing: T) Int {
  thing.put("hello")
}
"#;
        let d = check_str(src);
        assert!(
            d.iter().any(|d| d.message.contains("expected `Int`")
                || (d.message.contains("`Int`") && d.message.contains("`String`"))),
            "expected arg-type mismatch diagnostic, got: {:?}",
            d
        );
    }

    /// Trait method declares one Int param; calling it with an Int
    /// is clean. Pins the happy-path so the new sig-check doesn't
    /// over-reject.
    #[test]
    fn body_call_with_matching_arg_type_is_clean() {
        assert_clean(
            r#"
trait Foo { fn put(n: Int) Int }
fn useIt<T: Foo>(thing: T) Int {
  thing.put(42)
}
"#,
        );
    }

    /// **Method-level generic inference at a trait-bound call site.**
    /// `thing.map_to(lambda)` on a `<T: Mapper>` body must allocate
    /// a fresh Var for the method's `U`, unify it with the lambda's
    /// return type, and record the inferred binding into
    /// `generic_instantiations` keyed on the call's span — so codegen
    /// can emit the explicit `<::cute::String>` template arg
    /// downstream. Without this, C++ template deduction can't bind
    /// `U` through a raw lambda passed as `std::function<U(qint64)>`.
    #[test]
    fn body_trait_method_with_own_generic_records_inferred_arg() {
        let module = pixie_syntax::parse(
            pixie_syntax::span::FileId(0),
            r#"
trait Mapper {
  fn mapTo<U>(f: fn(Int) -> U) U
}

fn run<T: Mapper>(thing: T) {
  let s : String = thing.mapTo({ |x: Int| "got" })
}
"#,
        )
        .unwrap();
        let resolved = pixie_hir::resolve(&module, &pixie_hir::ProjectInfo::default());
        let result = check_program(&module, &resolved.program);
        assert!(
            result.diagnostics.is_empty(),
            "expected clean type check, got {:?}",
            result.diagnostics
        );
        // Look for a recorded instantiation with `String` as one of
        // the args — this is the trait method's `U` being bound to
        // String from the lambda body's trailing expression.
        let saw_string = result
            .generic_instantiations
            .values()
            .any(|args| args.iter().any(|t| matches!(t, Type::Prim(Prim::String))));
        assert!(
            saw_string,
            "expected `String` to be recorded for `U`, got {:?}",
            result.generic_instantiations
        );
    }

    /// **Method-level generic inference threads through the lambda's
    /// param type, too.** When the trait method's `f` parameter has
    /// type `fn(Int) -> U` and the user passes a typed lambda with
    /// `Int` arg, the inferred `U` should still bind to the lambda's
    /// return. Pin the case where the wider signature uses two
    /// generics to avoid accidental over-coupling between recv and
    /// method generics.
    #[test]
    fn body_trait_method_with_own_generic_unifies_through_lambda_return() {
        let src = r#"
trait Mapper {
  fn mapTo<U>(f: fn(Int) -> U) U
}

fn run<T: Mapper>(thing: T) {
  let s : Int = thing.mapTo({ |x: Int| x + 1 })
}
"#;
        // Should be clean — the lambda returns Int, U binds to Int,
        // and the let's `Int` annotation matches the return.
        assert_clean(src);
    }

    /// **Direct call on a value-typed binding records inferred
    /// method-level generics.** `let p : QPoint = ...; p.map_to(lambda)`
    /// must build the trait method's FnTy on the fly and route through
    /// `synth_generic_call` — exactly like the trait-bound branch — so
    /// `U → <lambda's ret>` lands in `generic_instantiations`. Without
    /// this, the codegen would emit the namespace dispatch with no
    /// explicit template arg, and C++ template deduction would fail
    /// to bind `U` through `std::function<U(qint64)>`.
    #[test]
    fn direct_call_on_extern_type_records_inferred_method_generic() {
        let module = pixie_syntax::parse(
            pixie_syntax::span::FileId(0),
            r#"
trait Mapper {
  pub fn mapTo<U>(f: fn(Int) -> U) U
}
impl Mapper for QPoint {
  pub fn mapTo<U>(f: fn(Int) -> U) U { f(self.manhattanLength()) }
}
fn run {
  let p : QPoint = QPoint(3, 4)
  let s : String = p.mapTo({ |x: Int| "got" })
}
"#,
        )
        .unwrap();
        let resolved = pixie_hir::resolve(&module, &pixie_hir::ProjectInfo::default());
        let result = check_program(&module, &resolved.program);
        let saw_string = result
            .generic_instantiations
            .values()
            .any(|args| args.iter().any(|t| matches!(t, Type::Prim(Prim::String))));
        assert!(
            saw_string,
            "expected `String` to be recorded for `U` at the direct call, got {:?}",
            result.generic_instantiations
        );
    }

    /// **Direct call on a builtin generic** (`List<T>`) routes through
    /// the same FnTy build / instantiate cycle. Mirrors the codegen-
    /// side `direct_call_on_list_typed_binding_routes_through_trait_namespace`
    /// pin, but on the type-checker side.
    #[test]
    fn direct_call_on_builtin_generic_records_inferred_method_generic() {
        let module = pixie_syntax::parse(
            pixie_syntax::span::FileId(0),
            r#"
trait Mapper {
  pub fn mapTo<U>(f: fn(Int) -> U) U
}
impl<T> Mapper for List<T> {
  pub fn mapTo<U>(f: fn(Int) -> U) U { f(0) }
}
fn run {
  let xs : List<Int> = [1, 2, 3]
  let s : String = xs.mapTo({ |x: Int| "got" })
}
"#,
        )
        .unwrap();
        let resolved = pixie_hir::resolve(&module, &pixie_hir::ProjectInfo::default());
        let result = check_program(&module, &resolved.program);
        let saw_string = result
            .generic_instantiations
            .values()
            .any(|args| args.iter().any(|t| matches!(t, Type::Prim(Prim::String))));
        assert!(
            saw_string,
            "expected `String` to be recorded for `U` on a List<Int> direct call, got {:?}",
            result.generic_instantiations
        );
    }

    /// **`Self` in a trait method's return type substitutes to the
    /// receiver's bound `T` at body-side trait-method validation.**
    /// `fn duplicate<T: Cloneable>(thing: T) -> T { thing.cloned() }`
    /// where `Cloneable.cloned -> Self` should clean — the trait
    /// method's `Self` resolves to the bound `T`, which matches the
    /// fn's return type.
    #[test]
    fn trait_self_in_return_substitutes_to_bound_t_in_generic_body() {
        assert_clean(
            r#"
trait Cloneable { fn cloned Self }
fn duplicate<T: Cloneable>(thing: T) T {
  thing.cloned()
}
"#,
        );
    }

    /// **`Self` in a trait method substitutes to the concrete
    /// receiver type at direct-call dispatch.** `let p : QPoint;
    /// p.identity()` (where `Identity.identity -> Self`) returns
    /// QPoint — assignment to a QPoint binding is clean.
    #[test]
    fn trait_self_substitutes_to_recv_at_direct_call() {
        assert_clean(
            r#"
trait Identity { fn Identity Self }
impl Identity for QPoint {
  fn Identity Self { self }
}
fn run QPoint {
  let p : QPoint = QPoint(3, 4)
  p.Identity()
}
"#,
        );
    }

    /// **No registered impl ⇒ no spurious recording.** A direct call
    /// on an extern type whose simple base name has no impl in scope
    /// must NOT route through `synth_generic_call` — soft-pass keeps
    /// the call clean and `generic_instantiations` stays empty for it.
    /// Pin the negative so a future refactor doesn't accidentally
    /// route every external receiver through the trait dispatch.
    #[test]
    fn direct_call_on_extern_without_impl_stays_soft_pass() {
        let module = pixie_syntax::parse(
            pixie_syntax::span::FileId(0),
            r#"
fn run {
  let p : QPoint = QPoint(3, 4)
  let _ = p.manhattanLength()
}
"#,
        )
        .unwrap();
        let resolved = pixie_hir::resolve(&module, &pixie_hir::ProjectInfo::default());
        let result = check_program(&module, &resolved.program);
        assert!(
            result.diagnostics.is_empty(),
            "expected clean type check, got {:?}",
            result.diagnostics
        );
        // No impl is in scope, so the trait-impl routing must NOT
        // fire — generic_instantiations should not have an entry
        // referencing this call's span.
        assert!(
            result.generic_instantiations.is_empty(),
            "expected no spurious recording on a no-impl direct call, got {:?}",
            result.generic_instantiations
        );
    }

    /// Multiple-bounds dispatch: when a method exists on more than
    /// one bound trait, the first-match's signature is used. (Order
    /// follows the source order of the bound list.) Pin the
    /// behavior so future refactors of the bound walk don't
    /// silently flip it.
    #[test]
    fn body_call_resolves_signature_via_first_matching_bound() {
        let src = r#"
trait Reader { fn read Int }
trait Writer { fn read String }
fn useIt<T: Reader + Writer>(thing: T) Int {
  thing.read()
}
"#;
        // First bound (Reader) wins -> read() returns Int -> clean.
        let d = check_str(src);
        assert!(
            d.is_empty(),
            "expected first-bound resolution to clean, got: {:?}",
            d
        );
    }

    /// `impl Iterable for QStringList` lifts QStringList from
    /// soft-pass to strictly checked. A call site that infers
    /// `T = QStringList` against bound `Iterable` is now affirmed
    /// (not just unchallenged).
    #[test]
    fn extern_type_with_registered_impl_satisfies_bound_strictly() {
        // Sanity: the call type-checks clean when the impl is in
        // scope. Without the parametric-impl path this would
        // soft-pass; with the path it strictly passes — same outer
        // observation, but the rationale is "we have a registered
        // impl" rather than "we don't know either way".
        assert_clean(
            r#"
trait Iterable { fn first Int }
impl Iterable for QStringList { fn first Int { 0 } }
fn first<T: Iterable>(xs: T) T { xs }
fn run(xs: QStringList) {
  let _ = first(xs)
}
"#,
        );
    }

    /// Once `QStringList` has any `impl X for QStringList`
    /// registered, it's no longer soft-passing every bound — calls
    /// requiring an unrelated trait are rejected because we now
    /// have positive visibility into its impl set.
    #[test]
    fn extern_type_with_partial_impl_rejects_unrelated_bound() {
        let src = r#"
trait Foo { fn f Int }
trait Bar { fn b Int }
impl Foo for QStringList { fn f Int { 0 } }
fn wantBar<T: Bar>(xs: T) T { xs }
fn run(xs: QStringList) {
  let _ = wantBar(xs)
}
"#;
        let d = check_str(src);
        assert!(
            d.iter()
                .any(|m| m.message.contains("does not implement trait `Bar`")
                    && m.message.contains("QStringList")),
            "expected rejection — once QStringList has any impl, the lookup is strict; got: {:?}",
            d
        );
    }

    /// `impl<T> Foo for List<T>` registers under "List". A call
    /// with `T = List<Int>` is now checked positively against the
    /// impls map (the existing Generic-base branch already keyed
    /// by base, so this confirms the parametric form integrates).
    #[test]
    fn parametric_impl_satisfies_generic_base_bound() {
        assert_clean(
            r#"
trait Foo { fn x Int }
impl<T> Foo for List<T> { fn x Int { 0 } }
fn run(xs: List<Int>) Int {
  useIt(xs)
}
fn useIt<T: Foo>(thing: T) Int {
  thing.x()
}
"#,
        );
    }

    /// With a user `init(initial: Int) { ... }`, `T.new(arg)` checks
    /// the arg against the declared init signature. Wrong type → error.
    #[test]
    fn t_new_arg_type_mismatch_against_user_init_errors() {
        let src = r#"
class Counter {
  pub prop count : Int, default: 0
  init(initial: Int) {
    count = initial
  }
}
fn main {
  let c = Counter.new("oops")
}
"#;
        assert_one_error_containing(src, "expected `Int`, found `String`");
    }

    /// Matching arg type against the user init is clean.
    #[test]
    fn t_new_with_matching_init_args_is_clean() {
        let src = r#"
class Counter {
  pub prop count : Int, default: 0
  init(initial: Int) {
    count = initial
  }
}
fn main {
  let c = Counter.new(42)
}
"#;
        assert_clean(src);
    }

    /// With multiple inits (overload), the right arity wins.
    #[test]
    fn t_new_picks_init_overload_by_arity() {
        let src = r#"
class Pair {
  prop a : Int, default: 0
  prop b : Int, default: 0
  init() { a = 0  b = 0 }
  init(a: Int, b: Int) { a = a  b = b }
}
fn main {
  let p1 = Pair.new()
  let p2 = Pair.new(1, 2)
}
"#;
        assert_clean(src);
    }

    /// No init matching the call-site arity → diagnostic that lists
    /// the available arities so the user can fix the call.
    #[test]
    fn t_new_no_arity_match_reports_declared_arities() {
        let src = r#"
class Counter {
  init(initial: Int) { }
}
fn main {
  let c = Counter.new(1, 2)
}
"#;
        assert_one_error_containing(src, "no `init` on class `Counter` accepts 2 argument(s)");
    }

    /// Without any user `init`, `T.new(args)` keeps the historical
    /// foreign-soft acceptance (so existing classes that never
    /// declared an init keep working — this is a backward-compat
    /// pin).
    #[test]
    fn t_new_without_init_decl_is_foreign_soft() {
        let src = r#"
class Counter {
  pub prop count : Int, default: 0
}
fn main {
  let c = Counter.new()
  let d = Counter.new(1, 2, 3)
}
"#;
        assert_clean(src);
    }

    /// `@field` writes inside `init` body are checked against the
    /// declared property type — same machinery as fn-body checking.
    #[test]
    fn init_body_at_field_assignment_type_checks() {
        let src = r#"
class Counter {
  pub prop count : Int, default: 0
  init() {
    count = "nope"
  }
}
"#;
        assert_one_error_containing(src, "expected `Int`, found `String`");
    }

    /// `deinit` body is type-checked too: a property write has to
    /// match the declared type.
    #[test]
    fn deinit_body_at_field_assignment_type_checks() {
        let src = r#"
class Counter {
  pub prop count : Int, default: 0
  deinit {
    count = "gone"
  }
}
"#;
        assert_one_error_containing(src, "expected `Int`, found `String`");
    }

    // ---- overload-by-arg-type --------------------------------------

    /// Two same-named methods on a class with different arities both
    /// resolve cleanly when called with the matching arg count.
    #[test]
    fn class_method_overload_by_arity_resolves_correctly() {
        let src = r#"
class Greeter {
  pub fn greet String { "hi" }
  pub fn greet(name: String) String { name }
}
fn useIt(g: Greeter) {
  let _ = g.greet()
  let _ = g.greet("world")
}
"#;
        let d = check_str(src);
        assert!(d.is_empty(), "expected clean, got {:?}", d);
    }

    /// Two same-arity overloads pick by exact arg type at the call site.
    #[test]
    fn class_method_overload_by_arg_type_resolves_correctly() {
        let src = r#"
class Tag {
  pub fn matches(other: String) Bool { true }
  pub fn matches(other: Int) Bool { true }
}
fn useIt(t: Tag) {
  let _ = t.matches("x")
  let _ = t.matches(42)
}
"#;
        let d = check_str(src);
        assert!(d.is_empty(), "expected clean, got {:?}", d);
    }

    /// `init(name: String)` and `init(id: Int)` disambiguate by arg
    /// type at `T.new(...)` — picking the first by arity alone would
    /// fail for the `Int` ctor here.
    #[test]
    fn init_overload_by_arg_type_resolves_correctly() {
        let src = r#"
class Tag {
  prop Label : String, default: ""
  init(name: String) { Label = name }
  init(id: Int) { Label = "tag" }
}
fn makeBoth {
  let a = Tag.new("foo")
  let b = Tag.new(42)
}
"#;
        let d = check_str(src);
        assert!(d.is_empty(), "expected clean, got {:?}", d);
    }

    /// Free-fn overload: same name, different param types, both
    /// callable based on the actual arg type.
    #[test]
    fn top_level_fn_overload_resolves_by_arg_type() {
        let src = r#"
fn fmt(x: Int) String { "int" }
fn fmt(x: String) String { x }
fn run {
  let _ = fmt(1)
  let _ = fmt("hi")
}
"#;
        let d = check_str(src);
        assert!(d.is_empty(), "expected clean, got {:?}", d);
    }

    /// When no overload matches the actual arg types, the diagnostic
    /// renders the candidate set so the user can see what shapes are
    /// available.
    #[test]
    fn overload_no_match_emits_candidate_set_diagnostic() {
        let src = r#"
fn fmt(x: Int) String { "int" }
fn fmt(x: String) String { x }
fn run {
  let _ = fmt(true)
}
"#;
        let d = check_str(src);
        assert!(
            d.iter().any(|d| d.message.contains("no overload of `fmt`")
                && d.message.contains("candidates:")
                && d.message.contains("fmt(Int)")
                && d.message.contains("fmt(String)")),
            "missing candidate-set diagnostic: {:?}",
            d
        );
    }

    /// `foo(Int)` vs `foo(Float)` called with an Int picks the exact
    /// match (Tier 2). Without an exact candidate, the resolver widens
    /// to Float (Tier 3).
    #[test]
    fn overload_int_vs_float_picks_exact_then_widens() {
        let src = r#"
fn foo(x: Int) Int { x }
fn foo(x: Float) Float { x }
fn run {
  let a = foo(1)
  let b = foo(1.0)
}
"#;
        let d = check_str(src);
        assert!(d.is_empty(), "expected clean, got {:?}", d);
    }

    /// External-soft-pass tiebreak: when every candidate matches
    /// only via `is_subtype`'s External-touching soft-pass, the
    /// resolver picks the first-declared overload deterministically.
    /// Without this, calling `KLocalizedString.toString()` against
    /// the binding's two overloads would be flagged ambiguous.
    #[test]
    fn overload_external_soft_pass_falls_back_to_first_declared() {
        // Two overloads taking external types; called with one
        // external arg → both candidates match via is_subtype's
        // External soft-pass, the first one wins.
        let src = r#"
class Box {
  pub fn pack(item: QString) QString { item }
  pub fn pack(item: QByteArray) QString { "x" }
}
fn useIt(b: Box, s: QString) {
  let _ = b.pack(s)
}
"#;
        let d = check_str(src);
        // Should be clean — the resolver picks the first declared
        // overload via the External fallback rather than emitting an
        // "ambiguous call" diagnostic.
        assert!(d.is_empty(), "expected clean, got {:?}", d);
    }

    /// Trait with overloaded methods: `<T: Foo>` body call site picks
    /// the right overload by arg type. Without overload-aware trait
    /// dispatch, both `thing.x()` and `thing.x(42)` would silently
    /// bind to the first declared `fn x` regardless.
    #[test]
    fn trait_bound_overload_resolves_by_arg_type() {
        let src = r#"
trait Foo {
  fn x Int
  fn x(y: Int) Int
}
fn useIt<T: Foo>(thing: T) Int {
  let a = thing.x()
  let b = thing.x(42)
  a + b
}
"#;
        let d = check_str(src);
        assert!(d.is_empty(), "expected clean, got {:?}", d);
    }

    /// Direct-call dispatch via `try_dispatch_via_impl` (External /
    /// Generic receivers with a registered impl) also overload-resolves
    /// against the trait's overloads.
    #[test]
    fn direct_call_trait_overload_resolves_by_arg_type() {
        let src = r#"
trait Foo {
  fn x Int
  fn x(y: Int) Int
}
impl Foo for QPoint {
  fn x Int { 0 }
  fn x(y: Int) Int { y }
}
fn useIt(p: QPoint) Int {
  let a = p.x()
  let b = p.x(99)
  a + b
}
"#;
        let d = check_str(src);
        assert!(d.is_empty(), "expected clean, got {:?}", d);
    }

    // ---- weak / unowned constraint checks ----------------------------

    /// `weak` was removed in pixie: handles are stale-aware, so `T?`
    /// already reads as nil after its target is removed.
    #[test]
    fn weak_is_rejected_as_removed() {
        let src = r#"
class Parent { }
class Child {
  weak let parent : Parent?
}
"#;
        let d = check_str(src);
        assert!(
            d.iter().any(|x| x.message.contains("removed in pixie")),
            "expected removed-keyword diagnostic, got: {:?}",
            d
        );
    }


    // ---- QSS shorthand value-shape checks --------------------------------

    /// Length-typed shorthand keys (`borderRadius`, `fontSize`,
    /// `padding*`, `margin*`) accept Int / Float / String. A string
    /// `"abc"` would have flowed straight through to `border-radius:
    /// abc;` in the synthesised QSS — Qt then silently discards the
    /// rule at runtime — so the type checker rejects it up-front.
    #[test]
    fn qss_length_rejects_int_with_unit_string_alone() {
        // Int + Float + String are all accepted (tested by other
        // cases). This case verifies that a Bool is rejected with a
        // useful "expected" message.
        let src = r#"
style Bad { borderRadius: true }
"#;
        let d = check_str(src);
        assert_eq!(d.len(), 1, "expected one diag, got: {:?}", d);
        assert!(
            d[0].message
                .contains("QSS shorthand `borderRadius` expects Int, Float, or String"),
            "message: {}",
            d[0].message
        );
    }

    /// Color shorthand keys (`color`, `background`, `borderColor`)
    /// require a String — Cute has no first-class Color type, so an
    /// Int literal can't be a valid hex / rgb form.
    #[test]
    fn qss_color_rejects_int_literal() {
        let src = r#"
style Bad { color: 42 }
"#;
        let d = check_str(src);
        assert_eq!(d.len(), 1, "expected one diag, got: {:?}", d);
        assert!(
            d[0].message
                .contains("QSS shorthand `color` expects String (got `Int`)"),
            "message: {}",
            d[0].message
        );
    }

    /// `fontWeight` is the only Numeric-shape key today — Int / Float
    /// / String are all fine, anything else errors.
    #[test]
    fn qss_numeric_accepts_int_and_string() {
        // Int form
        assert_clean(r#"style A { fontWeight: 500 }"#);
        // String form (allows `"bold"`, `"normal"`, etc.)
        assert_clean(r#"style B { fontWeight: "bold" }"#);
    }

    /// `textAlign` maps to `qproperty-alignment`; Cute string is
    /// required, the formatter then maps `"left"` / `"right"` /
    /// `"center"` etc. to `AlignLeft` / `AlignRight` / `AlignCenter`.
    #[test]
    fn qss_align_requires_string() {
        let src = r#"style A { textAlign: 1 }"#;
        let d = check_str(src);
        assert_eq!(d.len(), 1, "expected one diag, got: {:?}", d);
        assert!(
            d[0].message
                .contains("QSS shorthand `textAlign` expects String"),
            "message: {}",
            d[0].message
        );
    }

    /// Pseudo-class prefixes (`hover.X`, `pressed.X`) carry the same
    /// shape as their bare counterpart — `hover.background` is still
    /// a Color shape, so an Int literal there fails the same way.
    #[test]
    fn qss_pseudo_prefix_inherits_base_shape() {
        let src = r#"style A { hover.background: 0 }"#;
        let d = check_str(src);
        assert_eq!(d.len(), 1, "expected one diag, got: {:?}", d);
        assert!(
            d[0].message
                .contains("QSS shorthand `hover.background` expects String"),
            "message: {}",
            d[0].message
        );
    }

    /// Inline shorthand on a widget element — same rule path. Even
    /// though `QPushButton` is a Qt class with no entry in the
    /// project type table, the QSS shorthand check fires before the
    /// parent-class lookup so the bad value is caught.
    #[test]
    fn qss_shorthand_on_widget_element_is_checked_directly() {
        let src = r#"
view Main {
  QMainWindow {
    QPushButton { color: 99 }
  }
}
"#;
        let d = check_str(src);
        assert_eq!(d.len(), 1, "expected one diag, got: {:?}", d);
        assert!(
            d[0].message
                .contains("QSS shorthand `color` expects String"),
            "message: {}",
            d[0].message
        );
    }

    /// Keys outside the shorthand vocabulary keep their existing
    /// behaviour: synth runs (so member access still gets type-
    /// checked) but no shape error fires. `colour` (UK spelling)
    /// would still produce a C++ compile error via `setColour(...)`,
    /// just like before this feature landed.
    #[test]
    fn qss_unknown_key_does_not_emit_shape_error() {
        // No shape check, synth succeeds (string literal), no diag.
        let src = r#"style A { colour: "red" }"#;
        assert_clean(src);
    }

    // ---- SignalRef -----------------------------------------------------

    /// Smoke test: `obj.signal.connect { ... }` with a 0-arg signal and
    /// a bare-block handler type-checks clean. Pins the SignalRef path's
    /// happy case so it doesn't regress to the old Unknown soft-pass.
    #[test]
    fn signal_connect_zero_arg_signal_passes() {
        assert_clean(
            r#"
class X {
  signal pinged
}
fn run(x: X) {
  x.pinged.connect {
  }
}
"#,
        );
    }

    /// 1-arg signal + 1-param handler with explicit type that matches
    /// the signal's declared parameter type — clean.
    #[test]
    fn signal_connect_typed_param_matches_passes() {
        assert_clean(
            r#"
class X {
  signal valueChanged(v: Int)
}
fn run(x: X) {
  x.valueChanged.connect { |n: Int|
  }
}
"#,
        );
    }

    /// Signal handler whose trailing expression returns a non-Void
    /// value: still clean. The `Type::Unknown` we use as the expected
    /// `ret` in `synth_signal_method_call` deliberately soft-passes any
    /// concrete return type, since Qt discards signal-handler returns.
    #[test]
    fn signal_connect_handler_with_non_void_return_passes() {
        assert_clean(
            r#"
class X {
  signal valueChanged(v: Int)
}
fn run(x: X) {
  x.valueChanged.connect { |n: Int|
    n + 1
  }
}
"#,
        );
    }

    /// Wrong-arity handler: signal takes 1 param, handler takes 2.
    /// Caught at type-check; the previous Unknown soft-pass would
    /// have accepted this silently.
    #[test]
    fn signal_connect_arity_mismatch_errors() {
        let d = check_str(
            r#"
class X {
  signal valueChanged(v: Int)
}
fn run(x: X) {
  x.valueChanged.connect { |a, b|
  }
}
"#,
        );
        assert!(
            d.iter().any(|d| d
                .message
                .contains("signal `X::valueChanged` handler expects 1 param(s), got 2")),
            "{:?}",
            d,
        );
    }

    /// Bare-block handler on a signal that has parameters: arity error.
    /// Bare blocks are no-arg callables; a 1-param signal needs a
    /// `{ |x| ... }` shape.
    #[test]
    fn signal_connect_bare_block_with_signal_params_errors() {
        let d = check_str(
            r#"
class X {
  signal valueChanged(v: Int)
}
fn run(x: X) {
  x.valueChanged.connect {
  }
}
"#,
        );
        assert!(
            d.iter().any(|d| d
                .message
                .contains("signal `X::valueChanged` handler expects 1 param(s)")),
            "{:?}",
            d,
        );
    }

    /// Wrong-typed handler param: signal carries Int, handler annotated
    /// as String. Caught by the existing Lambda-vs-Fn arm in `check`
    /// mode — emits "lambda parameter declared `String` but call site
    /// expects `Int`".
    #[test]
    fn signal_connect_param_type_mismatch_errors() {
        let d = check_str(
            r#"
class X {
  signal valueChanged(v: Int)
}
fn run(x: X) {
  x.valueChanged.connect { |a: String|
  }
}
"#,
        );
        assert!(
            d.iter()
                .any(|d| d.message.contains("declared `String`")
                    && d.message.contains("expects `Int`")),
            "expected lambda-param type-mismatch diag, got: {:?}",
            d,
        );
    }

    /// `.disconnect()` with zero args / no block: clean.
    #[test]
    fn signal_disconnect_zero_args_passes() {
        assert_clean(
            r#"
class X {
  signal pinged
}
fn run(x: X) {
  x.pinged.disconnect()
}
"#,
        );
    }

    /// `.disconnect(handler)` is rejected — the type-check arm only
    /// accepts the no-arg shape.
    #[test]
    fn signal_disconnect_with_args_errors() {
        let d = check_str(
            r#"
class X {
  signal pinged
}
fn run(x: X) {
  x.pinged.disconnect {
  }
}
"#,
        );
        assert!(
            d.iter().any(|d| d
                .message
                .contains("`disconnect` on signal `X::pinged` takes no arguments")),
            "{:?}",
            d,
        );
    }

    /// Any method other than connect / disconnect on a signal value is
    /// a real error. Previously soft-passed (silently OK).
    #[test]
    fn signal_unknown_method_errors() {
        let d = check_str(
            r#"
class X {
  signal pinged
}
fn run(x: X) {
  x.pinged.foo()
}
"#,
        );
        assert!(
            d.iter().any(|d| {
                let m = &d.message;
                m.contains("no method `foo` on signal `X::pinged`")
                    && m.contains("only `connect` and `disconnect` are supported")
            }),
            "{:?}",
            d,
        );
    }

    /// Regression: typo on the signal name in a member-access position
    /// still surfaces a "did you mean ..." suggestion. The "no member
    /// found" fallthrough at the bottom of `K::Member` for class
    /// receivers always included signal names in its candidate set, but
    /// this test pins it now that the SignalRef path takes over the
    /// happy case.
    #[test]
    fn signal_typo_member_access_suggests_close_match() {
        let d = check_str(
            r#"
class X {
  signal valueChanged
}
fn run(x: X) {
  x.valueChnged.connect {
  }
}
"#,
        );
        assert!(
            d.iter()
                .any(|d| d.message.contains("did you mean `valueChanged`")),
            "{:?}",
            d,
        );
    }

    /// Signal ref refines through inheritance: the declaring class is
    /// preserved on the SignalRef, so `.connect` on a subclass receiver
    /// still type-checks against the parent's signal params.
    #[test]
    fn signal_connect_through_inheritance_passes() {
        assert_clean(
            r#"
class A {
  signal valueChanged(v: Int)
}
class B < A {}
fn run(b: B) {
  b.valueChanged.connect { |n: Int|
  }
}
"#,
        );
    }
}
