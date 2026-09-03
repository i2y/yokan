//! Hand-written recursive-descent parser for pixie.
//!
//! Designed to accept the spec's three sample programs (TodoItem class,
//! list_view trailing-block call, error union with `?` propagation +
//! `case ... when ok/err`). Error recovery is panic-mode: on a hard
//! parse error we resync to the next `}` or item-keyword.

use crate::ast::*;
use crate::lex::{LexError, lex};
use crate::span::{FileId, Span};
use crate::token::{StrSeg, Token, TokenKind};

#[derive(Debug)]
pub struct ParseError {
    pub span: Span,
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ParseError {}

impl From<LexError> for ParseError {
    fn from(e: LexError) -> Self {
        ParseError {
            span: e.span,
            message: e.message,
        }
    }
}

pub fn parse(file: FileId, src: &str) -> Result<Module, ParseError> {
    let tokens = lex(file, src)?;
    let mut p = Parser {
        tokens,
        pos: 0,
        src,
        file,
        disable_trailing_block: false,
        binding_mode: false,
        next_block_id: 0,
    };
    let mut module = p.module()?;
    apply_class_member_rewrite(&mut module);
    Ok(module)
}

/// Lift `ModelList<T>` to `(List<T>, true)`; anything else passes
/// through as `(ty, false)`. Downstream sees the stored `List<T>`
/// shape with a flag asking codegen to wrap it in
/// `cute::ModelList<T*>*` (a public-derived `QRangeModel`).
fn unwrap_model_list_surface(ty: TypeExpr) -> (TypeExpr, bool) {
    let TypeKind::Named { path, args } = &ty.kind else {
        return (ty, false);
    };
    if path.len() != 1 || path[0].name != "ModelList" || args.len() != 1 {
        return (ty, false);
    }
    let inner = args[0].clone();
    let list_span = ty.span;
    let list_path = vec![Ident {
        name: "List".to_string(),
        span: path[0].span,
    }];
    let lifted = TypeExpr {
        kind: TypeKind::Named {
            path: list_path,
            args: vec![inner],
        },
        span: list_span,
    };
    (lifted, true)
}

/// Rewrite bare `K::Ident(name)` to `K::AtIdent(name)` inside
/// class / arc / struct method bodies when `name` matches a property
/// or field of the enclosing type. Downstream consumers (codegen,
/// type-check, HIR, LSP) see the same `K::AtIdent` shape they did
/// when users wrote `@x` directly, so the visibility / bindable /
/// weak / model / nullable dispatch is unchanged.
///
/// Method names are NOT rewritten — `self.method()` and the bare
/// `method()` self-call form are the access paths there. Only "data"
/// members (props + plain fields) participate.
///
/// Lexical-scope shadowing is not detected: a `let X = 0` inside a
/// method whose enclosing class declares `prop X` still rewrites
/// later `X` reads to `m_X`. Reach a shadowed member via
/// `self.X()`. Cute's case rule (PascalCase = member, lowercase =
/// local) keeps the collision rare in practice.
fn apply_class_member_rewrite(module: &mut Module) {
    for item in &mut module.items {
        match item {
            Item::Class(c) => {
                let members = collect_class_member_names(&c.members);
                if members.is_empty() {
                    continue;
                }
                for m in &mut c.members {
                    rewrite_class_member_body(m, &members);
                }
            }
            Item::Struct(s) => {
                let members: std::collections::HashSet<String> =
                    s.fields.iter().map(|f| f.name.name.clone()).collect();
                if members.is_empty() {
                    continue;
                }
                for fn_decl in &mut s.methods {
                    if let Some(body) = &mut fn_decl.body {
                        rewrite_block(body, &members);
                    }
                }
            }
            Item::Store(s) => {
                // Same rewrite as Item::Class, plus the state-field
                // names — those desugar to `prop X` and method bodies
                // reference them as bare idents.
                let mut members = collect_class_member_names(&s.members);
                for sf in &s.state_fields {
                    if matches!(sf.kind, StateFieldKind::Property { .. }) {
                        members.insert(sf.name.name.clone());
                    }
                }
                if members.is_empty() {
                    continue;
                }
                for m in &mut s.members {
                    rewrite_class_member_body(m, &members);
                }
            }
            // impl bodies access the for-type via `self.X()`; resolving
            // its member set would require cross-module info we don't
            // have at parse time.
            Item::Impl(_) => {}
            _ => {}
        }
    }
}

/// Build the active member set for a fn body, removing names that
/// are shadowed by the fn's own parameters. Without this mask,
/// `init(Label: String) { Label = Label }` would rewrite both sides
/// to `m_Label` and silently lose the `Label = param` assignment.
///
/// Returns a `Cow` so the common case (no param shadows a member —
/// Cute's case rule keeps params lowercase and members PascalCase)
/// borrows the existing set without allocating.
fn mask_params<'a>(
    members: &'a std::collections::HashSet<String>,
    params: &[Param],
) -> std::borrow::Cow<'a, std::collections::HashSet<String>> {
    if !params.iter().any(|p| members.contains(&p.name.name)) {
        return std::borrow::Cow::Borrowed(members);
    }
    let mut out = members.clone();
    for p in params {
        out.remove(&p.name.name);
    }
    std::borrow::Cow::Owned(out)
}

fn collect_class_member_names(members: &[ClassMember]) -> std::collections::HashSet<String> {
    members
        .iter()
        .filter_map(|m| match m {
            ClassMember::Property(p) => Some(p.name.name.clone()),
            ClassMember::Field(f) => Some(f.name.name.clone()),
            // Signals are reached via `emit Foo` syntax, not bare ident.
            // Methods are reached via `self.method()` or bare `method()`.
            ClassMember::Signal(_)
            | ClassMember::Fn(_)
            | ClassMember::Slot(_)
            | ClassMember::Init(_)
            | ClassMember::Deinit(_) => None,
        })
        .collect()
}

fn rewrite_class_member_body(m: &mut ClassMember, members: &std::collections::HashSet<String>) {
    match m {
        ClassMember::Fn(f) | ClassMember::Slot(f) => {
            if let Some(body) = &mut f.body {
                let effective = mask_params(members, &f.params);
                rewrite_block(body, &effective);
            }
        }
        ClassMember::Init(i) => {
            let effective = mask_params(members, &i.params);
            rewrite_block(&mut i.body, &effective);
        }
        ClassMember::Deinit(d) => rewrite_block(&mut d.body, members),
        // Initializer expressions on props / fields evaluate in the
        // class context too: `default: Y + 1` should rewrite `Y` if
        // Y is a sibling member.
        ClassMember::Property(p) => {
            if let Some(init) = &mut p.default {
                rewrite_expr(init, members);
            }
            if let Some(init) = &mut p.binding {
                rewrite_expr(init, members);
            }
            if let Some(init) = &mut p.fresh {
                rewrite_expr(init, members);
            }
        }
        ClassMember::Field(f) => {
            if let Some(init) = &mut f.default {
                rewrite_expr(init, members);
            }
        }
        ClassMember::Signal(_) => {}
    }
}

fn rewrite_block(b: &mut Block, members: &std::collections::HashSet<String>) {
    for s in &mut b.stmts {
        rewrite_stmt(s, members);
    }
    if let Some(t) = &mut b.trailing {
        rewrite_expr(t, members);
    }
}

fn rewrite_stmt(s: &mut Stmt, members: &std::collections::HashSet<String>) {
    match s {
        Stmt::Let { value, .. } | Stmt::Var { value, .. } => {
            rewrite_expr(value, members);
        }
        Stmt::Expr(e) => rewrite_expr(e, members),
        Stmt::Return { value, .. } => {
            if let Some(v) = value {
                rewrite_expr(v, members);
            }
        }
        Stmt::Emit { args, .. } => {
            for a in args {
                rewrite_expr(a, members);
            }
        }
        Stmt::Assign { target, value, .. } => {
            rewrite_expr(target, members);
            rewrite_expr(value, members);
        }
        Stmt::For { iter, body, .. } => {
            rewrite_expr(iter, members);
            rewrite_block(body, members);
        }
        Stmt::While { cond, body, .. } => {
            rewrite_expr(cond, members);
            rewrite_block(body, members);
        }
        Stmt::Break { .. } | Stmt::Continue { .. } => {}
        Stmt::Batch { body, .. } => rewrite_block(body, members),
    }
}

fn rewrite_expr(e: &mut Expr, members: &std::collections::HashSet<String>) {
    match &mut e.kind {
        ExprKind::Ident(name) => {
            if members.contains(name) {
                e.kind = ExprKind::AtIdent(std::mem::take(name));
            }
        }
        // .field on a receiver is not a member of the enclosing
        // class — only the receiver gets walked.
        ExprKind::Member { receiver, .. } | ExprKind::SafeMember { receiver, .. } => {
            rewrite_expr(receiver, members);
        }
        ExprKind::MethodCall {
            receiver,
            args,
            block,
            ..
        }
        | ExprKind::SafeMethodCall {
            receiver,
            args,
            block,
            ..
        } => {
            rewrite_expr(receiver, members);
            for a in args {
                rewrite_expr(a, members);
            }
            if let Some(b) = block {
                rewrite_expr(b, members);
            }
        }
        ExprKind::Call {
            callee,
            args,
            block,
            ..
        } => {
            rewrite_expr(callee, members);
            for a in args {
                rewrite_expr(a, members);
            }
            if let Some(b) = block {
                rewrite_expr(b, members);
            }
        }
        ExprKind::Index { receiver, index } => {
            rewrite_expr(receiver, members);
            rewrite_expr(index, members);
        }
        ExprKind::Block(b) => rewrite_block(b, members),
        ExprKind::Lambda { body, .. } => rewrite_block(body, members),
        ExprKind::Unary { expr, .. } => rewrite_expr(expr, members),
        ExprKind::Binary { lhs, rhs, .. } => {
            rewrite_expr(lhs, members);
            rewrite_expr(rhs, members);
        }
        ExprKind::Try(inner) => rewrite_expr(inner, members),
        ExprKind::If {
            cond,
            then_b,
            else_b,
            let_binding,
        } => {
            rewrite_expr(cond, members);
            rewrite_block(then_b, members);
            if let Some(eb) = else_b {
                rewrite_block(eb, members);
            }
            if let Some((_, init)) = let_binding {
                rewrite_expr(init, members);
            }
        }
        ExprKind::Case { scrutinee, arms } => {
            rewrite_expr(scrutinee, members);
            for arm in arms {
                rewrite_block(&mut arm.body, members);
            }
        }
        ExprKind::Await(inner) => rewrite_expr(inner, members),
        ExprKind::Kwarg { value, .. } => rewrite_expr(value, members),
        // view / widget element bodies have their own emission path
        // and don't carry bare prop refs through this AST shape.
        ExprKind::Element(_) => {}
        ExprKind::Array(items) => {
            for it in items {
                rewrite_expr(it, members);
            }
        }
        ExprKind::Map(pairs) => {
            for (k, v) in pairs {
                rewrite_expr(k, members);
                rewrite_expr(v, members);
            }
        }
        ExprKind::Range { start, end, .. } => {
            rewrite_expr(start, members);
            rewrite_expr(end, members);
        }
        ExprKind::Int(_)
        | ExprKind::Float(_)
        | ExprKind::Bool(_)
        | ExprKind::Nil
        | ExprKind::Str(_)
        | ExprKind::Sym(_)
        | ExprKind::AtIdent(_)
        | ExprKind::SelfRef
        | ExprKind::Path(_) => {}
    }
    // String-interp parts carry sub-expressions; the outer match
    // already moved on (Str is in the leaf arm), so reach in here.
    if let ExprKind::Str(parts) = &mut e.kind {
        for p in parts {
            match p {
                StrPart::Interp(inner) => rewrite_expr(inner, members),
                StrPart::InterpFmt { expr, .. } => rewrite_expr(expr, members),
                StrPart::Text(_) => {}
            }
        }
    }
}

/// Like `parse`, but accepts the relaxed grammar used in `.qpi` Qt
/// binding files. Specifically: bare `prop x : T` (no modifiers) is
/// accepted because binding files describe an external Qt class's
/// Q_PROPERTY surface where the notify signal name isn't always
/// observable from the binding alone. User code (`.cute`) goes through
/// `parse` and gets the strict rule.
pub fn parse_binding(file: FileId, src: &str) -> Result<Module, ParseError> {
    let tokens = lex(file, src)?;
    let mut p = Parser {
        tokens,
        pos: 0,
        src,
        file,
        disable_trailing_block: false,
        binding_mode: true,
        next_block_id: 0,
    };
    p.module()
}

/// Parse a single Cute expression from a source slice. Used by string-
/// interpolation lowering: each `#{...}` body is re-fed through this entry
/// point to produce a real `Expr`.
/// The `@rust("path")` argument, unquoted. `.rpi` files use it to say
/// which Rust item a pixie declaration corresponds to (§8.74).
/// A FIELD's `@rust(..)`, split into the Rust field's name and its
/// type (§8.78): `@rust("len")` names only, `@rust("len: u64")` both.
/// The split is the first `:` that is not part of a `::`, so a
/// qualified type (`p: std::path::PathBuf`) survives.
pub fn rust_field_attr(attrs: &[Attribute]) -> (Option<String>, Option<String>) {
    let Some(raw) = rust_attr(attrs) else {
        return (None, None);
    };
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b':' {
            if bytes.get(i + 1) == Some(&b':') {
                i += 2;
                continue;
            }
            let name = raw[..i].trim().to_string();
            let ty = raw[i + 1..].trim().to_string();
            return (
                (!name.is_empty()).then_some(name),
                (!ty.is_empty()).then_some(ty),
            );
        }
        i += 1;
    }
    (Some(raw), None)
}

pub fn rust_attr(attrs: &[Attribute]) -> Option<String> {
    attrs
        .iter()
        .find(|a| a.name.name == "rust")
        .and_then(|a| a.args.first())
        .map(|p| p.trim().trim_matches('"').to_string())
}

pub fn parse_expression(file: FileId, src: &str) -> Result<Expr, ParseError> {
    let tokens = lex(file, src)?;
    let mut p = Parser {
        tokens,
        pos: 0,
        src,
        file,
        disable_trailing_block: false,
        binding_mode: false,
        next_block_id: 0,
    };
    let e = p.expression()?;
    Ok(e)
}

/// Parse the expression occupying `at` in `src` — an interpolation's
/// slice, re-parsed in place (§8.59). The slice is lexed on its own
/// and every token span is then shifted to where it really is, so a
/// diagnostic inside `#{ .. }` points at the interpolation instead of
/// at the first byte of the file. The parser keeps the FULL source,
/// which is what a nested interpolation needs to slice itself.
fn parse_expression_at(file: FileId, src: &str, at: Span) -> Result<Expr, ParseError> {
    let base = at.range().start as u32;
    let mut tokens = lex(file, &src[at.range()]).map_err(|e| ParseError {
        span: Span::new(
            file,
            e.span.range().start as u32 + base,
            e.span.range().end as u32 + base,
        ),
        message: e.message,
    })?;
    for t in tokens.iter_mut() {
        let r = t.span.range();
        t.span = Span::new(file, r.start as u32 + base, r.end as u32 + base);
    }
    let mut p = Parser {
        tokens,
        pos: 0,
        src,
        file,
        disable_trailing_block: false,
        binding_mode: false,
        next_block_id: 0,
    };
    p.expression()
}

struct Parser<'src> {
    tokens: Vec<Token>,
    pos: usize,
    src: &'src str,
    file: FileId,
    /// When true, `expr_postfix` will not consume `{...}` as a trailing block.
    /// Set inside the heads of `case`/`if`/`while` so that `case foo {` does
    /// not interpret the case body as `foo`'s trailing block.
    disable_trailing_block: bool,
    /// True when parsing a `.qpi` binding file. Relaxes user-source-
    /// only invariants that don't hold for foreign-Qt-surface
    /// descriptions (e.g. bare `prop x : T` without modifiers).
    #[allow(dead_code)] // was the bare-prop exemption; kept for future binding-only shapes
    binding_mode: bool,
    /// Monotonic counter for `block_id`s assigned to items declared
    /// inside `prop ( ... )` / `let ( ... )` / `var ( ... )` blocks.
    /// Each block consumes one id; every item the block expands into
    /// is tagged with that id so the formatter can re-emit the
    /// original block form. IDs are unique per parse and have no
    /// semantic meaning beyond grouping.
    next_block_id: u32,
}

/// What follows a balanced `< types >` pair when speculatively parsing
/// at expression position. Determines whether the construct is a
/// generic-class instantiation (`Box<Int>.new()`) or an explicit-type-args
/// generic-fn call (`make<Int>(0)`).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum TypeArgsClose {
    Dot,
    LParen,
}

/// Which kind keyword introduced a `kw ( ... )` class-member block.
/// Used by `class_member_block` to dispatch the per-item parser.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum BlockKw {
    Prop,
    Let,
    Var,
}

impl BlockKw {
    fn as_str(self) -> &'static str {
        match self {
            BlockKw::Prop => "prop",
            BlockKw::Let => "let",
            BlockKw::Var => "var",
        }
    }
}

impl<'src> Parser<'src> {
    // ---- helpers ---------------------------------------------------------

    fn peek(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }

    fn peek_span(&self) -> Span {
        self.tokens[self.pos].span
    }

    fn peek_kind(&self, off: usize) -> &TokenKind {
        let i = (self.pos + off).min(self.tokens.len() - 1);
        &self.tokens[i].kind
    }

    fn bump(&mut self) -> Token {
        let t = self.tokens[self.pos].clone();
        if !matches!(t.kind, TokenKind::Eof) {
            self.pos += 1;
        }
        t
    }

    fn eat(&mut self, k: &TokenKind) -> bool {
        if std::mem::discriminant(self.peek()) == std::mem::discriminant(k) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, k: &TokenKind, what: &str) -> Result<Token, ParseError> {
        if std::mem::discriminant(self.peek()) == std::mem::discriminant(k) {
            Ok(self.bump())
        } else {
            Err(self.err(format!("expected {what}, found {:?}", self.peek())))
        }
    }

    fn err(&self, msg: impl Into<String>) -> ParseError {
        ParseError {
            span: self.peek_span(),
            message: msg.into(),
        }
    }

    fn skip_newlines(&mut self) {
        while matches!(self.peek(), TokenKind::Newline | TokenKind::Semicolon) {
            self.bump();
        }
    }

    fn at_stmt_end(&self) -> bool {
        matches!(
            self.peek(),
            TokenKind::Newline | TokenKind::Semicolon | TokenKind::RBrace | TokenKind::Eof
        )
    }

    fn ident(&mut self) -> Result<Ident, ParseError> {
        let span = self.peek_span();
        match self.peek().clone() {
            TokenKind::Ident(name) => {
                self.bump();
                Ok(Ident { name, span })
            }
            // `view` / `widget` are reserved at item-decl position
            // (`view Foo {...}` / `widget Foo {...}`) but should not
            // shadow plain identifier uses elsewhere - param names like
            // `widget: QWidget`, member accesses like `mainwin.widget()`,
            // etc. The item parser matches the bare token kinds before
            // ever calling `ident()`, so accepting them here as
            // identifiers reaches everything downstream.
            TokenKind::View => {
                self.bump();
                Ok(Ident {
                    name: "view".to_string(),
                    span,
                })
            }
            // `style` is an item-level keyword, but it doubles as a
            // common element-member key (`Label { style: Card }`) and
            // a perfectly valid identifier in expressions. The item
            // parser matches `Style` directly before ever reaching
            // here, so accepting it as an ident in all other positions
            // is safe.
            TokenKind::Style => {
                self.bump();
                Ok(Ident {
                    name: "style".to_string(),
                    span,
                })
            }
            // `error` is reserved as a legacy item-position keyword
            // (`error FileError { ... }`), but Qt's API has dozens of
            // methods / signals literally named `error()` (QNetworkReply,
            // QSqlQuery, QFile, ...). The item parser matches `Error`
            // before ever reaching here, so accepting it as an ident in
            // member / parameter / expression context lets bindings
            // reference real Qt names without rename gymnastics.
            TokenKind::Error => {
                self.bump();
                Ok(Ident {
                    name: "error".to_string(),
                    span,
                })
            }
            other => Err(self.err(format!("expected identifier, found {other:?}"))),
        }
    }

    // ---- top level -------------------------------------------------------

    fn module(&mut self) -> Result<Module, ParseError> {
        let start = self.peek_span();
        self.skip_newlines();
        let mut items = Vec::new();
        while !matches!(self.peek(), TokenKind::Eof) {
            items.push(self.item()?);
            self.skip_newlines();
        }
        let end = self.peek_span();
        Ok(Module {
            items,
            span: start.join(end),
        })
    }

    fn item(&mut self) -> Result<Item, ParseError> {
        let is_pub = self.eat(&TokenKind::Pub);
        match self.peek() {
            TokenKind::Use => {
                // Disambiguate `use qml "..."` (foreign QML module
                // import) from `use foo` / `use foo.{...}` (Cute
                // source module import). `qml` is a contextual
                // keyword recognised only here — staying out of the
                // global keyword set lets `qml_app(...)` etc. keep
                // working as identifiers everywhere else.
                let is_qml = matches!(self.peek_kind(1), TokenKind::Ident(s) if s == "qml")
                    && matches!(self.peek_kind(2), TokenKind::Str(_));
                if is_qml {
                    if is_pub {
                        return Err(self.err(
                            "`pub use qml \"...\"` is not meaningful — QML imports are file-local"
                                .to_string(),
                        ));
                    }
                    self.parse_use_qml_item().map(Item::UseQml)
                } else {
                    self.parse_use_item(is_pub).map(Item::Use)
                }
            }
            TokenKind::Class => self.class_decl(is_pub).map(Item::Class),
            TokenKind::Store => self.store_decl(is_pub).map(Item::Store),
            TokenKind::Struct => self.struct_decl(is_pub).map(Item::Struct),
            TokenKind::Enum => self.enum_decl(is_pub, /*is_extern=*/ false).map(Item::Enum),
            TokenKind::Flags => self.flags_decl(is_pub, /*is_extern=*/ false).map(Item::Flags),
            TokenKind::Error => self.error_decl(is_pub).map(Item::Enum),
            TokenKind::Fn | TokenKind::Async | TokenKind::Static => self.fn_decl(is_pub).map(Item::Fn),
            TokenKind::Let => self.top_level_let(is_pub).map(Item::Let),
            TokenKind::Var => Err(self.err(
                "top-level `var` is not supported — module-level mutable state has too many static-init-order and thread-safety footguns. Use `let X : T = ...` for an immutable file-scope binding, or move the mutable state inside a class.".to_string(),
            )),
            TokenKind::Test => {
                if is_pub {
                    return Err(self.err(
                        "`pub test` is not meaningful — tests are runner-internal and never exported".to_string(),
                    ));
                }
                self.bump();
                // Two surface forms:
                //   * `test fn camelCase { body }`     (compact, pre-1.x)
                //   * `test "free form name" { body }` (Mint-inspired)
                if matches!(self.peek(), TokenKind::Str(_)) {
                    let f = self.parse_string_named_test_fn(/*display_prefix=*/ None)?;
                    return Ok(Item::Fn(f));
                }
                let mut f = self.fn_decl(is_pub)?;
                f.is_test = true;
                Ok(Item::Fn(f))
            }
            TokenKind::Suite => {
                if is_pub {
                    return Err(self.err(
                        "`pub suite` is not meaningful — suites are test-only and not exported".to_string(),
                    ));
                }
                self.suite_decl().map(Item::Suite)
            }
            TokenKind::View => self.view_decl(is_pub).map(Item::View),
            TokenKind::Style => self.style_decl(is_pub).map(Item::Style),
            TokenKind::Trait => self.trait_decl(is_pub).map(Item::Trait),
            TokenKind::Impl => {
                if is_pub {
                    return Err(self.err(
                        "`pub impl` is not allowed — impl visibility flows from the trait + for-type"
                            .to_string(),
                    ));
                }
                self.impl_decl().map(Item::Impl)
            }
            other => Err(self.err(format!(
                "expected item (use/class/store/struct/enum/error/fn/view/style/trait/impl), found {other:?}"
            ))),
        }
    }

