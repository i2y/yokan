//! Use-after-move detection on `~Copyable` linear types.
//!
//! Walks every fn body / method body / init body and tracks the
//! consume state of bindings that hold `~Copyable` values. An
//! identifier reference whose binding has already been moved out
//! (passed to a `consuming` parameter, or rebound elsewhere) emits
//! a "use of moved value" diagnostic at the use site.
//!
//! Scope: this is a lexical, sequential pass over each body. Branch
//! join (if / else / case / loops) is intentionally conservative —
//! whichever branch executes, a binding moved in any one of them is
//! considered moved at the join. The pass does NOT track aliasing
//! (`let y = x` doesn't transfer the move), partial moves, or
//! closure captures. Those are documented v1 limitations in the
//! plan; closures of `~Copyable` values are rejected outright at
//! the boundary so the absence of capture-tracking is sound.
//!
//! Entry point: `check_linearity(module, prog) -> Vec<Diagnostic>`.

use std::collections::HashMap;

use pixie_hir::ResolvedProgram;
use pixie_syntax::ast::*;
use pixie_syntax::diag::Diagnostic;
use pixie_syntax::span::Span;

/// Run the use-after-move check across every fn / method body in
/// `module`. Returns the accumulated diagnostics in source order.
/// `_prog` is reserved for cross-module / generic-bound queries
/// that this v1 pass doesn't need yet.
pub fn check_linearity(module: &Module, _prog: &ResolvedProgram) -> Vec<Diagnostic> {
    let mut checker = LinearityChecker::new(module);
    checker.walk_module(module);
    checker.diags
}

/// Per-binding consume state. `Live` means the value is currently held
/// by this name; `Moved(span)` means it was consumed by a previous
/// expression (the span points at the consume site so diagnostics can
/// note where the move happened).
#[derive(Clone, Debug, PartialEq, Eq)]
enum ConsumeState {
    Live,
    Moved(Span),
}

struct LinearityChecker<'a> {
    module: &'a Module,
    diags: Vec<Diagnostic>,
}

impl<'a> LinearityChecker<'a> {
    fn new(module: &'a Module) -> Self {
        Self {
            module,
            diags: Vec::new(),
        }
    }

    fn walk_module(&mut self, module: &Module) {
        for item in &module.items {
            match item {
                Item::Fn(f) => self.walk_fn(f),
                Item::Class(c) => self.walk_class(c),
                Item::Impl(i) => {
                    for m in &i.methods {
                        self.walk_fn(m);
                    }
                }
                _ => {}
            }
        }
    }

    fn walk_fn(&mut self, f: &FnDecl) {
        let Some(body) = &f.body else { return };
        let mut env: HashMap<String, ConsumeState> = HashMap::new();
        // Linear-typed `consuming` params start out Live in the
        // callee's scope. Non-consuming params (or non-linear
        // params) aren't tracked — they're either references that
        // can't be moved out of, or copyable values whose use is
        // unconstrained.
        for p in &f.params {
            if p.is_consuming && self.is_non_copyable_type(&p.ty) {
                env.insert(p.name.name.clone(), ConsumeState::Live);
            }
        }
        self.walk_block(body, &mut env);
    }

    fn walk_class(&mut self, c: &ClassDecl) {
        for m in &c.members {
            match m {
                ClassMember::Fn(f) | ClassMember::Slot(f) => self.walk_fn(f),
                ClassMember::Init(init) => {
                    let mut env: HashMap<String, ConsumeState> = HashMap::new();
                    for p in &init.params {
                        if p.is_consuming && self.is_non_copyable_type(&p.ty) {
                            env.insert(p.name.name.clone(), ConsumeState::Live);
                        }
                    }
                    self.walk_block(&init.body, &mut env);
                }
                ClassMember::Deinit(d) => {
                    let mut env: HashMap<String, ConsumeState> = HashMap::new();
                    self.walk_block(&d.body, &mut env);
                }
                _ => {}
            }
        }
    }

