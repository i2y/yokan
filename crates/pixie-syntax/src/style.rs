//! Style env + element splice — cute's `cute_codegen::style` pass,
//! hoisted into the syntax crate so BOTH execution tiers run the one
//! implementation: the driver splices before check/codegen (compiled
//! tier) and `pixie_interp::reload_from_source` splices before
//! `extract_view` (rung-2 tier). Styles therefore hot-reload like
//! view bodies — they are excluded from the AOT fingerprint.
//!
//! `style X { padding: 16 ... }` is a top-level item that produces no
//! runtime artifact — it lives only to be merged into element bodies.
//! This module owns the resolution: (1) build a module-scoped table
//! from every `Item::Style`, flattening aliases (`style BigCard =
//! Card + Big`) with right-wins merge semantics; (2) walk every
//! `view` element tree replacing `style: <expr>` members with the
//! resolved flat (key, value) property list. After the pass, the rest
//! of the pipeline sees only ordinary properties.
//!
//! Divergence from cute: an unresolvable `style:` value is an error
//! here (cute fell through to QML, which might accept a literal
//! `style` property; pixie's elements have none, so falling through
//! could only defer the failure with a worse message). Styles are
//! module-local — a cross-module reference reports unknown.

use crate::ast::{
    BinOp, Block, CaseArm, Element, ElementMember, Expr, ExprKind, Item, Module, Stmt, StyleBody,
    StyleDecl, StyleEntry, ViewDecl,
};
use crate::span::Span;
use std::collections::{HashMap, HashSet};

/// A splice failure: unknown style name, alias cycle, or an
/// expression shape `style:` can't mean (anything but names and `+`).
#[derive(Debug, Clone)]
pub struct StyleError {
    pub span: Span,
    pub message: String,
}

/// Resolved per-module style table. Each entry name maps to its
/// flattened (key, value) list — alias chains and `+` merges are
/// already collapsed.
pub struct StyleEnv {
    resolved: HashMap<String, Vec<(String, Expr)>>,
}

impl StyleEnv {
    /// Build the env from every `Item::Style` in a module, plus
    /// `foreign` styles contributed by `use`d modules (their `pub`
    /// styles). A local declaration shadows a foreign one silently;
    /// two foreign styles sharing a name is an ambiguity error, and
    /// duplicate locals stay an error.
    pub fn build(module: &Module, foreign: &[&StyleDecl]) -> Result<Self, StyleError> {
        let mut raw: HashMap<String, &StyleDecl> = HashMap::new();
        for s in foreign {
            if raw.insert(s.name.name.clone(), s).is_some() {
                return Err(StyleError {
                    span: s.name.span,
                    message: format!(
                        "style `{}` is `pub` in more than one used module",
                        s.name.name
                    ),
                });
            }
        }
        let mut local_seen: HashSet<String> = HashSet::new();
        for item in &module.items {
            if let Item::Style(s) = item {
                if !local_seen.insert(s.name.name.clone()) {
                    return Err(StyleError {
                        span: s.name.span,
                        message: format!("duplicate style `{}`", s.name.name),
                    });
                }
                raw.insert(s.name.name.clone(), s);
            }
        }
        let mut resolved: HashMap<String, Vec<(String, Expr)>> = HashMap::new();
        for name in raw.keys().cloned().collect::<Vec<_>>() {
            if !resolved.contains_key(&name) {
                let mut visiting: HashSet<String> = HashSet::new();
                let span = raw[&name].span;
                resolve_named_style(&name, span, &raw, &mut resolved, &mut visiting)?;
            }
        }
        Ok(StyleEnv { resolved })
    }

