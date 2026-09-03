//! AST→AST pre-passes, run before HIR resolve (which has `unreachable!`
//! arms for the shells these remove). Ported from the ancestor's
//! cute-codegen desugar trio with pixie's semantics: no implicit
//! `QObject` super on synthesized classes, and `state` cells live on
//! `view` (pixie's single substrate), not `widget`.

use std::collections::HashSet;

use pixie_syntax::ast::*;
use pixie_syntax::span::Span;

/// Flatten `suite "X" { test "y" { body } }` into sibling `Item::Fn`
/// entries (`display_name: Some("X / y")` is set by the parser).
pub fn desugar_suite(mut module: Module) -> Module {
    if !module.items.iter().any(|i| matches!(i, Item::Suite(_))) {
        return module;
    }
    let mut new_items: Vec<Item> = Vec::with_capacity(module.items.len() + 4);
    for item in std::mem::take(&mut module.items) {
        match item {
            Item::Suite(s) => {
                for t in s.tests {
                    new_items.push(Item::Fn(t));
                }
            }
            other => new_items.push(other),
        }
    }
    module.items = new_items;
    module
}

/// `store Foo { state x : T = init; fn ... }` rewrites to a plain class
/// (every member forced `pub`, each `state` a (prop, signal) pair) plus
/// `let Foo : Foo = Foo.new()`. Codegen lifts class-typed top-level lets
/// to World singletons, so `Foo.x` resolves through `singleton_ref`.
pub fn desugar_store(mut module: Module) -> Module {
    if !module.items.iter().any(|i| matches!(i, Item::Store(_))) {
        return module;
    }
    let mut new_items: Vec<Item> = Vec::with_capacity(module.items.len() + 4);
    for item in std::mem::take(&mut module.items) {
        match item {
            Item::Store(s) => {
                let span = s.span;
                let is_pub = s.is_pub;
                let name = s.name.clone();
                new_items.push(Item::Class(synthesize_store_class(s)));
                new_items.push(Item::Let(synthesize_singleton_let(name, span, is_pub)));
            }
            other => new_items.push(other),
        }
    }
    module.items = new_items;
    module
}

fn synthesize_store_class(s: StoreDecl) -> ClassDecl {
    let span = s.span;
    let mut members: Vec<ClassMember> =
        Vec::with_capacity(s.state_fields.len() * 2 + s.members.len());
    for sf in s.state_fields {
        let StateFieldKind::Property { ty } = sf.kind else {
            continue;
        };
        let notify = Ident {
            name: synth_notify_name(&sf.name.name),
            span: sf.span,
        };
        members.push(ClassMember::Property(PropertyDecl {
            weak: false,
            name: sf.name,
            ty,
            notify: Some(notify.clone()),
            default: Some(sf.init_expr),
            is_pub: true,
            bindable: false,
            binding: None,
            fresh: None,
            model: false,
            constant: false,
            span: sf.span,
            block_id: None,
        }));
        members.push(ClassMember::Signal(SignalDecl {
            name: notify,
            params: Vec::new(),
            is_pub: true,
            span: sf.span,
        }));
    }
    for mut m in s.members {
        m.set_pub(true);
        members.push(m);
    }
    ClassDecl {
        name: s.name,
        generics: Vec::new(),
        super_class: None,
        members,
        is_pub: s.is_pub,
        is_extern_value: false,
        is_arc: false,
        is_copyable: true,
        span,
    }
}

fn synthesize_singleton_let(name: Ident, span: Span, is_pub: bool) -> LetDecl {
    let ty = TypeExpr {
        kind: TypeKind::Named {
            path: vec![name.clone()],
            args: Vec::new(),
        },
        span,
    };
    let value = Expr {
        kind: ExprKind::MethodCall {
            receiver: Box::new(Expr {
                kind: ExprKind::Ident(name.name.clone()),
                span,
            }),
            method: Ident {
                name: "new".into(),
                span,
            },
            args: Vec::new(),
            block: None,
            type_args: Vec::new(),
        },
        span,
    };
    LetDecl {
        name,
        ty,
        value,
        is_pub,
        span,
    }
}

/// Desugar view-side `state X : T = init` cells into a synthesized
/// holder class plus a hidden object state field:
///
/// ```text
/// class __MainState { pub prop X : T, notify: :XChanged, default: init }
/// view Main {
///   let __pixie_state = __MainState()
///   // bare `X` reads  → __pixie_state.X
///   // `X = expr`      → __pixie_state.X = expr
/// }
/// ```
pub fn desugar_view_state(mut module: Module) -> Module {
    let mut new_items: Vec<Item> = Vec::with_capacity(module.items.len());
    for item in std::mem::take(&mut module.items) {
        match item {
            Item::View(v) => {
                let prop_fields: Vec<&StateField> = v
                    .state_fields
                    .iter()
                    .filter(|sf| matches!(sf.kind, StateFieldKind::Property { .. }))
                    .collect();
                if prop_fields.is_empty() {
                    new_items.push(Item::View(v));
                    continue;
                }
                let holder_name = format!("__{}State", v.name.name);
                let prop_names: HashSet<String> =
                    prop_fields.iter().map(|p| p.name.name.clone()).collect();
                let synth_class = synthesize_holder_class(&holder_name, &prop_fields, v.span);
                new_items.push(Item::Class(synth_class));

                let new_state_fields = rewrite_view_state_fields(&v, &holder_name);
                let new_root = rewrite_element(&v.root, &prop_names);
                new_items.push(Item::View(ViewDecl {
                    name: v.name,
                    params: v.params,
                    state_fields: new_state_fields,
                    root: new_root,
                    is_pub: v.is_pub,
                    span: v.span,
                }));
            }
            other => new_items.push(other),
        }
    }
    module.items = new_items;
    module
}