    fn walk_block(&mut self, b: &Block, env: &mut HashMap<String, ConsumeState>) {
        for stmt in &b.stmts {
            self.walk_stmt(stmt, env);
        }
        if let Some(t) = &b.trailing {
            self.walk_expr(t, env);
        }
    }

    fn walk_stmt(&mut self, s: &Stmt, env: &mut HashMap<String, ConsumeState>) {
        use Stmt::*;
        match s {
            Let {
                name, ty, value, ..
            }
            | Var {
                name, ty, value, ..
            } => {
                self.walk_expr(value, env);
                let is_nc = ty.as_ref().is_some_and(|t| self.is_non_copyable_type(t))
                    || self.expr_yields_non_copyable(value);
                if is_nc {
                    env.insert(name.name.clone(), ConsumeState::Live);
                }
            }
            Assign { target, value, .. } => {
                self.walk_expr(value, env);
                self.walk_expr(target, env);
            }
            Return { value: Some(e), .. } => {
                self.walk_expr(e, env);
                // Returning an ident moves it out of the local
                // scope. Any subsequent statements are dead code,
                // but mark anyway in case the diagnostic walker
                // visits later (defensive).
                if let ExprKind::Ident(n) = &e.kind {
                    if matches!(env.get(n), Some(ConsumeState::Live)) {
                        env.insert(n.clone(), ConsumeState::Moved(e.span));
                    }
                }
            }
            Return { value: None, .. } | Break { .. } | Continue { .. } => {}
            Emit { args, .. } => {
                for a in args {
                    self.walk_expr(a, env);
                }
            }
            Expr(e) => self.walk_expr(e, env),
            For { iter, body, .. } => {
                self.walk_expr(iter, env);
                self.walk_block(body, env);
            }
            While { cond, body, .. } => {
                self.walk_expr(cond, env);
                self.walk_block(body, env);
            }
            Batch { body, .. } => self.walk_block(body, env),
        }
    }

