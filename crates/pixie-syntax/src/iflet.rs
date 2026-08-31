//! `if let pat = init { .. } else { .. }` → the `case` it already is.
//!
//! The parser has built this shape since the fork and no lowerer ever
//! read it, so it answered "not lowerable yet (M2)" in four places
//! (method bodies, `init` bodies, view bodies, handler bodies). It is
//! not a deferred feature: a two-armed `case` is exactly what it
//! means, and that lowers in every one of those places already.
//!
//! Desugaring here rather than in each lowerer is the §8.56 rule —
//! both execution tiers walk one tree, so they cannot disagree about
//! what `if let` accepts. The CHECKER sees the `case` too, which is
//! the part a per-lowerer desugar would have missed: arm
//! exhaustiveness, the binding's type, and the scrutinee's.

use crate::ast::*;
use crate::span::Span;

/// Rewrite every `if let` in the module. Returns the module unchanged
/// when there are none.
pub fn desugar_module(m: &Module) -> Module {
    let mut out = m.clone();
    for item in out.items.iter_mut() {
        item_mut(item);
    }
    out
}

fn item_mut(item: &mut Item) {
    match item {
        Item::Class(c) => {
            for member in c.members.iter_mut() {
                match member {
                    ClassMember::Fn(f) | ClassMember::Slot(f) => {
                        if let Some(b) = f.body.as_mut() {
                            block_mut(b);
                        }
                    }
                    ClassMember::Init(i) => block_mut(&mut i.body),
                    ClassMember::Deinit(d) => block_mut(&mut d.body),
                    ClassMember::Property(p) => {
                        if let Some(d) = p.default.as_mut() {
                            expr_mut(d);
                        }
                        if let Some(b) = p.binding.as_mut() {
                            expr_mut(b);
                        }
                    }
                    ClassMember::Field(f) => {
                        if let Some(d) = f.default.as_mut() {
                            expr_mut(d);
                        }
                    }
                    ClassMember::Signal(_) => {}
                }
            }
        }
        Item::Fn(f) => {
            if let Some(b) = f.body.as_mut() {
                block_mut(b);
            }
        }
        Item::View(v) => {
            element_mut(&mut v.root);
            for sf in v.state_fields.iter_mut() {
                expr_mut(&mut sf.init_expr);
            }
        }
        Item::Struct(s) => {
            for f in s.fields.iter_mut() {
                if let Some(d) = f.default.as_mut() {
                    expr_mut(d);
                }
            }
            for m in s.methods.iter_mut() {
                if let Some(b) = m.body.as_mut() {
                    block_mut(b);
                }
            }
        }
        Item::Impl(i) => {
            for m in i.methods.iter_mut() {
                if let Some(b) = m.body.as_mut() {
                    block_mut(b);
                }
            }
        }
        Item::Let(l) => expr_mut(&mut l.value),
        _ => {}
    }
}

fn element_mut(el: &mut Element) {
    for m in el.members.iter_mut() {
        element_member_mut(m);
    }
}

fn element_member_mut(m: &mut ElementMember) {
    match m {
        ElementMember::Property { value, .. } => expr_mut(value),
        ElementMember::Child(c) => element_mut(c),
        ElementMember::Stmt(s) => stmt_mut(s),
    }
}

fn block_mut(b: &mut Block) {
    for s in b.stmts.iter_mut() {
        stmt_mut(s);
    }
    if let Some(t) = b.trailing.as_mut() {
        expr_mut(t);
    }
}

fn stmt_mut(s: &mut Stmt) {
    match s {
        Stmt::Let { value, .. } | Stmt::Var { value, .. } => expr_mut(value),
        Stmt::Assign { target, value, .. } => {
            expr_mut(target);
            expr_mut(value);
        }
        Stmt::Expr(e) => expr_mut(e),
        Stmt::Return { value, .. } => {
            if let Some(v) = value.as_mut() {
                expr_mut(v);
            }
        }
        Stmt::Emit { args, .. } => {
            for a in args.iter_mut() {
                expr_mut(a);
            }
        }
        Stmt::For { iter, body, .. } => {
            expr_mut(iter);
            block_mut(body);
        }
        Stmt::While { cond, body, .. } => {
            expr_mut(cond);
            block_mut(body);
        }
        Stmt::Batch { body, .. } => block_mut(body),
        Stmt::Break { .. } | Stmt::Continue { .. } => {}
    }
}