fn synthesize_holder_class(name: &str, props: &[&StateField], span: Span) -> ClassDecl {
    let mut members: Vec<ClassMember> = Vec::with_capacity(props.len() * 2);
    for sf in props {
        let StateFieldKind::Property { ty } = &sf.kind else {
            continue;
        };
        let notify_ident = Ident {
            name: synth_notify_name(&sf.name.name),
            span: sf.span,
        };
        members.push(ClassMember::Property(PropertyDecl {
            weak: false,
            name: sf.name.clone(),
            ty: ty.clone(),
            notify: Some(notify_ident.clone()),
            default: Some(sf.init_expr.clone()),
            is_pub: true,
            bindable: false,
            binding: None,
            fresh: None,
            model: false,
            constant: false,
            span: sf.span,
            block_id: None,
        }));
        members.push(ClassMember::Signal(SignalDecl {
            name: notify_ident,
            params: Vec::new(),
            is_pub: true,
            span: sf.span,
        }));
    }
    ClassDecl {
        name: Ident {
            name: name.into(),
            span,
        },
        generics: Vec::new(),
        super_class: None,
        members,
        is_pub: false,
        is_extern_value: false,
        is_arc: false,
        is_copyable: true,
        span,
    }
}

/// Keep `let`-form Object fields, drop `state` cells, and prepend the
/// synthesized `let __pixie_state = <holder>()` so the holder exists
/// before any user sub-object that might read it.
fn rewrite_view_state_fields(v: &ViewDecl, holder_name: &str) -> Vec<StateField> {
    let mut out: Vec<StateField> = Vec::with_capacity(v.state_fields.len());
    let span = v.span;
    let init_expr = Expr {
        kind: ExprKind::Call {
            callee: Box::new(Expr {
                kind: ExprKind::Ident(holder_name.into()),
                span,
            }),
            args: Vec::new(),
            block: None,
            type_args: Vec::new(),
        },
        span,
    };
    out.push(StateField {
        name: Ident {
            name: "__pixie_state".into(),
            span,
        },
        kind: StateFieldKind::Object,
        init_expr,
        span,
    });
    for sf in &v.state_fields {
        match &sf.kind {
            StateFieldKind::Property { .. } => continue,
            StateFieldKind::Object => out.push(sf.clone()),
        }
    }
    out
}

fn state_member(name: &str, span: Span) -> Expr {
    Expr {
        kind: ExprKind::Member {
            receiver: Box::new(Expr {
                kind: ExprKind::Ident("__pixie_state".into()),
                span,
            }),
            name: Ident {
                name: name.into(),
                span,
            },
        },
        span,
    }
}

fn rewrite_element(e: &Element, props: &HashSet<String>) -> Element {
    Element {
        module_path: e.module_path.clone(),
        name: e.name.clone(),
        members: e
            .members
            .iter()
            .map(|m| rewrite_element_member(m, props))
            .collect(),
        span: e.span,
    }
}

fn rewrite_element_member(m: &ElementMember, props: &HashSet<String>) -> ElementMember {
    match m {
        ElementMember::Property { key, value, span } => {
            // Text handlers bind an implicit `text` argument; it shadows
            // any state cell of the same name inside the handler body, so
            // the state rewrite must not capture it there. The toggles'
            // `onToggle` binds an implicit `checked`, the Slider's
            // `onChange` an implicit `value`, and the choosers'
            // `onSelect` an implicit `index` — all by the same rule.
            let value = if key == "onTextChanged" || key == "onSubmitted" {
                let mut scoped = props.clone();
                scoped.remove("text");
                rewrite_expr(value, &scoped)
            } else if key == "onToggle" {
                let mut scoped = props.clone();
                scoped.remove("checked");
                rewrite_expr(value, &scoped)
            } else if key == "onChange" {
                // The implicit `value` argument of the Slider and the
                // two number fields, same rule (its TYPE differs per
                // widget; the name it shadows does not).
                let mut scoped = props.clone();
                scoped.remove("value");
                rewrite_expr(value, &scoped)
            } else if key == "onSelect" {
                let mut scoped = props.clone();
                scoped.remove("index");
                rewrite_expr(value, &scoped)
            } else {
                rewrite_expr(value, props)
            };
            ElementMember::Property {
                key: key.clone(),
                value,
                span: *span,
            }
        }
        ElementMember::Child(c) => ElementMember::Child(rewrite_element(c, props)),
        ElementMember::Stmt(s) => ElementMember::Stmt(rewrite_stmt(s, props)),
    }
}

fn rewrite_stmt(s: &Stmt, props: &HashSet<String>) -> Stmt {
    match s {
        Stmt::Let {
            name,
            ty,
            value,
            span,
            block_id,
        } => Stmt::Let {
            name: name.clone(),
            ty: ty.clone(),
            value: rewrite_expr(value, props),
            span: *span,
            block_id: *block_id,
        },
        Stmt::Var {
            name,
            ty,
            value,
            span,
            block_id,
        } => Stmt::Var {
            name: name.clone(),
            ty: ty.clone(),
            value: rewrite_expr(value, props),
            span: *span,
            block_id: *block_id,
        },
        Stmt::Expr(e) => Stmt::Expr(rewrite_expr(e, props)),
        Stmt::Return { value, span } => Stmt::Return {
            value: value.as_ref().map(|v| rewrite_expr(v, props)),
            span: *span,
        },
        Stmt::Emit { signal, args, span } => Stmt::Emit {
            signal: signal.clone(),
            args: args.iter().map(|a| rewrite_expr(a, props)).collect(),
            span: *span,
        },
        Stmt::Assign {
            target,
            op,
            value,
            span,
        } => {
            let new_target = if let ExprKind::Ident(name) = &target.kind {
                if props.contains(name) {
                    state_member(name, target.span)
                } else {
                    target.clone()
                }
            } else {
                rewrite_expr(target, props)
            };
            Stmt::Assign {
                target: new_target,
                op: *op,
                value: rewrite_expr(value, props),
                span: *span,
            }
        }
        Stmt::For {
            binding,
            index,
            iter,
            body,
            span,
        } => Stmt::For {
            binding: binding.clone(),
            index: index.clone(),
            iter: rewrite_expr(iter, props),
            body: rewrite_block(body, props),
            span: *span,
        },
        Stmt::While { cond, body, span } => Stmt::While {
            cond: rewrite_expr(cond, props),
            body: rewrite_block(body, props),
            span: *span,
        },
        Stmt::Batch { body, span } => Stmt::Batch {
            body: rewrite_block(body, props),
            span: *span,
        },
        Stmt::Break { span } => Stmt::Break { span: *span },
        Stmt::Continue { span } => Stmt::Continue { span: *span },
    }
}