    /// Reduce a `style: <expr>` value to a flat (key, value) list.
    /// Idents look up by name; `+` recursively merges right-wins.
    fn resolve_expr(&self, e: &Expr) -> Result<Vec<(String, Expr)>, StyleError> {
        match &e.kind {
            ExprKind::Ident(name) => self.resolved.get(name).cloned().ok_or_else(|| StyleError {
                span: e.span,
                message: format!("unknown style `{name}`"),
            }),
            ExprKind::Binary {
                op: BinOp::Add,
                lhs,
                rhs,
            } => {
                let l = self.resolve_expr(lhs)?;
                let r = self.resolve_expr(rhs)?;
                Ok(merge_entries(l, r))
            }
            _ => Err(StyleError {
                span: e.span,
                message: "`style:` takes a style name or a `+` merge of style names".to_string(),
            }),
        }
    }
}

/// Right-wins merge. Keys present in `rhs` override keys in `lhs`; the
/// resulting order keeps lhs entries (minus overridden ones) followed
/// by rhs entries in their original order.
fn merge_entries(lhs: Vec<(String, Expr)>, rhs: Vec<(String, Expr)>) -> Vec<(String, Expr)> {
    let rhs_keys: HashSet<&str> = rhs.iter().map(|(k, _)| k.as_str()).collect();
    let mut out: Vec<(String, Expr)> = lhs
        .into_iter()
        .filter(|(k, _)| !rhs_keys.contains(k.as_str()))
        .collect();
    out.extend(rhs);
    out
}

fn resolve_named_style(
    name: &str,
    at: Span,
    raw: &HashMap<String, &StyleDecl>,
    resolved: &mut HashMap<String, Vec<(String, Expr)>>,
    visiting: &mut HashSet<String>,
) -> Result<Vec<(String, Expr)>, StyleError> {
    if let Some(r) = resolved.get(name) {
        return Ok(r.clone());
    }
    if !visiting.insert(name.to_string()) {
        return Err(StyleError {
            span: at,
            message: format!("style alias cycle through `{name}`"),
        });
    }
    let entries = match raw.get(name) {
        Some(decl) => match &decl.body {
            StyleBody::Lit(es) => entries_from_literal(es),
            StyleBody::Alias(rhs) => resolve_style_expr(rhs, raw, resolved, visiting)?,
        },
        None => {
            return Err(StyleError {
                span: at,
                message: format!("unknown style `{name}`"),
            });
        }
    };
    visiting.remove(name);
    resolved.insert(name.to_string(), entries.clone());
    Ok(entries)
}

fn resolve_style_expr(
    e: &Expr,
    raw: &HashMap<String, &StyleDecl>,
    resolved: &mut HashMap<String, Vec<(String, Expr)>>,
    visiting: &mut HashSet<String>,
) -> Result<Vec<(String, Expr)>, StyleError> {
    match &e.kind {
        ExprKind::Ident(name) => resolve_named_style(name, e.span, raw, resolved, visiting),
        ExprKind::Binary {
            op: BinOp::Add,
            lhs,
            rhs,
        } => {
            let l = resolve_style_expr(lhs, raw, resolved, visiting)?;
            let r = resolve_style_expr(rhs, raw, resolved, visiting)?;
            Ok(merge_entries(l, r))
        }
        _ => Err(StyleError {
            span: e.span,
            message: "a style alias is a style name or a `+` merge of style names".to_string(),
        }),
    }
}

fn entries_from_literal(entries: &[StyleEntry]) -> Vec<(String, Expr)> {
    entries
        .iter()
        .map(|e| (e.key.clone(), e.value.clone()))
        .collect()
}

/// Rewrite a module so every `style: <expr>` element member is
/// replaced by the inline (key, value) properties it resolves to.
/// Later members win in pixie's per-element property lookup only by
/// being FOUND first or last — the splice preserves cute's contract
/// by writing entries at the `style:` member's position, so an
/// explicit prop written after `style:` overrides the style's value
/// exactly when the element lookup takes the later occurrence.
pub fn desugar_module(module: &Module) -> Result<Module, StyleError> {
    desugar_module_with(module, &[])
}

