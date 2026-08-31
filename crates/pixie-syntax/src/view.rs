//! What a view body contributes, flattened into one sequence.
//!
//! An element's members and a block's statements say the same three
//! things in two different AST shapes: here is a child, here is a
//! repeater, here is a conditional. Both execution tiers walk the
//! result of this module, which is what stops them from disagreeing
//! about what a `for` body may hold — the tier gate can only catch a
//! divergence that produces different output, and "one tier accepts
//! this program and the other does not" produces none.

use crate::ast::{Block, Element, ElementMember, Expr, ExprKind, Ident, Stmt};
use crate::span::Span;

/// One contribution to a parent's child list.
pub enum ViewItem<'a> {
    /// A child element.
    Child(&'a Element),
    /// `for <binding> in <iter> { .. }`.
    Repeat {
        binding: &'a Ident,
        iter: &'a Expr,
        body: &'a Block,
        span: Span,
    },
    /// The whole `if` expression — the caller destructures it, since
    /// the two tiers want different halves of it.
    Cond(&'a Expr),
    /// The whole `case` expression (§8.69). `if let` desugars into
    /// one, so a view gets both from the same lowering.
    Match(&'a Expr),
    /// Anything else. The caller reports it; this module does not
    /// know which tier's wording to use.
    Other(Span),
}

impl ViewItem<'_> {
    /// Where to point a diagnostic about this item.
    pub fn span(&self) -> Span {
        match self {
            ViewItem::Child(el) => el.span,
            ViewItem::Repeat { span, .. } => *span,
            ViewItem::Cond(e) => e.span,
            ViewItem::Match(e) => e.span,
            ViewItem::Other(s) => *s,
        }
    }
}

/// The items an element's members contribute, in source order.
/// Properties are left out: they belong to the element's own
/// lowering, and each tier validates them against its own allowlist.
pub fn items_of_members(members: &[ElementMember]) -> Vec<ViewItem<'_>> {
    let mut out = Vec::new();
    for m in members {
        match m {
            ElementMember::Property { .. } => {}
            ElementMember::Child(c) => out.push(ViewItem::Child(c)),
            ElementMember::Stmt(s) => out.push(item_of_stmt(s)),
        }
    }
    out
}

/// The items a `for` body or an `if` branch contributes, in source
/// order. A block's trailing expression is its last item — which is
/// why a single-element body and a several-element one are the same
/// shape here, and why neither needs a rule of its own.
pub fn items_of_block(b: &Block) -> Vec<ViewItem<'_>> {
    let mut out: Vec<ViewItem<'_>> = b.stmts.iter().map(item_of_stmt).collect();
    if let Some(t) = &b.trailing {
        out.push(item_of_expr(t));
    }
    out
}

fn item_of_stmt(s: &Stmt) -> ViewItem<'_> {
    match s {
        Stmt::For {
            binding,
            iter,
            body,
            span,
        } => ViewItem::Repeat {
            binding,
            iter,
            body,
            span: *span,
        },
        Stmt::Expr(e) => item_of_expr(e),
        Stmt::Let { span, .. } | Stmt::Var { span, .. } => ViewItem::Other(*span),
        _ => ViewItem::Other(stmt_span(s)),
    }
}

fn item_of_expr(e: &Expr) -> ViewItem<'_> {
    match &e.kind {
        ExprKind::Element(el) => ViewItem::Child(el),
        ExprKind::If { .. } => ViewItem::Cond(e),
        ExprKind::Case { .. } => ViewItem::Match(e),
        _ => ViewItem::Other(e.span),
    }
}

fn stmt_span(s: &Stmt) -> Span {
    match s {
        Stmt::Let { span, .. }
        | Stmt::Var { span, .. }
        | Stmt::Assign { span, .. }
        | Stmt::Return { span, .. }
        | Stmt::Emit { span, .. }
        | Stmt::For { span, .. }
        | Stmt::While { span, .. }
        | Stmt::Break { span, .. }
        | Stmt::Continue { span, .. }
        | Stmt::Batch { span, .. } => *span,
        Stmt::Expr(e) => e.span,
    }
}