fn rewrite_block(b: &Block, props: &HashSet<String>) -> Block {
    Block {
        stmts: b.stmts.iter().map(|s| rewrite_stmt(s, props)).collect(),
        trailing: b
            .trailing
            .as_ref()
            .map(|t| Box::new(rewrite_expr(t, props))),
        span: b.span,
    }
}

fn rewrite_expr(e: &Expr, props: &HashSet<String>) -> Expr {
    let kind = match &e.kind {
        ExprKind::Ident(name) if props.contains(name) => {
            return state_member(name, e.span);
        }
        k @ (ExprKind::Int(_)
        | ExprKind::Float(_)
        | ExprKind::Bool(_)
        | ExprKind::Nil
        | ExprKind::Sym(_)
        | ExprKind::Ident(_)
        | ExprKind::AtIdent(_)
        | ExprKind::SelfRef
        | ExprKind::Path(_)) => k.clone(),
        ExprKind::Str(parts) => ExprKind::Str(
            parts
                .iter()
                .map(|p| match p {
                    StrPart::Text(t) => StrPart::Text(t.clone()),
                    StrPart::Interp(inner) => {
                        StrPart::Interp(Box::new(rewrite_expr(inner, props)))
                    }
                    StrPart::InterpFmt { expr, format_spec } => StrPart::InterpFmt {
                        expr: Box::new(rewrite_expr(expr, props)),
                        format_spec: format_spec.clone(),
                    },
                })
                .collect(),
        ),
        ExprKind::Call {
            callee,
            args,
            block,
            type_args,
        } => ExprKind::Call {
            callee: Box::new(rewrite_expr(callee, props)),
            args: args.iter().map(|a| rewrite_expr(a, props)).collect(),
            block: block.as_ref().map(|b| Box::new(rewrite_expr(b, props))),
            type_args: type_args.clone(),
        },
        ExprKind::MethodCall {
            receiver,
            method,
            args,
            block,
            type_args,
        } => ExprKind::MethodCall {
            receiver: Box::new(rewrite_expr(receiver, props)),
            method: method.clone(),
            args: args.iter().map(|a| rewrite_expr(a, props)).collect(),
            block: block.as_ref().map(|b| Box::new(rewrite_expr(b, props))),
            type_args: type_args.clone(),
        },
        ExprKind::Member { receiver, name } => ExprKind::Member {
            receiver: Box::new(rewrite_expr(receiver, props)),
            name: name.clone(),
        },
        ExprKind::SafeMember { receiver, name } => ExprKind::SafeMember {
            receiver: Box::new(rewrite_expr(receiver, props)),
            name: name.clone(),
        },
        ExprKind::SafeMethodCall {
            receiver,
            method,
            args,
            block,
            type_args,
        } => ExprKind::SafeMethodCall {
            receiver: Box::new(rewrite_expr(receiver, props)),
            method: method.clone(),
            args: args.iter().map(|a| rewrite_expr(a, props)).collect(),
            block: block.as_ref().map(|b| Box::new(rewrite_expr(b, props))),
            type_args: type_args.clone(),
        },
        ExprKind::Index { receiver, index } => ExprKind::Index {
            receiver: Box::new(rewrite_expr(receiver, props)),
            index: Box::new(rewrite_expr(index, props)),
        },
        ExprKind::Block(b) => ExprKind::Block(rewrite_block(b, props)),
        ExprKind::Lambda { params, body } => ExprKind::Lambda {
            params: params.clone(),
            body: rewrite_block(body, props),
        },
        ExprKind::Unary { op, expr } => ExprKind::Unary {
            op: *op,
            expr: Box::new(rewrite_expr(expr, props)),
        },
        ExprKind::Binary { op, lhs, rhs } => ExprKind::Binary {
            op: *op,
            lhs: Box::new(rewrite_expr(lhs, props)),
            rhs: Box::new(rewrite_expr(rhs, props)),
        },
        ExprKind::Try(inner) => ExprKind::Try(Box::new(rewrite_expr(inner, props))),
        ExprKind::If {
            cond,
            then_b,
            else_b,
            let_binding,
        } => ExprKind::If {
            cond: Box::new(rewrite_expr(cond, props)),
            then_b: rewrite_block(then_b, props),
            else_b: else_b.as_ref().map(|b| rewrite_block(b, props)),
            let_binding: let_binding
                .as_ref()
                .map(|(p, init)| (p.clone(), Box::new(rewrite_expr(init, props)))),
        },
        ExprKind::Case { scrutinee, arms } => ExprKind::Case {
            scrutinee: Box::new(rewrite_expr(scrutinee, props)),
            arms: arms
                .iter()
                .map(|a| CaseArm {
                    pattern: a.pattern.clone(),
                    body: rewrite_block(&a.body, props),
                    span: a.span,
                })
                .collect(),
        },
        ExprKind::Await(inner) => ExprKind::Await(Box::new(rewrite_expr(inner, props))),
        ExprKind::Kwarg { key, value } => ExprKind::Kwarg {
            key: key.clone(),
            value: Box::new(rewrite_expr(value, props)),
        },
        ExprKind::Element(el) => ExprKind::Element(rewrite_element(el, props)),
        ExprKind::Array(items) => {
            ExprKind::Array(items.iter().map(|i| rewrite_expr(i, props)).collect())
        }
        ExprKind::Map(entries) => ExprKind::Map(
            entries
                .iter()
                .map(|(k, v)| (rewrite_expr(k, props), rewrite_expr(v, props)))
                .collect(),
        ),
        ExprKind::Range {
            start,
            end,
            inclusive,
        } => ExprKind::Range {
            start: Box::new(rewrite_expr(start, props)),
            end: Box::new(rewrite_expr(end, props)),
            inclusive: *inclusive,
        },
    };
    Expr { kind, span: e.span }
}