/// `desugar_module` with `pub` styles from `use`d modules in scope —
/// the driver's cross-module leg. The rung-2 reload path reaches the
/// same env through the foreign-styles source snippet the emitter
/// bakes into the binary.
pub fn desugar_module_with(module: &Module, foreign: &[&StyleDecl]) -> Result<Module, StyleError> {
    let env = StyleEnv::build(module, foreign)?;
    let mut new_items = Vec::with_capacity(module.items.len());
    for item in &module.items {
        match item {
            Item::View(v) => new_items.push(Item::View(ViewDecl {
                root: desugar_element(&v.root, &env)?,
                ..v.clone()
            })),
            other => new_items.push(other.clone()),
        }
    }
    Ok(Module {
        items: new_items,
        span: module.span,
    })
}

fn desugar_element(e: &Element, env: &StyleEnv) -> Result<Element, StyleError> {
    let mut new_members: Vec<ElementMember> = Vec::with_capacity(e.members.len());
    for m in &e.members {
        match m {
            ElementMember::Property { key, value, span } if key == "style" => {
                for (k, v) in env.resolve_expr(value)? {
                    new_members.push(ElementMember::Property {
                        key: k,
                        value: v,
                        span: *span,
                    });
                }
            }
            ElementMember::Property { key, value, span } => {
                // The value may itself hold a nested element; recurse
                // so inner `style:` references get spliced too.
                new_members.push(ElementMember::Property {
                    key: key.clone(),
                    value: desugar_expr(value, env)?,
                    span: *span,
                });
            }
            ElementMember::Child(c) => {
                new_members.push(ElementMember::Child(desugar_element(c, env)?));
            }
            ElementMember::Stmt(s) => {
                new_members.push(ElementMember::Stmt(desugar_stmt(s, env)?));
            }
        }
    }
    Ok(Element {
        module_path: e.module_path.clone(),
        name: e.name.clone(),
        members: new_members,
        span: e.span,
    })
}

fn desugar_stmt(s: &Stmt, env: &StyleEnv) -> Result<Stmt, StyleError> {
    Ok(match s {
        Stmt::Expr(e) => Stmt::Expr(desugar_expr(e, env)?),
        Stmt::For {
            binding,
            iter,
            body,
            span,
        } => Stmt::For {
            binding: binding.clone(),
            iter: iter.clone(),
            body: desugar_block(body, env)?,
            span: *span,
        },
        // Other statement forms cannot legally hold an
        // element-position body in current pixie, so nothing inside
        // them needs rewriting.
        other => other.clone(),
    })
}

fn desugar_expr(e: &Expr, env: &StyleEnv) -> Result<Expr, StyleError> {
    Ok(match &e.kind {
        ExprKind::Element(el) => Expr {
            kind: ExprKind::Element(desugar_element(el, env)?),
            span: e.span,
        },
        ExprKind::If {
            cond,
            then_b,
            else_b,
            let_binding,
        } => Expr {
            kind: ExprKind::If {
                cond: cond.clone(),
                then_b: desugar_block(then_b, env)?,
                else_b: match else_b {
                    Some(b) => Some(desugar_block(b, env)?),
                    None => None,
                },
                let_binding: let_binding.clone(),
            },
            span: e.span,
        },
        ExprKind::Case { scrutinee, arms } => {
            let mut new_arms = Vec::with_capacity(arms.len());
            for arm in arms {
                new_arms.push(CaseArm {
                    pattern: arm.pattern.clone(),
                    body: desugar_block(&arm.body, env)?,
                    span: arm.span,
                });
            }
            Expr {
                kind: ExprKind::Case {
                    scrutinee: scrutinee.clone(),
                    arms: new_arms,
                },
                span: e.span,
            }
        }
        _ => e.clone(),
    })
}

fn desugar_block(b: &Block, env: &StyleEnv) -> Result<Block, StyleError> {
    let mut stmts = Vec::with_capacity(b.stmts.len());
    for s in &b.stmts {
        stmts.push(desugar_stmt(s, env)?);
    }
    Ok(Block {
        stmts,
        trailing: match &b.trailing {
            Some(e) => Some(Box::new(desugar_expr(e, env)?)),
            None => None,
        },
        span: b.span,
    })
}