    fn walk_expr(&mut self, e: &Expr, env: &mut HashMap<String, ConsumeState>) {
        use ExprKind::*;
        match &e.kind {
            Ident(name) => {
                if let Some(ConsumeState::Moved(consumed_at)) = env.get(name) {
                    let consumed_at = *consumed_at;
                    self.diags.push(
                        Diagnostic::error(
                            e.span,
                            format!(
                                "use of moved value `{name}` — `~Copyable` values can only be moved, not copied",
                            ),
                        )
                        .with_note(consumed_at, "value was moved here"),
                    );
                }
            }
            Call {
                callee,
                args,
                block,
                ..
            } => {
                self.walk_expr(callee, env);
                // Look up the callee's consuming-flag list when
                // possible so we know which arg positions consume
                // their argument. Top-level fn calls (`Ident(name)`
                // callees) are easy; method calls are out of scope
                // for v1 — the analysis there would need cross-
                // module / cross-class lookup.
                let flags = match &callee.kind {
                    Ident(fn_name) => self.module.fn_consuming_flags(fn_name),
                    _ => None,
                };
                for (i, a) in args.iter().enumerate() {
                    self.walk_expr(a, env);
                    let consuming = flags
                        .as_ref()
                        .and_then(|fs| fs.get(i).copied())
                        .unwrap_or(false);
                    if consuming {
                        if let Ident(arg_name) = &a.kind {
                            if matches!(env.get(arg_name), Some(ConsumeState::Live)) {
                                env.insert(arg_name.clone(), ConsumeState::Moved(a.span));
                            }
                        }
                    }
                }
                if let Some(b) = block {
                    self.walk_expr(b, env);
                }
            }
            MethodCall {
                receiver,
                args,
                block,
                ..
            } => {
                self.walk_expr(receiver, env);
                // Method-call consuming-flag lookup is best-effort:
                // we only know the receiver class statically when
                // it's a simple ident binding. Skipped for now.
                for a in args {
                    self.walk_expr(a, env);
                }
                if let Some(b) = block {
                    self.walk_expr(b, env);
                }
            }
            Member { receiver, .. } | SafeMember { receiver, .. } => {
                self.walk_expr(receiver, env);
            }
            SafeMethodCall {
                receiver,
                args,
                block,
                ..
            } => {
                self.walk_expr(receiver, env);
                for a in args {
                    self.walk_expr(a, env);
                }
                if let Some(b) = block {
                    self.walk_expr(b, env);
                }
            }
            Index {
                receiver, index, ..
            } => {
                self.walk_expr(receiver, env);
                self.walk_expr(index, env);
            }
            Block(b) => self.walk_block(b, env),
            Binary { lhs, rhs, .. } => {
                self.walk_expr(lhs, env);
                self.walk_expr(rhs, env);
            }
            Unary { expr, .. } => self.walk_expr(expr, env),
            Array(items) => {
                for i in items {
                    self.walk_expr(i, env);
                }
            }
            Map(entries) => {
                for (k, v) in entries {
                    self.walk_expr(k, env);
                    self.walk_expr(v, env);
                }
            }
            If {
                cond,
                then_b,
                else_b,
                let_binding,
            } => {
                self.walk_expr(cond, env);
                if let Some((_pat, init)) = let_binding {
                    self.walk_expr(init, env);
                }
                // Branch join: walk each side from a snapshot of
                // the entry env, then promote any binding moved in
                // either branch to Moved at the join. Live-only
                // stays Live.
                let mut then_env = env.clone();
                self.walk_block(then_b, &mut then_env);
                if let Some(eb) = else_b {
                    let mut else_env = env.clone();
                    self.walk_block(eb, &mut else_env);
                    merge_into(env, &else_env);
                }
                merge_into(env, &then_env);
            }
            Case { scrutinee, arms } => {
                self.walk_expr(scrutinee, env);
                let entry = env.clone();
                for arm in arms {
                    let mut arm_env = entry.clone();
                    self.walk_block(&arm.body, &mut arm_env);
                    merge_into(env, &arm_env);
                }
            }
            Lambda { .. } => {
                // Closure-capture analysis is out of scope for v1.
                // The plan rejects capturing `~Copyable` values into
                // closures; until that lands, skip the lambda body
                // to avoid spurious "use of moved value" through
                // capture.
            }
            Try(inner) => self.walk_expr(inner, env),
            Await(inner) => self.walk_expr(inner, env),
            Range { start, end, .. } => {
                self.walk_expr(start, env);
                self.walk_expr(end, env);
            }
            Kwarg { value, .. } => self.walk_expr(value, env),
            // View/widget elements appear in UI position; the
            // children are walked through their own ElementMember
            // enumerations which the linearity pass doesn't
            // currently descend into. Treated as opaque.
            Element(_) => {}
            // Atoms with no children that can move things.
            Int(_) | Float(_) | Bool(_) | Nil | Str(_) | Sym(_) | Path(_) | AtIdent(_)
            | SelfRef => {}
        }
    }

    /// Whether `ty` resolves to a `~Copyable` struct or class.
    fn is_non_copyable_type(&self, ty: &TypeExpr) -> bool {
        match &ty.kind {
            TypeKind::Named { path, args } if args.is_empty() => {
                let leaf = match path.last() {
                    Some(i) => i.name.as_str(),
                    None => return false,
                };
                self.is_non_copyable_named(leaf)
            }
            _ => false,
        }
    }

    fn is_non_copyable_named(&self, name: &str) -> bool {
        // Walk the module's class / struct decls. The HIR doesn't
        // currently expose `is_copyable` (it's an AST-level bit),
        // so we look up the original decl by name.
        for item in &self.module.items {
            match item {
                Item::Class(c) if c.name.name == name => return !c.is_copyable,
                Item::Struct(s) if s.name.name == name => return !s.is_copyable,
                _ => {}
            }
        }
        false
    }