// ---------------------------------------------------------------------------
// §12.1 — module-qualifier erasure.
//
// The checker's visibility layer understands `F.x` (aliases,
// items_by_module); the EMITTER's namespace is flat. After every
// module is loaded, qualified references erase to bare names —
// `F.helper()` → `helper()`, `F.Store.prop` → `Store.prop`,
// `F.Class()` → `Class()` — which the flat space resolves, because
// the driver separately rejects cross-module duplicate item names.
// Shadow safety is a hard error, not a guess: a qualifier that is
// also ANY declared name or binder in the module refuses to erase.


/// Every name that could legally appear as a bare value/receiver in
/// this module: item names, prop/state names, params, locals, loop
/// and pattern binders. A module qualifier colliding with any of
/// these is ambiguous and refused.
pub fn declared_names(module: &Module) -> HashSet<String> {
    let mut out = HashSet::new();
    let add_fn = |f: &pixie_syntax::ast::FnDecl, out: &mut HashSet<String>| {
        for p in &f.params {
            out.insert(p.name.name.clone());
        }
        if let Some(b) = &f.body {
            collect_block(b, out);
        }
    };
    for item in &module.items {
        match item {
            Item::Class(c) => {
                out.insert(c.name.name.clone());
                for m in &c.members {
                    match m {
                        ClassMember::Property(p) => {
                            out.insert(p.name.name.clone());
                        }
                        ClassMember::Fn(f) | ClassMember::Slot(f) => add_fn(f, &mut out),
                        _ => {}
                    }
                }
            }
            Item::Struct(s) => {
                out.insert(s.name.name.clone());
                for m in &s.methods {
                    add_fn(m, &mut out);
                }
            }
            Item::Fn(f) => {
                out.insert(f.name.name.clone());
                add_fn(f, &mut out);
            }
            Item::Enum(e) => {
                out.insert(e.name.name.clone());
            }
            Item::Trait(t) => {
                out.insert(t.name.name.clone());
            }
            Item::Let(l) => {
                out.insert(l.name.name.clone());
            }
            Item::View(v) => {
                for sf in &v.state_fields {
                    out.insert(sf.name.name.clone());
                }
                collect_element(&v.root, &mut out);
            }
            _ => {}
        }
    }
    out
}

fn collect_block(b: &pixie_syntax::ast::Block, out: &mut HashSet<String>) {
    for s in &b.stmts {
        collect_stmt(s, out);
    }
    if let Some(t) = &b.trailing {
        collect_expr(t, out);
    }
}

fn collect_stmt(s: &Stmt, out: &mut HashSet<String>) {
    match s {
        Stmt::Let { name, value, .. } | Stmt::Var { name, value, .. } => {
            out.insert(name.name.clone());
            collect_expr(value, out);
        }
        Stmt::Assign { target, value, .. } => {
            collect_expr(target, out);
            collect_expr(value, out);
        }
        Stmt::Expr(e) => collect_expr(e, out),
        Stmt::Return { value: Some(v), .. } => collect_expr(v, out),
        Stmt::For {
            binding,
            index,
            iter,
            body,
            ..
        } => {
            out.insert(binding.name.clone());
            if let Some(i) = index {
                out.insert(i.name.clone());
            }
            collect_expr(iter, out);
            collect_block(body, out);
        }
        Stmt::While { cond, body, .. } => {
            collect_expr(cond, out);
            collect_block(body, out);
        }
        Stmt::Batch { body, .. } => collect_block(body, out),
        Stmt::Emit { args, .. } => {
            for a in args {
                collect_expr(a, out);
            }
        }
        _ => {}
    }
}

fn collect_pattern(p: &Pattern, out: &mut HashSet<String>) {
    match p {
        Pattern::Bind { name, .. } => {
            out.insert(name.name.clone());
        }
        Pattern::Ctor { args, .. } => {
            for a in args {
                collect_pattern(a, out);
            }
        }
        _ => {}
    }
}

fn collect_expr(e: &Expr, out: &mut HashSet<String>) {
    walk_children(e, &mut |child| collect_expr(child, out));
    if let ExprKind::Case { arms, .. } = &e.kind {
        for arm in arms {
            collect_pattern(&arm.pattern, out);
        }
    }
    if let ExprKind::Lambda { params, .. } = &e.kind {
        for p in params {
            out.insert(p.name.name.clone());
        }
    }
}

fn collect_element(el: &Element, out: &mut HashSet<String>) {
    for m in &el.members {
        match m {
            ElementMember::Property { value, .. } => collect_expr(value, out),
            ElementMember::Child(c) => collect_element(c, out),
            ElementMember::Stmt(s) => collect_stmt(s, out),
        }
    }
}

/// Immutable child walk (no rewriting) — shared by the collector.
fn walk_children(e: &Expr, f: &mut impl FnMut(&Expr)) {
    use ExprKind as K;
    match &e.kind {
        K::Call { callee, args, block, .. } => {
            f(callee);
            args.iter().for_each(&mut *f);
            if let Some(b) = block {
                f(b);
            }
        }
        K::MethodCall { receiver, args, block, .. }
        | K::SafeMethodCall { receiver, args, block, .. } => {
            f(receiver);
            args.iter().for_each(&mut *f);
            if let Some(b) = block {
                f(b);
            }
        }
        K::Member { receiver, .. } | K::SafeMember { receiver, .. } => f(receiver),
        K::Index { receiver, index } => {
            f(receiver);
            f(index);
        }
        K::Unary { expr, .. } | K::Try(expr) | K::Await(expr) => f(expr),
        K::Binary { lhs, rhs, .. } => {
            f(lhs);
            f(rhs);
        }
        K::If { cond, then_b, else_b, let_binding } => {
            f(cond);
            for s in &then_b.stmts {
                walk_stmt_exprs(s, f);
            }
            if let Some(t) = &then_b.trailing {
                f(t);
            }
            if let Some(eb) = else_b {
                for s in &eb.stmts {
                    walk_stmt_exprs(s, f);
                }
                if let Some(t) = &eb.trailing {
                    f(t);
                }
            }
            if let Some((_, init)) = let_binding {
                f(init);
            }
        }
        K::Case { scrutinee, arms } => {
            f(scrutinee);
            for arm in arms {
                for s in &arm.body.stmts {
                    walk_stmt_exprs(s, f);
                }
                if let Some(t) = &arm.body.trailing {
                    f(t);
                }
            }
        }
        K::Block(b) | K::Lambda { body: b, .. } => {
            for s in &b.stmts {
                walk_stmt_exprs(s, f);
            }
            if let Some(t) = &b.trailing {
                f(t);
            }
        }
        K::Array(items) => items.iter().for_each(&mut *f),
        K::Map(entries) => {
            for (k, v) in entries {
                f(k);
                f(v);
            }
        }
        K::Range { start, end, .. } => {
            f(start);
            f(end);
        }
        K::Kwarg { value, .. } => f(value),
        K::Str(parts) => {
            for p in parts {
                if let pixie_syntax::ast::StrPart::Interp(inner) = p {
                    f(inner);
                }
            }
        }
        K::Element(el) => {
            for m in &el.members {
                match m {
                    ElementMember::Property { value, .. } => f(value),
                    ElementMember::Child(_) | ElementMember::Stmt(_) => {}
                }
            }
        }
        _ => {}
    }
}