    /// Parse `trait Name { fn sig; fn sig; }`. Methods may either be
    /// abstract (no body — implementers must supply one) or carry a
    /// default body (acts as a fallback when an `impl` block omits
    /// the method). `pub` on a method is a no-op since visibility
    /// flows from the trait itself.
    fn trait_decl(&mut self, is_pub: bool) -> Result<TraitDecl, ParseError> {
        let start = self.peek_span();
        self.bump(); // `trait`
        let name = self.ident()?;
        self.skip_newlines();
        self.expect(&TokenKind::LBrace, "`{` for trait body")?;
        self.skip_newlines();
        let mut methods = Vec::new();
        while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
            // Trait method visibility is irrelevant — every impl
            // method is implicitly public via its impl. Accept and
            // silently ignore a leading `pub` so user-source style
            // can stay uniform with class members.
            let _ = self.eat(&TokenKind::Pub);
            if !matches!(
                self.peek(),
                TokenKind::Fn | TokenKind::Async | TokenKind::Static
            ) {
                return Err(self.err("expected `fn` inside trait body".to_string()));
            }
            let f = self.fn_decl(false)?;
            methods.push(f);
            self.skip_newlines();
        }
        let end = self.peek_span();
        self.expect(&TokenKind::RBrace, "`}` to close trait body")?;
        Ok(TraitDecl {
            name,
            methods,
            is_pub,
            span: start.join(end),
        })
    }

    /// Parse `impl<T...> Trait for ForType { fn ...; fn ...; }`.
    /// Every method must have a body — the impl is supplying concrete
    /// implementations, not declaring more abstract surface.
    ///
    /// `<T...>` impl-level generics introduce type variables that the
    /// for-type expression and impl method bodies can reference
    /// (`impl<T> Iterable for List<T>`). Bounds on those generics
    /// are accepted but not enforced beyond the parser's record-and-
    /// validate-by-name phase.
    ///
    /// `for_type` is parsed as a full `TypeExpr` so parametric
    /// instantiations (`List<T>`, `Box<Int>`, `Map<K, V>`) and extern
    /// types (`QStringList`) round-trip cleanly.
    fn impl_decl(&mut self) -> Result<ImplDecl, ParseError> {
        let start = self.peek_span();
        self.bump(); // `impl`
        let generics = self.maybe_generics()?;
        let trait_name = self.ident()?;
        self.expect(&TokenKind::For, "`for`")?;
        let for_type = self.type_expr()?;
        self.skip_newlines();
        self.expect(&TokenKind::LBrace, "`{` for impl body")?;
        self.skip_newlines();
        let mut methods = Vec::new();
        while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
            // Impl methods are always public via the impl; accept a
            // leading `pub` for parity with class-member parsing.
            let _ = self.eat(&TokenKind::Pub);
            if !matches!(
                self.peek(),
                TokenKind::Fn | TokenKind::Async | TokenKind::Static
            ) {
                return Err(self.err("expected `fn` inside impl block".to_string()));
            }
            let f = self.fn_decl(false)?;
            if f.body.is_none() {
                return Err(self.err("impl methods must have a body".to_string()));
            }
            methods.push(f);
            self.skip_newlines();
        }
        let end = self.peek_span();
        self.expect(&TokenKind::RBrace, "`}` to close impl body")?;
        Ok(ImplDecl {
            trait_name,
            for_type,
            generics,
            methods,
            span: start.join(end),
        })
    }



    // ---- view (Cute UI DSL) ---------------------------------------------

    fn view_decl(&mut self, is_pub: bool) -> Result<ViewDecl, ParseError> {
        let start = self.peek_span();
        self.bump(); // `view`
        let name = self.ident()?;
        // Optional exposed-parameter list: `view Card(label: String) { ... }`.
        // The same shape as fn params - reuse the parser. Without parens
        // the view is parameterless and the next token must be `{`.
        let params = if matches!(self.peek(), TokenKind::LParen) {
            self.params()?
        } else {
            Vec::new()
        };
        self.skip_newlines();
        self.expect(&TokenKind::LBrace, "`{` for view body")?;
        self.skip_newlines();
        let state_fields = self.parse_state_fields(/*allow_let_form=*/ true)?;
        let root = self.parse_element()?;
        self.skip_newlines();
        let end = self.peek_span();
        self.expect(&TokenKind::RBrace, "`}` to close view")?;
        Ok(ViewDecl {
            name,
            params,
            state_fields,
            root,
            is_pub,
            span: start.join(end),
        })
    }


    /// `style Card { padding: 16, ... }` (literal body) or
    /// `style BigCard = Card + Big` (alias body). Keys may be dotted
    /// (`font.bold`); the parser joins them into a single string per
    /// entry so codegen can emit them as either QML property paths or
    /// QtWidgets setter names. Optional commas / newlines separate
    /// entries inside `{ ... }`.
    fn style_decl(&mut self, is_pub: bool) -> Result<StyleDecl, ParseError> {
        let start = self.peek_span();
        self.bump(); // `style`
        let name = self.ident()?;
        if matches!(self.peek(), TokenKind::Eq) {
            self.bump(); // `=`
            let rhs = self.expression()?;
            let end = rhs.span;
            return Ok(StyleDecl {
                name,
                body: StyleBody::Alias(rhs),
                is_pub,
                span: start.join(end),
            });
        }
        self.skip_newlines();
        self.expect(&TokenKind::LBrace, "`{` for style body or `=` for alias")?;
        self.skip_newlines();
        let mut entries: Vec<StyleEntry> = Vec::new();
        while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
            let e_start = self.peek_span();
            // Dotted key: `font.bold` -> "font.bold".
            let mut key_parts = vec![self.ident()?.name];
            while matches!(self.peek(), TokenKind::Dot)
                && matches!(self.peek_kind(1), TokenKind::Ident(_))
            {
                self.bump();
                key_parts.push(self.ident()?.name);
            }
            let key = key_parts.join(".");
            self.expect(&TokenKind::Colon, "`:` after style entry key")?;
            let value = self.expression()?;
            let e_end = value.span;
            entries.push(StyleEntry {
                key,
                value,
                span: e_start.join(e_end),
            });
            // Optional comma between entries; newline always separates.
            self.skip_newlines();
            self.eat(&TokenKind::Comma);
            self.skip_newlines();
        }
        let end = self.peek_span();
        self.expect(&TokenKind::RBrace, "`}` to close style body")?;
        Ok(StyleDecl {
            name,
            body: StyleBody::Lit(entries),
            is_pub,
            span: start.join(end),
        })
    }

    /// Consume zero or more state declarations at the head of a view /
    /// widget / store body.
    ///
    /// - `state <name> : <ty> = <expr>` — primitive prop-style
    ///   cell, accepted in every flavor (lowers to QML root-level
    ///   `property` for views, synth holder for widgets, prop+signal
    ///   for stores).
    /// - `let <name> = <expr>` — sub-object state holder. Accepted
    ///   only when `allow_let_form` is true (view / widget); store
    ///   bodies pass `false` because singletons have no parent-tree
    ///   lifetime model for sub-objects.
    ///
    /// `state` is a contextual keyword: only opens a state-prop
    /// declaration when followed by `IDENT :`. Anywhere else it
    /// parses as a plain identifier (preserves `signal
    /// stateChanged(state: Int)` and similar pre-existing uses).
    fn parse_state_fields(&mut self, allow_let_form: bool) -> Result<Vec<StateField>, ParseError> {
        let mut fields = Vec::new();
        loop {
            self.skip_newlines();
            let is_let = allow_let_form && matches!(self.peek(), TokenKind::Let);
            let is_state = matches!(self.peek(), TokenKind::Ident(s) if s == "state")
                && matches!(self.peek_kind(1), TokenKind::Ident(_))
                && matches!(self.peek_kind(2), TokenKind::Colon);
            if !is_let && !is_state {
                break;
            }
            let start = self.peek_span();
            self.bump(); // `let` or `state`
            let name = self.ident()?;
            let kind = if is_state {
                self.expect(&TokenKind::Colon, "`:` after `state <name>`")?;
                let ty = self.type_expr()?;
                StateFieldKind::Property { ty }
            } else {
                StateFieldKind::Object
            };
            self.expect(&TokenKind::Eq, "`=` after state field name")?;
            // Disable trailing-block during the init expression so the
            // root element's `{` doesn't get pulled in as a trailing
            // block of, say, a constructor call.
            let prev = std::mem::replace(&mut self.disable_trailing_block, true);
            let init_expr = self.expression()?;
            self.disable_trailing_block = prev;
            let end = self.tokens[self.pos.saturating_sub(1)].span;
            fields.push(StateField {
                name,
                kind,
                init_expr,
                span: start.join(end),
            });
            let _ = self.eat(&TokenKind::Semicolon);
        }
        Ok(fields)
    }

    fn parse_element(&mut self) -> Result<Element, ParseError> {
        let start = self.peek_span();
        // Element head can be `Foo` or `model.Counter` - the leading
        // segments form the module qualifier and the final segment is
        // the type name. Dot-disambiguation: keep peeling while the
        // next two tokens are `.` then an ident; stop when we run out
        // of segments. The single-Ident case is the common one.
        let mut path: Vec<Ident> = vec![self.ident()?];
        while matches!(self.peek(), TokenKind::Dot)
            && matches!(self.peek_kind(1), TokenKind::Ident(_))
        {
            self.bump();
            path.push(self.ident()?);
        }
        let name = path.pop().expect("element head has at least one ident");
        let module_path = path;
        self.skip_newlines();
        self.expect(&TokenKind::LBrace, "`{` to open element body")?;
        let mut members = Vec::new();
        loop {
            self.skip_newlines();
            if matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
                break;
            }
            members.push(self.parse_element_member()?);
        }
        let end = self.peek_span();
        self.expect(&TokenKind::RBrace, "`}` to close element")?;
        Ok(Element {
            module_path,
            name,
            members,
            span: start.join(end),
        })
    }

    fn parse_element_member(&mut self) -> Result<ElementMember, ParseError> {
        // `if cond { El { ... } }` / `for x in xs { El { ... } }` /
        // `case x { when pat { El { ... } } ... }` reuse the
        // language-core if/for/case constructs - the body block's
        // trailing expression becomes a `K::Element(...)`. Codegen
        // pulls those elements out via `trailing_element` and renders
        // them with the appropriate visibility / Repeater wrapper.
        match self.peek() {
            TokenKind::If => {
                let span = self.peek_span();
                let if_expr = self.if_expr_with_element_body()?;
                return Ok(ElementMember::Stmt(Stmt::Expr(Expr {
                    kind: if_expr.kind,
                    span,
                })));
            }
            TokenKind::For => {
                return self.parse_element_member_for();
            }
            TokenKind::Case => {
                let span = self.peek_span();
                let case_expr = self.case_expr_with_element_body()?;
                return Ok(ElementMember::Stmt(Stmt::Expr(Expr {
                    kind: case_expr.kind,
                    span,
                })));
            }
            _ => {}
        }
        // Both `key: value` and `Child { ... }` start with an identifier,
        // and both can be dotted: `font.bold: ...` (property) vs.
        // `model.Counter { ... }` (cross-module child element). The
        // disambiguator is the token AFTER the (possibly-dotted)
        // identifier chain: a `:` means property, a `{` means element.
        // `style` doubles as a contextual keyword in this position.
        if !matches!(self.peek(), TokenKind::Ident(_) | TokenKind::Style) {
            return Err(self.err(format!(
                "expected property name or child element, found {:?}",
                self.peek()
            )));
        }
        // Walk the (possibly-dotted) head: `Ident (Dot Ident)*` skipping
        // newlines after each segment. We stop at the first non-`.`
        // token after an ident and inspect it.
        let mut probe = 1usize;
        loop {
            while matches!(self.peek_kind(probe), TokenKind::Newline) {
                probe += 1;
            }
            if !matches!(self.peek_kind(probe), TokenKind::Dot)
                || !matches!(self.peek_kind(probe + 1), TokenKind::Ident(_))
            {
                break;
            }
            probe += 2;
        }
        match self.peek_kind(probe) {
            TokenKind::Colon => self.parse_element_property(),
            TokenKind::LBrace => self.parse_element().map(ElementMember::Child),
            other => Err(self.err(format!(
                "expected `:` (property) or `{{` (child element) after `{:?}`, found {:?}",
                self.peek(),
                other
            ))),
        }
    }

    /// Parse `if cond { El { ... } } [else if ...] [else { El { ... } }]`
    /// using the language-core `ExprKind::If`. The body of each
    /// branch is a `Block` whose only content is a trailing
    /// `K::Element(...)`. `else if` is encoded as `else { if ... }`
    /// the same way Rust / many other languages do, so the AST has
    /// no special `else if` node.
    fn if_expr_with_element_body(&mut self) -> Result<Expr, ParseError> {
        let start = self.peek_span();
        self.bump(); // `if`
        // `if let pat = init { El }` in view position (§8.69) — the
        // same shape a statement `if` takes, so a view can name what
        // an optional holds while deciding whether to show it.
        let let_binding = if matches!(self.peek(), TokenKind::Let) {
            self.bump(); // `let`
            let pat = self.pattern()?;
            self.expect(&TokenKind::Eq, "`=` after `if let pattern`")?;
            let prev = std::mem::replace(&mut self.disable_trailing_block, true);
            let init = self.expression()?;
            self.disable_trailing_block = prev;
            Some((Box::new(pat), Box::new(init)))
        } else {
            None
        };
        let (cond, then_b) = if let_binding.is_some() {
            let body = self.parse_view_block("`{` for if body in view")?;
            (
                Expr {
                    kind: ExprKind::Bool(true),
                    span: start,
                },
                body,
            )
        } else {
            self.parse_cond_and_element_block("if")?
        };
        let else_b = self.parse_else_with_element_body()?;
        let end = self.tokens[self.pos.saturating_sub(1)].span;
        Ok(Expr {
            kind: ExprKind::If {
                cond: Box::new(cond),
                then_b,
                else_b,
                let_binding,
            },
            span: start.join(end),
        })
    }

    fn parse_cond_and_element_block(
        &mut self,
        ctx: &'static str,
    ) -> Result<(Expr, Block), ParseError> {
        let prev = std::mem::replace(&mut self.disable_trailing_block, true);
        let cond = self.expression()?;
        self.disable_trailing_block = prev;
        let _ = ctx;
        let block = self.parse_view_block("`{` for if body in view")?;
        Ok((cond, block))
    }

    /// Parse the optional `else` / `else if` tail after an element-
    /// position if. Returns the encoded `Option<Block>` for the
    /// language-core `ExprKind::If::else_b`. `else if` becomes a
    /// Block whose trailing expression is another K::If.
    fn parse_else_with_element_body(&mut self) -> Result<Option<Block>, ParseError> {
        let mut probe = 0usize;
        while matches!(self.peek_kind(probe), TokenKind::Newline) {
            probe += 1;
        }
        if !matches!(self.peek_kind(probe), TokenKind::Else) {
            return Ok(None);
        }
        self.skip_newlines();
        let else_span = self.peek_span();
        self.bump(); // `else`
        if matches!(self.peek(), TokenKind::If) {
            // `else if`: synthesize `else { if ... }`. The inner if
            // recurses for its own else chain.
            let inner = self.if_expr_with_element_body()?;
            let inner_span = inner.span;
            return Ok(Some(Block {
                stmts: Vec::new(),
                trailing: Some(Box::new(inner)),
                span: else_span.join(inner_span),
            }));
        }
        // Terminal `else { .. }`.
        Ok(Some(self.parse_view_block("`{` for else body in view")?))
    }

    /// `case scrutinee { when pat { El { ... } } ... }` -> language-
    /// core `ExprKind::Case` whose arm bodies are Blocks with a
    /// trailing `K::Element(el)`. Pattern parsing reuses the
    /// existing `pattern()` helper, so `when ok(v)` etc. all work
    /// identically to fn-body case.
    fn case_expr_with_element_body(&mut self) -> Result<Expr, ParseError> {
        let start = self.peek_span();
        self.bump(); // `case`
        let scrutinee = self.expr_no_block()?;
        self.skip_newlines();
        self.expect(&TokenKind::LBrace, "`{` for case arms in view")?;
        self.skip_newlines();
        let mut arms: Vec<CaseArm> = Vec::new();
        while matches!(self.peek(), TokenKind::When) {
            let arm_start = self.peek_span();
            self.bump(); // `when`
            let pattern = self.pattern()?;
            let body = self.parse_view_block("`{` for arm body in view-case")?;
            let arm_end = body.span;
            arms.push(CaseArm {
                pattern,
                body,
                span: arm_start.join(arm_end),
            });
            self.skip_newlines();
        }
        let end = self.peek_span();
        self.expect(&TokenKind::RBrace, "`}` to close case in view")?;
        Ok(Expr {
            kind: ExprKind::Case {
                scrutinee: Box::new(scrutinee),
                arms,
            },
            span: start.join(end),
        })
    }

    /// The `{ .. }` body of a view-position `if` / `else` / `for` /
    /// `case` arm: zero or more view members, which is the same
    /// vocabulary a container element's body holds (§8.56). Encoded
    /// as a Block so the language-core `If`/`For` nodes are unchanged
    /// — leading members become statements, the last becomes the
    /// trailing expression.
    ///
    /// Properties are rejected here: `spacing: 4` inside an `if`
    /// belongs to no element.
    fn parse_view_block(&mut self, what: &'static str) -> Result<Block, ParseError> {
        self.skip_newlines();
        let lbrace_span = self.peek_span();
        self.expect(&TokenKind::LBrace, what)?;
        let mut items: Vec<Stmt> = Vec::new();
        loop {
            self.skip_newlines();
            if matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
                break;
            }
            let span = self.peek_span();
            match self.parse_element_member()? {
                ElementMember::Child(el) => items.push(Stmt::Expr(Expr {
                    kind: ExprKind::Element(el),
                    span,
                })),
                ElementMember::Stmt(st) => items.push(st),
                ElementMember::Property { span, .. } => {
                    return Err(ParseError {
                        span,
                        message: "a property belongs to an element, not to an `if` or `for` body"
                            .into(),
                    });
                }
            }
        }
        let rbrace_span = self.peek_span();
        self.expect(&TokenKind::RBrace, what)?;
        // A trailing expression is the block's value, which is the
        // shape a one-element body has always had — keep it, so
        // everything that reads `body.trailing` (the component
        // splice, the checker, the fingerprint) sees what it did.
        let trailing = match items.last() {
            Some(Stmt::Expr(_)) => match items.pop() {
                Some(Stmt::Expr(e)) => Some(Box::new(e)),
                _ => unreachable!("just matched"),
            },
            _ => None,
        };
        Ok(Block {
            stmts: items,
            trailing,
            span: lbrace_span.join(rbrace_span),
        })
    }

    fn parse_element_member_for(&mut self) -> Result<ElementMember, ParseError> {
        let start = self.peek_span();
        self.bump(); // `for`
        let binding = self.ident()?;
        let index = self.for_index()?;
        match self.peek() {
            TokenKind::Ident(s) if s == "in" => {
                self.bump();
            }
            other => {
                return Err(self.err(format!(
                    "expected `in` after for-binding, found {:?}",
                    other
                )));
            }
        }
        let prev = std::mem::replace(&mut self.disable_trailing_block, true);
        let iter = self.expression()?;
        self.disable_trailing_block = prev;
        let body = self.parse_view_block("`{` for for body in view")?;
        let end = body.span;
        Ok(ElementMember::Stmt(Stmt::For {
            binding,
            index,
            iter,
            body,
            span: start.join(end),
        }))
    }

    fn parse_element_property(&mut self) -> Result<ElementMember, ParseError> {
        let start = self.peek_span();
        // Parse a (possibly dotted) key.
        let mut key_parts = vec![self.ident()?.name];
        while matches!(self.peek(), TokenKind::Dot)
            && matches!(self.peek_kind(1), TokenKind::Ident(_))
        {
            self.bump(); // `.`
            key_parts.push(self.ident()?.name);
        }
        let key = key_parts.join(".");
        self.expect(&TokenKind::Colon, "`:` after property key")?;
        // Allow an Element literal as the value: `prop: Page { title: ... }`.
        // This is required for Kirigami patterns like
        // `pageStack.initialPage: Kirigami.Page { ... }` where the
        // assigned object is a QML element subtree, not a regular
        // expression. Distinguished from trailing-block-call by the
        // body's first content (`Ident :` or `Ident {` = element
        // member; anything else falls through to expression).
        let value = if self.peek_looks_like_element_value() {
            let elem = self.parse_element()?;
            let span = elem.span;
            Expr {
                kind: ExprKind::Element(elem),
                span,
            }
        } else {
            self.expression()?
        };
        let end = value.span;
        Ok(ElementMember::Property {
            key,
            value,
            span: start.join(end),
        })
    }

    /// Heuristic for "is the next token sequence an Element literal
    /// in property-value position?". Looks for `Ident (.Ident)* {` and
    /// then checks the first token inside the braces — if it looks
    /// like an element member (`Ident :` or `Ident {`), treat it as
    /// an Element. Anything else falls through to the standard
    /// expression parser.
    fn peek_looks_like_element_value(&self) -> bool {
        if !matches!(self.peek(), TokenKind::Ident(_)) {
            return false;
        }
        // Skip past `Ident (.Ident)*` to find the `{`.
        let mut i = 1;
        while matches!(self.peek_kind(i), TokenKind::Dot)
            && matches!(self.peek_kind(i + 1), TokenKind::Ident(_))
        {
            i += 2;
        }
        if !matches!(self.peek_kind(i), TokenKind::LBrace) {
            return false;
        }
        // Inside `{`: skip newlines, then look at the first token.
        let mut j = i + 1;
        while matches!(self.peek_kind(j), TokenKind::Newline) {
            j += 1;
        }
        // Empty `{}` — treat as Element (zero-member element body).
        if matches!(self.peek_kind(j), TokenKind::RBrace) {
            return true;
        }
        // First content token must be an ident (or `style` keyword
        // which doubles as a property key).
        if !matches!(self.peek_kind(j), TokenKind::Ident(_) | TokenKind::Style) {
            return false;
        }
        // Skip the ident's dotted suffix (for `font.bold:` etc.).
        let mut k = j + 1;
        while matches!(self.peek_kind(k), TokenKind::Dot)
            && matches!(self.peek_kind(k + 1), TokenKind::Ident(_))
        {
            k += 2;
        }
        // `:` (property) or `{` (child element) confirms an Element body.
        matches!(self.peek_kind(k), TokenKind::Colon | TokenKind::LBrace)
    }

    /// Parse a `use ...` directive. The leading `pub` is consumed by
    /// `item()` before dispatching here, so this entry just sees the
    /// `use` token at `self.peek()`. Forms accepted:
    ///   - `use foo`              -> Module(None)
    ///   - `use foo as bar`       -> Module(Some("bar"))
    ///   - `use foo.bar`          -> path = [foo, bar], Module(None)
    ///   - `use foo.{X, Y}`       -> Names([X, Y])
    ///   - `use foo.{X as A, Y}`  -> Names([X as A, Y])
    fn parse_use_item(&mut self, is_pub: bool) -> Result<UseItem, ParseError> {
        let start = self.peek_span();
        self.bump(); // `use`
        let mut path = vec![self.ident()?];
        while matches!(self.peek(), TokenKind::Dot) {
            // Selective-import brace list begins with `.{` after the
            // module path: `use foo.{X, Y}`. The `{` isn't a path
            // segment, so peek ahead one token before consuming the
            // dot.
            if matches!(self.peek_kind(1), TokenKind::LBrace) {
                self.bump(); // `.`
                let names = self.parse_use_name_list()?;
                let end = self.tokens[self.pos.saturating_sub(1)].span;
                return Ok(UseItem {
                    path,
                    kind: UseKind::Names(names),
                    is_pub,
                    span: start.join(end),
                });
            }
            self.bump(); // `.`
            path.push(self.ident()?);
        }
        // Optional `as <alias>` for whole-module imports.
        let alias = if matches!(self.peek(), TokenKind::Ident(s) if s == "as") {
            self.bump();
            Some(self.ident()?)
        } else {
            None
        };
        let end = self.tokens[self.pos.saturating_sub(1)].span;
        Ok(UseItem {
            path,
            kind: UseKind::Module(alias),
            is_pub,
            span: start.join(end),
        })
    }

    /// Parse `use qml "uri" [as Alias]`. The dispatcher in
    /// `parse_top_level_item` already confirmed the contextual `qml`
    /// keyword + string literal pattern; this function consumes them
    /// and the optional alias.
    fn parse_use_qml_item(&mut self) -> Result<UseQmlItem, ParseError> {
        let start = self.peek_span();
        self.bump(); // `use`
        // The contextual `qml` ident — we already verified its shape
        // in the caller, but consume it cleanly here.
        match self.peek() {
            TokenKind::Ident(s) if s == "qml" => {
                self.bump();
            }
            _ => return Err(self.err("expected `qml` after `use`")),
        }
        // The URI, as a string literal. We accept any Str token, but
        // the segments must collapse to a single text segment — a
        // QML URI doesn't have interpolation or formatting.
        let module_uri = match self.peek().clone() {
            TokenKind::Str(segs) => {
                self.bump();
                let mut out = String::new();
                for seg in segs {
                    match seg {
                        StrSeg::Text(t) => out.push_str(&t),
                        _ => {
                            return Err(self.err(
                                "QML module URI must be a plain string literal (no `#{...}` interpolation)"
                            ));
                        }
                    }
                }
                out
            }
            other => {
                return Err(self.err(format!("expected QML module URI string, found {other:?}")));
            }
        };
        // Optional `as <Alias>`.
        let alias = if matches!(self.peek(), TokenKind::Ident(s) if s == "as") {
            self.bump();
            Some(self.ident()?)
        } else {
            None
        };
        let end = self.tokens[self.pos.saturating_sub(1)].span;
        Ok(UseQmlItem {
            module_uri,
            alias,
            span: start.join(end),
        })
    }

    /// Parse `{ X, Y as A, Z }` after `use foo.`. Trailing comma
    /// allowed; newlines tolerated.
    fn parse_use_name_list(&mut self) -> Result<Vec<UseName>, ParseError> {
        self.expect(&TokenKind::LBrace, "`{` for selective import list")?;
        self.skip_newlines();
        let mut names = Vec::new();
        while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
            let n_start = self.peek_span();
            let name = self.ident()?;
            let alias = if matches!(self.peek(), TokenKind::Ident(s) if s == "as") {
                self.bump();
                Some(self.ident()?)
            } else {
                None
            };
            let n_end = self.tokens[self.pos.saturating_sub(1)].span;
            names.push(UseName {
                name,
                alias,
                span: n_start.join(n_end),
            });
            self.skip_newlines();
            if !self.eat(&TokenKind::Comma) {
                break;
            }
            self.skip_newlines();
        }
        self.expect(&TokenKind::RBrace, "`}` to close selective import list")?;
        Ok(names)
    }

    // ---- class -----------------------------------------------------------

    /// Parse a top-level `let X : T = expr` declaration. The type
    /// annotation is **required** here (unlike statement-level `let`,
    /// which can infer from the value) because file-scope bindings
    /// have no surrounding context to disambiguate at codegen time —
    /// `static const auto` C++ inference is brittle when the value is
    /// itself constructed via `T.new()` or template-heavy expressions.
    fn top_level_let(&mut self, is_pub: bool) -> Result<LetDecl, ParseError> {
        let start = self.peek_span();
        self.bump(); // `let`
        let name = self.ident()?;
        self.expect(
            &TokenKind::Colon,
            "top-level `let` requires an explicit type annotation (e.g. `let X : Int = 0`)",
        )?;
        let ty = self.type_expr()?;
        self.expect(&TokenKind::Eq, "`=` after top-level let type")?;
        let value = self.expression()?;
        let end = self.tokens[self.pos.saturating_sub(1)].span;
        Ok(LetDecl {
            name,
            ty,
            value,
            is_pub,
            span: start.join(end),
        })
    }

    fn class_decl(&mut self, is_pub: bool) -> Result<ClassDecl, ParseError> {
        let start = self.peek_span();
        self.bump(); // `class`
        let name = self.ident()?;
        let generics = self.maybe_generics()?;
        // Optional `: ~Copyable` annotation. Sits between generics and
        // the optional `< Super` clause so the latter can still pick
        // up a clean parse off `Lt`.
        let is_copyable = self.parse_optional_copyable_annotation()?;
        let super_class = if matches!(self.peek(), TokenKind::Lt) {
            self.bump();
            let t = self.type_expr()?;
            // Reject the legacy `class X < Object` form. Reference
            // types now live under their own keyword (`ref`); the
            // magic `Object` sentinel is gone.
            if let TypeKind::Named { path, .. } = &t.kind {
                if let Some(last) = path.last() {
                    if last.name == "Object" {
                        return Err(self.err(
                            format!(
                                "`class {} < Object` is no longer supported — use `arc {} {{ ... }}` for reference-counted (ARC) types",
                                name.name, name.name,
                            ),
                        ));
                    }
                }
            }
            Some(t)
        } else {
            None
        };
        self.skip_newlines();
        self.expect(&TokenKind::LBrace, "`{`")?;
        self.skip_newlines();
        let mut members = Vec::new();
        let mut seen_deinit = false;
        while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
            for m in self.class_member()? {
                if matches!(&m, ClassMember::Deinit(_)) {
                    if seen_deinit {
                        return Err(self.err("only one `deinit` per class is allowed".to_string()));
                    }
                    seen_deinit = true;
                }
                members.push(m);
            }
            self.skip_newlines();
        }
        let end = self.peek_span();
        self.expect(&TokenKind::RBrace, "`}`")?;
        Ok(ClassDecl {
            name,
            generics,
            super_class,
            members,
            is_pub,
            is_extern_value: false,
            is_arc: false,
            is_copyable,
            span: start.join(end),
        })
    }

    /// `: ~Copyable` opt-in for linear-type semantics. Returns `false`
    /// (i.e. *not* copyable) when the annotation is present, `true`
    /// otherwise. Used by `class_decl` / `arc_decl` / `struct_decl`.
    /// The leading `:` is also used by property type ascription, but
    /// the contexts here only ever follow the name+generics — there's
    /// no ambiguity.
    fn parse_optional_copyable_annotation(&mut self) -> Result<bool, ParseError> {
        if !matches!(self.peek(), TokenKind::Colon) {
            return Ok(true);
        }
        // Look one ahead for `~`; if not, we're not in a `~Copyable`
        // position and the `:` belongs to something else (e.g. a
        // future type-bound syntax). Conservative: only consume when
        // we see `~`.
        if !matches!(self.peek_kind(1), TokenKind::Tilde) {
            return Ok(true);
        }
        self.bump(); // `:`
        self.bump(); // `~`
        let kw = self.ident()?;
        if kw.name != "Copyable" {
            return Err(self.err(format!(
                "expected `~Copyable`, got `~{}`; only `Copyable` is recognised here",
                kw.name,
            )));
        }
        Ok(false)
    }


    /// Parse `store Name { <body> }` — declarative singleton.
    ///
    /// Differs from a class body: only the `state X : T = init` form
    /// of state field is accepted (the view/widget `let X = sub_obj()`
    /// Object-kind shape doesn't fit a singleton's lifetime model);
    /// no generics, no `< Super`, no `~Copyable`.
    fn store_decl(&mut self, is_pub: bool) -> Result<StoreDecl, ParseError> {
        let start = self.peek_span();
        self.bump(); // `store`
        let name = self.ident()?;
        self.skip_newlines();
        self.expect(&TokenKind::LBrace, "`{` for store body")?;
        self.skip_newlines();
        let state_fields = self.parse_state_fields(/*allow_let_form=*/ false)?;
        let mut members: Vec<ClassMember> = Vec::new();
        let mut seen_deinit = false;
        while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
            for m in self.class_member()? {
                if matches!(&m, ClassMember::Deinit(_)) {
                    if seen_deinit {
                        return Err(self.err("only one `deinit` per store is allowed".to_string()));
                    }
                    seen_deinit = true;
                }
                members.push(m);
            }
            self.skip_newlines();
        }
        let end = self.peek_span();
        self.expect(&TokenKind::RBrace, "`}` to close store body")?;
        Ok(StoreDecl {
            name,
            state_fields,
            members,
            is_pub,
            span: start.join(end),
        })
    }

    /// Parse `suite "X" { test "y" { body } ... }` (Mint-inspired).
    /// Each contained `test "y" { ... }` becomes a regular `FnDecl`
    /// (is_test, synth name, `display_name = Some("X / y")`). Suites
    /// are flat — nested `suite { ... }` is rejected.
    fn suite_decl(&mut self) -> Result<SuiteDecl, ParseError> {
        let start = self.peek_span();
        self.bump(); // `suite`
        let (name, name_span) =
            self.expect_simple_string_literal("`suite` requires a string literal name")?;
        self.skip_newlines();
        self.expect(&TokenKind::LBrace, "`{` for suite body")?;
        self.skip_newlines();
        let mut tests: Vec<FnDecl> = Vec::new();
        while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
            match self.peek() {
                TokenKind::Test => {
                    self.bump();
                    // Compact `test fn camelCase` declares at top level
                    // alongside the suite, not inside it.
                    if !matches!(self.peek(), TokenKind::Str(_)) {
                        return Err(self.err(format!(
                            "test inside `suite \"{}\"` must use the string-named form `test \"...\" {{ ... }}`",
                            name,
                        )));
                    }
                    let f = self.parse_string_named_test_fn(Some(&name))?;
                    tests.push(f);
                }
                TokenKind::Suite => {
                    return Err(self.err(
                        "nested `suite` blocks are not supported in v1.x — keep grouping flat"
                            .to_string(),
                    ));
                }
                other => {
                    return Err(self.err(format!(
                        "expected `test \"...\" {{ ... }}` inside `suite \"{}\"`, found {other:?}",
                        name,
                    )));
                }
            }
            self.skip_newlines();
        }
        let end = self.peek_span();
        self.expect(&TokenKind::RBrace, "`}` to close suite body")?;
        Ok(SuiteDecl {
            name,
            name_span,
            tests,
            span: start.join(end),
        })
    }

    /// Parse `"<test name>" { body }`. Caller has already consumed
    /// `test`. Returns a synth `FnDecl` whose `display_name` carries
    /// the original prose for TAP output.
    fn parse_string_named_test_fn(
        &mut self,
        display_prefix: Option<&str>,
    ) -> Result<FnDecl, ParseError> {
        let (test_name, name_span) = self.expect_simple_string_literal(
            "expected a string literal after `test`: `test \"...\" { body }`",
        )?;
        self.skip_newlines();
        let body = self.block()?;
        let body_span = body.span;
        let synth_id = self.alloc_block_id();
        let display_name = match display_prefix {
            Some(prefix) => format!("{prefix} / {test_name}"),
            None => test_name,
        };
        Ok(FnDecl {
            is_async: false,
            name: Ident {
                name: format!("__cuteStrTest{synth_id}"),
                span: name_span,
            },
            generics: Vec::new(),
            params: Vec::new(),
            return_ty: None,
            body: Some(body),
            is_pub: false,
            is_test: true,
            display_name: Some(display_name),
            attributes: Vec::new(),
            is_static: false,
            span: name_span.join(body_span),
        })
    }

    /// Consume a single string literal that contains no interpolations
    /// (`#{...}`) or format specs. Used by `suite "X"` and
    /// `test "y"` where the literal is metadata, not runtime text.
    fn expect_simple_string_literal(&mut self, ctx: &str) -> Result<(String, Span), ParseError> {
        let span = self.peek_span();
        match self.peek().clone() {
            TokenKind::Str(parts) => match parts.as_slice() {
                [crate::token::StrSeg::Text(s)] => {
                    let s = s.clone();
                    self.bump();
                    Ok((s, span))
                }
                _ => Err(self.err(format!(
                    "{ctx}: string interpolation `#{{...}}` is not allowed here",
                ))),
            },
            other => Err(self.err(format!("{ctx}, found {other:?}"))),
        }
    }

    fn class_member(&mut self) -> Result<Vec<ClassMember>, ParseError> {
        let mut is_pub = self.eat(&TokenKind::Pub);
        // Optional `weak` / `unowned` modifier before `prop` / `let` /
        // `var`. These only apply to reference-typed storage on
        // class / arc members; the parser eats one of them and rejects
        // the contradictory `weak unowned` / `unowned weak` pairing
        // up front. Whether the held type is actually a reference
        // type is checked at the type-check layer.
        // Accept `pub` after `weak` / `unowned` too — the canonical
        // form is `pub weak let X` but `weak pub let X` is harmless,
        // so unify both spellings here.
        // `unowned` went with the keyword (§8.71) — a handle cannot
        // dangle, so non-null non-owning had nothing to add.
        let (weak, unowned) = match self.peek() {
            TokenKind::Weak => {
                self.bump();
                (true, false)
            }
            _ => (false, false),
        };
        // `weak pub let X` shorthand — accept stray `pub` after the
        // weak/unowned modifier (the script-driven migration sometimes
        // emits this order).
        if !is_pub {
            is_pub = self.eat(&TokenKind::Pub);
        }
        match self.peek() {
            // `unowned` stays removed: it promises the pointee
            // outlives the holder, which nothing in pixie can check —
            // and a plain handle already cannot dangle (§3.4).
            // `weak` came BACK in §8.44, because refcounting made
            // cycles the one thing that leaks and this is how an
            // author breaks one. It changes only the edge count; the
            // stale-awareness it needs is already in every handle.
            TokenKind::Property if unowned => Err(self.err(
                "`unowned` was removed in pixie — a handle cannot dangle, so \
                 non-null non-owning has nothing to add. Use `weak prop` to \
                 break a reference cycle."
                    .to_string(),
            )),
            TokenKind::Property if weak => self
                .property_decl_weak(is_pub, true)
                .map(|p| vec![ClassMember::Property(p)]),
            // Block form: `prop ( ... )` / `let ( ... )` / `var ( ... )`.
            // Sugar for declaring multiple decls of the same kind under
            // a shared header (Go-style). The block header carries the
            // shared modifiers (`is_pub`, `weak`, `unowned`); each item
            // inside is parsed as a single decl body.
            TokenKind::Property if matches!(self.peek_kind(1), TokenKind::LParen) => {
                self.class_member_block(BlockKw::Prop, is_pub, weak, unowned)
            }
            TokenKind::Let if matches!(self.peek_kind(1), TokenKind::LParen) => {
                self.class_member_block(BlockKw::Let, is_pub, weak, unowned)
            }
            TokenKind::Var if matches!(self.peek_kind(1), TokenKind::LParen) => {
                self.class_member_block(BlockKw::Var, is_pub, weak, unowned)
            }
            TokenKind::Property => self
                .property_decl(is_pub)
                .map(|p| vec![ClassMember::Property(p)]),
            TokenKind::Let | TokenKind::Var => {
                let is_mut = matches!(self.peek(), TokenKind::Var);
                self.field_decl(is_pub, is_mut, weak, unowned)
                    .map(|f| vec![ClassMember::Field(f)])
            }
            other if weak || unowned => {
                let mod_kw = if weak { "weak" } else { "unowned" };
                Err(self.err(format!(
                    "`{mod_kw}` modifier only applies to `let` / `var` storage; found {other:?}"
                )))
            }
            TokenKind::Signal => self
                .signal_decl(is_pub)
                .map(|s| vec![ClassMember::Signal(s)]),
            TokenKind::Slot => {
                self.bump(); // `slot`
                let f = self.fn_decl(is_pub)?;
                Ok(vec![ClassMember::Slot(f)])
            }
            TokenKind::Fn | TokenKind::Async | TokenKind::Static => {
                self.fn_decl(is_pub).map(|f| vec![ClassMember::Fn(f)])
            }
            TokenKind::Init => {
                if is_pub {
                    return Err(self.err(
                        "`init` is always callable via `T.new(args)`; remove the `pub` modifier"
                            .to_string(),
                    ));
                }
                self.init_decl().map(|i| vec![ClassMember::Init(i)])
            }
            TokenKind::Deinit => {
                if is_pub {
                    return Err(self.err(
                        "`deinit` cannot be marked `pub` (it runs implicitly on destruction)"
                            .to_string(),
                    ));
                }
                self.deinit_decl().map(|d| vec![ClassMember::Deinit(d)])
            }
            other => Err(self.err(format!(
                "expected class member (prop/let/var/signal/slot/fn/init/deinit), found {other:?}"
            ))),
        }
    }

    /// Parse a `prop ( ... )` / `let ( ... )` / `var ( ... )` block.
    /// Items inside are newline-separated; each item carries the same
    /// per-decl shape it would have at top level (modifiers etc.). The
    /// block header's `is_pub` / `weak` / `unowned` flags apply to every
    /// contained decl; per-decl `pub` is rejected to avoid the
    /// confusing "pub block + non-pub member" trap.
    fn class_member_block(
        &mut self,
        kw: BlockKw,
        is_pub: bool,
        weak: bool,
        unowned: bool,
    ) -> Result<Vec<ClassMember>, ParseError> {
        self.bump(); // `prop` / `let` / `var`
        self.expect(&TokenKind::LParen, "`(` to open block declaration")?;
        self.skip_newlines();
        // Allocate one block id for every item the block expands into.
        // The formatter groups consecutive members with the same id
        // and re-emits them as a `kw ( ... )` block.
        let block_id = self.alloc_block_id();
        let mut members = Vec::new();
        while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
            // Per-item `pub` is rejected — the block header already sets
            // visibility for every item, and a per-item flag would
            // conflict silently if it differed.
            if matches!(self.peek(), TokenKind::Pub) {
                return Err(self.err(format!(
                    "per-item `pub` is not allowed inside a `{}` block — set `pub` on the block header",
                    kw.as_str(),
                )));
            }
            let item_start = self.peek_span();
            match kw {
                BlockKw::Prop => {
                    let mut p = self.property_decl_body(is_pub, item_start)?;
                    p.block_id = Some(block_id);
                    members.push(ClassMember::Property(p));
                }
                BlockKw::Let | BlockKw::Var => {
                    let is_mut = matches!(kw, BlockKw::Var);
                    let mut f = self.field_decl_body(is_pub, is_mut, weak, unowned, item_start)?;
                    f.block_id = Some(block_id);
                    members.push(ClassMember::Field(f));
                }
            }
            self.skip_newlines();
            // Optional trailing comma between items (lenient — blocks
            // are newline-separated by convention but a stray `,` is
            // forgiven so users coming from comma-list languages don't
            // get stuck).
            if matches!(self.peek(), TokenKind::Comma) {
                self.bump();
                self.skip_newlines();
            }
        }
        self.expect(&TokenKind::RParen, "`)` to close block declaration")?;
        if members.is_empty() {
            return Err(self.err(format!(
                "empty `{}` block — declare at least one item, or remove the block",
                kw.as_str(),
            )));
        }
        Ok(members)
    }

    /// Allocate the next monotonic block id. Used by every block
    /// parser (`class_member_block`, `let_or_var_block`) to tag the
    /// items it expands into so the formatter can group them back
    /// into a `kw ( ... )` block.
    fn alloc_block_id(&mut self) -> u32 {
        let id = self.next_block_id;
        self.next_block_id = self.next_block_id.wrapping_add(1);
        id
    }

    /// `let name : T [= expr]` / `var name : T [= expr]` for class-body
    /// plain fields (no Q_PROPERTY, no NOTIFY, no setter/getter synth).
    /// `pub` opts the field into a public C++ getter (and a setter for
    /// `var`). Initializer is optional — when absent, the field is
    /// default-constructed at the C++ level (matching C++ in-class
    /// member-default semantics: `T m_x;`). `weak` / `unowned` propagate
    /// in from the caller's modifier-peek; they're stored on the AST
    /// for the type-check / codegen layers to enforce constraints.
    fn field_decl(
        &mut self,
        is_pub: bool,
        is_mut: bool,
        weak: bool,
        unowned: bool,
    ) -> Result<Field, ParseError> {
        let start = self.peek_span();
        self.bump(); // `let` / `var`
        self.field_decl_body(is_pub, is_mut, weak, unowned, start)
    }

    /// Parse a single field decl assuming the leading `let` / `var`
    /// keyword has already been consumed (used inside `let ( ... )` /
    /// `var ( ... )` blocks).
    fn field_decl_body(
        &mut self,
        is_pub: bool,
        is_mut: bool,
        weak: bool,
        unowned: bool,
        start: Span,
    ) -> Result<Field, ParseError> {
        let name = self.ident()?;
        self.expect(&TokenKind::Colon, "`:` for field type")?;
        let ty = self.type_expr()?;
        let default = if matches!(self.peek(), TokenKind::Eq) {
            self.bump();
            Some(self.expression()?)
        } else {
            None
        };
        let end = self.tokens[self.pos.saturating_sub(1)].span;
        Ok(Field {
            rust_name: None,
            rust_ty: None,
            name,
            ty,
            default,
            is_pub,
            is_mut,
            weak,
            unowned,
            span: start.join(end),
            // Single-decl path. The block path (`class_member_block`)
            // overrides this with the allocated block id after the
            // call returns.
            block_id: None,
        })
    }

    fn init_decl(&mut self) -> Result<InitDecl, ParseError> {
        let start = self.peek_span();
        self.bump(); // `init`
        let params = if matches!(self.peek(), TokenKind::LParen) {
            self.params()?
        } else {
            Vec::new()
        };
        self.skip_newlines();
        let body = self.block()?;
        let end = self.tokens[self.pos.saturating_sub(1)].span;
        Ok(InitDecl {
            params,
            body,
            span: start.join(end),
        })
    }

    fn deinit_decl(&mut self) -> Result<DeinitDecl, ParseError> {
        let start = self.peek_span();
        self.bump(); // `deinit`
        self.skip_newlines();
        let body = self.block()?;
        let end = self.tokens[self.pos.saturating_sub(1)].span;
        Ok(DeinitDecl {
            body,
            span: start.join(end),
        })
    }

    fn property_decl(&mut self, is_pub: bool) -> Result<PropertyDecl, ParseError> {
        self.property_decl_weak(is_pub, false)
    }

    fn property_decl_weak(
        &mut self,
        is_pub: bool,
        weak: bool,
    ) -> Result<PropertyDecl, ParseError> {
        let mut d = self.property_decl_inner(is_pub)?;
        d.weak = weak;
        Ok(d)
    }

    fn property_decl_inner(&mut self, is_pub: bool) -> Result<PropertyDecl, ParseError> {
        let start = self.peek_span();
        self.bump(); // `property`
        self.property_decl_body(is_pub, start)
    }

    /// Parse a single property decl assuming the leading `prop` keyword
    /// has already been consumed (or is absent — used inside `prop ( ... )`
    /// blocks where the kw appears once at the block header).
    fn property_decl_body(
        &mut self,
        is_pub: bool,
        start: Span,
    ) -> Result<PropertyDecl, ParseError> {
        let name = self.ident()?;
        self.expect(&TokenKind::Colon, "`:` for property type")?;
        let parsed_ty = self.type_expr()?;
        // `ModelList<T>` is the surface form of a QRangeModel-backed
        // list. Lift to `List<T>` + `model: true` so the rest of the
        // pipeline keeps the "surface type matches stored data"
        // invariant; codegen wraps storage in `cute::ModelList<T*>*`.
        let (ty, model_from_type) = unwrap_model_list_surface(parsed_ty);
        let mut notify = None;
        let mut default = None;
        let mut bindable = false;
        let mut binding = None;
        let mut fresh = None;
        let mut constant = false;
        let model = model_from_type;
        while matches!(self.peek(), TokenKind::Comma) {
            self.bump();
            // Syntactic shapes after `,`:
            //   `notify: :sym`        — wired notify signal
            //   `default: <expr>`     — initial value
            //   `bindable`            — bare flag, opt-in to QObjectBindableProperty
            //   `bind { <expr> }`     — derived property, auto-tracked via setBinding
            //   `fresh { <expr> }`    — function-style property, no caching, no
            //                            auto-tracking (QObjectComputedProperty)
            //
            // `, model` here is a parse error — the model surface is
            // `prop xs : ModelList<T>` (handled above the loop).
            let key = self.ident()?;
            match key.name.as_str() {
                "bindable" => {
                    bindable = true;
                }
                "constant" => {
                    // `, constant` opts the prop out of the auto-derived
                    // `<propName>Changed` notify. Lowers to a Qt CONSTANT
                    // Q_PROPERTY (no NOTIFY, no signal). Mutually exclusive
                    // with anything that implies a change-event — checked
                    // below alongside the existing combinatorial gates.
                    constant = true;
                }
                "bind" => {
                    self.expect(&TokenKind::LBrace, "`{` after `bind`")?;
                    let expr = self.expression()?;
                    self.expect(&TokenKind::RBrace, "`}` to close `bind` block")?;
                    binding = Some(expr);
                }
                "fresh" => {
                    self.expect(&TokenKind::LBrace, "`{` after `fresh`")?;
                    let expr = self.expression()?;
                    self.expect(&TokenKind::RBrace, "`}` to close `fresh` block")?;
                    fresh = Some(expr);
                }
                "model" => {
                    return Err(self.err(format!(
                        "the `, model` flag was retired — write the prop type as `ModelList<T>` instead (e.g. `prop {0} : ModelList<{0}Item>`)",
                        name.name,
                    )));
                }
                "notify" => {
                    self.expect(&TokenKind::Colon, "`:` after `notify`")?;
                    match self.peek().clone() {
                        TokenKind::Sym(s) => {
                            let span = self.peek_span();
                            self.bump();
                            notify = Some(Ident { name: s, span });
                        }
                        other => {
                            return Err(self.err(format!(
                                "expected `:foo` symbol after `notify:`, got {other:?}"
                            )));
                        }
                    }
                }
                "default" => {
                    self.expect(&TokenKind::Colon, "`:` after `default`")?;
                    default = Some(self.expression()?);
                }
                other => return Err(self.err(format!("unknown property attribute `{other}`"))),
            }
        }
        // Reject incoherent combinations early — diagnostics here are
        // simpler than waiting for HIR to figure it out from the lowered
        // shape.
        if binding.is_some() && fresh.is_some() {
            return Err(self.err(
                "`bind { ... }` and `fresh { ... }` are mutually exclusive on the same property"
                    .to_string(),
            ));
        }
        if binding.is_some() && default.is_some() {
            return Err(self.err(
                "`bind { ... }` and `default:` are mutually exclusive on the same property"
                    .to_string(),
            ));
        }
        if fresh.is_some() && default.is_some() {
            return Err(self.err(
                "`fresh { ... }` and `default:` are mutually exclusive on the same property"
                    .to_string(),
            ));
        }
        if binding.is_some() && notify.is_some() {
            return Err(self.err(
                "derived property (`bind { ... }`) auto-emits its NOTIFY signal; remove `notify:`"
                    .to_string(),
            ));
        }
        if binding.is_some() && bindable {
            return Err(self.err(
                "`bind { ... }` already implies bindable storage; remove the `bindable` flag"
                    .to_string(),
            ));
        }
        if fresh.is_some() && bindable {
            return Err(self.err(
                "`fresh { ... }` is function-style (recomputed per read); the `bindable` flag does not apply"
                    .to_string(),
            ));
        }
        if constant
            && (notify.is_some() || bindable || binding.is_some() || fresh.is_some() || model)
        {
            return Err(self.err(
                "`constant` is mutually exclusive with `notify:` / `bindable` / `bind { ... }` / `fresh { ... }` / `ModelList<T>` — pick one"
                    .to_string(),
            ));
        }
        if model && (binding.is_some() || fresh.is_some() || bindable) {
            // The ModelList<T> wrapper holds storage by value
            // (`QList<T*>&` reference into its private InnerHolder
            // base); bindable / bind / fresh swap that for
            // QObjectBindableProperty / QObjectComputedProperty,
            // which the wrapper can't borrow into.
            return Err(self.err(
                "ModelList<T> v1 requires plain storage; remove `bindable` / `bind { ... }` / `fresh { ... }`"
                    .to_string(),
            ));
        }
        // bare `prop x : T` (no modifier at all) is legal since
        // §8.25: it means "no default — the class's `init` must
        // assign it" (the emitter enforces definite assignment; the
        // shape is what makes `T`-typed props on generic classes
        // expressible at all). `.qpi`/`.rpi` binding files also list
        // bare props for foreign surfaces.
        let end = self.tokens[self.pos.saturating_sub(1)].span;
        Ok(PropertyDecl {
            name,
            ty,
            weak: false,
            notify,
            default,
            is_pub,
            bindable,
            binding,
            fresh,
            model,
            constant,
            span: start.join(end),
            // Single-decl path. The block path overrides this with
            // the allocated block id after the call returns.
            block_id: None,
        })
    }

    fn signal_decl(&mut self, is_pub: bool) -> Result<SignalDecl, ParseError> {
        let start = self.peek_span();
        self.bump(); // `signal`
        let name = self.ident()?;
        let params = if matches!(self.peek(), TokenKind::LParen) {
            self.params()?
        } else {
            Vec::new()
        };
        let end = self.tokens[self.pos.saturating_sub(1)].span;
        Ok(SignalDecl {
            name,
            params,
            is_pub,
            span: start.join(end),
        })
    }

    fn fn_decl(&mut self, is_pub: bool) -> Result<FnDecl, ParseError> {
        let start = self.peek_span();
        // Order: `[pub] [static] [async] fn`. `pub` is already
        // consumed by the caller (it lives at the item level).
        // `static` and `async` are mutually compatible — though the
        // pair is rare in practice (a static async factory).
        let is_static = if matches!(self.peek(), TokenKind::Static) {
            self.bump();
            true
        } else {
            false
        };
        let is_async = if matches!(self.peek(), TokenKind::Async) {
            self.bump();
            true
        } else {
            false
        };
        self.expect(&TokenKind::Fn, "`fn`")?;
        let name = self.ident()?;
        let generics = self.maybe_generics()?;
        let params = if matches!(self.peek(), TokenKind::LParen) {
            self.params()?
        } else {
            Vec::new()
        };
        // Go-style return type: the type sits right after `)` with no
        // separator. `fn open() !File { ... }` for error unions still
        // works (the `!` is the unambiguous prefix). `fn foo()
        // { ... }` is void-returning. Newline / semicolon / EOF after
        // `)` mean a signature without a body (used in trait method
        // declarations etc.).
        let return_ty = if matches!(
            self.peek(),
            TokenKind::LBrace
                | TokenKind::Newline
                | TokenKind::Semicolon
                | TokenKind::Eof
                | TokenKind::AtIdent(_)
        ) {
            None
        } else {
            Some(self.type_expr()?)
        };
        let attributes = self.fn_attributes()?;
        let body = if matches!(self.peek(), TokenKind::LBrace) {
            Some(self.block()?)
        } else {
            None
        };
        let end = self.tokens[self.pos.saturating_sub(1)].span;
        Ok(FnDecl {
            is_async,
            name,
            generics,
            params,
            return_ty,
            body,
            is_pub,
            is_test: false,
            display_name: None,
            attributes,
            is_static,
            span: start.join(end),
        })
    }

    /// Zero or more `@ident` / `@ident(arg, ...)` markers between an
    /// `fn` return type and its body / line terminator. Used on
    /// stdlib `.qpi` declarations (e.g. `@lifted_bool_ok` to flag a
    /// Qt method whose `bool*` ok-out parameter should be lifted to
    /// `Result<T, QtBoolError>` at the call site).
    ///
    /// Inner args are kept as raw source strings here — the
    /// language doesn't yet need typed attribute args.
    fn fn_attributes(&mut self) -> Result<Vec<Attribute>, ParseError> {
        let mut out = Vec::new();
        loop {
            if !matches!(self.peek(), TokenKind::AtIdent(_)) {
                break;
            }
            let tok = self.bump();
            let span_start = tok.span;
            let TokenKind::AtIdent(name) = tok.kind else {
                unreachable!("peek matched AtIdent")
            };
            let name_ident = Ident {
                name,
                span: span_start,
            };
            let mut args = Vec::new();
            if matches!(self.peek(), TokenKind::LParen) {
                self.bump();
                self.skip_newlines();
                while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
                    let arg_start = self.peek_span();
                    let mut depth: i32 = 0;
                    while !matches!(self.peek(), TokenKind::Eof) {
                        match self.peek() {
                            TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => {
                                depth += 1;
                                self.bump();
                            }
                            TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace
                                if depth > 0 =>
                            {
                                depth -= 1;
                                self.bump();
                            }
                            TokenKind::Comma | TokenKind::RParen if depth == 0 => break,
                            _ => {
                                self.bump();
                            }
                        }
                    }
                    let arg_end = self.tokens[self.pos.saturating_sub(1)].span;
                    let arg_span = arg_start.join(arg_end);
                    let raw = self.src[arg_span.range()].trim();
                    if !raw.is_empty() {
                        args.push(raw.to_string());
                    }
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                    self.skip_newlines();
                }
                self.expect(&TokenKind::RParen, "`)` to close attribute args")?;
            }
            let span_end = self.tokens[self.pos.saturating_sub(1)].span;
            out.push(Attribute {
                name: name_ident,
                args,
                span: span_start.join(span_end),
            });
            self.skip_newlines();
        }
        Ok(out)
    }

    fn params(&mut self) -> Result<Vec<Param>, ParseError> {
        self.expect(&TokenKind::LParen, "`(`")?;
        let mut out = Vec::new();
        self.skip_newlines();
        while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
            let p_start = self.peek_span();
            // Optional `escaping` annotation before the param name.
            // Only valid when the param's declared type is `fn(...)`;
            // anything else triggers an error after the type parses.
            let is_escaping = if matches!(self.peek(), TokenKind::Escaping) {
                self.bump();
                true
            } else {
                false
            };
            // Optional `consuming` annotation: the parameter is
            // consumed by move. Read by the linearity analysis
            // (`flow.rs`) that the driver runs on every build.
            let is_consuming = if matches!(self.peek(), TokenKind::Consuming) {
                self.bump();
                true
            } else {
                false
            };
            if is_escaping && is_consuming {
                return Err(self.err(
                    "`escaping` and `consuming` cannot be combined on the same parameter — `escaping` describes a borrowed closure, `consuming` describes a moved-from value".to_string(),
                ));
            }
            let name = self.ident()?;
            self.expect(&TokenKind::Colon, "`:` after parameter name")?;
            let ty = self.type_expr()?;
            if is_escaping && !matches!(ty.kind, TypeKind::Fn { .. }) {
                return Err(self.err(format!(
                    "`escaping` only applies to closure-typed parameters (`fn(...) -> ...`); `{}` is not a function type",
                    name.name,
                )));
            }
            let default = if matches!(self.peek(), TokenKind::Eq) {
                self.bump();
                Some(self.expression()?)
            } else {
                None
            };
            let p_end = self.tokens[self.pos.saturating_sub(1)].span;
            out.push(Param {
                name,
                ty,
                default,
                is_escaping,
                is_consuming,
                span: p_start.join(p_end),
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
            self.skip_newlines();
        }
        self.expect(&TokenKind::RParen, "`)`")?;
        Ok(out)
    }

    /// Speculation hook for the form-(b) generic-instantiation parse
    /// and for explicit-type-args generic-fn calls:
    ///
    /// `Box<Int>.new()`     → typed-receiver method call, returns `Some(Dot)`
    /// `make<Int>(0)`       → generic fn call, returns `Some(LParen)`
    /// otherwise            → returns `None`, treat the `<` as a comparison
    ///
    /// The scan only counts `<`/`>` depth — it does not validate that
    /// the contents are well-formed types. The actual type parse runs
    /// after the speculation commits, and any malformedness surfaces as
    /// a normal parse error.
    fn looks_like_type_args_close(&self) -> Option<TypeArgsClose> {
        if !matches!(self.peek(), TokenKind::Lt) {
            return None;
        }
        let mut depth = 1i32;
        let mut i: usize = 1;
        // Cap the lookahead so a runaway `<` in a long expression
        // doesn't drag the parser into linear-time backtracking.
        let cap: usize = 64;
        while i < cap {
            match self.peek_kind(i) {
                TokenKind::Lt => depth += 1,
                TokenKind::Gt => {
                    depth -= 1;
                    if depth == 0 {
                        return match self.peek_kind(i + 1) {
                            TokenKind::Dot => Some(TypeArgsClose::Dot),
                            TokenKind::LParen => Some(TypeArgsClose::LParen),
                            _ => None,
                        };
                    }
                }
                // Token shapes that can't appear inside a type-arg list
                // — bail to keep the heuristic simple. (`{ ; ) etc.)
                TokenKind::Eof
                | TokenKind::Newline
                | TokenKind::Semicolon
                | TokenKind::LBrace
                | TokenKind::RBrace
                | TokenKind::RParen
                | TokenKind::RBracket
                | TokenKind::FatArrow
                | TokenKind::Eq
                | TokenKind::EqEq
                | TokenKind::NotEq
                | TokenKind::AndAnd
                | TokenKind::OrOr
                | TokenKind::Plus
                | TokenKind::Minus
                | TokenKind::Star
                | TokenKind::Slash
                | TokenKind::Percent
                | TokenKind::PlusEq
                | TokenKind::MinusEq
                | TokenKind::StarEq
                | TokenKind::SlashEq => return None,
                _ => {}
            }
            i += 1;
        }
        None
    }

    fn maybe_generics(&mut self) -> Result<Vec<GenericParam>, ParseError> {
        if !matches!(self.peek(), TokenKind::Lt) {
            return Ok(Vec::new());
        }
        // Disambiguate: `Foo<T, U>` introduces generics only if followed by an
        // ident and either `,`, `>`, or `:` (bound). Otherwise the `<` is a
        // comparison and we leave it alone.
        if !matches!(self.peek_kind(1), TokenKind::Ident(_)) {
            return Ok(Vec::new());
        }
        if !matches!(
            self.peek_kind(2),
            TokenKind::Comma | TokenKind::Gt | TokenKind::Colon
        ) {
            return Ok(Vec::new());
        }
        self.bump(); // `<`
        let mut out = Vec::new();
        loop {
            let name = self.ident()?;
            let start = name.span;
            // Optional bounds: `T: Iterable + Comparable`. Each bound
            // is a bare ident (referencing a trait or interface name).
            // Bounds are recorded; full enforcement in the type checker
            // is follow-up work.
            let mut bounds = Vec::new();
            if self.eat(&TokenKind::Colon) {
                loop {
                    bounds.push(self.ident()?);
                    if !self.eat(&TokenKind::Plus) {
                        break;
                    }
                }
            }
            let end = self.tokens[self.pos.saturating_sub(1)].span;
            out.push(GenericParam {
                name,
                bounds,
                span: start.join(end),
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::Gt, "`>`")?;
        Ok(out)
    }

    // ---- struct / error --------------------------------------------------

    fn struct_decl(&mut self, is_pub: bool) -> Result<StructDecl, ParseError> {
        let start = self.peek_span();
        self.bump();
        let name = self.ident()?;
        let generics = self.maybe_generics()?;
        let is_copyable = self.parse_optional_copyable_annotation()?;
        // `struct Point @rust("geo::Point")` — the Rust type this
        // struct corresponds to, in a `.rpi` (§8.77).
        let rust_path = rust_attr(&self.fn_attributes()?);
        self.skip_newlines();
        self.expect(&TokenKind::LBrace, "`{`")?;
        self.skip_newlines();
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
            let member_pub = self.eat(&TokenKind::Pub);
            if matches!(
                self.peek(),
                TokenKind::Fn | TokenKind::Async | TokenKind::Static
            ) {
                let f = self.fn_decl(member_pub)?;
                methods.push(f);
                self.skip_newlines();
                continue;
            }
            // Field declaration. Cute v0.x onwards requires an explicit
            // `let` (immutable, init-once) or `var` (mutable) keyword on
            // every struct field, mirroring class / arc fields and Swift
            // stored properties. The bare form `name : Type` is rejected
            // with a migration hint — most existing usage was implicitly
            // mutable, so the mechanical rewrite is `name : T = v` →
            // `var name : T = v`. Tighten to `let` when the field is
            // never reassigned.
            //
            // Block form: `let ( ... )` / `var ( ... )` is also accepted
            // here, in case a struct has many homogeneous fields (e.g.
            // a Point with a long list of coordinates).
            let (is_mut, kw_kind) = match self.peek() {
                TokenKind::Let => {
                    self.bump();
                    (false, BlockKw::Let)
                }
                TokenKind::Var => {
                    self.bump();
                    (true, BlockKw::Var)
                }
                other => {
                    return Err(self.err(format!(
                        "struct fields require an explicit `let` (immutable) or `var` (mutable) keyword; bare fields were retired (was: `{name} : T`). Rewrite as `var {name} : T = ...` for the legacy mutable shape, or `let {name} : T` for init-once. Got: {other:?}",
                        name = match self.peek() {
                            TokenKind::Ident(n) => n.as_str(),
                            _ => "<field>",
                        },
                    )));
                }
            };
            // Block form support — `let ( ... )` / `var ( ... )` lets a
            // struct group several fields under one keyword. Same rule
            // as class members: per-item `pub` is rejected (the block
            // header sets visibility for every contained field).
            if matches!(self.peek(), TokenKind::LParen) {
                self.bump(); // `(`
                self.skip_newlines();
                while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
                    if matches!(self.peek(), TokenKind::Pub) {
                        return Err(self.err(format!(
                            "per-item `pub` is not allowed inside a `{}` block — set `pub` on the block header",
                            kw_kind.as_str(),
                        )));
                    }
                    let item_start = self.peek_span();
                    let fname = self.ident()?;
                    self.expect(&TokenKind::Colon, "`:`")?;
                    let ty = self.type_expr()?;
                    let default = if matches!(self.peek(), TokenKind::Eq) {
                        self.bump();
                        Some(self.expression()?)
                    } else {
                        None
                    };
                    let item_end = self.tokens[self.pos.saturating_sub(1)].span;
                    fields.push(Field {
                        rust_name: None,
                        rust_ty: None,
                        name: fname,
                        ty,
                        default,
                        is_pub: member_pub,
                        is_mut,
                        weak: false,
                        unowned: false,
                        span: item_start.join(item_end),
                        // Struct fields don't have a `let ( ... )` /
                        // `var ( ... )` block sugar at the syntax
                        // level (struct field syntax is comma-list,
                        // not block); always None.
                        block_id: None,
                    });
                    self.skip_newlines();
                    if matches!(self.peek(), TokenKind::Comma) {
                        self.bump();
                        self.skip_newlines();
                    }
                }
                self.expect(&TokenKind::RParen, "`)` to close block declaration")?;
                self.skip_newlines();
                self.eat(&TokenKind::Comma);
                self.skip_newlines();
                continue;
            }
            // Single field.
            let f_start = self.peek_span();
            let fname = self.ident()?;
            self.expect(&TokenKind::Colon, "`:`")?;
            let ty = self.type_expr()?;
            let default = if matches!(self.peek(), TokenKind::Eq) {
                self.bump();
                Some(self.expression()?)
            } else {
                None
            };
            // `var x : Float @rust("x")` — this field's half of the
            // correspondence (§8.77), optionally naming the Rust
            // field's type as well (§8.78).
            let (rust_name, rust_ty) = rust_field_attr(&self.fn_attributes()?);
            let f_end = self.tokens[self.pos.saturating_sub(1)].span;
            fields.push(Field {
                name: fname,
                rust_name,
                rust_ty,
                ty,
                default,
                is_pub: member_pub,
                is_mut,
                weak: false,
                unowned: false,
                span: f_start.join(f_end),
                block_id: None,
            });
            self.skip_newlines();
            self.eat(&TokenKind::Comma);
            self.skip_newlines();
        }
        let end = self.peek_span();
        self.expect(&TokenKind::RBrace, "`}`")?;
        Ok(StructDecl {
            name,
            rust_path,
            generics,
            fields,
            methods,
            is_pub,
            is_copyable,
            span: start.join(end),
        })
    }

    /// `error E { foo; bar(reason: String) }` is sugar for an
    /// `enum` decl with the `is_error` flag set. Same parser as
    /// `enum_decl` modulo the leading keyword and the implicit
    /// flag — both forms become `EnumDecl` at the AST level so
    /// HIR / type checker / codegen share a single sum-type
    /// pipeline. The `is_error` flag lets HIR auto-pick the
    /// module's default `!T` err type. Kept as a legacy alias for
    /// existing user code; new code should prefer plain `enum`.
    fn error_decl(&mut self, is_pub: bool) -> Result<EnumDecl, ParseError> {
        let start = self.peek_span();
        self.bump(); // `error`
        let name = self.ident()?;
        self.skip_newlines();
        self.expect(&TokenKind::LBrace, "`{` for error body")?;
        self.skip_newlines();
        let mut variants = Vec::new();
        while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
            let v_start = self.peek_span();
            let v_is_pub = self.eat(&TokenKind::Pub);
            let vname = self.ident()?;
            let mut fields = Vec::new();
            if matches!(self.peek(), TokenKind::LParen) {
                self.bump();
                self.skip_newlines();
                while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
                    let f_start = self.peek_span();
                    let fname = self.ident()?;
                    self.expect(&TokenKind::Colon, "`:`")?;
                    let fty = self.type_expr()?;
                    let f_end = self.tokens[self.pos.saturating_sub(1)].span;
                    fields.push(Field {
                        rust_name: None,
                        rust_ty: None,
                        name: fname,
                        ty: fty,
                        default: None,
                        is_pub: true,
                        is_mut: false,
                        weak: false,
                        unowned: false,
                        span: f_start.join(f_end),
                        block_id: None,
                    });
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                    self.skip_newlines();
                }
                self.expect(&TokenKind::RParen, "`)`")?;
            }
            let v_end = self.tokens[self.pos.saturating_sub(1)].span;
            variants.push(EnumVariant {
                name: vname,
                rust_name: None,
                value: None,
                fields,
                is_pub: v_is_pub,
                span: v_start.join(v_end),
            });
            self.skip_newlines();
            if matches!(self.peek(), TokenKind::Semicolon | TokenKind::Comma) {
                self.bump();
            }
            self.skip_newlines();
        }
        let end = self.peek_span();
        self.expect(&TokenKind::RBrace, "`}` to close error body")?;
        Ok(EnumDecl {
            name,
            rust_path: None,
            variants,
            is_pub,
            is_extern: false,
            is_error: true,
            cpp_namespace: None,
            span: start.join(end),
        })
    }

    /// Parse `enum Name { Variant; Variant = expr; ... }` or
    /// `extern enum Name { ... }` / `extern enum Ns::Name { ... }`.
    /// Variants are separated by `;` / `,` / newlines (any of the
    /// three). Each variant may carry an explicit `= expr` value.
    /// For extern enums the leading name may be C++-namespaced
    /// (`Qt::AlignmentFlag`); the Cute-side type name is the last
    /// segment, and the rest becomes the `cpp_namespace` so codegen
    /// can emit `Qt::AlignLeft` for `AlignmentFlag.AlignLeft`.
    fn enum_decl(&mut self, is_pub: bool, is_extern: bool) -> Result<EnumDecl, ParseError> {
        let start = self.peek_span();
        self.bump(); // `enum`
        // Accept `Name` or `Ns::Name` (or deeper). Only meaningful
        // for `extern enum`; user-side `enum` rejects the
        // namespaced form because it has no C++ counterpart.
        let mut path_segments: Vec<Ident> = vec![self.ident()?];
        while matches!(self.peek(), TokenKind::DoubleColon) {
            self.bump();
            path_segments.push(self.ident()?);
        }
        let name = path_segments.pop().unwrap();
        let cpp_namespace = if path_segments.is_empty() {
            None
        } else {
            if !is_extern {
                return Err(self.err(format!(
                    "namespaced enum name `{}` only allowed with `extern enum`; user-defined enums live in their declaring module",
                    path_segments
                        .iter()
                        .map(|i| i.name.as_str())
                        .collect::<Vec<_>>()
                        .join("::")
                )));
            }
            Some(
                path_segments
                    .iter()
                    .map(|i| i.name.clone())
                    .collect::<Vec<_>>()
                    .join("::"),
            )
        };
        self.skip_newlines();
        // `enum Color @rust("palette::Color")` — the Rust type this
        // enum corresponds to, in a `.rpi` (§8.74).
        let rust_path = rust_attr(&self.fn_attributes()?);
        self.skip_newlines();
        self.expect(&TokenKind::LBrace, "`{` for enum body")?;
        self.skip_newlines();
        let mut variants = Vec::new();
        while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
            let v_start = self.peek_span();
            let v_is_pub = self.eat(&TokenKind::Pub);
            let vname = self.ident()?;
            // Optional payload fields: `Node(left: Tree, right: Tree)`.
            // Mutually exclusive with `= expr`. Same form as
            // `error E { ... }` variants — fields are typed and
            // accessed by name in pattern bodies.
            let mut fields = Vec::new();
            if matches!(self.peek(), TokenKind::LParen) {
                self.bump();
                self.skip_newlines();
                while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
                    let f_start = self.peek_span();
                    let fname = self.ident()?;
                    self.expect(&TokenKind::Colon, "`:`")?;
                    let fty = self.type_expr()?;
                    let f_end = self.tokens[self.pos.saturating_sub(1)].span;
                    fields.push(Field {
                        rust_name: None,
                        rust_ty: None,
                        name: fname,
                        ty: fty,
                        default: None,
                        is_pub: true,
                        is_mut: false,
                        weak: false,
                        unowned: false,
                        span: f_start.join(f_end),
                        block_id: None,
                    });
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                    self.skip_newlines();
                }
                self.expect(&TokenKind::RParen, "`)`")?;
            }
            let value = if !fields.is_empty() {
                None
            } else if self.eat(&TokenKind::Eq) {
                Some(self.expression()?)
            } else {
                None
            };
            // `red @rust("Red")` — this variant's half of the
            // correspondence (§8.74).
            let rust_name = rust_attr(&self.fn_attributes()?);
            let v_end = self.tokens[self.pos.saturating_sub(1)].span;
            variants.push(EnumVariant {
                name: vname,
                rust_name,
                value,
                fields,
                is_pub: v_is_pub,
                span: v_start.join(v_end),
            });
            self.skip_newlines();
            // `;` or `,` between variants is optional; newline alone
            // is enough. Eat one if present.
            if matches!(self.peek(), TokenKind::Semicolon | TokenKind::Comma) {
                self.bump();
            }
            self.skip_newlines();
        }
        let end = self.peek_span();
        self.expect(&TokenKind::RBrace, "`}` to close enum body")?;
        Ok(EnumDecl {
            name,
            rust_path,
            variants,
            is_pub,
            is_extern,
            is_error: false,
            cpp_namespace,
            span: start.join(end),
        })
    }

    /// Parse `flags Name of EnumName` or `extern flags Name of
    /// EnumName`. The `of` connector is the contextual identifier
    /// `of` (lexed as a regular Ident); recognised only here.
    fn flags_decl(&mut self, is_pub: bool, is_extern: bool) -> Result<FlagsDecl, ParseError> {
        let start = self.peek_span();
        self.bump(); // `flags`
        // Same namespaced-name treatment as `enum_decl`: extern
        // flags may be qualified (`Qt::Alignment`) so codegen can
        // emit the proper C++ symbol; user flags stay bare.
        let mut path_segments: Vec<Ident> = vec![self.ident()?];
        while matches!(self.peek(), TokenKind::DoubleColon) {
            self.bump();
            path_segments.push(self.ident()?);
        }
        let name = path_segments.pop().unwrap();
        let cpp_namespace = if path_segments.is_empty() {
            None
        } else {
            if !is_extern {
                return Err(self.err(format!(
                    "namespaced flags name `{}` only allowed with `extern flags`",
                    path_segments
                        .iter()
                        .map(|i| i.name.as_str())
                        .collect::<Vec<_>>()
                        .join("::")
                )));
            }
            Some(
                path_segments
                    .iter()
                    .map(|i| i.name.clone())
                    .collect::<Vec<_>>()
                    .join("::"),
            )
        };
        // Expect contextual `of`.
        match self.peek() {
            TokenKind::Ident(s) if s == "of" => {
                self.bump();
            }
            other => {
                return Err(self.err(format!(
                    "expected `of <EnumName>` after flags name, found {other:?}"
                )));
            }
        }
        let of_name = self.ident()?;
        let end = self.tokens[self.pos.saturating_sub(1)].span;
        Ok(FlagsDecl {
            name,
            of: of_name,
            is_pub,
            is_extern,
            cpp_namespace,
            span: start.join(end),
        })
    }

    // ---- types -----------------------------------------------------------

    fn type_expr(&mut self) -> Result<TypeExpr, ParseError> {
        let start = self.peek_span();
        // Prefix `!T` => error union.
        if matches!(self.peek(), TokenKind::Bang) {
            self.bump();
            let inner = self.type_expr()?;
            let span = start.join(inner.span);
            return Ok(TypeExpr {
                kind: TypeKind::ErrorUnion(Box::new(inner)),
                span,
            });
        }
        let mut t = self.type_atom()?;
        // Postfix `?` for nullable.
        while matches!(self.peek(), TokenKind::Question) {
            let q = self.peek_span();
            self.bump();
            let span = t.span.join(q);
            t = TypeExpr {
                kind: TypeKind::Nullable(Box::new(t)),
                span,
            };
        }
        Ok(t)
    }

    fn type_atom(&mut self) -> Result<TypeExpr, ParseError> {
        let start = self.peek_span();

        // `fn(T1, T2, ...) -> R` function type literal. Parsed inline here
        // because `fn` is a keyword and would otherwise fail `self.ident()`.
        if matches!(self.peek(), TokenKind::Fn) {
            self.bump();
            self.expect(&TokenKind::LParen, "`(` for fn type")?;
            let mut params = Vec::new();
            self.skip_newlines();
            while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
                params.push(self.type_expr()?);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
                self.skip_newlines();
            }
            self.expect(&TokenKind::RParen, "`)` for fn type")?;
            self.expect(&TokenKind::Arrow, "`->` in fn type")?;
            let ret = self.type_expr()?;
            let end = self.tokens[self.pos.saturating_sub(1)].span;
            return Ok(TypeExpr {
                kind: TypeKind::Fn {
                    params,
                    ret: Box::new(ret),
                },
                span: start.join(end),
            });
        }

        let head = self.ident()?;
        let mut path = vec![head];
        while matches!(self.peek(), TokenKind::Dot) {
            self.bump();
            path.push(self.ident()?);
        }
        let mut args = Vec::new();
        // Generic args `<...>`. Since `<` is also Lt, only consume if followed
        // by something type-ish (ident or `!`).
        if matches!(self.peek(), TokenKind::Lt)
            && matches!(self.peek_kind(1), TokenKind::Ident(_) | TokenKind::Bang)
        {
            self.bump();
            loop {
                args.push(self.type_expr()?);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(&TokenKind::Gt, "`>`")?;
        } else if matches!(self.peek(), TokenKind::LParen)
            && path
                .last()
                .map_or(false, |i| i.name == "Future" || i.name == "fn")
        {
            // Allow `Future(T)` / `fn(...)` notation for type expressions.
            self.bump();
            self.skip_newlines();
            while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
                args.push(self.type_expr()?);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
                self.skip_newlines();
            }
            self.expect(&TokenKind::RParen, "`)` for type argument list")?;
        }
        let end = self.tokens[self.pos.saturating_sub(1)].span;
        Ok(TypeExpr {
            kind: TypeKind::Named { path, args },
            span: start.join(end),
        })
    }

    // ---- statements & blocks --------------------------------------------

    fn block(&mut self) -> Result<Block, ParseError> {
        let start = self.peek_span();
        self.expect(&TokenKind::LBrace, "`{`")?;
        self.skip_newlines();
        let mut stmts = Vec::new();
        let mut trailing: Option<Box<Expr>> = None;
        while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
            let mut new_stmts = self.statement()?;
            self.skip_newlines();
            // The very last expression-statement before `}` becomes
            // the block's trailing value. Block-form `let ( ... )` /
            // `var ( ... )` always returns Stmt::Let / Stmt::Var (no
            // trailing-expression case to consider).
            let last = new_stmts.pop();
            stmts.extend(new_stmts);
            if let Some(last_stmt) = last {
                if matches!(self.peek(), TokenKind::RBrace) {
                    if let Stmt::Expr(e) = last_stmt {
                        trailing = Some(Box::new(e));
                        break;
                    } else {
                        stmts.push(last_stmt);
                    }
                } else {
                    stmts.push(last_stmt);
                }
            }
        }
        let end = self.peek_span();
        self.expect(&TokenKind::RBrace, "`}`")?;
        Ok(Block {
            stmts,
            trailing,
            span: start.join(end),
        })
    }

    /// Parse one or more statements. The common case returns a one-
    /// element Vec; the `let ( ... )` / `var ( ... )` block form
    /// returns N elements, one per binding inside the block.
    fn statement(&mut self) -> Result<Vec<Stmt>, ParseError> {
        match self.peek() {
            // `let ( ... )` / `var ( ... )` block — group several local
            // bindings under one keyword. Mirrors class member block
            // form. No `pub` / `weak` / `unowned` allowed (statement
            // scope is implicit + per-binding modifiers don't apply).
            TokenKind::Let if matches!(self.peek_kind(1), TokenKind::LParen) => {
                self.let_or_var_block(false)
            }
            TokenKind::Var if matches!(self.peek_kind(1), TokenKind::LParen) => {
                self.let_or_var_block(true)
            }
            TokenKind::Let | TokenKind::Var => self.let_or_var().map(|s| vec![s]),
            TokenKind::Return => self.return_stmt().map(|s| vec![s]),
            TokenKind::Emit => self.emit_stmt().map(|s| vec![s]),
            TokenKind::For => self.for_stmt().map(|s| vec![s]),
            TokenKind::While => self.while_stmt().map(|s| vec![s]),
            TokenKind::Break => {
                let span = self.peek_span();
                self.bump();
                Ok(vec![Stmt::Break { span }])
            }
            TokenKind::Continue => {
                let span = self.peek_span();
                self.bump();
                Ok(vec![Stmt::Continue { span }])
            }
            TokenKind::Batch => {
                let start = self.peek_span();
                self.bump(); // `batch`
                self.skip_newlines();
                let body = self.block()?;
                let end = body.span;
                Ok(vec![Stmt::Batch {
                    body,
                    span: start.join(end),
                }])
            }
            _ => {
                let lhs = self.expression()?;
                // Possibly `lhs = rhs` / `lhs += rhs` etc.
                if let Some(op) = assign_op(self.peek()) {
                    let op_span = self.peek_span();
                    self.bump();
                    let rhs = self.expression()?;
                    let span = lhs.span.join(rhs.span);
                    return Ok(vec![Stmt::Assign {
                        target: lhs,
                        op,
                        value: rhs,
                        span: span.join(op_span),
                    }]);
                }
                Ok(vec![Stmt::Expr(lhs)])
            }
        }
    }

    /// Parse a `let ( ... )` or `var ( ... )` block of local bindings.
    /// Each item inside has the same shape as a standalone `let` /
    /// `var` (`name [: Type] = value`). Newlines separate items; a
    /// trailing comma between items is forgiven.
    fn let_or_var_block(&mut self, is_var: bool) -> Result<Vec<Stmt>, ParseError> {
        let kw_str = if is_var { "var" } else { "let" };
        self.bump(); // `let` / `var`
        self.expect(&TokenKind::LParen, "`(` to open block declaration")?;
        self.skip_newlines();
        // One block id per `let ( ... )` / `var ( ... )` block; the
        // formatter groups consecutive Stmt::Let / Stmt::Var with the
        // same id and re-emits them as a block.
        let block_id = self.alloc_block_id();
        let mut stmts = Vec::new();
        while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
            let item_start = self.peek_span();
            let name = self.ident()?;
            let ty = if matches!(self.peek(), TokenKind::Colon) {
                self.bump();
                Some(self.type_expr()?)
            } else {
                None
            };
            self.expect(&TokenKind::Eq, "`=` in let/var binding inside block")?;
            let value = self.expression()?;
            let span = item_start.join(value.span);
            stmts.push(if is_var {
                Stmt::Var {
                    name,
                    ty,
                    value,
                    span,
                    block_id: Some(block_id),
                }
            } else {
                Stmt::Let {
                    name,
                    ty,
                    value,
                    span,
                    block_id: Some(block_id),
                }
            });
            self.skip_newlines();
            if matches!(self.peek(), TokenKind::Comma) {
                self.bump();
                self.skip_newlines();
            }
        }
        self.expect(&TokenKind::RParen, "`)` to close block declaration")?;
        if stmts.is_empty() {
            return Err(self.err(format!(
                "empty `{kw_str}` block — declare at least one binding, or remove the block",
            )));
        }
        Ok(stmts)
    }

    fn while_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start = self.peek_span();
        self.bump(); // `while`
        // Same trailing-block disable trick the for-stmt uses so `cond { }`
        // parses as `cond` + `{}` body, not `cond { }` trailing-block call.
        let prev = std::mem::replace(&mut self.disable_trailing_block, true);
        let cond = self.expression()?;
        self.disable_trailing_block = prev;
        self.skip_newlines();
        let body = self.block()?;
        let end = body.span;
        Ok(Stmt::While {
            cond,
            body,
            span: start.join(end),
        })
    }

    fn for_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start = self.peek_span();
        self.bump(); // `for`
        let binding = self.ident()?;
        let index = self.for_index()?;
        // `in` is contextual (unreserved at the lexer); recognize the
        // string form of the next ident.
        match self.peek() {
            TokenKind::Ident(s) if s == "in" => {
                self.bump();
            }
            other => {
                return Err(self.err(format!(
                    "expected `in` after for-binding, found {:?}",
                    other
                )));
            }
        }
        let prev = std::mem::replace(&mut self.disable_trailing_block, true);
        let iter = self.expression()?;
        self.disable_trailing_block = prev;
        self.skip_newlines();
        let body = self.block()?;
        let end = body.span;
        Ok(Stmt::For {
            binding,
            index,
            iter,
            body,
            span: start.join(end),
        })
    }

    /// `for x, i in xs` — the optional second binding, the row's
    /// index. Written after the row so the one-binding form reads the
    /// same in both.
    fn for_index(&mut self) -> Result<Option<Ident>, ParseError> {
        if matches!(self.peek(), TokenKind::Comma) {
            self.bump();
            return Ok(Some(self.ident()?));
        }
        Ok(None)
    }

    fn let_or_var(&mut self) -> Result<Stmt, ParseError> {
        let start = self.peek_span();
        let is_var = matches!(self.peek(), TokenKind::Var);
        self.bump();
        let name = self.ident()?;
        let ty = if matches!(self.peek(), TokenKind::Colon) {
            self.bump();
            Some(self.type_expr()?)
        } else {
            None
        };
        self.expect(&TokenKind::Eq, "`=` in let/var binding")?;
        let value = self.expression()?;
        let span = start.join(value.span);
        Ok(if is_var {
            Stmt::Var {
                name,
                ty,
                value,
                span,
                block_id: None,
            }
        } else {
            Stmt::Let {
                name,
                ty,
                value,
                span,
                block_id: None,
            }
        })
    }

    fn return_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start = self.peek_span();
        self.bump();
        let value = if self.at_stmt_end() {
            None
        } else {
            Some(self.expression()?)
        };
        let end = value.as_ref().map(|e| e.span).unwrap_or(start);
        Ok(Stmt::Return {
            value,
            span: start.join(end),
        })
    }

    fn emit_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start = self.peek_span();
        self.bump();
        let signal = self.ident()?;
        let mut args = Vec::new();
        if matches!(self.peek(), TokenKind::LParen) {
            self.bump();
            self.skip_newlines();
            while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
                args.push(self.expression()?);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
                self.skip_newlines();
            }
            self.expect(&TokenKind::RParen, "`)`")?;
        }
        let end = self.tokens[self.pos.saturating_sub(1)].span;
        Ok(Stmt::Emit {
            signal,
            args,
            span: start.join(end),
        })
    }

    // ---- expressions -----------------------------------------------------

    fn expression(&mut self) -> Result<Expr, ParseError> {
        let lhs = self.expr_or(0)?;
        // Range is the lowest-precedence binary form; only valid in
        // contexts that consume an Expr (for-iter, plain expression
        // statement). v1 doesn't compose ranges with other operators.
        match self.peek() {
            TokenKind::DotDot | TokenKind::DotDotEq => {
                let inclusive = matches!(self.peek(), TokenKind::DotDotEq);
                self.bump();
                let rhs = self.expr_or(0)?;
                let span = lhs.span.join(rhs.span);
                Ok(Expr {
                    kind: ExprKind::Range {
                        start: Box::new(lhs),
                        end: Box::new(rhs),
                        inclusive,
                    },
                    span,
                })
            }
            _ => Ok(lhs),
        }
    }

    /// Convert lex-time `Vec<StrSeg>` (with `Interp(span)` placeholders)
    /// into AST `Vec<StrPart>` by re-parsing each interp slice.
    fn lift_str_segments(&self, segs: Vec<StrSeg>) -> Result<Vec<StrPart>, ParseError> {
        let mut out = Vec::with_capacity(segs.len());
        for seg in segs {
            match seg {
                StrSeg::Text(t) => out.push(StrPart::Text(t)),
                StrSeg::Interp(span) => {
                    let inner = parse_expression_at(self.file, self.src, span)?;
                    out.push(StrPart::Interp(Box::new(inner)));
                }
                StrSeg::InterpFmt { span, format_spec } => {
                    let inner = parse_expression_at(self.file, self.src, span)?;
                    out.push(StrPart::InterpFmt {
                        expr: Box::new(inner),
                        format_spec,
                    });
                }
            }
        }
        Ok(out)
    }

    /// Parse an expression where a following `{...}` should NOT be eaten as a
    /// trailing-block. Used for the head of `case`/`if`/`while` so the body's
    /// braces are reachable.
    fn expr_no_block(&mut self) -> Result<Expr, ParseError> {
        let prev = std::mem::replace(&mut self.disable_trailing_block, true);
        let r = self.expr_or(0);
        self.disable_trailing_block = prev;
        r
    }

    fn expr_or(&mut self, min_prec: u8) -> Result<Expr, ParseError> {
        let mut lhs = self.expr_unary()?;
        loop {
            let Some((op, prec)) = bin_op_prec(self.peek()) else {
                break;
            };
            if prec < min_prec {
                break;
            }
            self.bump();
            let rhs = self.expr_or(prec + 1)?;
            let span = lhs.span.join(rhs.span);
            lhs = Expr {
                kind: ExprKind::Binary {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
                span,
            };
        }
        Ok(lhs)
    }

    fn expr_unary(&mut self) -> Result<Expr, ParseError> {
        let start = self.peek_span();
        match self.peek() {
            TokenKind::Minus => {
                self.bump();
                let inner = self.expr_unary()?;
                let span = start.join(inner.span);
                Ok(Expr {
                    kind: ExprKind::Unary {
                        op: UnaryOp::Neg,
                        expr: Box::new(inner),
                    },
                    span,
                })
            }
            TokenKind::Bang => {
                self.bump();
                let inner = self.expr_unary()?;
                let span = start.join(inner.span);
                Ok(Expr {
                    kind: ExprKind::Unary {
                        op: UnaryOp::Not,
                        expr: Box::new(inner),
                    },
                    span,
                })
            }
            TokenKind::Await => {
                self.bump();
                let inner = self.expr_unary()?;
                let span = start.join(inner.span);
                Ok(Expr {
                    kind: ExprKind::Await(Box::new(inner)),
                    span,
                })
            }
            TokenKind::Try => {
                // `try expr` is the only Try form (Zig-style). It
                // unwraps an `!T` value, returning the success or
                // early-returning the error from the surrounding
                // `!U` function. Postfix `?` was retired so that
                // `expr?.member` unambiguously means SafeMember
                // (see `expr_postfix`'s `Question` arm).
                self.bump();
                let inner = self.expr_unary()?;
                let span = start.join(inner.span);
                Ok(Expr {
                    kind: ExprKind::Try(Box::new(inner)),
                    span,
                })
            }
            _ => self.expr_postfix(),
        }
    }

    fn expr_postfix(&mut self) -> Result<Expr, ParseError> {
        let mut e = self.expr_atom()?;
        loop {
            match self.peek() {
                TokenKind::Dot => {
                    self.bump();
                    let name = self.ident()?;
                    let span = e.span.join(name.span);
                    if matches!(self.peek(), TokenKind::LParen) {
                        let args = self.call_args()?;
                        let block = self.maybe_trailing_block()?;
                        let end = block
                            .as_ref()
                            .map(|b| b.span)
                            .unwrap_or(self.tokens[self.pos.saturating_sub(1)].span);
                        let span = e.span.join(end);
                        e = Expr {
                            kind: ExprKind::MethodCall {
                                receiver: Box::new(e),
                                method: name,
                                args,
                                block,
                                type_args: Vec::new(),
                            },
                            span,
                        };
                    } else {
                        e = Expr {
                            kind: ExprKind::Member {
                                receiver: Box::new(e),
                                name,
                            },
                            span,
                        };
                    }
                }
                // Generic-class instantiation `Ident<types>.method(args)`
                // or generic-fn explicit type-args `Ident<types>(args)`.
                // Distinguished from a comparison `Ident < expr` by
                // requiring a balanced `<...>` followed by `.` or `(`.
                TokenKind::Lt if matches!(e.kind, ExprKind::Ident(_)) => {
                    let Some(close) = self.looks_like_type_args_close() else {
                        break;
                    };
                    self.bump(); // `<`
                    let mut type_args: Vec<TypeExpr> = Vec::new();
                    loop {
                        type_args.push(self.type_expr()?);
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect(&TokenKind::Gt, "`>`")?;
                    match close {
                        TypeArgsClose::Dot => {
                            self.expect(&TokenKind::Dot, "`.`")?;
                            let name = self.ident()?;
                            // Re-create the call shape: type-args attach
                            // to the method call, not to the receiver Ident.
                            let args = self.call_args()?;
                            let block = self.maybe_trailing_block()?;
                            let end = block
                                .as_ref()
                                .map(|b| b.span)
                                .unwrap_or(self.tokens[self.pos.saturating_sub(1)].span);
                            let span = e.span.join(end);
                            e = Expr {
                                kind: ExprKind::MethodCall {
                                    receiver: Box::new(e),
                                    method: name,
                                    args,
                                    block,
                                    type_args,
                                },
                                span,
                            };
                        }
                        TypeArgsClose::LParen => {
                            // Explicit type args on a generic fn call:
                            // `make<Int>(0)`. Currently the type-checker's
                            // unification path infers T anyway, so the
                            // explicit args are mostly a hint for human
                            // readers + a guard against ambiguous
                            // inference. We carry them as `type_args` on
                            // a Call to keep them around for future use
                            // (codegen could emit `make<qint64>(...)`).
                            let args = self.call_args()?;
                            let block = self.maybe_trailing_block()?;
                            let end = block
                                .as_ref()
                                .map(|b| b.span)
                                .unwrap_or(self.tokens[self.pos.saturating_sub(1)].span);
                            let span = e.span.join(end);
                            e = Expr {
                                kind: ExprKind::Call {
                                    callee: Box::new(e),
                                    args,
                                    block,
                                    type_args,
                                },
                                span,
                            };
                        }
                    }
                }
                TokenKind::LParen => {
                    let args = self.call_args()?;
                    let block = self.maybe_trailing_block()?;
                    let end = block
                        .as_ref()
                        .map(|b| b.span)
                        .unwrap_or(self.tokens[self.pos.saturating_sub(1)].span);
                    let span = e.span.join(end);
                    e = Expr {
                        kind: ExprKind::Call {
                            callee: Box::new(e),
                            args,
                            block,
                            type_args: Vec::new(),
                        },
                        span,
                    };
                }
                TokenKind::LBracket => {
                    self.bump();
                    let idx = self.expression()?;
                    let close = self.expect(&TokenKind::RBracket, "`]`")?;
                    let span = e.span.join(close.span);
                    e = Expr {
                        kind: ExprKind::Index {
                            receiver: Box::new(e),
                            index: Box::new(idx),
                        },
                        span,
                    };
                }
                // `?.` is the null-safe member / method access. Detect
                // by peeking past the `?` for a `.`. `?` alone (followed
                // by anything else) is the postfix Try / error-propagate
                // operator handled in the next arm.
                TokenKind::Question if matches!(self.peek_kind(1), TokenKind::Dot) => {
                    self.bump(); // `?`
                    self.bump(); // `.`
                    let name = self.ident()?;
                    if matches!(self.peek(), TokenKind::LParen) {
                        let args = self.call_args()?;
                        let block = self.maybe_trailing_block()?;
                        let end = block
                            .as_ref()
                            .map(|b| b.span)
                            .unwrap_or(self.tokens[self.pos.saturating_sub(1)].span);
                        let span = e.span.join(end);
                        e = Expr {
                            kind: ExprKind::SafeMethodCall {
                                receiver: Box::new(e),
                                method: name,
                                args,
                                block,
                                type_args: Vec::new(),
                            },
                            span,
                        };
                    } else {
                        let span = e.span.join(name.span);
                        e = Expr {
                            kind: ExprKind::SafeMember {
                                receiver: Box::new(e),
                                name,
                            },
                            span,
                        };
                    }
                }
                // Bare postfix `?` after an expression is no longer
                // accepted as the Try shorthand. The token is reserved
                // for `?.` (null-safe member, handled in the arm
                // above) and `T?` (nullable type suffix, handled by
                // the type parser). For error propagation, use the
                // prefix `try expr` form. This rule is what disambiguates
                // `expr?.member`: it can ONLY mean SafeMember now;
                // there is no second interpretation as Try followed
                // by `.member`.
                TokenKind::Question => {
                    return Err(self.err(
                        "postfix `?` is no longer a Try shorthand; \
                         use `try expr` for error propagation, or \
                         `expr?.member` for null-safe member access",
                    ));
                }
                TokenKind::LBrace
                    if !self.disable_trailing_block && can_take_trailing_block(&e) =>
                {
                    // `foo { |x| ... }` / `foo { stmts }` trailing block.
                    let block = self.block_or_lambda()?;
                    let span = e.span.join(block.span);
                    e = Expr {
                        kind: ExprKind::Call {
                            callee: Box::new(e),
                            args: Vec::new(),
                            block: Some(Box::new(block)),
                            type_args: Vec::new(),
                        },
                        span,
                    };
                }
                _ => break,
            }
        }
        Ok(e)
    }

    fn expr_atom(&mut self) -> Result<Expr, ParseError> {
        let span = self.peek_span();
        match self.peek().clone() {
            TokenKind::Int(v) => {
                self.bump();
                Ok(Expr {
                    kind: ExprKind::Int(v),
                    span,
                })
            }
            TokenKind::Float(v) => {
                self.bump();
                Ok(Expr {
                    kind: ExprKind::Float(v),
                    span,
                })
            }
            TokenKind::Bool(v) => {
                self.bump();
                Ok(Expr {
                    kind: ExprKind::Bool(v),
                    span,
                })
            }
            TokenKind::Nil => {
                self.bump();
                Ok(Expr {
                    kind: ExprKind::Nil,
                    span,
                })
            }
            TokenKind::Str(segs) => {
                self.bump();
                let parts = self.lift_str_segments(segs)?;
                Ok(Expr {
                    kind: ExprKind::Str(parts),
                    span,
                })
            }
            TokenKind::Sym(s) => {
                self.bump();
                Ok(Expr {
                    kind: ExprKind::Sym(s),
                    span,
                })
            }
            TokenKind::AtIdent(name) => {
                Err(self.err(format!(
                    "the `@` prefix on `@{name}` was retired — write the bare member name `{name}` (class-member resolution is automatic in method bodies)"
                )))
            }
            TokenKind::Self_ => {
                self.bump();
                Ok(Expr {
                    kind: ExprKind::SelfRef,
                    span,
                })
            }
            TokenKind::Ident(name) => {
                self.bump();
                // Keyword-arg shorthand: `key: value` only valid inside call-arg
                // context; the call_args parser detects this. Here we just
                // produce an Ident.
                Ok(Expr {
                    kind: ExprKind::Ident(name),
                    span,
                })
            }
            TokenKind::LParen => {
                let lparen_span = self.peek_span();
                self.bump();
                self.skip_newlines();
                let mut e = self.expression()?;
                self.skip_newlines();
                let rparen_tok = self.expect(&TokenKind::RParen, "`)`")?;
                // Cute strips paren wrappers (no Paren variant in
                // ExprKind), so the inner expression's span ends at the
                // last inner token. Extend it to cover the parens
                // themselves so source-text fallbacks (the formatter's
                // `write_span_verbatim` for `bind { (a / b) * c }`,
                // `default:` exprs, `fresh { ... }`) don't drop the
                // parens and emit a syntactically broken substring.
                e.span = e.span.join(lparen_span).join(rparen_tok.span);
                Ok(e)
            }
            TokenKind::LBrace => {
                // `{` at expression position is either a map literal
                // (`{ key: value, ... }`), a block (`{ stmts; trailing }`)
                // or a lambda (`{ |x| ... }`). Disambiguate by the first
                // non-newline token: a map literal must start with
                // `<key>: <value>` where the key is an identifier or
                // a string literal (and the next non-newline token is `:`).
                if self.looks_like_map_literal() {
                    self.parse_map_literal()
                } else {
                    self.block_or_lambda()
                }
            }
            TokenKind::LBracket => self.parse_array_literal(),
            TokenKind::Case => self.case_expr(),
            TokenKind::If => self.if_expr(),
            other => Err(self.err(format!("expected expression, found {other:?}"))),
        }
    }

    /// True when the upcoming `{ ... }` should parse as a map literal:
    /// the first non-newline token is an identifier or string literal,
    /// and the token after it (also skipping newlines) is `:`. Anything
    /// else (e.g. `{ |x| ... }`, `{ let x = 1 }`, `{ if cond { ... } }`)
    /// stays a block/lambda.
    fn looks_like_map_literal(&self) -> bool {
        // Walk the lookahead skipping newlines so we don't get tripped
        // up by formatted input. We need the FIRST non-newline token and
        // the SECOND one (also skipping newlines after the first).
        let mut i = 1usize; // skip the LBrace itself
        while matches!(self.peek_kind(i), TokenKind::Newline) {
            i += 1;
        }
        // `{}` — the EMPTY map (§8.68). It is also, syntactically, an
        // empty block, but a block that produces nothing is not a
        // thing anyone writes in expression position, while an empty
        // map is what `[]` is for a list.
        if matches!(self.peek_kind(i), TokenKind::RBrace) {
            return true;
        }
        if !matches!(self.peek_kind(i), TokenKind::Ident(_) | TokenKind::Str(_)) {
            return false;
        }
        i += 1;
        while matches!(self.peek_kind(i), TokenKind::Newline) {
            i += 1;
        }
        matches!(self.peek_kind(i), TokenKind::Colon)
    }

    fn parse_array_literal(&mut self) -> Result<Expr, ParseError> {
        let start = self.peek_span();
        self.bump(); // `[`
        self.skip_newlines();
        let mut items = Vec::new();
        while !matches!(self.peek(), TokenKind::RBracket | TokenKind::Eof) {
            items.push(self.expression()?);
            self.skip_newlines();
            if !self.eat(&TokenKind::Comma) {
                break;
            }
            self.skip_newlines();
        }
        let end = self.peek_span();
        self.expect(&TokenKind::RBracket, "`]` to close array literal")?;
        Ok(Expr {
            kind: ExprKind::Array(items),
            span: start.join(end),
        })
    }

    fn parse_map_literal(&mut self) -> Result<Expr, ParseError> {
        let start = self.peek_span();
        self.bump(); // `{`
        self.skip_newlines();
        let mut entries: Vec<(Expr, Expr)> = Vec::new();
        while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
            let key_span = self.peek_span();
            let key = match self.peek().clone() {
                TokenKind::Str(segs) => {
                    self.bump();
                    let parts = self.lift_str_segments(segs)?;
                    Expr {
                        kind: ExprKind::Str(parts),
                        span: key_span,
                    }
                }
                TokenKind::Ident(name) => {
                    self.bump();
                    Expr {
                        kind: ExprKind::Ident(name),
                        span: key_span,
                    }
                }
                other => {
                    return Err(self.err(format!(
                        "expected map-literal key (identifier or string), found {:?}",
                        other
                    )));
                }
            };
            self.skip_newlines();
            self.expect(&TokenKind::Colon, "`:` after map-literal key")?;
            self.skip_newlines();
            let value = self.expression()?;
            entries.push((key, value));
            self.skip_newlines();
            if !self.eat(&TokenKind::Comma) {
                break;
            }
            self.skip_newlines();
        }
        let end = self.peek_span();
        self.expect(&TokenKind::RBrace, "`}` to close map literal")?;
        Ok(Expr {
            kind: ExprKind::Map(entries),
            span: start.join(end),
        })
    }

    fn call_args(&mut self) -> Result<Vec<Expr>, ParseError> {
        self.expect(&TokenKind::LParen, "`(`")?;
        self.skip_newlines();
        let mut args = Vec::new();
        while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
            // Detect `key: value` keyword-arg shorthand. The key is
            // normally a bare identifier, but a few real keywords
            // (`view`, ...) double as common kwarg names in builtin
            // intrinsics like `qml_app(view: Main, ...)`. Accept those
            // here as identifiers in this position only.
            let key_is_ident = matches!(self.peek(), TokenKind::Ident(_) | TokenKind::View);
            if key_is_ident
                && matches!(self.peek_kind(1), TokenKind::Colon)
                && !matches!(self.peek_kind(2), TokenKind::Newline | TokenKind::RParen)
            {
                let kspan = self.peek_span();
                let key_name = match self.peek() {
                    TokenKind::Ident(s) => s.clone(),
                    TokenKind::View => "view".to_string(),
                    _ => unreachable!(),
                };
                self.bump(); // key token
                let key = Ident {
                    name: key_name,
                    span: kspan,
                };
                self.bump(); // `:`
                let value = self.expression()?;
                let span = kspan.join(value.span);
                args.push(Expr {
                    kind: ExprKind::Kwarg {
                        key,
                        value: Box::new(value),
                    },
                    span,
                });
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
                self.skip_newlines();
                continue;
            }
            args.push(self.expression()?);
            if !self.eat(&TokenKind::Comma) {
                break;
            }
            self.skip_newlines();
        }
        // Multi-line call with no trailing comma: the last newline before
        // `)` would otherwise leave `peek = Newline` and break expect.
        self.skip_newlines();
        self.expect(&TokenKind::RParen, "`)`")?;
        Ok(args)
    }

    fn maybe_trailing_block(&mut self) -> Result<Option<Box<Expr>>, ParseError> {
        if !self.disable_trailing_block && matches!(self.peek(), TokenKind::LBrace) {
            let b = self.block_or_lambda()?;
            Ok(Some(Box::new(b)))
        } else {
            Ok(None)
        }
    }

    /// `{ |x, y| body }` (lambda) or `{ stmts }` (plain block).
    fn block_or_lambda(&mut self) -> Result<Expr, ParseError> {
        let start = self.peek_span();
        self.expect(&TokenKind::LBrace, "`{`")?;
        self.skip_newlines();
        if matches!(self.peek(), TokenKind::Pipe) {
            // Block-arg list `|x, y|`.
            self.bump();
            let mut params = Vec::new();
            while !matches!(self.peek(), TokenKind::Pipe | TokenKind::Eof) {
                let p_start = self.peek_span();
                let name = self.ident()?;
                let ty = if matches!(self.peek(), TokenKind::Colon) {
                    self.bump();
                    self.type_expr()?
                } else {
                    // Untyped block param - placeholder type, resolved later.
                    TypeExpr {
                        kind: TypeKind::Named {
                            path: vec![Ident {
                                name: "_".into(),
                                span: p_start,
                            }],
                            args: vec![],
                        },
                        span: p_start,
                    }
                };
                let p_end = self.tokens[self.pos.saturating_sub(1)].span;
                params.push(Param {
                    name,
                    ty,
                    default: None,
                    is_escaping: false,
                    is_consuming: false,
                    span: p_start.join(p_end),
                });
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(&TokenKind::Pipe, "closing `|`")?;
            self.skip_newlines();
            let mut stmts = Vec::new();
            let mut trailing = None;
            while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
                let mut new_stmts = self.statement()?;
                self.skip_newlines();
                let last = new_stmts.pop();
                stmts.extend(new_stmts);
                if let Some(last_stmt) = last {
                    if matches!(self.peek(), TokenKind::RBrace) {
                        if let Stmt::Expr(e) = last_stmt {
                            trailing = Some(Box::new(e));
                            break;
                        } else {
                            stmts.push(last_stmt);
                        }
                    } else {
                        stmts.push(last_stmt);
                    }
                }
            }
            let end = self.peek_span();
            self.expect(&TokenKind::RBrace, "`}`")?;
            let span = start.join(end);
            Ok(Expr {
                kind: ExprKind::Lambda {
                    params,
                    body: Block {
                        stmts,
                        trailing,
                        span,
                    },
                },
                span,
            })
        } else {
            // Plain block.
            let mut stmts = Vec::new();
            let mut trailing = None;
            while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
                let mut new_stmts = self.statement()?;
                self.skip_newlines();
                let last = new_stmts.pop();
                stmts.extend(new_stmts);
                if let Some(last_stmt) = last {
                    if matches!(self.peek(), TokenKind::RBrace) {
                        if let Stmt::Expr(e) = last_stmt {
                            trailing = Some(Box::new(e));
                            break;
                        } else {
                            stmts.push(last_stmt);
                        }
                    } else {
                        stmts.push(last_stmt);
                    }
                }
            }
            let end = self.peek_span();
            self.expect(&TokenKind::RBrace, "`}`")?;
            let span = start.join(end);
            Ok(Expr {
                kind: ExprKind::Block(Block {
                    stmts,
                    trailing,
                    span,
                }),
                span,
            })
        }
    }

    fn case_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.peek_span();
        self.bump(); // `case`
        let scrutinee = self.expr_no_block()?;
        self.skip_newlines();
        self.expect(&TokenKind::LBrace, "`{` for case arms")?;
        self.skip_newlines();
        let mut arms = Vec::new();
        while matches!(self.peek(), TokenKind::When) {
            let arm_start = self.peek_span();
            self.bump(); // `when`
            let pattern = self.pattern()?;
            self.skip_newlines();
            let body = self.block()?;
            let arm_end = body.span;
            arms.push(CaseArm {
                pattern,
                body,
                span: arm_start.join(arm_end),
            });
            self.skip_newlines();
        }
        let end = self.peek_span();
        self.expect(&TokenKind::RBrace, "`}` to close case")?;
        Ok(Expr {
            kind: ExprKind::Case {
                scrutinee: Box::new(scrutinee),
                arms,
            },
            span: start.join(end),
        })
    }

    fn pattern(&mut self) -> Result<Pattern, ParseError> {
        let span = self.peek_span();
        match self.peek().clone() {
            TokenKind::Ident(name) if name == "_" => {
                self.bump();
                Ok(Pattern::Wild { span })
            }
            TokenKind::Ident(name) => {
                let head = self.bump();
                let head_id = Ident {
                    name,
                    span: head.span,
                };
                if matches!(self.peek(), TokenKind::LParen) {
                    self.bump();
                    self.skip_newlines();
                    let mut args = Vec::new();
                    while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
                        // Inside a pattern, a bare identifier is a binding;
                        // an identifier followed by `(` is a nested
                        // constructor pattern (e.g. `err(IoError(msg))`
                        // matches a payload variant of an error/enum).
                        if let TokenKind::Ident(n) = self.peek().clone() {
                            if matches!(self.peek_kind(1), TokenKind::LParen) {
                                args.push(self.pattern()?);
                            } else {
                                let bspan = self.peek_span();
                                self.bump();
                                args.push(Pattern::Bind {
                                    name: Ident {
                                        name: n,
                                        span: bspan,
                                    },
                                    span: bspan,
                                });
                            }
                        } else {
                            args.push(self.pattern()?);
                        }
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                        self.skip_newlines();
                    }
                    let end = self.peek_span();
                    self.expect(&TokenKind::RParen, "`)`")?;
                    Ok(Pattern::Ctor {
                        name: head_id,
                        args,
                        span: span.join(end),
                    })
                } else {
                    Ok(Pattern::Ctor {
                        name: head_id,
                        args: Vec::new(),
                        span,
                    })
                }
            }
            TokenKind::Int(_)
            | TokenKind::Float(_)
            | TokenKind::Bool(_)
            | TokenKind::Str(_)
            | TokenKind::Nil => {
                let value = self.expression()?;
                Ok(Pattern::Literal { value, span })
            }
            other => Err(self.err(format!("unexpected pattern `{other:?}`"))),
        }
    }

    fn if_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.peek_span();
        self.bump(); // `if`
        // `if let pat = init { ... }` — pattern destructuring + binding.
        let let_binding = if matches!(self.peek(), TokenKind::Let) {
            self.bump(); // `let`
            let pat = self.pattern()?;
            self.expect(&TokenKind::Eq, "`=` after `if let pattern`")?;
            let init = self.expr_no_block()?;
            Some((Box::new(pat), Box::new(init)))
        } else {
            None
        };
        let cond = if let_binding.is_some() {
            // Synthesize a true literal for codegen's existing `if`
            // path. The `let_binding` field tells lowering to ignore
            // this and emit the pattern-test instead.
            Expr {
                kind: ExprKind::Bool(true),
                span: start,
            }
        } else {
            self.expr_no_block()?
        };
        self.skip_newlines();
        let then_b = self.block()?;
        let else_b = if matches!(self.peek(), TokenKind::Else) {
            self.bump();
            self.skip_newlines();
            Some(self.block()?)
        } else {
            None
        };
        let end = else_b.as_ref().map(|b| b.span).unwrap_or(then_b.span);
        Ok(Expr {
            kind: ExprKind::If {
                cond: Box::new(cond),
                then_b,
                else_b,
                let_binding,
            },
            span: start.join(end),
        })
    }
}

