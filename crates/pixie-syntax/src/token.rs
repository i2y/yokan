//! Token kinds and the `Token` struct.
//!
//! String literals are decomposed into segments at lex time so that the
//! parser does not need to re-scan their bodies for `#{...}` interpolations.

use crate::span::Span;

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    // -- Literals -------------------------------------------------------
    Int(i64),
    Float(f64),
    /// A string literal, possibly containing `#{...}` interpolations.
    /// Each segment is either a raw text chunk or an `Interp` placeholder
    /// whose replacement is the byte range (within the original source)
    /// of the `#{...}` expression - the parser will re-tokenize that
    /// range on demand.
    Str(Vec<StrSeg>),
    /// `:foo` symbol literal. The string is the identifier without the colon.
    Sym(String),
    /// `true` / `false`.
    Bool(bool),
    Nil,

    // -- Identifiers ----------------------------------------------------
    Ident(String),
    /// `@foo` instance-variable shorthand. The string is the name without `@`.
    AtIdent(String),

    // -- Keywords -------------------------------------------------------
    Class,
    Struct,
    /// `enum` — leading keyword for both user-defined enums
    /// (`enum Color { Red; Green; Blue }`) and C++ enum bindings
    /// (`extern enum Foo { Bar = 1 }`). Distinct type at the
    /// language level — Cute doesn't auto-convert to Int.
    Enum,
    /// `flags` — leading keyword for QFlags<E>-shaped types over
    /// an existing enum. Allows `|` / `&` / `^` on the flags type
    /// while keeping the underlying enum free of bitwise ops. The
    /// connector to its underlying enum (`flags X of E`) is the
    /// contextual identifier `of` — lexed as a regular Ident, only
    /// recognised by the parser in this position.
    Flags,
    Property,
    Signal,
    Slot,
    Fn,
    Init,
    Deinit,
    /// `test` — leading keyword for `test fn name { body }` test
    /// declarations. Marks the following `fn` as a unit test that the
    /// runner should discover and execute under `cute test`.
    Test,
    Emit,
    Case,
    When,
    /// Legacy alias for `enum E { ... }` with `is_error: true`. Kept so
    /// existing code using `error E { ... }` continues to parse without
    /// migration. Internally lowers to `EnumDecl { is_error: true, ... }`
    /// in the AST — there is no separate `Item::Error` variant.
    Error,
    Async,
    Await,
    Use,
    View,
    Widget,
    Style,
    Try,
    If,
    Else,
    For,
    While,
    /// `break` — early exit from the innermost `for` / `while` loop.
    Break,
    /// `continue` — skip to the next iteration of the innermost loop.
    Continue,
    /// `batch { ... }` — opens a scope that defers Qt 6 property
    /// binding notifications until the block exits, so multiple
    /// bindable-prop writes appear atomic to dependent bindings
    /// (glitch-free updates). Lowers to `QScopedPropertyUpdateGroup`.
    /// Has no observable effect on non-bindable props.
    Batch,
    Return,
    Let,
    Var,
    /// `pub` — visibility modifier on top-level decls (class / arc /
    /// struct / fn / view / widget / style / trait / error) and on
    /// class members (prop / let / var / fn / signal / slot, struct
    /// fields). Without `pub`, the decl is private to its declaring
    /// module (top-level) or class (member). `init` / `deinit` are
    /// always implicitly public.
    Pub,
    Self_, // `self` (lowercase keyword)
    Weak,
    Owned,
    Unowned,
    /// `consuming` — parameter modifier for linear types. Marks the
    /// argument as consumed by the call (lowers to `T&&` C++ signature
    /// and `std::move(x)` at the callsite).
    Consuming,
    /// `escaping` — parameter modifier on closure-typed params. Marks
    /// the closure as potentially-escaping (stored, returned, captured
    /// across an event-loop turn), so codegen lowers it to
    /// `std::function<F>` instead of the default
    /// `cute::function_ref<F>` borrow. Replaces the previous
    /// `@escaping` syntax — `@` is no longer a sigil in Cute.
    Escaping,
    /// `~` — used in `struct X: ~Copyable { ... }` to opt the type
    /// into linear semantics (deleted copy ctor / assignment, defaulted
    /// move). Also used in `: ~Copyable` super-clause syntax for arc /
    /// class. Does NOT collide with bitwise NOT (Cute uses `!` for
    /// boolean and has no bitwise operators yet).
    Tilde,
    /// `extern` — leading keyword for `extern value Name { ... }`,
    /// the binding form for plain C++ value types (QColor, QPoint,
    /// etc.). The follow-on word `value` is matched as a contextual
    /// ident so it can still be used freely as an identifier.
    Extern,
    /// `trait Foo { fn bar -> Baz }` — declares a nominal interface
    /// (Swift `protocol` / Rust `trait`). Bodies are abstract method
    /// signatures; the type checker uses these to resolve
    /// generic-bound method calls. Default-method bodies aren't
    /// supported in v1.
    Trait,
    /// `impl Foo for Bar { fn bar -> Baz { ... } }` — registers
    /// `Bar` as conforming to trait `Foo` and supplies the bodies.
    /// Methods land on `Bar` at codegen time so a `Bar` value can
    /// be called with the trait method directly. Retroactive
    /// conformance: `Bar` doesn't have to be declared in the same
    /// module.
    Impl,
    /// `arc Name { ... }` — a non-QObject class managed by
    /// `cute::Arc<T>` (atomic reference counting). No `< Super`
    /// clause (final by construction), no `signal` / `slot` (no
    /// QMetaObject), no `@qml_element`. The shape this used to
    /// take, `class X < Object { ... }`, is no longer accepted —
    /// the magic `Object` sentinel is gone.
    Arc,
    /// `store Name { ... }` — declarative singleton state object,
    /// inspired by Mint lang. Desugars (AST pre-pass) into a
    /// `class Name < QObject { ... }` with implicit `pub` on every
    /// member plus a top-level `let Name = Name.new()` that the
    /// existing `Q_GLOBAL_STATIC` post-pass at `cpp.rs:624` lifts
    /// to a process-lifetime singleton. `state` fields inside the
    /// body lower to `prop` (Q_PROPERTY + auto-NOTIFY) so QML
    /// bindings on `Name.field` re-render reactively.
    Store,
    /// `suite "X" { test "y" { body } ... }` — leading keyword for a
    /// test grouping. Each contained `test "y"` becomes a regular
    /// `is_test` fn whose runner-output display name is `"X / y"`.
    /// The legacy compact form `test fn camelCase { ... }` stays
    /// untouched (suite is purely additive sugar).
    Suite,
    /// `static fn name(...)` on a class — receiverless, callable as
    /// `ClassName.name(args)` and lowered to `ClassName::name(args)`.
    Static,

    // -- Punctuation ----------------------------------------------------
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Dot,
    /// `..` — exclusive range used in for-iterators: `0..N`.
    DotDot,
    /// `..=` — inclusive range: `0..=N`.
    DotDotEq,
    /// `?` — postfix nullable type, postfix error-propagate, or ternary head.
    Question,
    /// `!` — error-union prefix, or boolean not.
    Bang,
    Colon,
    DoubleColon,
    Semicolon,
    Newline,  // significant only as a statement separator inside blocks
    Pipe,     // `|`  — block-arg delimiter and bitwise or
    Amp,      // `&`
    Caret,    // `^`  — bitwise xor
    Arrow,    // `->`
    FatArrow, // `=>`
    Lt,
    LtEq,
    Gt,
    GtEq,
    EqEq,
    NotEq,
    Eq,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    AndAnd,
    OrOr,

    /// `<` used by generics. The lexer does not distinguish; the parser
    /// disambiguates contextually. Same `Lt` token is used.
    // -- Sentinels ------------------------------------------------------
    Eof,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StrSeg {
    /// Literal text (with escape sequences already expanded).
    Text(String),
    /// `#{ <byte range> }` interpolation. The span covers the inner
    /// expression bytes (between `#{` and `}`); the parser pulls the
    /// matching slice out of the source map and parses it as an expression.
    Interp(Span),
    /// `#{ <expr-bytes> : <format-spec> }` — a Python-style format-spec
    /// suffix attached to the interp. The `span` is just the expression
    /// part (the lexer has already split it from the spec); `format_spec`
    /// is the literal spec text without leading `:` (e.g. `.2f`, `08d`,
    /// `>20`). Codegen translates the spec to a target-specific call
    /// (e.g. `QString::number(v, 'f', 2)` for C++ output).
    InterpFmt { span: Span, format_spec: String },
}