fn walk_stmt_exprs(s: &Stmt, f: &mut impl FnMut(&Expr)) {
    match s {
        Stmt::Let { value, .. } | Stmt::Var { value, .. } => f(value),
        Stmt::Assign { target, value, .. } => {
            f(target);
            f(value);
        }
        Stmt::Expr(e) => f(e),
        Stmt::Return { value: Some(v), .. } => f(v),
        Stmt::For { iter, body, .. } => {
            f(iter);
            for s2 in &body.stmts {
                walk_stmt_exprs(s2, f);
            }
            if let Some(t) = &body.trailing {
                f(t);
            }
        }
        Stmt::While { cond, body, .. } => {
            f(cond);
            for s2 in &body.stmts {
                walk_stmt_exprs(s2, f);
            }
            if let Some(t) = &body.trailing {
                f(t);
            }
        }
        Stmt::Batch { body, .. } => {
            for s2 in &body.stmts {
                walk_stmt_exprs(s2, f);
            }
            if let Some(t) = &body.trailing {
                f(t);
            }
        }
        Stmt::Emit { args, .. } => args.iter().for_each(&mut *f),
        _ => {}
    }
}

/// Context for qualified-reference erasure: `quals` maps a local
/// qualifier (alias or leaf) to its canonical module; `resolve`
/// answers (module, member) with the FINAL post-mangling bare name,
/// following `pub use` re-export chains. A miss is a hard error.
pub struct EraseCtx<'a> {
    pub quals: &'a std::collections::HashMap<String, String>,
    pub resolve: &'a dyn Fn(&str, &str) -> Option<String>,
    pub error: Option<(Span, String)>,
}

impl EraseCtx<'_> {
    fn target(&mut self, qual: &str, member: &str, span: Span) -> Option<String> {
        let module = self.quals.get(qual)?.clone();
        match (self.resolve)(&module, member) {
            Some(f) => Some(f),
            None => {
                if self.error.is_none() {
                    self.error = Some((
                        span,
                        format!("module `{module}` has no item `{member}`"),
                    ));
                }
                // Leave the node untouched; the driver reports the
                // recorded error and stops before codegen.
                None
            }
        }
    }
}

/// Rewrite `Q.x` / `Q.m(...)` (and `Q.T` in type position) to the
/// resolved bare names, bottom-up through the whole module. Call only
/// after the shadow check passed.
pub fn erase_module_qualifiers(module: &mut Module, cx: &mut EraseCtx) {
    if cx.quals.is_empty() {
        return;
    }
    for item in &mut module.items {
        match item {
            Item::Class(c) => {
                for m in &mut c.members {
                    match m {
                        ClassMember::Fn(f) | ClassMember::Slot(f) => erase_fn(f, cx),
                        ClassMember::Property(p) => {
                            erase_type(&mut p.ty, cx);
                            if let Some(d) = &mut p.default {
                                erase_expr(d, cx);
                            }
                        }
                        _ => {}
                    }
                }
            }
            Item::Struct(s) => {
                for f in &mut s.fields {
                    erase_type(&mut f.ty, cx);
                }
                for m in &mut s.methods {
                    erase_fn(m, cx);
                }
            }
            Item::Impl(i) => {
                erase_type(&mut i.for_type, cx);
                for m in &mut i.methods {
                    erase_fn(m, cx);
                }
            }
            Item::Fn(f) => erase_fn(f, cx),
            Item::Enum(e) => {
                for v in &mut e.variants {
                    for fl in &mut v.fields {
                        erase_type(&mut fl.ty, cx);
                    }
                }
            }
            Item::Trait(t) => {
                for m in &mut t.methods {
                    erase_fn(m, cx);
                }
            }
            Item::Let(l) => {
                erase_type(&mut l.ty, cx);
                erase_expr(&mut l.value, cx);
            }
            Item::View(v) => {
                for sf in &mut v.state_fields {
                    if let StateFieldKind::Property { ty } = &mut sf.kind {
                        erase_type(ty, cx);
                    }
                    erase_expr(&mut sf.init_expr, cx);
                }
                erase_element(&mut v.root, cx);
            }
            _ => {}
        }
    }
}

fn erase_fn(f: &mut FnDecl, cx: &mut EraseCtx) {
    for p in &mut f.params {
        erase_type(&mut p.ty, cx);
    }
    if let Some(t) = &mut f.return_ty {
        erase_type(t, cx);
    }
    if let Some(b) = &mut f.body {
        erase_block(b, cx);
    }
}

fn erase_type(t: &mut TypeExpr, cx: &mut EraseCtx) {
    match &mut t.kind {
        TypeKind::Named { path, args } => {
            if path.len() == 2 && cx.quals.contains_key(&path[0].name) {
                let span = path[0].span;
                if let Some(final_name) = cx.target(&path[0].name.clone(), &path[1].name.clone(), span) {
                    let mut seg = path[1].clone();
                    seg.name = final_name;
                    *path = vec![seg];
                }
            }
            for a in args {
                erase_type(a, cx);
            }
        }
        TypeKind::Nullable(inner) | TypeKind::ErrorUnion(inner) => erase_type(inner, cx),
        TypeKind::Fn { params, ret } => {
            for p in params {
                erase_type(p, cx);
            }
            erase_type(ret, cx);
        }
        TypeKind::SelfType => {}
    }
}