fn assign_op(t: &TokenKind) -> Option<AssignOp> {
    Some(match t {
        TokenKind::Eq => AssignOp::Eq,
        TokenKind::PlusEq => AssignOp::PlusEq,
        TokenKind::MinusEq => AssignOp::MinusEq,
        TokenKind::StarEq => AssignOp::StarEq,
        TokenKind::SlashEq => AssignOp::SlashEq,
        _ => return None,
    })
}

fn bin_op_prec(t: &TokenKind) -> Option<(BinOp, u8)> {
    Some(match t {
        TokenKind::OrOr => (BinOp::Or, 1),
        TokenKind::AndAnd => (BinOp::And, 2),
        TokenKind::EqEq => (BinOp::Eq, 3),
        TokenKind::NotEq => (BinOp::NotEq, 3),
        TokenKind::Lt => (BinOp::Lt, 4),
        TokenKind::LtEq => (BinOp::LtEq, 4),
        TokenKind::Gt => (BinOp::Gt, 4),
        TokenKind::GtEq => (BinOp::GtEq, 4),
        // Bitwise ops sit between comparison and additive — same
        // shape as C / Rust. Useful for `flags X | Y` style
        // expressions on QFlags-typed Cute values; `Pipe` /
        // `Amp` / `Caret` are also matched here. The `Pipe`
        // case has a parser-side disambiguation against the
        // block-arg `|x|` syntax (handled in the trailing-block
        // recogniser, not here).
        TokenKind::Pipe => (BinOp::BitOr, 4),
        TokenKind::Caret => (BinOp::BitXor, 4),
        TokenKind::Amp => (BinOp::BitAnd, 4),
        TokenKind::Plus => (BinOp::Add, 5),
        TokenKind::Minus => (BinOp::Sub, 5),
        TokenKind::Star => (BinOp::Mul, 6),
        TokenKind::Slash => (BinOp::Div, 6),
        TokenKind::Percent => (BinOp::Mod, 6),
        _ => return None,
    })
}