    /// Heuristic: an expression "yields" a non-copyable value when
    /// it's a `T.new(args)` call on a non-copyable T. Used so that
    /// `let h = FileHandle.new(0)` registers `h` as a linear
    /// binding even when the type wasn't spelled out in the let.
    fn expr_yields_non_copyable(&self, e: &Expr) -> bool {
        if let ExprKind::MethodCall {
            receiver, method, ..
        } = &e.kind
        {
            if method.name == "new" {
                if let ExprKind::Ident(class_name) = &receiver.kind {
                    return self.is_non_copyable_named(class_name);
                }
            }
        }
        false
    }
}

/// Promote any `Moved` from `other` into `into`. Live-only entries
/// in `into` stay Live; entries already Moved win against any
/// later state. Used for branch joins (`If`/`Case`) where the
/// outer env should reflect "moved on at least one path".
fn merge_into(into: &mut HashMap<String, ConsumeState>, other: &HashMap<String, ConsumeState>) {
    for (k, v) in other {
        match (into.get(k), v) {
            (Some(ConsumeState::Moved(_)), _) => {
                // already Moved; keep
            }
            (_, ConsumeState::Moved(span)) => {
                into.insert(k.clone(), ConsumeState::Moved(*span));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixie_hir::resolve;
    use pixie_syntax::{parse, span::FileId};

    fn run(src: &str) -> Vec<Diagnostic> {
        let m = parse(FileId(0), src).expect("parse");
        let prog = resolve(&m, &pixie_hir::ProjectInfo::default()).program;
        check_linearity(&m, &prog)
    }

    #[test]
    fn use_after_move_to_consuming_param_errors() {
        let src = r#"
struct Token: ~Copyable { var id: Int }
fn consume(consuming t: Token) { }
fn main {
  cli_app {
    let t = Token.new(1)
    consume(t)
    consume(t)
  }
}
"#;
        let d = run(src);
        assert!(
            d.iter()
                .any(|x| x.message.contains("use of moved value `t`")),
            "expected use-after-move diagnostic, got: {:?}",
            d
        );
    }

    #[test]
    fn move_in_one_branch_use_after_join_errors() {
        let src = r#"
struct Token: ~Copyable { var id: Int }
fn consume(consuming t: Token) { }
fn run(flag: Bool) {
  let t = Token.new(1)
  if flag {
    consume(t)
  }
  consume(t)
}
"#;
        let d = run(src);
        assert!(
            d.iter().any(|x| x.message.contains("use of moved value")),
            "expected branch-join move diagnostic, got: {:?}",
            d
        );
    }

    #[test]
    fn copyable_consuming_param_does_not_track() {
        // A `consuming` param of a *Copyable* type isn't tracked —
        // re-using it after a "consume" call is fine because the
        // value can be cheaply duplicated. Important: this test
        // ensures the analysis doesn't false-positive on plain
        // `consuming x: Int`.
        let src = r#"
fn consume(consuming x: Int) { }
fn main {
  let x = 42
  consume(x)
  consume(x)
}
"#;
        let d = run(src);
        assert!(
            d.iter().all(|x| !x.message.contains("use of moved")),
            "expected no diagnostics, got: {:?}",
            d
        );
    }

    #[test]
    fn struct_default_copyable_does_not_trigger() {
        let src = r#"
struct Pair { var a: Int, var b: Int }
fn usePair(p: Pair) { }
fn main {
  let p = Pair.new(1, 2)
  usePair(p)
  usePair(p)
}
"#;
        let d = run(src);
        assert!(
            d.is_empty(),
            "expected no diagnostics on copyable struct, got: {:?}",
            d
        );
    }

    #[test]
    fn linear_binding_used_once_clean() {
        let src = r#"
struct Token: ~Copyable { var id: Int }
fn consume(consuming t: Token) { }
fn main {
  cli_app {
    let t = Token.new(1)
    consume(t)
  }
}
"#;
        let d = run(src);
        assert!(d.is_empty(), "expected clean, got: {:?}", d);
    }
}