fn erase_block(b: &mut Block, cx: &mut EraseCtx) {
    for s in &mut b.stmts {
        erase_stmt(s, cx);
    }
    if let Some(t) = &mut b.trailing {
        erase_expr(t, cx);
    }
}

fn erase_stmt(s: &mut Stmt, cx: &mut EraseCtx) {
    match s {
        Stmt::Let { ty, value, .. } | Stmt::Var { ty, value, .. } => {
            if let Some(t) = ty {
                erase_type(t, cx);
            }
            erase_expr(value, cx);
        }
        Stmt::Assign { target, value, .. } => {
            erase_expr(target, cx);
            erase_expr(value, cx);
        }
        Stmt::Expr(e) => erase_expr(e, cx),
        Stmt::Return { value: Some(v), .. } => erase_expr(v, cx),
        Stmt::For { iter, body, .. } => {
            erase_expr(iter, cx);
            erase_block(body, cx);
        }
        Stmt::While { cond, body, .. } => {
            erase_expr(cond, cx);
            erase_block(body, cx);
        }
        Stmt::Batch { body, .. } => erase_block(body, cx),
        Stmt::Emit { args, .. } => {
            for a in args {
                erase_expr(a, cx);
            }
        }
        _ => {}
    }
}

fn erase_element(el: &mut Element, cx: &mut EraseCtx) {
    // Element qualifiers are NOT erased: a qualified element can only
    // mean a cross-module view component (§8.29), and the component
    // splice resolves it from the module's own `use` items — erasing
    // here would leave a bare name the splice cannot attribute.
    for m in &mut el.members {
        match m {
            ElementMember::Property { value, .. } => erase_expr(value, cx),
            ElementMember::Child(c) => erase_element(c, cx),
            ElementMember::Stmt(s) => erase_stmt(s, cx),
        }
    }
}

fn erase_expr(e: &mut Expr, cx: &mut EraseCtx) {
    use ExprKind as K;
    match &mut e.kind {
        K::Call { callee, args, block, type_args } => {
            erase_expr(callee, cx);
            for a in args.iter_mut() {
                erase_expr(a, cx);
            }
            if let Some(b) = block {
                erase_expr(b, cx);
            }
            for t in type_args {
                erase_type(t, cx);
            }
        }
        K::MethodCall { receiver, args, block, type_args, .. }
        | K::SafeMethodCall { receiver, args, block, type_args, .. } => {
            erase_expr(receiver, cx);
            for a in args.iter_mut() {
                erase_expr(a, cx);
            }
            if let Some(b) = block {
                erase_expr(b, cx);
            }
            for t in type_args {
                erase_type(t, cx);
            }
        }
        K::Member { receiver, .. } | K::SafeMember { receiver, .. } => {
            erase_expr(receiver, cx)
        }
        K::Index { receiver, index } => {
            erase_expr(receiver, cx);
            erase_expr(index, cx);
        }
        K::Unary { expr, .. } | K::Try(expr) | K::Await(expr) => erase_expr(expr, cx),
        K::Binary { lhs, rhs, .. } => {
            erase_expr(lhs, cx);
            erase_expr(rhs, cx);
        }
        K::If { cond, then_b, else_b, let_binding } => {
            erase_expr(cond, cx);
            erase_block(then_b, cx);
            if let Some(eb) = else_b {
                erase_block(eb, cx);
            }
            if let Some((_, init)) = let_binding {
                erase_expr(init, cx);
            }
        }
        K::Case { scrutinee, arms } => {
            erase_expr(scrutinee, cx);
            for arm in arms {
                erase_block(&mut arm.body, cx);
            }
        }
        K::Block(b) | K::Lambda { body: b, .. } => erase_block(b, cx),
        K::Array(items) => {
            for i in items.iter_mut() {
                erase_expr(i, cx);
            }
        }
        K::Map(entries) => {
            for (k, v) in entries.iter_mut() {
                erase_expr(k, cx);
                erase_expr(v, cx);
            }
        }
        K::Range { start, end, .. } => {
            erase_expr(start, cx);
            erase_expr(end, cx);
        }
        K::Kwarg { value, .. } => erase_expr(value, cx),
        K::Str(parts) => {
            for p in parts.iter_mut() {
                if let StrPart::Interp(inner) = p {
                    erase_expr(inner, cx);
                }
            }
        }
        K::Element(el) => erase_element(el, cx),
        _ => {}
    }
    let replacement = match &e.kind {
        K::Member { receiver, name } => match &receiver.kind {
            K::Ident(q) if cx.quals.contains_key(q) => {
                let q = q.clone();
                let n = name.clone();
                cx.target(&q, &n.name, e.span).map(|f| K::Ident(f))
            }
            _ => None,
        },
        K::MethodCall { receiver, method, args, block, type_args } => match &receiver.kind {
            K::Ident(q) if cx.quals.contains_key(q) => {
                let q = q.clone();
                let m = method.clone();
                let (args, block, type_args) = (args.clone(), block.clone(), type_args.clone());
                cx.target(&q, &m.name, e.span).map(|f| K::Call {
                    callee: Box::new(Expr {
                        kind: K::Ident(f),
                        span: m.span,
                    }),
                    args,
                    block,
                    type_args,
                })
            }
            _ => None,
        },
        _ => None,
    };
    if let Some(kind) = replacement {
        e.kind = kind;
    }
}

// ---------------------------------------------------------------------------
// §12.1 completion — cross-module name resolution with mangling.
//
// The emitter's namespace is flat, so a surface name declared in more
// than one loaded module ("contested") is renamed per declaring
// module to `<name>__<module_path_with_underscores>`, and every
// reference — bare, qualified, selective, re-exported — is rewritten
// to the final unique name before the merge. Uncontested names are
// untouched (zero diff for collision-free projects). Shadowing is
// never guessed at: a rewrite key that also appears as a binder in
// the module is a hard error naming both sides.

