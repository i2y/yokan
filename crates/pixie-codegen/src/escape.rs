//! Escape analysis for method-local objects (§11.24 / §8.42).
//!
//! `World::remove` has always existed and the emitter never called
//! it, so memory was monotonic in objects CREATED rather than objects
//! live. Measured: 200 000 method-local `Class()` instances cost
//! 20 MB that never came back, growing linearly.
//!
//! This closes that, and only that. It is deliberately the smallest
//! sound answer rather than the most complete one:
//!
//! - **Deterministic.** A reclaim happens at a point in the program,
//!   not when a collector decides. No pauses, no root descriptors, no
//!   runtime bookkeeping — the emitter writes one more statement.
//! - **Conservative by construction.** The rule is a WHITELIST: an
//!   object stays local only if every mention of it is a property
//!   read or a property write on it. Anything else — an argument, a
//!   return, an assignment source, a method receiver, a bare mention
//!   — is an escape. New syntax that this file has not been taught
//!   about therefore reads as an escape, which is the safe direction.
//! - **Failure is loud, not silent.** Handles are generational
//!   (§3.4), so if this analysis were ever wrong the program panics
//!   with "stale handle" at the next access. It cannot corrupt
//!   memory or hand back the wrong object.
//!
//! What it does NOT do: reclaim anything an object graph holds
//! (§8.40 made those real), anything a store keeps, or row seats
//! (grow-only by design, §8.30). Those need a collector or ownership
//! links, and that decision is still open.

use pixie_syntax::ast::{Block, Expr, ExprKind, Stmt, StrPart};

/// Does `name` escape anywhere in these statements?
///
/// The caller passes the statements that FOLLOW the binding, plus the
/// block's trailing expression. `true` means "do not reclaim", and
/// every unknown shape answers `true`.
pub fn escapes(name: &str, rest: &[Stmt], trailing: Option<&Expr>) -> bool {
    rest.iter().any(|s| stmt_escapes(name, s))
        || trailing.is_some_and(|t| expr_escapes(name, t))
}

fn block_escapes(name: &str, b: &Block) -> bool {
    escapes(name, &b.stmts, b.trailing.as_deref())
}

fn opt_block_escapes(name: &str, b: Option<&Block>) -> bool {
    b.is_some_and(|b| block_escapes(name, b))
}

fn stmt_escapes(name: &str, s: &Stmt) -> bool {
    match s {
        // `let y = x` aliases the object under a second name, and the
        // alias is not tracked — treat the binding as an escape.
        Stmt::Let { value, .. } | Stmt::Var { value, .. } => expr_escapes(name, value),
        Stmt::Assign { target, value, .. } => {
            // Assigning INTO the object (`x.p = v`) is fine; assigning
            // the object anywhere is not.
            expr_escapes(name, value) || assign_target_escapes(name, target)
        }
        Stmt::Expr(e) => expr_escapes(name, e),
        // Returning the object lets it out; returning one of its
        // property values does not.
        Stmt::Return { value, .. } => value.as_ref().is_some_and(|v| expr_escapes(name, v)),
        Stmt::For { iter, body, .. } => expr_escapes(name, iter) || block_escapes(name, body),
        Stmt::While { cond, body, .. } => expr_escapes(name, cond) || block_escapes(name, body),
        Stmt::Emit { args, .. } => args.iter().any(|a| expr_escapes(name, a)),
        Stmt::Break { .. } | Stmt::Continue { .. } => false,
        // Anything this file has not been taught: assume it escapes.
        _ => true,
    }
}

/// An assignment target mentioning `x`. `x.p = v` and `x.a.b = v` are
/// writes THROUGH the object and keep it local; a bare `x = v`
/// rebinds the name, which loses track of the object.
fn assign_target_escapes(name: &str, target: &Expr) -> bool {
    match &target.kind {
        // A bare rebinding (`x = other`) loses the object.
        ExprKind::Ident(n) | ExprKind::AtIdent(n) => n == name,
        // Anything else is a write THROUGH something, and reaching a
        // place to write is the same question as reading a chain.
        _ => expr_escapes(name, target),
    }
}