fn can_take_trailing_block(e: &Expr) -> bool {
    matches!(
        e.kind,
        ExprKind::Ident(_)
            | ExprKind::Path(_)
            | ExprKind::Call { .. }
            | ExprKind::MethodCall { .. }
            | ExprKind::Member { .. }
            | ExprKind::SafeMethodCall { .. }
            | ExprKind::SafeMember { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::pretty;
    use crate::span::FileId;

    fn parse_str(src: &str) -> Module {
        match parse(FileId(0), src) {
            Ok(m) => m,
            Err(e) => panic!("parse failed: {e:?}\nsource: {src}"),
        }
    }

    #[test]
    fn parse_todo_item_class() {
        let src = r#"
class TodoItem < Base {
  prop text : String, default: ""
  prop done : Bool, notify: :stateChanged

  signal stateChanged

  fn toggle {
    done = !done
    emit stateChanged
  }
}
"#;
        let m = parse_str(src);
        assert_eq!(m.items.len(), 1);
        let Item::Class(c) = &m.items[0] else {
            panic!("not a class")
        };
        assert_eq!(c.name.name, "TodoItem");
        assert_eq!(c.members.len(), 4);
        assert!(matches!(&c.members[0], ClassMember::Property(p) if p.name.name == "text"));
        let ClassMember::Property(p2) = &c.members[1] else {
            panic!()
        };
        assert_eq!(p2.notify.as_ref().unwrap().name, "stateChanged");
    }

    #[test]
    fn parse_property_with_bindable_flag() {
        let src = r#"
class Counter < Base {
  prop Count : Int, notify: :countChanged, bindable
}
"#;
        let m = parse_str(src);
        let Item::Class(c) = &m.items[0] else {
            panic!("not a class")
        };
        let ClassMember::Property(p) = &c.members[0] else {
            panic!()
        };
        assert!(p.bindable, "bindable flag not picked up");
        assert!(p.binding.is_none());
        assert_eq!(p.notify.as_ref().unwrap().name, "countChanged");
    }

    #[test]
    fn parse_property_with_bind_block_creates_computed() {
        let src = r#"
class Book < Base {
  prop Page : Int, default: 0
  prop Total : Int, default: 0
  prop Ratio : Float, bind { (1.0 * Page) / Total }
}
"#;
        let m = parse_str(src);
        let Item::Class(c) = &m.items[0] else {
            panic!("not a class")
        };
        let ClassMember::Property(p) = &c.members[2] else {
            panic!()
        };
        assert!(p.binding.is_some(), "bind expression missing");
        assert!(
            !p.bindable,
            "bind alone shouldn't auto-set bindable flag at parse time"
        );
        assert!(p.notify.is_none());
        assert!(p.default.is_none());
    }

    #[test]
    fn parse_property_bind_and_default_is_a_syntax_error() {
        let src = r#"
class X < Base {
  prop y : Int, default: 0, bind { 1 }
}
"#;
        let r = parse(FileId(0), src);
        assert!(r.is_err(), "should reject bind + default combo");
    }

    #[test]
    fn parse_property_bind_and_notify_is_a_syntax_error() {
        let src = r#"
class X < Base {
  prop y : Int, notify: :yChanged, bind { 1 }
}
"#;
        let r = parse(FileId(0), src);
        assert!(r.is_err(), "should reject bind + notify combo");
    }

    /// `prop xs : ModelList<T>` is the canonical surface form for a
    /// QRangeModel-backed list. Parser lifts the wrapper to `List<T>`
    /// for downstream and sets `model: true` on the PropertyDecl so
    /// codegen emits the `cute::ModelList<T*>*` adapter.
    #[test]
    fn parse_property_with_model_list_type() {
        let src = r#"
class Store < Base {
  prop Items : ModelList<Book>
}
"#;
        let m = parse_str(src);
        let Item::Class(c) = &m.items[0] else {
            panic!("not a class")
        };
        let ClassMember::Property(p) = &c.members[0] else {
            panic!()
        };
        assert!(p.model, "model flag should be set from ModelList<T> type");
        assert!(!p.bindable);
        assert!(p.binding.is_none());
        assert!(p.fresh.is_none());
        // The lifted `ty` should be `List<Book>` so codegen sees the
        // underlying storage shape.
        let TypeKind::Named { path, args } = &p.ty.kind else {
            panic!("expected Named type, got {:?}", p.ty.kind);
        };
        assert_eq!(path[0].name, "List");
        assert_eq!(args.len(), 1);
    }

    #[test]
    fn parse_property_model_list_with_default_and_notify_is_clean() {
        let src = r#"
class Store < Base {
  prop Items : ModelList<Book>, notify: :itemsChanged, default: []
}
"#;
        let m = parse_str(src);
        let Item::Class(c) = &m.items[0] else {
            panic!("not a class")
        };
        let ClassMember::Property(p) = &c.members[0] else {
            panic!()
        };
        assert!(p.model);
        assert_eq!(p.notify.as_ref().unwrap().name, "itemsChanged");
        assert!(p.default.is_some());
    }

    /// Legacy `, model` flag was retired — writing it now points the
    /// user at the `ModelList<T>` wrapper-type form.
    #[test]
    fn parse_property_with_legacy_model_flag_is_a_helpful_error() {
        let src = r#"
class Store < Base {
  prop Items : List<Book>, model
}
"#;
        let err = parse(FileId(0), src).expect_err("legacy `, model` should error");
        assert!(
            err.message.contains("`, model` flag was retired"),
            "diagnostic should explain the retirement: {}",
            err.message,
        );
        assert!(
            err.message.contains("ModelList"),
            "diagnostic should suggest the wrapper type: {}",
            err.message,
        );
    }

    #[test]
    fn parse_property_model_list_and_bindable_is_a_syntax_error() {
        let src = r#"
class Store < Base {
  prop Items : ModelList<Book>, bindable
}
"#;
        let r = parse(FileId(0), src);
        assert!(r.is_err(), "should reject bindable + ModelList in v1");
    }

    #[test]
    fn parse_property_model_list_and_bind_block_is_a_syntax_error() {
        let src = r#"
class Store < Base {
  prop Items : ModelList<Book>, bind { other }
}
"#;
        let r = parse(FileId(0), src);
        assert!(r.is_err(), "should reject bind block plus ModelList in v1");
    }

    #[test]
    fn parse_use_qml_with_and_without_alias() {
        let src = r#"
use qml "org.kde.kirigami"
use qml "org.kde.kirigami" as Kirigami
"#;
        let m = parse_str(src);
        let mut iter = m.items.iter();
        match iter.next().expect("first item") {
            Item::UseQml(u) => {
                assert_eq!(u.module_uri, "org.kde.kirigami");
                assert!(u.alias.is_none());
            }
            other => panic!("expected UseQml, got {:?}", other),
        }
        match iter.next().expect("second item") {
            Item::UseQml(u) => {
                assert_eq!(u.module_uri, "org.kde.kirigami");
                assert_eq!(u.alias.as_ref().map(|a| a.name.as_str()), Some("Kirigami"));
            }
            other => panic!("expected UseQml, got {:?}", other),
        }
    }

    #[test]
    fn parse_use_qml_does_not_eat_qml_app_call() {
        // Sanity: `qml_app(...)` is a function call (an expression
        // statement inside fn main), NOT a `use qml` declaration.
        // The contextual recognition only fires after a leading `use`.
        let src = r#"fn main { qml_app(view: Main, module: "App") }"#;
        let m = parse_str(src);
        // No UseQml items at top level — qml_app stays an inner call.
        assert!(!m.items.iter().any(|i| matches!(i, Item::UseQml(_))));
    }

    #[test]
    fn parse_form_b_generic_class_instantiation() {
        // `Box<Int>.new()` parses as a MethodCall whose receiver is
        // the bare `Box` ident and whose `type_args` carries `[Int]`.
        // Distinguished from `box < int` comparison by the trailing
        // `>.method(` shape.
        let src = "fn run { let b = Box<Int>.new() }";
        let m = parse_str(src);
        let Item::Fn(f) = &m.items[0] else { panic!() };
        let body = f.body.as_ref().unwrap();
        let Stmt::Let { value, .. } = &body.stmts[0] else {
            panic!("expected let stmt, got {:?}", body.stmts[0])
        };
        let ExprKind::MethodCall {
            receiver,
            method,
            type_args,
            ..
        } = &value.kind
        else {
            panic!("expected method call, got {:?}", value.kind)
        };
        assert!(matches!(&receiver.kind, ExprKind::Ident(s) if s == "Box"));
        assert_eq!(method.name, "new");
        assert_eq!(type_args.len(), 1);
    }

    #[test]
    fn parse_form_b_does_not_eat_real_comparison() {
        // `box < threshold && active` is NOT form (b) — there is no
        // `>` followed by `.` so the lookahead bails.
        let src = "fn run { let ok = box < threshold && active }";
        let m = parse_str(src);
        let Item::Fn(f) = &m.items[0] else { panic!() };
        let body = f.body.as_ref().unwrap();
        let Stmt::Let { value, .. } = &body.stmts[0] else {
            panic!("expected let stmt")
        };
        // Top of the expression must be a binary operator (the `&&`
        // or the inner `<`), not a method call.
        assert!(
            matches!(&value.kind, ExprKind::Binary { .. }),
            "expected binary expression, got {:?}",
            value.kind
        );
    }

    #[test]
    fn parse_list_view_trailing_block() {
        // Bare top-level expressions are not items; the spec sample appears
        // inside an enclosing function or class method, so test that shape.
        let wrapped = "fn render { listView(todos) { |item| checkbox(bind: item.done) } }";
        let m = parse_str(wrapped);
        let Item::Fn(f) = &m.items[0] else { panic!() };
        let body = f.body.as_ref().unwrap();
        let trailing = body.trailing.as_ref().unwrap();
        let ExprKind::Call { block: Some(_), .. } = &trailing.kind else {
            panic!("expected trailing-block call, got {:?}", trailing.kind);
        };
    }

    #[test]
    fn parse_error_decl_with_payload() {
        let src = r#"
error FileError {
  notFound
  permissionDenied
  ioError(message: String)
}
"#;
        let m = parse_str(src);
        let Item::Enum(e) = &m.items[0] else { panic!() };
        assert!(e.is_error);
        assert_eq!(e.variants.len(), 3);
        assert_eq!(e.variants[2].fields.len(), 1);
        assert_eq!(e.variants[2].fields[0].name.name, "message");
    }

    #[test]
    fn parse_error_union_return_type_and_propagation() {
        let src = r#"
fn loadConfig(path: String) !Config {
  let file = try File.open(path)
  let text = try file.readAll
  try parse(text)
}
"#;
        let m = parse_str(src);
        let Item::Fn(f) = &m.items[0] else { panic!() };
        assert!(matches!(
            f.return_ty.as_ref().unwrap().kind,
            TypeKind::ErrorUnion(_)
        ));
    }

    #[test]
    fn parse_case_when_ok_err() {
        let src = r#"
fn handle {
  case loadConfig("/etc/cute.conf") {
    when ok(cfg)  { apply(cfg) }
    when err(e)   { log(e) }
  }
}
"#;
        let m = parse_str(src);
        let Item::Fn(f) = &m.items[0] else { panic!() };
        let body = f.body.as_ref().unwrap();
        let trailing = body
            .trailing
            .as_ref()
            .expect("case is the trailing expression");
        let ExprKind::Case { arms, .. } = &trailing.kind else {
            panic!()
        };
        assert_eq!(arms.len(), 2);
    }

    #[test]
    fn parse_lowercase_class_is_private() {
        // Go-style case visibility: a lowercase-named class is
        // module-private. PascalCase names derive `is_pub: true`
        // automatically (covered by the `pub class` test above);
        // this case verifies the lowercase counterpart still falls
        // through to private.
        let src = "class counter { prop count : Int, default: 0 }";
        let m = parse_str(src);
        let Item::Class(c) = &m.items[0] else {
            panic!("not a class")
        };
        assert!(!c.is_pub, "lowercase-named class should be private");
    }

    /// `pub class` makes the class exported across modules.
    #[test]
    fn parse_pub_class_sets_is_pub() {
        let src = "pub class Counter { prop count : Int, default: 0 }";
        let m = parse_str(src);
        let Item::Class(c) = &m.items[0] else {
            panic!("not a class")
        };
        assert!(c.is_pub, "`pub class` should set isPub");
    }

    /// Without `pub`, a class is private to its declaring module.
    #[test]
    fn parse_class_without_pub_is_private() {
        let src = "class Counter { prop count : Int, default: 0 }";
        let m = parse_str(src);
        let Item::Class(c) = &m.items[0] else {
            panic!("not a class")
        };
        assert!(!c.is_pub, "no-pub class should default to private");
    }

    /// `store Foo { state X : T = init; fn bump { X = X + 1 } }` parses
    /// into `Item::Store(StoreDecl)` with state_fields populated and
    /// the fn body's bare-X references rewritten to `@X` by the
    /// post-parse `apply_class_member_rewrite` pass.
    #[test]
    fn parse_store_decl_with_state_and_method() {
        let src = "store Counter { state value : Int = 0\nfn bump { value = value + 1 } }";
        let m = parse_str(src);
        let Item::Store(s) = &m.items[0] else {
            panic!("expected Store item, got {:?}", &m.items[0]);
        };
        assert_eq!(s.name.name, "Counter");
        assert!(!s.is_pub, "no-pub store should default to private");
        assert_eq!(s.state_fields.len(), 1, "state field count");
        assert_eq!(s.state_fields[0].name.name, "value");
        assert!(matches!(
            &s.state_fields[0].kind,
            StateFieldKind::Property { .. }
        ));
        // Member fn body should have `value` rewritten to `@value`.
        let ClassMember::Fn(f) = &s.members[0] else {
            panic!("expected fn member, got {:?}", &s.members[0]);
        };
        assert_eq!(f.name.name, "bump");
        let body = f.body.as_ref().expect("fn body");
        let Stmt::Assign { target, value, .. } = &body.stmts[0] else {
            panic!(
                "expected assignment as first stmt, got {:?}",
                &body.stmts[0]
            );
        };
        assert!(
            matches!(&target.kind, ExprKind::AtIdent(n) if n == "value"),
            "LHS `value` should rewrite to AtIdent: {:?}",
            target.kind,
        );
        let ExprKind::Binary { lhs, .. } = &value.kind else {
            panic!("expected binary expr, got {:?}", &value.kind);
        };
        assert!(
            matches!(&lhs.kind, ExprKind::AtIdent(n) if n == "value"),
            "RHS `value` should rewrite to AtIdent: {:?}",
            lhs.kind,
        );
    }

    /// `suite "X" { test "y" { body } }` parses into Item::Suite with
    /// each contained test stored as a synth FnDecl carrying
    /// `is_test: true` and `display_name: Some("X / y")`.
    #[test]
    fn parse_suite_with_string_named_tests() {
        let src = r#"
suite "compute" {
  test "adds positive numbers" { let a = 1 }
  test "handles zero" { let b = 0 }
}
"#;
        let m = parse_str(src);
        let Item::Suite(s) = &m.items[0] else {
            panic!("expected Suite item, got {:?}", &m.items[0]);
        };
        assert_eq!(s.name, "compute");
        assert_eq!(s.tests.len(), 2);
        assert!(s.tests[0].is_test);
        assert_eq!(
            s.tests[0].display_name.as_deref(),
            Some("compute / adds positive numbers"),
        );
        assert!(s.tests[1].is_test);
        assert_eq!(
            s.tests[1].display_name.as_deref(),
            Some("compute / handles zero"),
        );
        // Synth fn names should be unique placeholders, not the
        // original strings — those would collide with the spaces /
        // punctuation users put inside `test "..."`.
        assert!(s.tests[0].name.name.starts_with("__cuteStrTest"));
        assert_ne!(s.tests[0].name.name, s.tests[1].name.name);
    }

    /// Top-level `test "y" { body }` (outside any suite) parses to a
    /// regular `is_test` FnDecl with `display_name: Some("y")`.
    #[test]
    fn parse_top_level_string_named_test() {
        let src = r#"test "single standalone case" { let x = 1 }"#;
        let m = parse_str(src);
        let Item::Fn(f) = &m.items[0] else {
            panic!("expected Fn item, got {:?}", &m.items[0]);
        };
        assert!(f.is_test);
        assert_eq!(f.display_name.as_deref(), Some("single standalone case"),);
        // Compact `test fn camelCase` form keeps display_name=None.
        let src2 = "test fn addition { let x = 1 }";
        let m2 = parse_str(src2);
        let Item::Fn(f2) = &m2.items[0] else {
            panic!("expected Fn item");
        };
        assert!(f2.is_test);
        assert_eq!(f2.display_name, None);
        assert_eq!(f2.name.name, "addition");
    }

    /// Inside a suite body, only the string-named form is accepted —
    /// `test fn camelCase` is rejected with a clear migration hint.
    #[test]
    fn parse_suite_rejects_compact_test_fn() {
        let src = r#"suite "x" { test fn add { let x = 1 } }"#;
        let err = parse(FileId(0), src).unwrap_err();
        assert!(
            err.message.contains("must use the string-named form"),
            "expected migration hint, got: {}",
            err.message,
        );
    }

    /// Nested suites are not supported in v1.x.
    #[test]
    fn parse_nested_suite_rejected() {
        let src = r#"suite "outer" { suite "inner" { test "x" { } } }"#;
        let err = parse(FileId(0), src).unwrap_err();
        assert!(
            err.message.contains("nested `suite`"),
            "expected nested-suite-rejection diagnostic, got: {}",
            err.message,
        );
    }

    /// `pub suite` and `pub test` are nonsense (test/suite are runner-
    /// internal); the parser rejects them with a clear message.
    #[test]
    fn parse_pub_suite_and_pub_test_rejected() {
        let err1 = parse(FileId(0), r#"pub suite "x" { }"#).unwrap_err();
        assert!(
            err1.message.contains("`pub suite` is not meaningful"),
            "expected pub-suite-rejection, got: {}",
            err1.message,
        );
        let err2 = parse(FileId(0), r#"pub test "x" { }"#).unwrap_err();
        assert!(
            err2.message.contains("`pub test` is not meaningful"),
            "expected pub-test-rejection, got: {}",
            err2.message,
        );
    }

    /// `pub store Foo { ... }` sets the `is_pub` flag.
    #[test]
    fn parse_pub_store_decl() {
        let src = "pub store Hub { state items : List = [] }";
        let m = parse_str(src);
        let Item::Store(s) = &m.items[0] else {
            panic!("expected Store item");
        };
        assert!(s.is_pub, "`pub store` should set isPub");
    }

    /// `let X = sub_obj()` Object-kind state fields are rejected
    /// inside a store body (singletons don't own sub-objects via
    /// state-field syntax — use `prop` or write the init in init{}).
    #[test]
    fn parse_store_rejects_object_kind_state_field() {
        let src = "store Foo { let bar = Bar() }";
        let err = parse(FileId(0), src).unwrap_err();
        // The slim parser only accepts `state X : T = ...`; the bare
        // `let X = ...` form falls through to class_member parsing,
        // which rejects it (class members don't have a top-level `let`
        // form without a type annotation).
        assert!(
            err.message.contains("let") || err.message.contains("expected"),
            "expected diagnostic about `let`, got: {}",
            err.message,
        );
    }


    #[test]
    fn parse_legacy_class_lt_object_rejected_with_pointer_to_arc() {
        let src = "class Token < Object { prop text : String, default: \"\" }";
        let err = parse(FileId(0), src).unwrap_err();
        assert!(
            err.message
                .contains("`class Token < Object` is no longer supported"),
            "expected migration hint, got: {}",
            err.message,
        );
        assert!(
            err.message.contains("arc Token"),
            "expected suggestion to use `arc`, got: {}",
            err.message,
        );
    }




    #[test]
    fn parse_dotted_element_head_separates_module_and_name() {
        let src = "view Main { Column { model.Counter { id: c } } }";
        let m = parse_str(src);
        let Item::View(v) = &m.items[0] else {
            panic!("not a view")
        };
        // v.root is `Column { ... }`, its child is `model.Counter`.
        let ElementMember::Child(counter) = &v.root.members[0] else {
            panic!("expected child")
        };
        assert_eq!(
            counter
                .module_path
                .iter()
                .map(|i| i.name.clone())
                .collect::<Vec<_>>(),
            vec!["model".to_string()],
            "module path should be [\"model\"]"
        );
        assert_eq!(counter.name.name, "Counter");
    }

    #[test]
    fn parse_dotted_property_key_still_works() {
        let src = "view Main { Label { font.bold: true } }";
        let m = parse_str(src);
        let Item::View(v) = &m.items[0] else { panic!() };
        // v.root is the Label element directly.
        assert_eq!(v.root.name.name, "Label");
        let ElementMember::Property { key, .. } = &v.root.members[0] else {
            panic!()
        };
        assert_eq!(key, "font.bold");
    }

    #[test]
    fn parse_style_decl_literal_body() {
        let src = r##"
style Card {
  padding: 16
  background: "#fff"
  font.bold: true
}
"##;
        let m = parse_str(src);
        let Item::Style(s) = &m.items[0] else {
            panic!("not a style decl")
        };
        assert_eq!(s.name.name, "Card");
        let StyleBody::Lit(entries) = &s.body else {
            panic!("not a literal style body")
        };
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].key, "padding");
        assert_eq!(entries[1].key, "background");
        assert_eq!(entries[2].key, "font.bold");
    }

    #[test]
    fn parse_style_decl_alias_with_merge() {
        let src = "style BigCard = Card + Big";
        let m = parse_str(src);
        let Item::Style(s) = &m.items[0] else {
            panic!("not a style decl")
        };
        assert_eq!(s.name.name, "BigCard");
        let StyleBody::Alias(rhs) = &s.body else {
            panic!("not an alias body")
        };
        let ExprKind::Binary { op, lhs, rhs: rrhs } = &rhs.kind else {
            panic!("not a binary")
        };
        assert!(matches!(op, BinOp::Add));
        assert!(matches!(&lhs.kind, ExprKind::Ident(name) if name == "Card"));
        assert!(matches!(&rrhs.kind, ExprKind::Ident(name) if name == "Big"));
    }

    #[test]
    fn parse_element_with_style_member() {
        let src = r#"
view Main {
  Label { style: Card; text: "hi" }
}
"#;
        let m = parse_str(src);
        let Item::View(v) = &m.items[0] else {
            panic!("not a view")
        };
        let Some(member) = v.root.members.first() else {
            panic!("no members")
        };
        let ElementMember::Property { key, .. } = member else {
            panic!("expected style property")
        };
        assert_eq!(key, "style");
    }

    #[test]
    fn parse_test_fn_marks_is_test() {
        let src = r#"
test fn equalityWorks {
  assertEq(1, 1)
}
"#;
        let m = parse_str(src);
        let Item::Fn(f) = &m.items[0] else {
            panic!("expected an Fn item, got {:?}", m.items[0]);
        };
        assert!(f.is_test, "expected isTest=true on `test fn`");
        assert_eq!(f.name.name, "equalityWorks");
        assert!(!f.is_pub, "test fns are never pub");
    }

    #[test]
    fn parse_fn_attribute_marker_after_return_type() {
        let src = r#"
fn parseInt(s: String) !Int @lifted_bool_ok
"#;
        let m = parse_str(src);
        let Item::Fn(f) = &m.items[0] else {
            panic!("expected an Fn item, got {:?}", m.items[0]);
        };
        assert_eq!(f.attributes.len(), 1);
        assert_eq!(f.attributes[0].name.name, "lifted_bool_ok");
        assert!(f.attributes[0].args.is_empty());
        assert!(f.return_ty.is_some(), "return type should still be parsed");
    }

    #[test]
    fn parse_fn_attribute_with_args() {
        let src = r#"
fn parseInt(s: String) !Int @lifted_bool_ok(idx=0)
"#;
        let m = parse_str(src);
        let Item::Fn(f) = &m.items[0] else {
            panic!("expected an Fn item, got {:?}", m.items[0]);
        };
        assert_eq!(f.attributes.len(), 1);
        assert_eq!(f.attributes[0].name.name, "lifted_bool_ok");
        assert_eq!(f.attributes[0].args, vec!["idx=0".to_string()]);
    }

    #[test]
    fn bare_postfix_question_is_a_syntax_error() {
        // Postfix `?` after an expression used to be a Try shorthand
        // (`expr?` ≡ `try expr`). It was retired so that `expr?.x`
        // unambiguously means SafeMember. The parser should now
        // reject the bare form with a message that nudges the user
        // toward `try expr`.
        let src = r#"
fn run {
  let _ = parseInt("42")?
}
"#;
        let err = parse(FileId(0), src).expect_err("expected parse error");
        assert!(
            err.message.contains("`try expr`"),
            "diagnostic should suggest `try expr`, got: {}",
            err.message
        );
    }

    #[test]
    fn parse_safe_member_and_safe_method_call() {
        // `?.` is a postfix operator. `recv?.x` parses as
        // SafeMember; `recv?.x(args)` parses as SafeMethodCall.
        // Try (error propagation) uses the prefix-only `try expr`
        // form; postfix `?` after an expression is a syntax error
        // (see `bare_postfix_question_is_a_syntax_error`).
        let src = r#"
fn run(p: Person?) {
  let n = p?.name
  let g = p?.greet("hi")
  let v = try parseInt("42")
}
"#;
        let m = parse_str(src);
        let pretty_s = pretty(&m);
        // Pretty doesn't dive into expressions — confirm the parser
        // didn't error and produced one fn item.
        assert!(pretty_s.contains("Fn run"));

        // Walk the body to assert each form parsed as expected.
        let Item::Fn(f) = &m.items[0] else {
            panic!("not a fn");
        };
        let body = f.body.as_ref().expect("fn body");
        let mut saw_safe_member = false;
        let mut saw_safe_method = false;
        let mut saw_try = false;
        for stmt in &body.stmts {
            if let Stmt::Let { value, .. } = stmt {
                match &value.kind {
                    ExprKind::SafeMember { .. } => saw_safe_member = true,
                    ExprKind::SafeMethodCall { .. } => saw_safe_method = true,
                    ExprKind::Try(_) => saw_try = true,
                    _ => {}
                }
            }
        }
        assert!(saw_safe_member, "expected K::SafeMember from `p?.name`");
        assert!(
            saw_safe_method,
            "expected K::SafeMethodCall from `p?.greet(...)`"
        );
        assert!(saw_try, "expected K::Try from prefix `try parseInt(...)`");
    }

    /// `fn first<T>(xs: T) T { ... }` — bare generic still parses
    /// as today (one param, zero bounds). The new struct shape
    /// (`Vec<GenericParam>`) doesn't change this surface.
    #[test]
    fn parse_fn_with_bare_generic() {
        let src = "fn first<T>(xs: T) T { xs }";
        let m = parse_str(src);
        let Item::Fn(f) = &m.items[0] else {
            panic!("expected Fn");
        };
        assert_eq!(f.generics.len(), 1);
        assert_eq!(f.generics[0].name.name, "T");
        assert!(f.generics[0].bounds.is_empty());
    }

    /// `fn first<T: Iterable>(xs: T) -> T` — single-bound generic.
    /// The bound is recorded as a raw ident; the type checker
    /// doesn't enforce it yet (parser-only milestone).
    #[test]
    fn parse_fn_with_single_bound_generic() {
        let src = "fn first<T: Iterable>(xs: T) T { xs }";
        let m = parse_str(src);
        let Item::Fn(f) = &m.items[0] else {
            panic!("expected Fn");
        };
        assert_eq!(f.generics.len(), 1);
        assert_eq!(f.generics[0].name.name, "T");
        assert_eq!(f.generics[0].bounds.len(), 1);
        assert_eq!(f.generics[0].bounds[0].name, "Iterable");
    }

    /// `fn first<T: Iterable + Comparable>(xs: T) -> T` — bound list
    /// joined with `+`. Parser collects every bound; order is
    /// preserved.
    #[test]
    fn parse_fn_with_multiple_bound_generic() {
        let src = "fn first<T: Iterable + Comparable>(xs: T) T { xs }";
        let m = parse_str(src);
        let Item::Fn(f) = &m.items[0] else {
            panic!("expected Fn");
        };
        assert_eq!(f.generics[0].bounds.len(), 2);
        assert_eq!(f.generics[0].bounds[0].name, "Iterable");
        assert_eq!(f.generics[0].bounds[1].name, "Comparable");
    }

    /// `fn pair<A: Iterable, B>` — mixing bound and unbounded
    /// params.
    #[test]
    fn parse_fn_with_mixed_bounds() {
        let src = "fn pair<A: Iterable, B>(a: A, b: B) A { a }";
        let m = parse_str(src);
        let Item::Fn(f) = &m.items[0] else {
            panic!("expected Fn");
        };
        assert_eq!(f.generics.len(), 2);
        assert_eq!(f.generics[0].name.name, "A");
        assert_eq!(f.generics[0].bounds[0].name, "Iterable");
        assert_eq!(f.generics[1].name.name, "B");
        assert!(f.generics[1].bounds.is_empty());
    }

    /// Class-level generics also accept bounds.
    #[test]
    fn parse_class_with_bounded_generic() {
        let src = "class Box<T: Drawable> < Base { let x : T }";
        let m = parse_str(src);
        let Item::Class(c) = &m.items[0] else {
            panic!("expected Class");
        };
        assert_eq!(c.generics.len(), 1);
        assert_eq!(c.generics[0].name.name, "T");
        assert_eq!(c.generics[0].bounds[0].name, "Drawable");
    }

    /// `trait Iterable { fn iter Iter }` parses as Item::Trait.
    /// Method bodies must be absent — abstract sigs only.
    #[test]
    fn parse_trait_decl_collects_abstract_methods() {
        let src = "trait Iterable { fn iter Iter\nfn size Int }";
        let m = parse_str(src);
        let Item::Trait(t) = &m.items[0] else {
            panic!("expected Trait item");
        };
        assert_eq!(t.name.name, "Iterable");
        assert_eq!(t.methods.len(), 2);
        assert_eq!(t.methods[0].name.name, "iter");
        assert!(t.methods[0].body.is_none(), "trait method must be abstract");
        assert_eq!(t.methods[1].name.name, "size");
    }

    /// Trait methods may carry a default body that impls can omit.
    /// The parser accepts both shapes; the FnDecl's `body` field
    /// distinguishes (None = abstract, Some = default).
    #[test]
    fn trait_method_with_default_body_is_accepted() {
        let src = "trait Greeter {\n  fn name String\n  fn greet String { \"hi\" }\n}";
        let m = parse_str(src);
        let Item::Trait(t) = &m.items[0] else {
            panic!("expected Trait");
        };
        assert_eq!(t.methods.len(), 2);
        assert!(t.methods[0].body.is_none(), "abstract method has no body");
        assert!(
            t.methods[1].body.is_some(),
            "default method carries its body"
        );
    }

    /// `impl Iterable for MyList { fn iter Iter { ... } }` parses
    /// with the trait + target captured and method bodies populated.
    #[test]
    fn parse_impl_decl_collects_concrete_methods() {
        let src = "impl Iterable for MyList { fn iter Iter { nil }\nfn size Int { 0 } }";
        let m = parse_str(src);
        let Item::Impl(i) = &m.items[0] else {
            panic!("expected Impl item");
        };
        assert_eq!(i.trait_name.name, "Iterable");
        assert_eq!(
            crate::ast::type_expr_base_name(&i.for_type),
            Some("MyList".to_string())
        );
        assert!(i.generics.is_empty(), "non-parametric impl has no generics");
        assert_eq!(i.methods.len(), 2);
        assert!(i.methods[0].body.is_some(), "impl method must have a body");
        assert!(i.methods[1].body.is_some());
    }

    /// An abstract method (no body) inside an impl block is rejected.
    #[test]
    fn impl_method_without_body_is_rejected() {
        let src = "impl Iterable for MyList { fn iter Iter }";
        let err = parse(FileId(0), src).expect_err("should reject");
        assert!(
            err.message.contains("impl methods must have a body"),
            "diagnostic should explain why: {}",
            err.message
        );
    }

    /// `impl<T> Foo for List<T> { ... }` parses with impl-level
    /// generics + a parametric for-type. Both pieces need to land
    /// on the AST so HIR can register the impl by base name and
    /// codegen can splice methods onto the right class.
    #[test]
    fn parse_parametric_impl_for_generic_type() {
        let src = "impl<T> Foo for List<T> { fn first T { nil } }";
        let m = parse_str(src);
        let Item::Impl(i) = &m.items[0] else {
            panic!("expected Impl item");
        };
        assert_eq!(i.trait_name.name, "Foo");
        assert_eq!(i.generics.len(), 1);
        assert_eq!(i.generics[0].name.name, "T");
        assert_eq!(
            crate::ast::type_expr_base_name(&i.for_type),
            Some("List".to_string())
        );
        // Confirm `List<T>` round-trips through the renderer.
        assert_eq!(crate::ast::type_expr_render(&i.for_type), "List<T>");
        assert_eq!(i.methods.len(), 1);
    }

    /// `impl Foo for QStringList { ... }` is the simple form on an
    /// extern type. The for-type is now a TypeExpr (was Ident);
    /// confirm the simple-name shape still round-trips.
    #[test]
    fn parse_impl_for_extern_type() {
        let src = "impl Iterable for QStringList { fn first Int { 0 } }";
        let m = parse_str(src);
        let Item::Impl(i) = &m.items[0] else {
            panic!("expected Impl item");
        };
        assert!(i.generics.is_empty());
        assert_eq!(
            crate::ast::type_expr_base_name(&i.for_type),
            Some("QStringList".to_string())
        );
        assert_eq!(crate::ast::type_expr_render(&i.for_type), "QStringList");
    }

    /// `impl<K, V> Foo for Map<K, V> { ... }` covers multi-param
    /// impl-level generics + a parametric for-type with multiple
    /// args.
    #[test]
    fn parse_impl_with_multi_param_generics() {
        let src = "impl<K, V> Foo for Map<K, V> { fn at(k: K) V { nil } }";
        let m = parse_str(src);
        let Item::Impl(i) = &m.items[0] else {
            panic!("expected Impl item");
        };
        assert_eq!(i.generics.len(), 2);
        assert_eq!(i.generics[0].name.name, "K");
        assert_eq!(i.generics[1].name.name, "V");
        assert_eq!(
            crate::ast::type_expr_base_name(&i.for_type),
            Some("Map".to_string())
        );
        assert_eq!(crate::ast::type_expr_render(&i.for_type), "Map<K, V>");
    }

    /// Pretty-printer (used by `cute fmt`) round-trips the bound
    /// syntax. `T: A + B` should re-emit as-is.
    #[test]
    fn fmt_preserves_generic_bounds() {
        use crate::format_source;
        use crate::span::SourceMap;
        let src = "fn first<T: Iterable + Comparable>(xs: T) T { xs }\n";
        let mut sm = SourceMap::default();
        let fid = sm.add("t.cute".to_string(), src.to_string());
        let formatted = format_source(fid, src).expect("format");
        assert!(
            formatted.contains("T: Iterable + Comparable"),
            "expected bound to round-trip, got:\n{formatted}"
        );
    }

    #[test]
    fn pretty_round_trip_smoke() {
        let src = r#"
class TodoItem < Base {
  prop text : String, default: ""
  signal stateChanged
  fn toggle {
    emit stateChanged
  }
}
"#;
        let m = parse_str(src);
        let s = pretty(&m);
        assert!(s.contains("Class TodoItem < Base"));
        assert!(s.contains("Property text : String"));
        assert!(s.contains("Signal stateChanged"));
        assert!(s.contains("Fn toggle"));
    }

    /// `init(params) { body }` and `deinit { body }` parse as
    /// dedicated ClassMember variants.
    #[test]
    fn parse_init_and_deinit_class_members() {
        let src = r#"
class Counter {
  prop count : Int, default: 0
  init(initial: Int) { count = initial }
  deinit { }
}
"#;
        let m = parse_str(src);
        let Item::Class(c) = &m.items[0] else {
            panic!("expected Class item");
        };
        assert_eq!(c.members.len(), 3);
        let ClassMember::Init(i) = &c.members[1] else {
            panic!("expected Init at index 1");
        };
        assert_eq!(i.params.len(), 1);
        assert_eq!(i.params[0].name.name, "initial");
        assert!(matches!(&c.members[2], ClassMember::Deinit(_)));
    }

    /// Multiple `init`s on one class are accepted (overload).
    #[test]
    fn parse_multiple_inits_accepted_as_overload() {
        let src = r#"
class Pair {
  prop a : Int, default: 0
  init() { }
  init(a: Int, b: Int) { }
}
"#;
        let m = parse_str(src);
        let Item::Class(c) = &m.items[0] else {
            panic!("expected Class item");
        };
        let inits: Vec<_> = c
            .members
            .iter()
            .filter_map(|m| match m {
                ClassMember::Init(i) => Some(i),
                _ => None,
            })
            .collect();
        assert_eq!(inits.len(), 2);
        assert_eq!(inits[0].params.len(), 0);
        assert_eq!(inits[1].params.len(), 2);
    }

    /// More than one `deinit` is rejected at parse time.
    #[test]
    fn double_deinit_is_a_syntax_error() {
        let src = "class X { deinit { } deinit { } }";
        let err = parse(FileId(0), src).expect_err("expected parse error");
        assert!(
            err.message.contains("only one `deinit`"),
            "diagnostic should call out the duplicate deinit, got: {}",
            err.message
        );
    }


    /// `weak prop` PARSES again (§8.44): refcounting made cycles the
    /// one thing that leaks, and this is how an author breaks one.
    /// `unowned prop` stays rejected — it promises the pointee
    /// outlives the holder, which nothing here can check, and a plain
    /// handle already cannot dangle.
    #[test]
    fn weak_prop_parses_and_unowned_does_not() {
        let src = "class Holder { weak prop child : Child }";
        let m = parse(FileId(0), src).expect("weak prop parses");
        let Item::Class(c) = &m.items[0] else {
            panic!("expected a class")
        };
        let ClassMember::Property(pd) = &c.members[0] else {
            panic!("expected a property")
        };
        assert!(pd.weak, "the weak flag reached the declaration");

        // `unowned` stopped being a word at all in §8.71 — it named a
        // reference kind pixie does not have, so it is an ordinary
        // identifier now and `unowned prop` reads as two idents.
        let src2 = "class Holder { unowned prop owner : Parent }";
        let err2 = parse(FileId(0), src2).expect_err("expected parse error");
        assert!(
            err2.message.contains("expected class member"),
            "an ordinary identifier where a member keyword goes, got: {}",
            err2.message
        );
    }


    /// `weak fn` / `unowned init` — modifier only applies to property /
    /// field declarations. Reject the misuse with a focused message
    /// rather than a generic "expected member" error.
    #[test]
    fn weak_on_non_storage_member_rejected() {
        let src = "class X { weak fn run { } }";
        let err = parse(FileId(0), src).expect_err("expected parse error");
        assert!(
            err.message.contains("`weak` modifier only applies"),
            "expected weak-only-on-storage diagnostic, got: {}",
            err.message
        );
    }


    /// Default (no `@escaping`) leaves `is_escaping` at false — codegen
    /// will lower the closure param to `cute::function_ref<F>`.
    #[test]
    fn default_closure_param_is_non_escaping() {
        let src = r#"
fn run(f: fn(Int) -> Int) {
}
"#;
        let m = parse_str(src);
        let Item::Fn(f) = &m.items[0] else {
            panic!("expected Fn");
        };
        assert!(!f.params[0].is_escaping);
    }


    /// Struct methods land in `StructDecl.methods`. Mixed with fields
    /// is fine — order isn't significant since fields go through one
    /// vec and methods through another.
    #[test]
    fn struct_with_methods_parses() {
        let src = r#"
struct Point {
  var x : Int = 0
  var y : Int = 0

  fn magnitudeSq Int {
    self.x * self.x + self.y * self.y
  }
}
"#;
        let m = parse_str(src);
        let Item::Struct(s) = &m.items[0] else {
            panic!("expected Struct");
        };
        assert_eq!(s.fields.len(), 2);
        assert!(s.fields[0].is_mut, "var field should set is_mut=true");
        assert!(s.fields[1].is_mut);
        assert_eq!(s.methods.len(), 1);
        assert_eq!(s.methods[0].name.name, "magnitudeSq");
    }

    /// `pub fn` inside struct body marks the method public.
    #[test]
    fn struct_pub_method_parses() {
        let src = r#"
struct Point {
  var x : Int = 0
  pub fn describe Int { self.x }
}
"#;
        let m = parse_str(src);
        let Item::Struct(s) = &m.items[0] else {
            panic!("expected Struct");
        };
        assert_eq!(s.methods.len(), 1);
        assert!(s.methods[0].is_pub);
    }

    /// `: ~Copyable` after struct name sets `is_copyable: false`
    /// on the parsed StructDecl.
    #[test]
    fn struct_non_copyable_parses() {
        let src = "struct Token: ~Copyable { let id: Int }";
        let m = parse_str(src);
        let Item::Struct(s) = &m.items[0] else {
            panic!("expected Struct");
        };
        assert!(!s.is_copyable);
        assert!(!s.fields[0].is_mut, "let field should set is_mut=false");
    }


    /// `: ~Anything` other than `Copyable` is rejected — only the
    /// specific `~Copyable` annotation is recognised here.
    #[test]
    fn tilde_other_keyword_rejected() {
        let src = "struct X: ~Movable { id: Int }";
        let err = parse(FileId(0), src).expect_err("expected parse error");
        assert!(
            err.message.contains("expected `~Copyable`"),
            "diagnostic should call out the only-Copyable rule, got: {}",
            err.message
        );
    }

    /// Default (no annotation) keeps `is_copyable: true`.
    #[test]
    fn default_struct_is_copyable() {
        let src = "struct Pair { var a: Int, var b: Int }";
        let m = parse_str(src);
        let Item::Struct(s) = &m.items[0] else {
            panic!("expected Struct");
        };
        assert!(s.is_copyable);
    }

    /// Struct fields require an explicit `let` / `var` keyword. Bare
    /// fields were retired in v0.x → v1 to match Swift / class member
    /// shape; the diagnostic guides the user toward `var name : T = ...`
    /// (legacy mutable shape) or `let name : T` (init-once).
    #[test]
    fn bare_struct_field_is_a_parse_error_with_migration_hint() {
        let src = "struct Point { x : Int = 0 }";
        let r = parse(FileId(0), src);
        assert!(r.is_err(), "bare field should be rejected");
        let msg = format!("{:?}", r.unwrap_err());
        assert!(
            msg.contains("require an explicit `let`") || msg.contains("`var`"),
            "diagnostic should mention let/var migration; got: {msg}",
        );
    }

    // ---- fn body let/var block declarations ----------------------------
    //
    // `let ( ... )` / `var ( ... )` at statement scope groups several
    // local bindings under one keyword. The block expands to N
    // individual Stmt::Let / Stmt::Var entries in the surrounding
    // Block — HIR / type-check / codegen never see a wrapper.

    #[test]
    fn let_block_in_fn_body_parses_to_multiple_let_stmts() {
        let src = r#"
fn main {
  let (
    inputFile  = "data.csv"
    outputFile = "result.csv"
    maxLines   = 1000
  )
  println(inputFile)
}
"#;
        let m = parse_str(src);
        let Item::Fn(f) = &m.items[0] else {
            panic!("expected Fn");
        };
        let body = f.body.as_ref().unwrap();
        // Three Stmt::Let from the block + the println Stmt::Expr =
        // 3 stmts + trailing println (or 4 stmts if println isn't trailing).
        let let_count = body
            .stmts
            .iter()
            .filter(|s| matches!(s, Stmt::Let { .. }))
            .count();
        assert_eq!(
            let_count, 3,
            "expected 3 let bindings, got: {:?}",
            body.stmts
        );
    }

    #[test]
    fn var_block_in_fn_body_parses_to_multiple_var_stmts() {
        let src = r#"
fn run {
  var (
    a = 1
    b = "hello"
    c = true
  )
  a = 2
}
"#;
        let m = parse_str(src);
        let Item::Fn(f) = &m.items[0] else {
            panic!("expected Fn");
        };
        let body = f.body.as_ref().unwrap();
        let var_count = body
            .stmts
            .iter()
            .filter(|s| matches!(s, Stmt::Var { .. }))
            .count();
        assert_eq!(
            var_count, 3,
            "expected 3 var bindings, got: {:?}",
            body.stmts
        );
    }

    #[test]
    fn let_block_with_typed_bindings() {
        let src = r#"
fn main {
  let (
    n : Int = 42
    s : String = "hi"
  )
}
"#;
        let m = parse_str(src);
        let Item::Fn(f) = &m.items[0] else {
            panic!("expected Fn");
        };
        let body = f.body.as_ref().unwrap();
        for s in &body.stmts {
            let Stmt::Let { ty, .. } = s else { continue };
            assert!(
                ty.is_some(),
                "typed binding should preserve type annotation"
            );
        }
    }

    #[test]
    fn let_block_inside_nested_block_works() {
        // Block form should compose with if / while / for / lambda
        // bodies — they all use the same `block()` helper. The lone
        // `if` here lands in the fn body's trailing expression slot,
        // not in `stmts[0]`.
        let src = r#"
fn run {
  if true {
    let (
      x = 1
      y = 2
    )
  }
}
"#;
        let m = parse_str(src);
        let Item::Fn(f) = &m.items[0] else {
            panic!("expected Fn");
        };
        let body = f.body.as_ref().unwrap();
        let if_expr = body
            .trailing
            .as_deref()
            .expect("if-expr should be the body's trailing expression");
        let ExprKind::If { then_b, .. } = &if_expr.kind else {
            panic!("expected If, got: {:?}", if_expr.kind);
        };
        let let_count = then_b
            .stmts
            .iter()
            .filter(|s| matches!(s, Stmt::Let { .. }))
            .count();
        assert_eq!(
            let_count, 2,
            "expected 2 lets in if body, got: {:?}",
            then_b.stmts
        );
    }

    #[test]
    fn empty_let_block_in_fn_body_is_a_parse_error() {
        let src = "fn main { let ( ) }";
        let r = parse(FileId(0), src);
        assert!(r.is_err(), "empty let block should be rejected");
        let msg = format!("{:?}", r.unwrap_err());
        assert!(
            msg.contains("empty `let` block"),
            "expected empty-block diagnostic; got {msg}",
        );
    }

    /// Block form `let ( ... )` / `var ( ... )` works inside struct
    /// body, mirroring class member blocks.
    #[test]
    fn struct_var_block_parses_to_multiple_fields() {
        let src = r#"
struct Vec3 {
  var (
    x : Float = 0.0
    y : Float = 0.0
    z : Float = 0.0
  )
}
"#;
        let m = parse_str(src);
        let Item::Struct(s) = &m.items[0] else {
            panic!("expected Struct");
        };
        assert_eq!(s.fields.len(), 3);
        for f in &s.fields {
            assert!(f.is_mut, "var-block items should all be mutable");
        }
    }



    /// `weak let` / `unowned var` on plain class fields parse
    /// the same way as on props — the modifier flows into Field.weak /
    /// Field.unowned. Field defaults use `= expr`, not the prop's
    /// `, default:` shape.
    #[test]
    fn weak_let_field_parses() {
        let src = r#"
class X {
  weak let a : Y?
  var b : Z
}
"#;
        let m = parse_str(src);
        let Item::Class(c) = &m.items[0] else {
            panic!("expected Class");
        };
        let ClassMember::Field(a) = &c.members[0] else {
            panic!("expected Field");
        };
        assert!(a.weak);
        assert!(!a.is_mut);
        let ClassMember::Field(b) = &c.members[1] else {
            panic!("expected Field");
        };
        assert!(!b.weak);
        assert!(b.is_mut);
    }

    /// `@x` in user source is a parse error; the diagnostic points
    /// at the bare-name alternative.
    #[test]
    fn at_ident_in_user_source_is_a_parse_error() {
        let src = "fn run { @count = 1 }";
        let err = parse(FileId(0), src).expect_err("should reject `@count`");
        assert!(
            err.message.contains("`@` prefix on `@count` was retired"),
            "diagnostic should explain the retirement, got: {}",
            err.message
        );
    }

    /// Inside a class method body, a bare `K::Ident(name)` whose
    /// name matches a class property / field is rewritten to
    /// `K::AtIdent(name)` by the post-parse pass. Codegen / type-check
    /// then see the same AtIdent shape they did when the user wrote
    /// `@x` directly.
    #[test]
    fn class_member_ident_in_method_body_lowers_to_atident() {
        let src = r#"
class X < Base {
  prop count : Int, default: 0
  fn incr { count = count + 1 }
}
"#;
        let m = parse_str(src);
        let Item::Class(c) = &m.items[0] else {
            panic!("expected Class");
        };
        let ClassMember::Fn(f) = c
            .members
            .iter()
            .find(|m| matches!(m, ClassMember::Fn(_)))
            .unwrap()
        else {
            unreachable!()
        };
        let body = f.body.as_ref().unwrap();
        let Stmt::Assign { target, value, .. } = &body.stmts[0] else {
            panic!("expected Assign stmt, got {:?}", body.stmts[0]);
        };
        assert!(
            matches!(&target.kind, ExprKind::AtIdent(n) if n == "count"),
            "LHS `count` should rewrite to AtIdent: {:?}",
            target.kind
        );
        let ExprKind::Binary { lhs, .. } = &value.kind else {
            panic!("expected Binary on RHS");
        };
        assert!(
            matches!(&lhs.kind, ExprKind::AtIdent(n) if n == "count"),
            "RHS `count` should rewrite to AtIdent: {:?}",
            lhs.kind
        );
    }

    /// The post-parse rewrite skips a member whose name is shadowed
    /// by the enclosing fn's parameter list. `init(label: String) {
    /// Label = label }` correctly resolves LHS `Label` to the
    /// member but RHS `label` to the parameter.
    #[test]
    fn class_member_rewrite_respects_param_shadowing() {
        let src = r#"
class Token {
  var Label : String = ""
  init(label: String) {
    Label = label
  }
}
"#;
        let m = parse_str(src);
        let Item::Class(c) = &m.items[0] else {
            panic!("expected Class");
        };
        let init = c.inits().next().unwrap();
        let Stmt::Assign { target, value, .. } = &init.body.stmts[0] else {
            panic!("expected Assign");
        };
        assert!(
            matches!(&target.kind, ExprKind::AtIdent(n) if n == "Label"),
            "member LHS should rewrite: {:?}",
            target.kind
        );
        assert!(
            matches!(&value.kind, ExprKind::Ident(n) if n == "label"),
            "param-shadowed RHS should stay as Ident: {:?}",
            value.kind
        );
    }

    // ---- top-level `let` declarations ----------------------------------

    #[test]
    fn top_level_let_parses_with_type_and_value() {
        let src = r#"
let MaxLines : Int = 1000
let Greeting : String = "hello"
"#;
        let m = parse_str(src);
        assert_eq!(m.items.len(), 2);
        let Item::Let(l1) = &m.items[0] else {
            panic!("expected Let, got {:?}", m.items[0]);
        };
        assert_eq!(l1.name.name, "MaxLines");
        assert!(!l1.is_pub);
        let Item::Let(l2) = &m.items[1] else {
            panic!("expected Let, got {:?}", m.items[1]);
        };
        assert_eq!(l2.name.name, "Greeting");
    }

    #[test]
    fn top_level_pub_let_sets_is_pub_true() {
        let src = "pub let Pi : Float = 3.14";
        let m = parse_str(src);
        let Item::Let(l) = &m.items[0] else {
            panic!("expected Let");
        };
        assert!(l.is_pub);
    }

    #[test]
    fn top_level_let_requires_type_annotation() {
        // Statement-scope `let` allows inference; top-level requires
        // explicit type for codegen's static-init lowering to work
        // without surrounding context.
        let src = "let X = 42";
        let r = parse(FileId(0), src);
        assert!(r.is_err(), "top-level let without type should be rejected");
        let msg = format!("{:?}", r.unwrap_err());
        assert!(
            msg.contains("requires an explicit type annotation"),
            "expected explicit-type diagnostic; got {msg}",
        );
    }

    #[test]
    fn top_level_var_is_a_parse_error_with_helpful_diagnostic() {
        let src = "var Counter : Int = 0";
        let r = parse(FileId(0), src);
        assert!(r.is_err(), "top-level var should be rejected");
        let msg = format!("{:?}", r.unwrap_err());
        assert!(
            msg.contains("top-level `var` is not supported"),
            "expected top-level-var diagnostic; got {msg}",
        );
    }


    // ---- var/let/prop block declarations -------------------------------
    //
    // `prop ( ... )` / `let ( ... )` / `var ( ... )` lets a class group
    // multiple decls of the same kind under a single header (Go-style).
    // Each item inside the block uses the same per-decl grammar it would
    // at top level. The post-parse AST contains individual decls — the
    // block form is purely sugar; HIR / codegen / type-check never see
    // a "block" wrapper.

    #[test]
    fn pub_prop_block_parses_to_multiple_individual_props() {
        let src = r#"
pub class Counter < Base {
  pub prop (
    count : Int, notify: :countChanged, default: 0
    label : String, notify: :labelChanged, default: ""
    ratio : Float, notify: :ratioChanged, default: 0.0
  )
}
"#;
        let m = parse_str(src);
        let Item::Class(c) = &m.items[0] else {
            panic!("expected Class");
        };
        assert_eq!(
            c.members.len(),
            3,
            "block should expand to 3 individual props"
        );
        for (i, expected_name) in ["count", "label", "ratio"].iter().enumerate() {
            let ClassMember::Property(p) = &c.members[i] else {
                panic!("member {i} not a Property");
            };
            assert_eq!(p.name.name, *expected_name);
            assert!(p.is_pub, "block-header `pub` should propagate to item {i}");
            assert!(
                p.notify.is_some(),
                "item {i} should have its own notify modifier preserved",
            );
        }
    }

    #[test]
    fn let_block_parses_to_multiple_individual_fields() {
        let src = r#"
class Counter < Base {
  let (
    salt : Int = 0
    seed : Int = 1
  )
}
"#;
        let m = parse_str(src);
        let Item::Class(c) = &m.items[0] else {
            panic!("expected Class");
        };
        assert_eq!(c.members.len(), 2);
        let ClassMember::Field(a) = &c.members[0] else {
            panic!("expected Field for salt");
        };
        assert_eq!(a.name.name, "salt");
        assert!(!a.is_mut, "let block items should be immutable");
        assert!(!a.is_pub, "no `pub` on block header → items not pub");
        let ClassMember::Field(b) = &c.members[1] else {
            panic!("expected Field for seed");
        };
        assert!(!b.is_mut);
    }


    #[test]
    fn empty_prop_block_is_a_parse_error() {
        let src = r#"
class C < Base {
  prop ( )
}
"#;
        let r = parse(FileId(0), src);
        assert!(r.is_err(), "empty block should be rejected");
        let msg = format!("{:?}", r.unwrap_err());
        assert!(
            msg.contains("empty `prop` block"),
            "expected empty-block diagnostic; got {msg}",
        );
    }

    #[test]
    fn per_item_pub_inside_block_is_a_parse_error() {
        // The block header sets visibility for every contained item;
        // a per-item `pub` would either be redundant or contradictory.
        let src = r#"
class C < Base {
  pub prop (
    pub count : Int, notify: :countChanged, default: 0
    label : String, notify: :labelChanged
  )
}
"#;
        let r = parse(FileId(0), src);
        assert!(r.is_err(), "per-item pub inside block should be rejected");
        let msg = format!("{:?}", r.unwrap_err());
        assert!(
            msg.contains("per-item `pub` is not allowed"),
            "expected per-item-pub diagnostic; got {msg}",
        );
    }

    #[test]
    fn class_member_rewrite_picks_up_block_props_in_method_bodies() {
        // The post-parse Ident→AtIdent rewrite walks class members to
        // build the "is this name a member?" set. Block-form props
        // must end up in that set just like individual `prop` decls,
        // so member access in method bodies works the same way.
        let src = r#"
class C < Base {
  prop (
    count : Int, notify: :countChanged, default: 0
  )
  fn incr { count = count + 1 }
}
"#;
        let m = parse_str(src);
        let Item::Class(c) = &m.items[0] else {
            panic!("expected Class");
        };
        let f = c
            .members
            .iter()
            .find_map(|m| match m {
                ClassMember::Fn(f) => Some(f),
                _ => None,
            })
            .expect("fn member missing");
        let body = f.body.as_ref().unwrap();
        let Stmt::Assign { target, .. } = &body.stmts[0] else {
            panic!("expected assign stmt in fn body");
        };
        assert!(
            matches!(&target.kind, ExprKind::AtIdent(n) if n == "count"),
            "block-form prop `count` should still rewrite to AtIdent: {:?}",
            target.kind,
        );
    }
    /// `consuming` parameter modifier sets `is_consuming: true`. Default
    /// (no modifier) is `false`.
    #[test]
    fn consuming_param_parses() {
        let src = "fn run(consuming x: Int, y: Int) { }";
        let m = parse_str(src);
        let Item::Fn(f) = &m.items[0] else {
            panic!("expected Fn");
        };
        assert!(f.params[0].is_consuming);
        assert!(!f.params[1].is_consuming);
    }
    /// `escaping consuming` (or vice versa) is rejected — the two
    /// modifiers describe contradictory borrow semantics.
    #[test]
    fn escaping_and_consuming_combo_rejected() {
        let src = "fn run(escaping consuming f: fn(Int) -> Int) { }";
        let err = parse(FileId(0), src).expect_err("expected parse error");
        assert!(
            err.message.contains("cannot be combined"),
            "diagnostic should call out the combo, got: {}",
            err.message
        );
    }
    /// `escaping` annotation (bare keyword, post-`@`-retirement) on
    /// closure-typed params sets `is_escaping: true` on the parsed
    /// Param. Default (no annotation) is `false` — closures are
    /// non-escaping by default and lower to `cute::function_ref<F>`
    /// at codegen.
    #[test]
    fn escaping_annotation_on_closure_param_parses() {
        let src = r#"
fn keep(escaping f: fn(Int) -> Int) {
}
"#;
        let m = parse_str(src);
        let Item::Fn(f) = &m.items[0] else {
            panic!("expected Fn item");
        };
        assert_eq!(f.params.len(), 1);
        assert!(f.params[0].is_escaping, "expected isEscaping=true");
    }
    /// `escaping` only applies to fn-typed parameters; applying it to
    /// e.g. `Int` is rejected at parse time.
    #[test]
    fn escaping_on_non_closure_param_rejected() {
        let src = "fn run(escaping x: Int) { }";
        let err = parse(FileId(0), src).expect_err("expected parse error");
        assert!(
            err.message.contains("`escaping` only applies"),
            "diagnostic should explain the fn-only restriction, got: {}",
            err.message
        );
    }
    /// `escaping` is the bare-keyword form of the param annotation
    /// (post-`@`-retirement). Mirrors the `consuming` keyword shape.
    #[test]
    fn bare_escaping_keyword_parses_on_closure_param() {
        let src = "fn keep(escaping f: fn(Int) -> Int) { }";
        let m = parse_str(src);
        let Item::Fn(f) = &m.items[0] else {
            panic!("expected Fn");
        };
        assert!(f.params[0].is_escaping);
    }

}