/// One module's item declarations (post store/suite desugar):
/// (surface name, is_pub).
pub fn item_decls(module: &Module) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    for item in &module.items {
        match item {
            Item::Class(c) => out.push((c.name.name.clone(), c.is_pub)),
            Item::Struct(s) => out.push((s.name.name.clone(), s.is_pub)),
            Item::Fn(f) => {
                if !f.is_test {
                    out.push((f.name.name.clone(), f.is_pub));
                }
            }
            Item::Enum(e) => out.push((e.name.name.clone(), e.is_pub)),
            Item::Trait(t) => out.push((t.name.name.clone(), t.is_pub)),
            Item::Let(l) => out.push((l.name.name.clone(), l.is_pub)),
            _ => {}
        }
    }
    out
}

pub fn mangle(name: &str, module: &str) -> String {
    format!("{name}__{}", module.replace('.', "_"))
}

/// Apply `map` (old bare name → final name) across a module: item
/// declaration names, value references, and type references. The
/// caller has already verified no key shadows a binder.
pub fn rename_in_module(module: &mut Module, map: &std::collections::HashMap<String, String>) {
    if map.is_empty() {
        return;
    }
    let r = |n: &mut Ident| {
        if let Some(new) = map.get(&n.name) {
            n.name = new.clone();
        }
    };
    for item in &mut module.items {
        match item {
            Item::Class(c) => {
                r(&mut c.name);
                for m in &mut c.members {
                    match m {
                        ClassMember::Property(p) => {
                            rename_type(&mut p.ty, map);
                            if let Some(d) = &mut p.default {
                                rename_expr(d, map);
                            }
                        }
                        ClassMember::Fn(f) | ClassMember::Slot(f) => rename_fn(f, map),
                        _ => {}
                    }
                }
            }
            Item::Struct(s) => {
                r(&mut s.name);
                for f in &mut s.fields {
                    rename_type(&mut f.ty, map);
                }
                for m in &mut s.methods {
                    rename_fn(m, map);
                }
            }
            Item::Fn(f) => {
                r(&mut f.name);
                rename_fn(f, map);
            }
            Item::Enum(e) => {
                r(&mut e.name);
                for v in &mut e.variants {
                    for fl in &mut v.fields {
                        rename_type(&mut fl.ty, map);
                    }
                }
            }
            Item::Trait(t) => {
                r(&mut t.name);
                for m in &mut t.methods {
                    rename_fn(m, map);
                }
            }
            Item::Impl(i) => {
                r(&mut i.trait_name);
                rename_type(&mut i.for_type, map);
                for g in &mut i.generics {
                    for b in &mut g.bounds {
                        r(b);
                    }
                }
                for m in &mut i.methods {
                    rename_fn(m, map);
                }
            }
            Item::Let(l) => {
                r(&mut l.name);
                rename_type(&mut l.ty, map);
                rename_expr(&mut l.value, map);
            }
            Item::View(v) => {
                for sf in &mut v.state_fields {
                    if let StateFieldKind::Property { ty } = &mut sf.kind {
                        rename_type(ty, map);
                    }
                    rename_expr(&mut sf.init_expr, map);
                }
                rename_element(&mut v.root, map);
            }
            _ => {}
        }
    }
}

fn rename_fn(f: &mut FnDecl, map: &std::collections::HashMap<String, String>) {
    for g in &mut f.generics {
        for b in &mut g.bounds {
            if let Some(new) = map.get(&b.name) {
                b.name = new.clone();
            }
        }
    }
    for p in &mut f.params {
        rename_type(&mut p.ty, map);
    }
    if let Some(t) = &mut f.return_ty {
        rename_type(t, map);
    }
    if let Some(b) = &mut f.body {
        rename_block(b, map);
    }
}

fn rename_type(t: &mut TypeExpr, map: &std::collections::HashMap<String, String>) {
    match &mut t.kind {
        TypeKind::Named { path, args } => {
            // Single-segment type names rename; qualified type paths
            // are handled by the qualifier-erasure pass first, so by
            // rename time they are single-segment too.
            if path.len() == 1 {
                if let Some(new) = map.get(&path[0].name) {
                    path[0].name = new.clone();
                }
            }
            for a in args {
                rename_type(a, map);
            }
        }
        TypeKind::Nullable(inner) | TypeKind::ErrorUnion(inner) => rename_type(inner, map),
        TypeKind::Fn { params, ret } => {
            for p in params {
                rename_type(p, map);
            }
            rename_type(ret, map);
        }
        TypeKind::SelfType => {}
    }
}

fn rename_block(b: &mut Block, map: &std::collections::HashMap<String, String>) {
    for s in &mut b.stmts {
        rename_stmt(s, map);
    }
    if let Some(t) = &mut b.trailing {
        rename_expr(t, map);
    }
}

fn rename_stmt(s: &mut Stmt, map: &std::collections::HashMap<String, String>) {
    match s {
        Stmt::Let { ty, value, .. } | Stmt::Var { ty, value, .. } => {
            if let Some(t) = ty {
                rename_type(t, map);
            }
            rename_expr(value, map);
        }
        Stmt::Assign { target, value, .. } => {
            rename_expr(target, map);
            rename_expr(value, map);
        }
        Stmt::Expr(e) => rename_expr(e, map),
        Stmt::Return { value: Some(v), .. } => rename_expr(v, map),
        Stmt::For { iter, body, .. } => {
            rename_expr(iter, map);
            rename_block(body, map);
        }
        Stmt::While { cond, body, .. } => {
            rename_expr(cond, map);
            rename_block(body, map);
        }
        Stmt::Batch { body, .. } => rename_block(body, map),
        Stmt::Emit { args, .. } => {
            for a in args {
                rename_expr(a, map);
            }
        }
        _ => {}
    }
}

fn rename_element(el: &mut Element, map: &std::collections::HashMap<String, String>) {
    for m in &mut el.members {
        match m {
            ElementMember::Property { value, .. } => rename_expr(value, map),
            ElementMember::Child(c) => rename_element(c, map),
            ElementMember::Stmt(s) => rename_stmt(s, map),
        }
    }
}