/// Does this expression use `name` in any way OTHER than reading a
/// property off it?
fn expr_escapes(name: &str, e: &Expr) -> bool {
    match &e.kind {
        // The whole point: a bare mention hands the object to whatever
        // context it sits in.
        ExprKind::Ident(n) | ExprKind::AtIdent(n) => n == name,
        // `x.p`, `x.a.b` — a read chain rooted at the object is safe,
        // and so is the object being the root of one.
        ExprKind::Member { receiver, .. } => match &receiver.kind {
            ExprKind::Ident(n) | ExprKind::AtIdent(n) if n == name => false,
            _ => expr_escapes(name, receiver),
        },
        // A method CALL on the object could store `self` — the emitter
        // does not analyse across bodies, so it escapes.
        // A method CALL on the object could store `self` — the emitter
        // does not analyse across bodies, so the RECEIVER position is
        // an escape. The arguments are not: passing `x.p` passes the
        // property's VALUE, and only passing `x` passes the object.
        // That distinction matters because interpolating a property
        // into a string handed to something else — `push("#{x.p}")` —
        // is the most ordinary thing done with a local object.
        ExprKind::MethodCall {
            receiver,
            args,
            block,
            ..
        } => {
            mentions(name, receiver)
                || args.iter().any(|a| expr_escapes(name, a))
                || block.as_ref().is_some_and(|b| mentions(name, b))
        }
        ExprKind::Call { callee, args, block, .. } => {
            mentions(name, callee)
                || args.iter().any(|a| expr_escapes(name, a))
                || block.as_ref().is_some_and(|b| mentions(name, b))
        }
        ExprKind::Index { receiver, index } => {
            expr_escapes(name, receiver) || expr_escapes(name, index)
        }
        ExprKind::Binary { lhs, rhs, .. } => {
            expr_escapes(name, lhs) || expr_escapes(name, rhs)
        }
        ExprKind::Unary { expr, .. }
        | ExprKind::Try(expr)
        | ExprKind::Await(expr) => expr_escapes(name, expr),
        ExprKind::Str(parts) => parts.iter().any(|p| match p {
            StrPart::Text(_) => false,
            StrPart::Interp(inner) => expr_escapes(name, inner),
            StrPart::InterpFmt { expr, .. } => expr_escapes(name, expr),
        }),
        // A literal holding the object hands it to a container; a
        // literal holding one of its property values does not.
        ExprKind::Array(items) => items.iter().any(|i| expr_escapes(name, i)),
        ExprKind::Map(entries) => entries
            .iter()
            .any(|(k, v)| expr_escapes(name, k) || expr_escapes(name, v)),
        ExprKind::If {
            cond,
            then_b,
            else_b,
            ..
        } => {
            expr_escapes(name, cond)
                || block_escapes(name, then_b)
                || opt_block_escapes(name, else_b.as_ref())
        }
        ExprKind::Case { scrutinee, arms, .. } => {
            expr_escapes(name, scrutinee) || arms.iter().any(|a| block_escapes(name, &a.body))
        }
        ExprKind::Block(b) => block_escapes(name, b),
        // A lambda captures; a closure outliving the scope is exactly
        // what reclaiming must not race.
        ExprKind::Lambda { body, .. } => block_escapes(name, body),
        ExprKind::Int(_)
        | ExprKind::Float(_)
        | ExprKind::Bool(_)
        | ExprKind::Nil
        | ExprKind::SelfRef
        | ExprKind::Path(_) => false,
        // Unknown shape: assume the object got away.
        _ => mentions(name, e),
    }
}

/// Does `name` appear at all? Used where ANY mention is an escape (a
/// call argument, a return value, a list element).
fn mentions(name: &str, e: &Expr) -> bool {
    let mut hit = false;
    walk(e, &mut |x| {
        if let ExprKind::Ident(n) | ExprKind::AtIdent(n) = &x.kind {
            if n == name {
                hit = true;
            }
        }
    });
    hit
}

/// A conservative pre-order walk. It does not need to be exhaustive
/// over the grammar to be SOUND, because it feeds `mentions`, whose
/// callers already treat an unknown shape as an escape — but the more
/// it covers, the fewer false escapes.
fn walk(e: &Expr, f: &mut impl FnMut(&Expr)) {
    f(e);
    match &e.kind {
        ExprKind::Member { receiver, .. } => walk(receiver, f),
        ExprKind::MethodCall {
            receiver,
            args,
            block,
            ..
        } => {
            walk(receiver, f);
            for a in args {
                walk(a, f);
            }
            if let Some(b) = block {
                walk(b, f);
            }
        }
        ExprKind::Call { callee, args, block, .. } => {
            walk(callee, f);
            for a in args {
                walk(a, f);
            }
            if let Some(b) = block {
                walk(b, f);
            }
        }
        ExprKind::Index { receiver, index } => {
            walk(receiver, f);
            walk(index, f);
        }
        ExprKind::Binary { lhs, rhs, .. } => {
            walk(lhs, f);
            walk(rhs, f);
        }
        ExprKind::Unary { expr, .. } | ExprKind::Try(expr) | ExprKind::Await(expr) => {
            walk(expr, f)
        }
        ExprKind::Str(parts) => {
            for p in parts {
                match p {
                    StrPart::Text(_) => {}
                    StrPart::Interp(inner) => walk(inner, f),
                    StrPart::InterpFmt { expr, .. } => walk(expr, f),
                }
            }
        }
        ExprKind::Array(items) => {
            for i in items {
                walk(i, f);
            }
        }
        ExprKind::Map(entries) => {
            for (k, v) in entries {
                walk(k, f);
                walk(v, f);
            }
        }
        ExprKind::If {
            cond,
            then_b,
            else_b,
            ..
        } => {
            walk(cond, f);
            walk_block(then_b, f);
            if let Some(b) = else_b {
                walk_block(b, f);
            }
        }
        ExprKind::Case { scrutinee, arms, .. } => {
            walk(scrutinee, f);
            for a in arms {
                walk_block(&a.body, f);
            }
        }
        ExprKind::Block(b) | ExprKind::Lambda { body: b, .. } => walk_block(b, f),
        ExprKind::Range { start, end, .. } => {
            walk(start, f);
            walk(end, f);
        }
        _ => {}
    }
}

fn walk_block(b: &Block, f: &mut impl FnMut(&Expr)) {
    for s in &b.stmts {
        walk_stmt(s, f);
    }
    if let Some(t) = &b.trailing {
        walk(t, f);
    }
}

fn walk_stmt(s: &Stmt, f: &mut impl FnMut(&Expr)) {
    match s {
        Stmt::Let { value, .. } | Stmt::Var { value, .. } => walk(value, f),
        Stmt::Assign { target, value, .. } => {
            walk(target, f);
            walk(value, f);
        }
        Stmt::Expr(e) => walk(e, f),
        Stmt::Return { value, .. } => {
            if let Some(v) = value {
                walk(v, f);
            }
        }
        Stmt::For { iter, body, .. } => {
            walk(iter, f);
            walk_block(body, f);
        }
        Stmt::While { cond, body, .. } => {
            walk(cond, f);
            walk_block(body, f);
        }
        Stmt::Emit { args, .. } => {
            for a in args {
                walk(a, f);
            }
        }
        _ => {}
    }
}