fn expr_mut(e: &mut Expr) {
    // Rewrite the node itself first, then walk what it became: the
    // `case` this produces carries the same blocks, which may hold
    // more `if let`s.
    if matches!(&e.kind, ExprKind::If { let_binding: Some(_), .. }) {
        rewrite_if_let(e);
    }
    match &mut e.kind {
        ExprKind::If {
            cond,
            then_b,
            else_b,
            ..
        } => {
            expr_mut(cond);
            block_mut(then_b);
            if let Some(b) = else_b.as_mut() {
                block_mut(b);
            }
        }
        ExprKind::Case { scrutinee, arms } => {
            expr_mut(scrutinee);
            for a in arms.iter_mut() {
                block_mut(&mut a.body);
            }
        }
        ExprKind::Element(el) => element_mut(el),
        ExprKind::Block(b) => block_mut(b),
        ExprKind::Binary { lhs, rhs, .. } => {
            expr_mut(lhs);
            expr_mut(rhs);
        }
        ExprKind::Unary { expr, .. } => expr_mut(expr),
        ExprKind::Member { receiver, .. } => expr_mut(receiver),
        ExprKind::Index { receiver, index } => {
            expr_mut(receiver);
            expr_mut(index);
        }
        ExprKind::Call { callee, args, block, .. } => {
            expr_mut(callee);
            for a in args.iter_mut() {
                expr_mut(a);
            }
            if let Some(b) = block.as_mut() {
                expr_mut(b);
            }
        }
        ExprKind::MethodCall { receiver, args, block, .. } => {
            expr_mut(receiver);
            for a in args.iter_mut() {
                expr_mut(a);
            }
            if let Some(b) = block.as_mut() {
                expr_mut(b);
            }
        }
        ExprKind::Str(parts) => {
            for p in parts.iter_mut() {
                match p {
                    StrPart::Text(_) => {}
                    StrPart::Interp(x) => expr_mut(x),
                    StrPart::InterpFmt { expr, .. } => expr_mut(expr),
                }
            }
        }
        ExprKind::Array(items) => {
            for i in items.iter_mut() {
                expr_mut(i);
            }
        }
        ExprKind::Map(entries) => {
            for (k, v) in entries.iter_mut() {
                expr_mut(k);
                expr_mut(v);
            }
        }
        ExprKind::Try(x) | ExprKind::Await(x) => expr_mut(x),
        ExprKind::Range { start, end, .. } => {
            expr_mut(start);
            expr_mut(end);
        }
        ExprKind::Lambda { body, .. } => block_mut(body),
        _ => {}
    }
}

/// `if let pat = init { A } else { B }` becomes
/// `case init { when pat { A } when <fallback> { B } }`.
fn rewrite_if_let(e: &mut Expr) {
    let ExprKind::If {
        then_b,
        else_b,
        let_binding,
        ..
    } = &mut e.kind
    else {
        return;
    };
    let Some((pat, init)) = let_binding.take() else {
        return;
    };
    let span = e.span;
    let fallback = fallback_pattern(&pat, span);
    let arms = vec![
        CaseArm {
            pattern: *pat,
            body: then_b.clone(),
            span,
        },
        CaseArm {
            pattern: fallback,
            body: else_b.clone().unwrap_or(Block {
                stmts: Vec::new(),
                trailing: None,
                span,
            }),
            span,
        },
    ];
    e.kind = ExprKind::Case {
        scrutinee: init,
        arms,
    };
}

/// The arm that runs when the pattern does not match. `case` asks for
/// the OTHER half by name for the two shapes that have one — a `T?`
/// wants `nil`, a fallible wants the opposite of `ok`/`err` — and a
/// wildcard covers everything else.
fn fallback_pattern(pat: &Pattern, span: Span) -> Pattern {
    let ctor = |n: &str| Pattern::Ctor {
        name: Ident {
            name: n.to_string(),
            span,
        },
        args: Vec::new(),
        span,
    };
    match pat {
        Pattern::Ctor { name, .. } => match name.name.as_str() {
            "some" => Pattern::Literal {
                value: Expr {
                    kind: ExprKind::Nil,
                    span,
                },
                span,
            },
            "ok" => ctor("err"),
            "err" => ctor("ok"),
            _ => Pattern::Wild { span },
        },
        _ => Pattern::Wild { span },
    }
}