fn rename_expr(e: &mut Expr, map: &std::collections::HashMap<String, String>) {
    use ExprKind as K;
    match &mut e.kind {
        K::Ident(n) => {
            if let Some(new) = map.get(n) {
                *n = new.clone();
            }
        }
        K::Call { callee, args, block, type_args } => {
            rename_expr(callee, map);
            for a in args.iter_mut() {
                rename_expr(a, map);
            }
            if let Some(b) = block {
                rename_expr(b, map);
            }
            for t in type_args {
                rename_type(t, map);
            }
        }
        K::MethodCall { receiver, args, block, type_args, .. }
        | K::SafeMethodCall { receiver, args, block, type_args, .. } => {
            rename_expr(receiver, map);
            for a in args.iter_mut() {
                rename_expr(a, map);
            }
            if let Some(b) = block {
                rename_expr(b, map);
            }
            for t in type_args {
                rename_type(t, map);
            }
        }
        K::Member { receiver, .. } | K::SafeMember { receiver, .. } => rename_expr(receiver, map),
        K::Index { receiver, index } => {
            rename_expr(receiver, map);
            rename_expr(index, map);
        }
        K::Unary { expr, .. } | K::Try(expr) | K::Await(expr) => rename_expr(expr, map),
        K::Binary { lhs, rhs, .. } => {
            rename_expr(lhs, map);
            rename_expr(rhs, map);
        }
        K::If { cond, then_b, else_b, let_binding } => {
            rename_expr(cond, map);
            rename_block(then_b, map);
            if let Some(eb) = else_b {
                rename_block(eb, map);
            }
            if let Some((_, init)) = let_binding {
                rename_expr(init, map);
            }
        }
        K::Case { scrutinee, arms } => {
            rename_expr(scrutinee, map);
            for arm in arms {
                rename_block(&mut arm.body, map);
            }
        }
        K::Block(b) | K::Lambda { body: b, .. } => rename_block(b, map),
        K::Array(items) => {
            for i in items.iter_mut() {
                rename_expr(i, map);
            }
        }
        K::Map(entries) => {
            for (k, v) in entries.iter_mut() {
                rename_expr(k, map);
                rename_expr(v, map);
            }
        }
        K::Range { start, end, .. } => {
            rename_expr(start, map);
            rename_expr(end, map);
        }
        K::Kwarg { value, .. } => rename_expr(value, map),
        K::Str(parts) => {
            for p in parts.iter_mut() {
                if let StrPart::Interp(inner) = p {
                    rename_expr(inner, map);
                }
            }
        }
        K::Element(el) => rename_element(el, map),
        _ => {}
    }
}

/// Binder names only (params, locals, loop/case/lambda binders, prop
/// and state names) — ITEM names excluded, so a contested item's own
/// name doesn't read as its own shadow.
pub fn binder_names(module: &Module) -> HashSet<String> {
    let mut out = HashSet::new();
    let add_fn = |f: &pixie_syntax::ast::FnDecl, out: &mut HashSet<String>| {
        for p in &f.params {
            out.insert(p.name.name.clone());
        }
        if let Some(b) = &f.body {
            collect_block(b, out);
        }
    };
    for item in &module.items {
        match item {
            Item::Class(c) => {
                for m in &c.members {
                    match m {
                        ClassMember::Property(p) => {
                            out.insert(p.name.name.clone());
                        }
                        ClassMember::Fn(f) | ClassMember::Slot(f) => add_fn(f, &mut out),
                        _ => {}
                    }
                }
            }
            Item::Struct(s) => {
                for m in &s.methods {
                    add_fn(m, &mut out);
                }
            }
            Item::Fn(f) => add_fn(f, &mut out),
            Item::Impl(i) => {
                for m in &i.methods {
                    add_fn(m, &mut out);
                }
            }
            Item::View(v) => {
                for sf in &v.state_fields {
                    out.insert(sf.name.name.clone());
                }
                collect_element(&v.root, &mut out);
            }
            _ => {}
        }
    }
    out
}

/// Does `name` occur anywhere as a bare identifier expression? Drives
/// the contested-ambiguity error (a name importable from two modules
/// is only an error if the module actually says it bare).
pub fn mentions_ident(module: &Module, name: &str) -> bool {
    let mut found = false;
    for item in &module.items {
        match item {
            Item::Class(c) => {
                for m in &c.members {
                    match m {
                        ClassMember::Fn(f) | ClassMember::Slot(f) => scan_fn(f, name, &mut found),
                        ClassMember::Property(p) => {
                            if let Some(d) = &p.default {
                                scan_expr(d, name, &mut found);
                            }
                        }
                        _ => {}
                    }
                }
            }
            Item::Struct(s) => {
                for m in &s.methods {
                    scan_fn(m, name, &mut found);
                }
            }
            Item::Impl(i) => {
                for m in &i.methods {
                    scan_fn(m, name, &mut found);
                }
            }
            Item::Fn(f) => scan_fn(f, name, &mut found),
            Item::Let(l) => scan_expr(&l.value, name, &mut found),
            Item::View(v) => {
                for sf in &v.state_fields {
                    scan_expr(&sf.init_expr, name, &mut found);
                }
                scan_element(&v.root, name, &mut found);
            }
            _ => {}
        }
        if found {
            return true;
        }
    }
    found
}

fn scan_fn(f: &pixie_syntax::ast::FnDecl, name: &str, found: &mut bool) {
    if let Some(b) = &f.body {
        for s in &b.stmts {
            walk_stmt_exprs(s, &mut |e| scan_expr(e, name, found));
        }
        if let Some(t) = &b.trailing {
            scan_expr(t, name, found);
        }
    }
}

fn scan_element(el: &Element, name: &str, found: &mut bool) {
    for m in &el.members {
        match m {
            ElementMember::Property { value, .. } => scan_expr(value, name, found),
            ElementMember::Child(c) => scan_element(c, name, found),
            ElementMember::Stmt(s) => {
                walk_stmt_exprs(s, &mut |e| scan_expr(e, name, found))
            }
        }
    }
}

fn scan_expr(e: &Expr, name: &str, found: &mut bool) {
    if *found {
        return;
    }
    if let ExprKind::Ident(n) = &e.kind {
        if n == name {
            *found = true;
            return;
        }
    }
    walk_children(e, &mut |c| scan_expr(c, name, found));
}
