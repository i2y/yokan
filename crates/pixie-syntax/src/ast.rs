//! pixie concrete AST.
//!
//! Span-bearing nodes only - this layer is purely syntactic. Names are not
//! resolved here; that's HIR's job. `@var` is preserved as-is and normalized
//! to `self.var` later.

use crate::span::Span;

#[derive(Debug, Clone)]
pub struct Module {
    pub items: Vec<Item>,
    pub span: Span,
}

impl Module {
    /// The parameter list of a top-level fn by name. `None` when the
    /// fn isn't user-defined here (binding, builtin, undefined).
    fn fn_params(&self, fn_name: &str) -> Option<&[Param]> {
        for item in &self.items {
            if let Item::Fn(f) = item {
                if f.name.name == fn_name {
                    return Some(&f.params);
                }
            }
        }
        None
    }

    /// The parameter list of a method on a user class. Struct methods
    /// are out of scope here — no caller needs them yet.
    fn method_params(&self, class_name: &str, method_name: &str) -> Option<&[Param]> {
        for item in &self.items {
            if let Item::Class(c) = item {
                if c.name.name != class_name {
                    continue;
                }
                for m in &c.members {
                    if let ClassMember::Fn(f) | ClassMember::Slot(f) = m {
                        if f.name.name == method_name {
                            return Some(&f.params);
                        }
                    }
                }
            }
        }
        None
    }

    /// The `is_consuming` flag of each parameter of a top-level fn.
    /// Used by codegen and the linearity checker so a `consuming`
    /// parameter receives `std::move(x)` at the call site and the flow
    /// analysis marks the matching argument as Moved.
    pub fn fn_consuming_flags(&self, fn_name: &str) -> Option<Vec<bool>> {
        self.fn_params(fn_name)
            .map(|ps| ps.iter().map(|p| p.is_consuming).collect())
    }

    /// Same as `fn_consuming_flags` but for a method on a user class.
    pub fn method_consuming_flags(&self, class_name: &str, method_name: &str) -> Option<Vec<bool>> {
        self.method_params(class_name, method_name)
            .map(|ps| ps.iter().map(|p| p.is_consuming).collect())
    }

    /// The declared type of each parameter of a top-level fn. Codegen
    /// uses this to bias a `List<T>` / `Map<K,V>` literal argument
    /// toward the typed container instead of the heterogeneous
    /// `QVariant*` default a bare literal lowers to.
    pub fn fn_param_types(&self, fn_name: &str) -> Option<Vec<TypeExpr>> {
        self.fn_params(fn_name)
            .map(|ps| ps.iter().map(|p| p.ty.clone()).collect())
    }

    /// Same as `fn_param_types` but for a method on a user class.
    pub fn method_param_types(&self, class_name: &str, method_name: &str) -> Option<Vec<TypeExpr>> {
        self.method_params(class_name, method_name)
            .map(|ps| ps.iter().map(|p| p.ty.clone()).collect())
    }
}

#[derive(Debug, Clone)]
pub enum Item {
    Use(UseItem),
    /// `use qml "org.kde.kirigami" as Kirigami` — declares a foreign
    /// QML module dependency. The leading `qml` after `use` is what
    /// distinguishes this from the Cute-source-module `use foo` form;
    /// `qml` is treated as a contextual keyword, not a global one,
    /// so existing identifiers like `qml_app(...)` keep working.
    UseQml(UseQmlItem),
    Class(ClassDecl),
    Struct(StructDecl),
    Fn(FnDecl),
    /// Cute-side declarative UI tree. Lowers at codegen time to a `.qml`
    /// file embedded via qrc - the user never writes QML directly.
    /// Targets QtQuick (mobile / Plasma 6 / Material themes / animations).
    View(ViewDecl),
    /// Cute-side declarative QtWidgets tree. Lowers to imperative C++
    /// that constructs a QWidget hierarchy. Targets traditional desktop
    /// (KDE Plasma / OS-native look via QStyle, Qt Creator / Dolphin /
    /// LibreOffice-style apps). Same `Element` AST as views, but emit
    /// path is `new T(parent); parent->setX(...)` rather than QML text.
    Widget(WidgetDecl),
    /// `style Card { padding: 16, background: "#fff" }` — a named bag of
    /// (key, value) property assignments that can be applied to any
    /// element via `style: Card`. Composes with `+` (`style BigCard =
    /// Card + Big`) where right wins on conflict. Style values do not
    /// exist at runtime: every reference is inlined at codegen time.
    Style(StyleDecl),
    /// `trait Iterable { fn iter -> Iter }` — declares a nominal
    /// interface (Swift protocol / Rust trait). Methods are abstract
    /// signatures only; default bodies aren't supported in v1.
    Trait(TraitDecl),
    /// `impl Iterable for MyList { fn iter -> Iter { ... } }` —
    /// registers `MyList` as conforming to `Iterable` and supplies
    /// the bodies. Codegen emits each impl method as a regular
    /// member of the target class. Retroactive conformance is
    /// allowed: `MyList` may live in any module.
    Impl(ImplDecl),
    /// `let MaxLines : Int = 1000` at file scope. Top-level
    /// immutable binding visible from every fn / class body in the
    /// module. Value types (Int / Float / Bool / String / extern
    /// value classes / structs) lower to `static const auto X = value;`.
    /// QObject types lower to `Q_GLOBAL_STATIC(T, X)` for thread-safe
    /// lazy init. Top-level `var` is intentionally not supported in v1
    /// — module-level mutable state has too many static-init-order
    /// and thread-safety footguns.
    Let(LetDecl),
    /// `enum Color { Red; Green; Blue }` — user-defined enum with
    /// optional explicit values per variant (`Blue = 7`). Lowers to
    /// `enum class Color : qint32` in C++. Distinct type — Cute
    /// doesn't auto-convert to Int (use `c.rawValue` for the int).
    /// `extern enum Foo { Bar = 1 }` for C++ enum bindings; lowers
    /// to a name resolution against the C++ namespace declared in
    /// the typesystem (no C++ definition emitted).
    Enum(EnumDecl),
    /// `flags Alignment of AlignmentFlag` — declares a QFlags<E>
    /// counterpart of an existing enum. Allows `|` / `&` / `^` /
    /// `.has(v)` on the flags type; the underlying enum on its own
    /// rejects these (so `Color.Red | Color.Green` is a type error).
    /// Lowers to `QFlags<E>` + a `Q_DECLARE_OPERATORS_FOR_FLAGS`
    /// counterpart at codegen time.
    Flags(FlagsDecl),
    /// `store Name { ... }` — declarative singleton state object,
    /// inspired by Mint lang. The pre-pass `desugar_store` lowers
    /// this into a regular `Item::Class` (QObject-derived, every
    /// member implicitly `pub`) plus a top-level `Item::Let` that
    /// the existing `Q_GLOBAL_STATIC` post-pass at `cpp.rs:624`
    /// lifts to a process-lifetime singleton. Use for global app
    /// state where parent-injection footguns and silent-leak lints
    /// both bite (current user, theme, navigation routes).
    Store(StoreDecl),
    /// `suite "X" { test "y" { body } ... }` — declarative test
    /// grouping (Mint-inspired). Each contained `test "y"` is a
    /// regular `FnDecl` with `is_test: true` and `display_name:
    /// Some("X / y")` so the runner emits `ok N - X / y` in TAP
    /// output. The compact `test fn camelCase { ... }` form (the
    /// only shape pre-1.x) stays available unchanged at the top
    /// level. Nested suites are not supported in v1.x — at most
    /// one level of grouping.
    Suite(SuiteDecl),
}

/// Top-level `let X : T = expr` declaration. Lives at module scope,
/// visible to every fn / class body in the same module. Always
/// immutable — the keyword `let` is required, `var` is intentionally
/// rejected at parse time.
#[derive(Debug, Clone)]
pub struct LetDecl {
    pub name: Ident,
    pub ty: TypeExpr,
    pub value: Expr,
    pub is_pub: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ViewDecl {
    pub name: Ident,
    /// Exposed parameters: `view Card(label: String, count: Int) { ... }`.
    /// Each param surfaces at the QML root as a typed `property`, so
    /// other views can instantiate the component with `Card { label:
    /// "..."; count: 42 }`. Empty for parameterless views.
    pub params: Vec<Param>,
    /// SwiftUI-style state declarations: `view Main { let counter =
    /// Counter(); ApplicationWindow { ... } }`. Each `let <name> =
    /// <expr>` line at the head of the body becomes a state field
    /// owned by this view. View lowering injects each as a QML
    /// `<Class> { id: <name> }` sibling at the root so QML binding
    /// can reach `<name>.x` from anywhere in the tree.
    pub state_fields: Vec<StateField>,
    pub root: Element,
    /// `pub view Foo { ... }` exports the view to other modules. Default
    /// (false) means the view is module-private. Views are typically
    /// only meaningful from the entry file's module so most demos won't
    /// need `pub`, but the field exists for parity with classes.
    pub is_pub: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct WidgetDecl {
    pub name: Ident,
    pub params: Vec<Param>,
    /// Same SwiftUI-style state declarations as `ViewDecl`. Widget
    /// lowering emits each as a C++ class member (initialized in the
    /// constructor before the tree is built) so handlers / setters
    /// inside the tree can reference them by name.
    pub state_fields: Vec<StateField>,
    pub root: Element,
    pub is_pub: bool,
    pub span: Span,
}

/// State declaration at the head of a `view` / `widget` body. Two
/// flavors:
///
/// - **`let <name> = <init_expr>`** (`StateFieldKind::Object`) —
///   sub-QObject state, equivalent to SwiftUI's `@StateObject`.
///   `init_expr` is typically a `Class()` constructor call.
/// - **`state <name> : <ty> = <init_expr>`** (`StateFieldKind::Property`) —
///   primitive reactive cell, equivalent to SwiftUI's `@State`. Lowers
///   to a QML root-level `property <ty> <name>: <init>`. The auto-
///   generated `<name>Changed` signal makes assignment trigger
///   bindings without a wrapper class.
#[derive(Debug, Clone)]
pub struct StateField {
    pub name: Ident,
    pub kind: StateFieldKind,
    pub init_expr: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum StateFieldKind {
    /// `let <name> = <init_expr>` — the existing form. Init is a
    /// QObject-class constructor call; the codegen instantiates the
    /// child as a sub-object of the view/widget.
    Object,
    /// `state <name> : <ty> = <init_expr>` — primitive Q_PROPERTY-
    /// style cell. The type is required and lowers to a QML property
    /// type (Int → int, Double → real, Bool → bool, String → string).
    Property { ty: TypeExpr },
}

#[derive(Debug, Clone)]
pub struct Element {
    /// Module qualifiers from `model.Counter { ... }` syntax. Empty
    /// for the common unqualified form (`Counter { ... }`). When
    /// non-empty, name resolution targets the corresponding pub item
    /// in the named module instead of the local / prelude path. The
    /// QML / C++ codegen still emits `name` as the type name -
    /// modules currently don't influence the registered Cute-side
    /// type identity, only the resolution path.
    pub module_path: Vec<Ident>,
    pub name: Ident,
    pub members: Vec<ElementMember>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ElementMember {
    /// `text: "hello"` or `anchors.centerIn: parent` (dotted keys joined
    /// with `.` into a single string at parse time so codegen can pass
    /// them through to QML verbatim).
    Property {
        key: String,
        value: Expr,
        span: Span,
    },
    /// Nested element: `Column { Text { ... } Button { ... } }`.
    Child(Element),
    /// A regular Cute statement (`if cond { El }`, `for x in xs { El }`,
    /// `case x { when p { El } }`, `let x = ...`) embedded as a member
    /// of the surrounding element. The intended shape is one of:
    ///   - `Stmt::Expr(ExprKind::If { ... })`     - conditional render
    ///   - `Stmt::Expr(ExprKind::Case { ... })`   - pattern render
    ///   - `Stmt::For { ... }`                    - repeater
    ///   - `Stmt::Let { ... }` / `Stmt::Var { ... }` - local binding
    /// Each branch / arm / loop body is a `Block` whose trailing
    /// expression is `ExprKind::Element(...)`; codegen pulls those
    /// out and renders them. Reusing the language-core `Stmt` /
    /// `ExprKind::If` / `ExprKind::Case` here means new control-flow
    /// features (e.g. `while`, guard expressions) automatically
    /// become available in element bodies without parallel AST
    /// nodes.
    Stmt(Stmt),
}

#[derive(Debug, Clone)]
pub struct UseItem {
    /// Path to the source module. `["foo"]` for `use foo`,
    /// `["foo", "bar"]` for `use foo.bar`. The last segment is the
    /// module name (file stem); preceding segments form the
    /// directory path under the project root.
    pub path: Vec<Ident>,
    /// Shape of the import: whole module (with optional alias), or
    /// a selective list of names to bring into local scope.
    pub kind: UseKind,
    /// `pub use foo.X` re-exports the imported name from the
    /// declaring module, making it visible to downstream importers.
    pub is_pub: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum UseKind {
    /// `use foo` or `use foo as bar`. `None` means the local module
    /// name is the leaf segment of `path`; `Some(ident)` overrides
    /// the local name. The whole module's pub items become reachable
    /// as bare names AND through the local module qualifier.
    Module(Option<Ident>),
    /// `use foo.{X, Y as Z}`. Only the listed names are brought into
    /// local scope; the module itself is not added to the qualifier
    /// set.
    Names(Vec<UseName>),
}

/// `use qml "uri" [as Alias]` — declares a foreign QML module
/// import. The compiler keeps an internal table mapping known URIs
/// (`org.kde.kirigami`, …) to their bundled `.qpi` binding + default
/// version. URIs not in the table type-check soft-pass: the import
/// line is still emitted into the generated QML so the runtime can
/// load it, but property / signal names are unchecked.
#[derive(Debug, Clone)]
pub struct UseQmlItem {
    /// QML module URI, e.g. `"org.kde.kirigami"`. Quoted in source so
    /// the lexer's existing string handling carries it.
    pub module_uri: String,
    /// Optional namespace alias. `Some(Kirigami)` lets the user write
    /// `Kirigami.PageRow { ... }` and emits `import ... as Kirigami`.
    /// `None` means flat names (`PageRow { ... }`) and an unaliased
    /// QML import; the binding loads with bare class names instead
    /// of the namespace-mangled form.
    pub alias: Option<Ident>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct UseName {
    /// The original name as declared in the source module.
    pub name: Ident,
    /// `Some(alias)` for `X as A`; `None` for the bare `X` form, in
    /// which case `name` is also the local name.
    pub alias: Option<Ident>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ClassDecl {
    pub name: Ident,
    pub generics: Vec<GenericParam>,
    pub super_class: Option<TypeExpr>,
    pub members: Vec<ClassMember>,
    /// `pub class Foo { ... }` exports the class to other modules.
    /// Default (false) means the class is private to its declaring
    /// `.cute` file's module. The visibility check in HIR rejects
    /// cross-module references to non-pub classes.
    pub is_pub: bool,
    /// `extern value Foo { ... }` — declares a plain C++ value type
    /// (QColor, QPoint, ...). No QObject ancestry, no Arc wrapper,
    /// `T.new(args)` lowers to `T(args)` (stack/value construction),
    /// and member access uses `.` instead of `->`. Codegen never
    /// emits a definition for these (they live in C++ headers
    /// pulled in via `[cpp] includes`); the binding only describes
    /// the type surface for type-check.
    pub is_extern_value: bool,
    /// `arc Foo { ... }` — Cute's ARC (atomic reference counting)
    /// class form. Lowers to `class Foo : public ::cute::ArcBase`
    /// and `T.new(...)` returns `cute::Arc<Foo>`. Mutually
    /// exclusive with `is_extern_value`. The grammar enforces no
    /// `< Super` clause and rejects `signal` / `slot` members at
    /// parse time (those require QObject's QMetaObject machinery).
    /// Replaces the older `class X < Object` form.
    pub is_arc: bool,
    /// `: ~Copyable` annotation — the type opts into linear semantics:
    /// codegen emits `X(const X&) = delete; operator=(const X&) = delete;`
    /// + defaulted move ops, and the type checker enforces use-after-move
    /// detection on bindings of this type. Default `true` (regular
    /// copyable).
    pub is_copyable: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct StructDecl {
    pub name: Ident,
    /// `struct Point @rust("geo::Point") { .. }` in a `.rpi` — the
    /// Rust type this one corresponds to (§8.77). The fields carry
    /// their own halves of the correspondence.
    pub rust_path: Option<String>,
    /// `struct Pair<T> { … }` — value-type generics (§8.25). Rustc
    /// infers instantiations at the positional constructor.
    pub generics: Vec<GenericParam>,
    pub fields: Vec<Field>,
    /// `fn name(args) -> R { body }` declared inside a struct body.
    /// Lowers to inline C++ member functions on the emitted struct;
    /// the body sees `self` as a pointer to the struct instance, so
    /// `self.x` works the same way as on classes (the field-access
    /// path skips the trailing `()` because struct fields are
    /// plain C++ members rather than getter methods). Default-empty
    /// for structs that only carry data.
    pub methods: Vec<FnDecl>,
    pub is_pub: bool,
    /// `struct X: ~Copyable { ... }` — see `ClassDecl::is_copyable`
    /// for the full semantics. Default `true`.
    pub is_copyable: bool,
    pub span: Span,
}

/// `enum Color { Red; Green; Blue = 7 }` — user-defined enum, or
/// `extern enum Foo { Bar = 1 }` — binding to a C++ enum.
///
/// User-defined enums emit as `enum class <Name> : qint32 { ... }`
/// in C++ (the `qint32` underlying makes the layout deterministic
/// for moc / Q_PROPERTY purposes). Extern enums emit no
/// definition — the C++ name is assumed to already exist via
/// `[cpp] includes` or stdlib bindings. The typesystem.toml that
/// drives `cute-qpi-gen` declares the C++ namespace prefix
/// (`Qt::`, `QSlider::`, ...) so codegen can lower
/// `AlignmentFlag.AlignLeft` to `Qt::AlignLeft` correctly.
#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: Ident,
    /// `enum Color @rust("palette::Color") { .. }` in a `.rpi` — the
    /// Rust type this one corresponds to (§8.74). The variants carry
    /// their own halves of the correspondence.
    pub rust_path: Option<String>,
    pub variants: Vec<EnumVariant>,
    pub is_pub: bool,
    /// `extern enum Foo { ... }` — Cute owns no C++ definition,
    /// just registers the type and its variants for resolution.
    /// Set when the leading `extern` keyword is present.
    pub is_extern: bool,
    /// `error E { ... }` parses as `EnumDecl` with this flag set.
    /// Semantics: same sum-type shape as plain `enum`, plus auto-
    /// registration as the module's default `!T` err type when
    /// it's the only is_error decl in the module. Lets `case x()
    /// { when err(e) { ... } }` resolve `e` to this enum's type.
    pub is_error: bool,
    /// Optional C++ namespace / enclosing scope for extern enums.
    /// `Qt`, `QSlider`, `Qt::DateFormat`, etc. Only meaningful when
    /// `is_extern` is true; codegen prefixes each variant access
    /// (`AlignmentFlag.AlignLeft` → `Qt::AlignLeft`). Set by the
    /// .qpi parser from the file's `#cpp_namespace ...` pragma or
    /// the typesystem.toml's per-class entry; user-side `enum`
    /// blocks leave this empty.
    pub cpp_namespace: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: Ident,
    /// `red @rust("Red")` — the Rust variant this one maps to
    /// (§8.74). `None` means the names already agree.
    pub rust_name: Option<String>,
    /// Optional explicit value: `Red = 1`. Mutually exclusive with
    /// `fields` (a payload-bearing variant doesn't carry a discriminator
    /// integer at the Cute surface). Stored as an Expr so `Red = 1 <<
    /// 0` / `Mask = Foo | Bar` parse; for simple integer cases the
    /// type checker resolves to a constant value.
    pub value: Option<Expr>,
    /// Payload fields: `Node(left: Tree, right: Tree)`. Empty for
    /// nullary variants (`Red`). When any variant in the enum has
    /// non-empty `fields`, codegen lowers the whole enum to a
    /// `std::variant<...>` tagged union (same shape as `error E
    /// { ... }`); otherwise the simpler `enum class : qint32`
    /// form is used.
    pub fields: Vec<Field>,
    pub is_pub: bool,
    pub span: Span,
}

/// `flags Alignment of AlignmentFlag` — QFlags<E>-shaped type
/// declared on top of an existing enum. Allows `|` / `&` / `^` /
/// `.has(v)` operations on the flags type; the underlying enum on
/// its own rejects bitwise ops. Lowers to `QFlags<E>` + a
/// `Q_DECLARE_OPERATORS_FOR_FLAGS` companion in C++ (extern flags
/// skip the declaration but keep the type-level distinction).
#[derive(Debug, Clone)]
pub struct FlagsDecl {
    pub name: Ident,
    /// The underlying enum's Cute name (`AlignmentFlag` for
    /// `flags Alignment of AlignmentFlag`). Resolved against the
    /// enum table at type-check time.
    pub of: Ident,
    pub is_pub: bool,
    pub is_extern: bool,
    /// C++ namespace for the QFlags<...> alias when extern.
    pub cpp_namespace: Option<String>,
    pub span: Span,
}

/// `suite "X" { test "y" { body } ... }` — Mint-inspired test
/// grouping. The parser produces this directly; the codegen test
/// runner walks both top-level `Item::Fn(is_test)` AND each suite's
/// `tests` so a TAP run sees them in source order. There's no
/// HIR / type-check / desugar layer for `Suite` itself — each
/// `tests[i]` is a regular `FnDecl` that those passes already
/// understand.
#[derive(Debug, Clone)]
pub struct SuiteDecl {
    /// The suite's display label, taken verbatim from the string
    /// literal in `suite "X" { ... }`. Used as the leading half of
    /// each test's TAP-output display name (`"X / y"`).
    pub name: String,
    /// Span of the suite's name string literal — used by `cute fmt`
    /// + LSP for hover / go-to-definition surfaces. Distinct from
    /// `span` (which covers the whole `suite "X" { ... }` block).
    pub name_span: Span,
    /// One entry per `test "y" { ... }` inside the body. Each
    /// `FnDecl` carries `is_test: true` + `display_name:
    /// Some("X / y")`. The actual `name` field is a synth
    /// identifier the codegen mangles into the C++ runner.
    pub tests: Vec<FnDecl>,
    pub span: Span,
}

/// `store Name { ... }` — declarative singleton state object. The
/// `desugar_store` pre-pass converts this into a regular `ClassDecl`
/// (super = QObject, every member forced `pub`) plus a top-level
/// `LetDecl` that the existing Q_GLOBAL_STATIC post-pass picks up.
#[derive(Debug, Clone)]
pub struct StoreDecl {
    pub name: Ident,
    /// `state X : T = init` declarations at the head of the body.
    /// Each lowers at desugar time to a `(PropertyDecl with notify
    /// + default, SignalDecl)` pair — the same pattern view/widget
    /// state-fields use. Always `StateFieldKind::Property`; the
    /// Object-kind (`let X = sub_obj()`) form is rejected at parse
    /// time because a singleton has no parent-injection lifetime
    /// model for sub-QObjects.
    pub state_fields: Vec<StateField>,
    /// Classic class members (fn, prop, signal, slot, init, deinit,
    /// var, let) in source order. Desugar inserts these unchanged
    /// into the synthesized class with implicit `pub` forced on
    /// every member — singletons are inherently global.
    pub members: Vec<ClassMember>,
    /// `pub store Foo { ... }` exports both the synthesized class
    /// and the `let Foo = Foo.new()` singleton across module
    /// boundaries. Default (false) keeps both private to the
    /// declaring module.
    pub is_pub: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct StyleDecl {
    pub name: Ident,
    pub body: StyleBody,
    pub is_pub: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum StyleBody {
    /// `style Name { key: val, key: val }` — direct (key, value) entries.
    /// Keys may be dotted (`font.bold`); the parser stores them as a
    /// pre-joined `String` so codegen can pass them through to QML
    /// verbatim or transform to setter form for QtWidgets.
    Lit(Vec<StyleEntry>),
    /// `style Name = <expr>` — alias / merge. The RHS reduces to a
    /// composition of style names and `+`, e.g. `Card + Big + Compact`.
    /// Resolved against the project's style table at codegen time.
    Alias(Expr),
}

#[derive(Debug, Clone)]
pub struct StyleEntry {
    pub key: String,
    pub value: Expr,
    pub span: Span,
}

/// `trait Iterable { fn iter -> Iter }`.
///
/// `methods` reuses `FnDecl` so a trait method signature looks like
/// any other fn decl, just with `body: None`. Default bodies aren't
/// supported in v1; the parser rejects a body inside a trait block.
#[derive(Debug, Clone)]
pub struct TraitDecl {
    pub name: Ident,
    pub methods: Vec<FnDecl>,
    pub is_pub: bool,
    pub span: Span,
}

/// `impl Iterable for MyList { fn iter -> Iter { ... } }`.
///
/// `for_type` is the implementing type expression. The simple form
/// (`impl Iterable for MyList`) names a class declared in user
/// code or an extern type from the bindings. The parametric form
/// (`impl<T> Iterable for List<T>` / `impl Iterable for Box<Int>`)
/// uses the impl-level `generics` to bind type variables that the
/// for-type expression references. Each method must have
/// `body: Some(...)` — the parser rejects an abstract method
/// inside an impl block.
#[derive(Debug, Clone)]
pub struct ImplDecl {
    pub trait_name: Ident,
    pub for_type: TypeExpr,
    /// Impl-level generic params (`impl<T> Foo for Bar<T>`). Empty
    /// for the simple form. Type variables introduced here are in
    /// scope only inside `for_type` and the impl methods' bodies.
    pub generics: Vec<GenericParam>,
    pub methods: Vec<FnDecl>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FnDecl {
    pub is_async: bool,
    pub name: Ident,
    pub generics: Vec<GenericParam>,
    pub params: Vec<Param>,
    pub return_ty: Option<TypeExpr>,
    pub body: Option<Block>, // None = abstract / external
    pub is_pub: bool,
    /// `true` for `test fn name { ... }` declarations. The codegen
    /// `cute test` mode collects these into a runner main.
    pub is_test: bool,
    /// `Some("compute / adds positive numbers")` for the string-
    /// named form (`test "adds positive numbers"` inside
    /// `suite "compute" { ... }`, or top-level `test "y" { ... }`).
    /// `None` for the compact `test fn camelCase` form and for any
    /// non-test fn. The TAP runner uses this in place of `name.name`
    /// so users see their original prose in the `ok N - …` line.
    pub display_name: Option<String>,
    /// `@name(args...)` markers between the return type and the body.
    /// Used so far by `@lifted_bool_ok` on Qt binding fns, where the
    /// codegen synthesizes a `bool*-ok` wrapper at the call site. Empty
    /// for user-written fns that don't need any.
    pub attributes: Vec<Attribute>,
    /// `true` for `static fn name(...)` declarations inside class /
    /// arc / struct / extern-value bodies and inside `.qpi` bindings.
    /// Static fns carry no implicit `self` and are callable as
    /// `ClassName.fn_name(args)` without an instance receiver. The
    /// codegen lowers these calls to `ClassName::fn_name(args)`
    /// (free static member). `false` for instance methods and for
    /// top-level free functions (where the concept doesn't apply).
    pub is_static: bool,
    pub span: Span,
}

/// `@name` or `@name(arg1, arg2, ...)` annotation on an `fn` decl.
/// Args are kept as raw source strings for now (no inner expression
/// parse). The full attribute set rides on `FnDecl::attributes`.
#[derive(Debug, Clone)]
pub struct Attribute {
    pub name: Ident,
    pub args: Vec<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ClassMember {
    Property(PropertyDecl),
    Signal(SignalDecl),
    Slot(FnDecl),
    Fn(FnDecl),
    Field(Field),
    /// `init(params) { body }` — user-defined constructor. Multiple
    /// inits per class are allowed; overload resolution happens at
    /// `T.new(args)` call sites.
    Init(InitDecl),
    /// `deinit { body }` — user-defined destructor. At most one per
    /// class.
    Deinit(DeinitDecl),
}

#[derive(Debug, Clone)]
pub struct InitDecl {
    pub params: Vec<Param>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct DeinitDecl {
    pub body: Block,
    pub span: Span,
}

impl ClassMember {
    /// Toggle the member's visibility flag. `init` / `deinit` carry
    /// no `is_pub` (always-public by construction) and are no-ops.
    pub fn set_pub(&mut self, value: bool) {
        match self {
            ClassMember::Property(p) => p.is_pub = value,
            ClassMember::Signal(s) => s.is_pub = value,
            ClassMember::Slot(f) | ClassMember::Fn(f) => f.is_pub = value,
            ClassMember::Field(f) => f.is_pub = value,
            ClassMember::Init(_) | ClassMember::Deinit(_) => {}
        }
    }
}

impl ClassDecl {
    pub fn inits(&self) -> impl Iterator<Item = &InitDecl> {
        self.members.iter().filter_map(|m| match m {
            ClassMember::Init(i) => Some(i),
            _ => None,
        })
    }

    pub fn deinit(&self) -> Option<&DeinitDecl> {
        self.members.iter().find_map(|m| match m {
            ClassMember::Deinit(d) => Some(d),
            _ => None,
        })
    }
}

#[derive(Debug, Clone)]
pub struct PropertyDecl {
    pub name: Ident,
    pub ty: TypeExpr,
    /// `weak prop x : C` — a reference that is NOT a counted edge.
    /// Reintroduced for pixie in §8.44: refcounting made cycles the
    /// one thing that leaks, and this is how an author breaks one.
    /// It costs no runtime machinery, because a pixie handle already
    /// observes staleness (§3.4) — `weak` only changes the count.
    pub weak: bool,
    pub notify: Option<Ident>, // `notify: :foo` -> Ident("foo")
    pub default: Option<Expr>,
    /// Class-member visibility. Defaults to `true` (public) to match
    /// Qt convention - properties exist to be QML-bindable. The
    /// `pub` keyword on a class member is accepted explicitly and
    /// reserved for future fine-grained control.
    pub is_pub: bool,
    /// Opt-in to Qt 6's `QObjectBindableProperty` storage. AtIdent
    /// reads (`@x`) lower to `m_x.value()` so the binding system records
    /// dependencies; writes (`@x = v`) go through `operator=` →
    /// `setValue`, which fires NOTIFY and invalidates dependent
    /// bindings.
    pub bindable: bool,
    /// `bind { expr }` body — a derived (read-only) property whose
    /// value is `expr`, re-evaluated automatically when any property
    /// it reads changes. Lowers to `QObjectBindableProperty` +
    /// `setBinding(lambda)`: dependency tracking is automatic via Qt
    /// 6's binding system, and the result is cached + invalidated
    /// lazily. Mutually exclusive with `default`, `notify`, `fresh`.
    pub binding: Option<Expr>,
    /// `fresh { expr }` body — a function-like property: every read
    /// re-evaluates `expr`, no caching, no automatic dep tracking.
    /// Lowers to `QObjectComputedProperty`. Useful when the expression
    /// reads state Qt's binding system can't observe (file size,
    /// current time, third-party getters). The synthesized
    /// `<x>_changed` notify is fanned out from input bindables in the
    /// constructor so QML/QtWidget bindings still pick up changes when
    /// the deps happen to be bindable. Mutually exclusive with
    /// `default`, `bind`, `bindable`.
    pub fresh: Option<Expr>,
    /// `, model` — additionally synthesize an item-model accessor for
    /// this collection-typed property. Codegen emits a sibling
    /// `<name>_model` Q_PROPERTY (typed `QAbstractItemModel*`) backed
    /// by a lazy `QRangeModel` that wraps the underlying storage. The
    /// raw collection prop is unchanged; the flag only adds a second
    /// accessor for QML view consumers (`Repeater { model:
    /// store.items_model }`). Concept-named (not `range_model`) so
    /// future implementation choices — `QStringListModel` for
    /// `List<String>`, table models for `List<List<T>>`, future Qt
    /// adapters — can swap in without changing user source.
    /// Mutually exclusive with `bind`, `fresh`, and `bindable` in v1
    /// (the storage shape needs to stay a plain `QList<T*>` so the
    /// QRangeModel ctor can wrap it directly).
    pub model: bool,
    /// `, constant` — opts the property out of the auto-derived
    /// `<propName>Changed` notify. Lowers to a Qt `CONSTANT`
    /// Q_PROPERTY (no NOTIFY clause emitted, no signal synthesized).
    /// Mutually exclusive with explicit `notify:`, `bindable`,
    /// `bind { ... }`, `fresh { ... }` (none of which make sense
    /// without a change-event).
    ///
    /// The default for plain `pub prop X : T = V` is *now* "auto-
    /// synthesize the conventional `<X>Changed` notify"; `, constant`
    /// is the explicit opt-out for storage that genuinely never
    /// changes (read-only metadata exposed to QML, etc.).
    pub constant: bool,
    pub span: Span,
    /// Source-block identifier for `prop ( ... )` block sugar. All
    /// items declared in the same block share the same `Some(id)`;
    /// items declared individually carry `None`. The formatter uses
    /// this to re-emit grouped decls as the original block form
    /// rather than expanding to one-per-line. IDs are unique per
    /// parse and have no semantic meaning beyond grouping — HIR /
    /// type-check / codegen all ignore the field.
    pub block_id: Option<u32>,
}

impl PropertyDecl {
    /// True iff the prop is backed by a Qt 6 property class
    /// (QObjectBindableProperty or QObjectComputedProperty) — i.e.
    /// any of `bindable`, `bind { ... }`, `fresh { ... }`. Kept on
    /// the AST type so HIR / type-check / codegen all agree on the
    /// classification.
    pub fn is_bindable_surface(&self) -> bool {
        self.bindable || self.binding.is_some() || self.fresh.is_some()
    }

    /// Convenience wrapper around the free `synth_notify_name`
    /// helper that takes a prop name. Kept on `PropertyDecl` so
    /// existing call sites (HIR signal_names walk + codegen
    /// metaobject emission) read naturally.
    pub fn synth_notify_name(&self) -> String {
        synth_notify_name(&self.name.name)
    }
}

/// Conventional synthesized NOTIFY signal name for a prop — camelCase
/// `<propName>Changed` to match Qt's signal naming (e.g. `textChanged`,
/// `enabledChanged`). The first letter of the prop name is lowercased
/// so a prop `Count` synthesises `countChanged`. Centralised so the
/// HIR signal_names walk, codegen metaobject emission, and the
/// `desugar_state` / `desugar_store` synth class builders can't drift.
pub fn synth_notify_name(prop_name: &str) -> String {
    let mut out = String::with_capacity(prop_name.len() + "Changed".len());
    let mut chars = prop_name.chars();
    if let Some(first) = chars.next() {
        for c in first.to_lowercase() {
            out.push(c);
        }
        out.extend(chars);
    }
    out.push_str("Changed");
    out
}

#[derive(Debug, Clone)]
pub struct SignalDecl {
    pub name: Ident,
    pub params: Vec<Param>,
    pub is_pub: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: Ident,
    /// `var x : Float @rust("x")` — the Rust field this one maps to
    /// (§8.77). `None` means the names already agree.
    pub rust_name: Option<String>,
    /// `var len : Int @rust("len: u64")` — the Rust field's TYPE,
    /// when it is not the one this pixie type writes by default
    /// (§8.78). Reading widens on its own; writing back needs the
    /// width named.
    pub rust_ty: Option<String>,
    pub ty: TypeExpr,
    pub default: Option<Expr>,
    pub is_pub: bool,
    /// `let` field → `false` (immutable; writable exactly once in `init`) /
    /// `var` field → `true` (assignable from class methods via `x = v`).
    /// Mutability bit for plain class fields not promoted to a Q_PROPERTY.
    pub is_mut: bool,
    /// `weak let/var x : T?` — non-owning reference. Same semantics
    /// as the property-level flag; type must be `T?`.
    pub weak: bool,
    /// `unowned let/var x : T` — non-owning, non-null reference; the
    /// pointee must outlive this object. Only valid for arc-class
    /// targets.
    pub unowned: bool,
    pub span: Span,
    /// Source-block identifier for `let ( ... )` / `var ( ... )`
    /// block sugar. See `PropertyDecl::block_id` for the protocol.
    pub block_id: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: Ident,
    pub ty: TypeExpr,
    pub default: Option<Expr>,
    /// `@escaping` annotation. Only meaningful when `ty` is a
    /// `TypeKind::Fn`. When false (the default for closure-typed
    /// params), the parameter lowers to `cute::function_ref<F>` —
    /// non-owning, must not outlive the caller's lambda. When true,
    /// the parameter lowers to the heavier owning `std::function<F>`
    /// (allocation + copy semantics) so the callee may store / return
    /// / forward the closure freely. Type-check rejects storing a
    /// non-escaping closure into anything that escapes.
    pub is_escaping: bool,
    /// `consuming` annotation. The argument is moved into the callee:
    /// C++ signature lowers to `T&&`, and the call site emits
    /// `std::move(x)`. Pairs with `~Copyable` types — flow analysis
    /// rejects use-after-move for any binding passed as a consuming
    /// argument.
    pub is_consuming: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

/// One entry in a `<T, U: Iterable + Comparable>` parameter list.
/// Bounds are stored as raw idents (e.g. `Iterable`); the type
/// checker doesn't enforce them yet — a follow-up session will wire
/// `trait` declarations and bound-resolved member lookup.
#[derive(Debug, Clone)]
pub struct GenericParam {
    pub name: Ident,
    pub bounds: Vec<Ident>,
    pub span: Span,
}

impl GenericParam {
    /// Convenience for callers that only care about the parameter
    /// name (the historical `Vec<Ident>` shape).
    pub fn ident_name(&self) -> &str {
        &self.name.name
    }
}

// ---- Types ----------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TypeExpr {
    pub kind: TypeKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum TypeKind {
    /// `Foo`, `Foo.Bar`, `Foo<T>`.
    Named {
        path: Vec<Ident>,
        args: Vec<TypeExpr>,
    },
    /// `T?`.
    Nullable(Box<TypeExpr>),
    /// `!T` - error union with the surrounding context's error type.
    ErrorUnion(Box<TypeExpr>),
    /// Future-only function type sketch. Not parsed yet.
    Fn {
        params: Vec<TypeExpr>,
        ret: Box<TypeExpr>,
    },
    /// `Future(T)` etc. modeled as Named for now; reserved for future use.
    SelfType,
}

/// Pretty-print a `TypeExpr` back to Cute surface syntax. Used by
/// the AST tree-printer (debug rendering) and by diagnostics that
/// want to refer to a type by what the user wrote (`List<T>` vs.
/// the bare base name).
pub fn type_expr_render(t: &TypeExpr) -> String {
    match &t.kind {
        TypeKind::Named { path, args } => {
            let head = path
                .iter()
                .map(|i| i.name.as_str())
                .collect::<Vec<_>>()
                .join(".");
            if args.is_empty() {
                head
            } else {
                let arg_s = args
                    .iter()
                    .map(type_expr_render)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{head}<{arg_s}>")
            }
        }
        TypeKind::Nullable(inner) => format!("{}?", type_expr_render(inner)),
        TypeKind::ErrorUnion(inner) => format!("!{}", type_expr_render(inner)),
        TypeKind::Fn { params, ret } => {
            let p = params
                .iter()
                .map(type_expr_render)
                .collect::<Vec<_>>()
                .join(", ");
            format!("fn({p}) -> {}", type_expr_render(ret))
        }
        TypeKind::SelfType => "Self".to_string(),
    }
}

/// The placeholder name the parser emits for `Self`. Lives as
/// `Named { path: [Ident("Self")], args: [] }` rather than a
/// dedicated `TypeKind::SelfType` variant — see
/// `substitute_self_type_expr` for the substitution path.
pub const SELF_TYPE_NAME: &str = "Self";

/// Walk `t`, replacing every reference to the `Self` placeholder
/// with `recv`. Used by codegen at impl-method emission to rewrite
/// trait method signatures whose return / param types reference
/// `Self` to the impl's concrete for-type.
pub fn substitute_self_type_expr(t: &TypeExpr, recv: &TypeExpr) -> TypeExpr {
    match &t.kind {
        TypeKind::SelfType => recv.clone(),
        TypeKind::Named { path, args } => {
            if path.len() == 1 && path[0].name == SELF_TYPE_NAME && args.is_empty() {
                return recv.clone();
            }
            TypeExpr {
                kind: TypeKind::Named {
                    path: path.clone(),
                    args: args
                        .iter()
                        .map(|a| substitute_self_type_expr(a, recv))
                        .collect(),
                },
                span: t.span,
            }
        }
        TypeKind::Nullable(inner) => TypeExpr {
            kind: TypeKind::Nullable(Box::new(substitute_self_type_expr(inner, recv))),
            span: t.span,
        },
        TypeKind::ErrorUnion(inner) => TypeExpr {
            kind: TypeKind::ErrorUnion(Box::new(substitute_self_type_expr(inner, recv))),
            span: t.span,
        },
        TypeKind::Fn { params, ret } => TypeExpr {
            kind: TypeKind::Fn {
                params: params
                    .iter()
                    .map(|p| substitute_self_type_expr(p, recv))
                    .collect(),
                ret: Box::new(substitute_self_type_expr(ret, recv)),
            },
            span: t.span,
        },
    }
}

/// Helper variant: substitute Self over an entire `FnDecl`'s param
/// types and return type, returning a new FnDecl. Used by codegen at
/// impl-method emission and the inline-into-class splice. The body is
/// passed through unchanged (it references `self` lowercase, not
/// `Self` capital, so codegen handles it via the existing receiver
/// path).
pub fn substitute_self_in_fn_decl(decl: &FnDecl, recv: &TypeExpr) -> FnDecl {
    let mut out = decl.clone();
    for p in &mut out.params {
        p.ty = substitute_self_type_expr(&p.ty, recv);
    }
    if let Some(t) = &mut out.return_ty {
        *t = substitute_self_type_expr(t, recv);
    }
    out
}

/// Extract the simple base name of a `TypeExpr`. `List<T>` → `List`,
/// `MyClass` → `MyClass`, `Foo.Bar` → `Bar`, `T?` → unwrap, `!T` →
/// unwrap. Returns `None` for fn types and the bare `Self`
/// keyword (no useful base).
///
/// Drives the `impls_for` registry key in HIR — both
/// `impl<T> Foo for List<T>` and `impl Foo for List<Int>` share the
/// `"List"` slot. The trait surface check at call sites then
/// matches on base, not full type-arg shape.
pub fn type_expr_base_name(t: &TypeExpr) -> Option<String> {
    match &t.kind {
        TypeKind::Named { path, .. } => path.last().map(|i| i.name.clone()),
        TypeKind::Nullable(inner) | TypeKind::ErrorUnion(inner) => type_expr_base_name(inner),
        TypeKind::Fn { .. } | TypeKind::SelfType => None,
    }
}

// ---- Expressions / Statements ---------------------------------------------

#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    /// If the final statement is an expression with no terminator, it is the
    /// block's value. This is the standard Ruby/Rust idiom.
    pub trailing: Option<Box<Expr>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let {
        name: Ident,
        ty: Option<TypeExpr>,
        value: Expr,
        span: Span,
        /// Source-block id for `let ( ... )` block sugar inside fn /
        /// lambda / nested-block bodies. `Some(id)` for items that
        /// shared a block; `None` for individual decls. Only used by
        /// the formatter for re-emission grouping.
        block_id: Option<u32>,
    },
    Var {
        name: Ident,
        ty: Option<TypeExpr>,
        value: Expr,
        span: Span,
        /// Source-block id for `var ( ... )` block sugar. See
        /// `Stmt::Let::block_id`.
        block_id: Option<u32>,
    },
    Expr(Expr),
    Return {
        value: Option<Expr>,
        span: Span,
    },
    Emit {
        signal: Ident,
        args: Vec<Expr>,
        span: Span,
    },
    Assign {
        target: Expr,
        op: AssignOp,
        value: Expr,
        span: Span,
    },
    /// `for x in xs { body }`. Lowers to a C++ range-based for in
    /// fn / class-method bodies. The same statement is also accepted
    /// as an `ElementMember::Stmt` so view/widget bodies can iterate
    /// to render per-row UI - the body's trailing `K::Element` is
    /// what gets repeated.
    For {
        binding: Ident,
        /// `for x, i in xs` — the row's position, bound alongside the
        /// row itself. `None` for the one-binding form. Over a range
        /// the index counts turns of the loop, which is the same
        /// number the binding already carries when the range starts
        /// at zero.
        index: Option<Ident>,
        iter: Expr,
        body: Block,
        span: Span,
    },
    /// `while cond { body }`. Cond must evaluate to Bool. Lowers
    /// directly to a C++ `while (cond) { body }` loop.
    While {
        cond: Expr,
        body: Block,
        span: Span,
    },
    /// `break` — exit the innermost surrounding `for` / `while`
    /// loop. Lowers to a C++ `break;`. Validity (must appear inside
    /// a loop) is checked at the HIR layer.
    Break {
        span: Span,
    },
    /// `continue` — skip to the next iteration of the innermost
    /// surrounding loop. Lowers to a C++ `continue;`.
    Continue {
        span: Span,
    },
    /// `batch { ... }` — Qt 6 property-update group scope. Bindable
    /// writes inside the block defer their notifications until scope
    /// exit, so dependent bindings re-evaluate atomically (glitch-free).
    /// No effect on non-bindable props.
    Batch {
        body: Block,
        span: Span,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum AssignOp {
    Eq,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Int(i64),
    Float(f64),
    Bool(bool),
    Nil,
    /// Interpolated string. Each part is either literal text or a parsed
    /// inner expression. Lowered at codegen time to QString concatenation.
    Str(Vec<StrPart>),
    Sym(String),
    Ident(String),
    AtIdent(String),
    SelfRef,
    /// Typed receiver-less name resolution: `Foo.Bar.baz`.
    Path(Vec<Ident>),
    /// `f(args)` or `f(args) { |x| ... }`. `type_args` is non-empty
    /// only for explicit generic-fn calls like `make<Int>(0)`.
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        block: Option<Box<Expr>>,
        type_args: Vec<TypeExpr>,
    },
    /// `recv.method(args)`. `block` available for trailing-block syntax.
    /// `type_args` carries explicit type arguments when the receiver was
    /// written with the form-(b) generic syntax `Box<Int>.new()` — empty
    /// in every other case.
    MethodCall {
        receiver: Box<Expr>,
        method: Ident,
        args: Vec<Expr>,
        block: Option<Box<Expr>>,
        type_args: Vec<TypeExpr>,
    },
    Member {
        receiver: Box<Expr>,
        name: Ident,
    },
    /// `recv?.name` — null-safe member access. If `recv` evaluates to
    /// nil, the whole expression is nil; otherwise it's `recv.name`.
    /// The result type is always `Nullable<R>` where `R` is the
    /// non-safe access's result type. v1: chains do NOT auto-extend
    /// past the next `.` — the result of `?.` is nullable, and
    /// further `.member` access on it is a type error unless wrapped
    /// in another `?.`.
    SafeMember {
        receiver: Box<Expr>,
        name: Ident,
    },
    /// `recv?.method(args)` — null-safe method call. Same lifting
    /// rules as `SafeMember`.
    SafeMethodCall {
        receiver: Box<Expr>,
        method: Ident,
        args: Vec<Expr>,
        block: Option<Box<Expr>>,
        type_args: Vec<TypeExpr>,
    },
    Index {
        receiver: Box<Expr>,
        index: Box<Expr>,
    },
    Block(Block),
    /// Explicit `{ |params| body }` block expression.
    Lambda {
        params: Vec<Param>,
        body: Block,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    /// `expr?` - error-union propagation, lowered to early-return.
    Try(Box<Expr>),
    If {
        cond: Box<Expr>,
        then_b: Block,
        else_b: Option<Block>,
        /// `if let pat = init { ... }` form. When `Some`, the expr is
        /// the initializer (a `T?` / `!T` / etc.); the `cond` field
        /// holds a synthetic `true` literal that's ignored by codegen.
        /// `pat` is matched against `init`'s value and its bindings
        /// flow into `then_b`. `else_b` runs when the pattern fails.
        let_binding: Option<(Box<Pattern>, Box<Expr>)>,
    },
    Case {
        scrutinee: Box<Expr>,
        arms: Vec<CaseArm>,
    },
    Await(Box<Expr>),
    /// `key: value` pairs, used inside `(...)` keyword-argument lists.
    Kwarg {
        key: Ident,
        value: Box<Expr>,
    },
    /// A view/widget element used in expression position. Lets the
    /// trailing expression of a `Block` be a UI element, which is
    /// what makes `if cond { Label { ... } }` work as a regular
    /// `ExprKind::If` whose then-branch trailing expression is this
    /// `Element`. Codegen for fn-body context typically rejects this
    /// (no meaningful runtime semantic outside a view/widget tree).
    Element(Element),
    /// `[a, b, c]`. Heterogeneous in principle (lowered to QVariantList
    /// in C++ context, JS array `[a,b,c]` in QML context).
    Array(Vec<Expr>),
    /// `{ key: value, key: value }`. Disambiguated from `{ ...stmts... }`
    /// blocks at parse time by peeking for a leading `<key>: <value>`
    /// pair. Keys may be string literals or identifiers; for identifiers
    /// the parser keeps them as `K::Ident(name)` so codegen can decide
    /// the surface representation per target.
    Map(Vec<(Expr, Expr)>),
    /// `start..end` (exclusive) or `start..=end` (inclusive). v1 is
    /// only valid as the iter of a `for x in <range>` — there's no
    /// runtime Range<T> type yet, codegen lowers it to a plain
    /// C-style for loop directly.
    Range {
        start: Box<Expr>,
        end: Box<Expr>,
        inclusive: bool,
    },
}

#[derive(Debug, Clone)]
pub enum StrPart {
    Text(String),
    Interp(Box<Expr>),
    /// `#{expr:fmt}` — interp with a format spec. `format_spec` is the
    /// literal spec body without leading `:` (e.g. `.2f`, `08d`, `>20`).
    /// Codegen lowers this to a target-specific formatted-print call.
    InterpFmt {
        expr: Box<Expr>,
        format_spec: String,
    },
}

#[derive(Debug, Clone)]
pub struct CaseArm {
    pub pattern: Pattern,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    /// `when foo { ... }` - matches a literal name or constructor.
    Ctor {
        name: Ident,
        args: Vec<Pattern>,
        span: Span,
    },
    /// `when 42` etc.
    Literal { value: Expr, span: Span },
    /// `when _`.
    Wild { span: Span },
    /// `when ok(v)` style binding pattern is modeled as Ctor with sub-Bind.
    Bind { name: Ident, span: Span },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Lt,
    LtEq,
    Gt,
    GtEq,
    Eq,
    NotEq,
    And,
    Or,
    /// `|` — bitwise OR. Currently only typed for `flags X of E`
    /// values (combine flag bits) and matched-pair Int operands.
    BitOr,
    /// `&` — bitwise AND. Same constraints as `BitOr`.
    BitAnd,
    /// `^` — bitwise XOR. Same constraints as `BitOr`.
    BitXor,
}

// ---- Pretty printer -------------------------------------------------------

pub fn pretty(module: &Module) -> String {
    let mut p = Printer::default();
    p.module(module);
    p.out
}

#[derive(Default)]
struct Printer {
    out: String,
    indent: usize,
}

impl Printer {
    fn line(&mut self, s: &str) {
        for _ in 0..self.indent {
            self.out.push_str("  ");
        }
        self.out.push_str(s);
        self.out.push('\n');
    }

    fn enter(&mut self, label: &str) {
        self.line(label);
        self.indent += 1;
    }

    fn leave(&mut self) {
        self.indent -= 1;
    }

    fn module(&mut self, m: &Module) {
        self.enter("Module");
        for item in &m.items {
            self.item(item);
        }
        self.leave();
    }

    fn item(&mut self, item: &Item) {
        match item {
            Item::Use(u) => {
                let path = u
                    .path
                    .iter()
                    .map(|i| i.name.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                self.line(&format!("Use {path}"));
            }
            Item::UseQml(u) => {
                let alias = u
                    .alias
                    .as_ref()
                    .map(|a| format!(" as {}", a.name))
                    .unwrap_or_default();
                self.line(&format!("UseQml \"{}\"{alias}", u.module_uri));
            }
            Item::Class(c) => {
                let sup = c
                    .super_class
                    .as_ref()
                    .map(|t| format!(" < {}", type_to_string(t)))
                    .unwrap_or_default();
                self.enter(&format!("Class {}{sup}", c.name.name));
                for m in &c.members {
                    self.member(m);
                }
                self.leave();
            }
            Item::Struct(s) => {
                self.enter(&format!("Struct {}", s.name.name));
                for f in &s.fields {
                    self.line(&format!("Field {}: {}", f.name.name, type_to_string(&f.ty)));
                }
                self.leave();
            }
            Item::Fn(f) => self.fn_decl(f, "Fn"),
            Item::View(v) => {
                self.enter(&format!("View {}", v.name.name));
                self.element(&v.root);
                self.leave();
            }
            Item::Widget(w) => {
                self.enter(&format!("Widget {}", w.name.name));
                self.element(&w.root);
                self.leave();
            }
            Item::Style(s) => match &s.body {
                StyleBody::Lit(entries) => {
                    self.enter(&format!("Style {}", s.name.name));
                    for e in entries {
                        self.line(&format!("Entry {}", e.key));
                    }
                    self.leave();
                }
                StyleBody::Alias(_) => {
                    self.line(&format!("Style {} = <expr>", s.name.name));
                }
            },
            Item::Trait(t) => {
                self.enter(&format!("Trait {}", t.name.name));
                for m in &t.methods {
                    self.line(&format!("Sig {}", m.name.name));
                }
                self.leave();
            }
            Item::Impl(i) => {
                self.enter(&format!(
                    "Impl {} for {}",
                    i.trait_name.name,
                    type_expr_render(&i.for_type)
                ));
                for m in &i.methods {
                    self.line(&format!("Method {}", m.name.name));
                }
                self.leave();
            }
            Item::Let(l) => {
                self.line(&format!(
                    "Let {} : {}",
                    l.name.name,
                    type_expr_render(&l.ty),
                ));
            }
            Item::Enum(e) => {
                let prefix = if e.is_extern {
                    "ExternEnum"
                } else if e.is_error {
                    "Error"
                } else {
                    "Enum"
                };
                self.enter(&format!("{prefix} {}", e.name.name));
                for v in &e.variants {
                    if v.fields.is_empty() {
                        self.line(&format!("Variant {}", v.name.name));
                    } else {
                        let fields = v
                            .fields
                            .iter()
                            .map(|f| format!("{}: {}", f.name.name, type_expr_render(&f.ty)))
                            .collect::<Vec<_>>()
                            .join(", ");
                        self.line(&format!("Variant {}({fields})", v.name.name));
                    }
                }
                self.leave();
            }
            Item::Flags(f) => {
                let prefix = if f.is_extern { "ExternFlags" } else { "Flags" };
                self.line(&format!("{prefix} {} of {}", f.name.name, f.of.name));
            }
            Item::Store(s) => {
                self.enter(&format!("Store {}", s.name.name));
                for sf in &s.state_fields {
                    if let StateFieldKind::Property { ty } = &sf.kind {
                        self.line(&format!(
                            "State {} : {}",
                            sf.name.name,
                            type_expr_render(ty),
                        ));
                    }
                }
                for m in &s.members {
                    self.member(m);
                }
                self.leave();
            }
            Item::Suite(s) => {
                self.enter(&format!("Suite \"{}\"", s.name));
                for t in &s.tests {
                    let label = t.display_name.as_deref().unwrap_or(t.name.name.as_str());
                    self.line(&format!("Test \"{}\"", label));
                }
                self.leave();
            }
        }
    }

    fn member(&mut self, m: &ClassMember) {
        match m {
            ClassMember::Property(p) => {
                let notify = p
                    .notify
                    .as_ref()
                    .map(|i| format!(" notify={}", i.name))
                    .unwrap_or_default();
                self.line(&format!(
                    "Property {} : {}{notify}",
                    p.name.name,
                    type_to_string(&p.ty)
                ));
            }
            ClassMember::Signal(s) => {
                let params = s
                    .params
                    .iter()
                    .map(|p| format!("{}: {}", p.name.name, type_to_string(&p.ty)))
                    .collect::<Vec<_>>()
                    .join(", ");
                self.line(&format!("Signal {}({params})", s.name.name));
            }
            ClassMember::Slot(f) => self.fn_decl(f, "Slot"),
            ClassMember::Fn(f) => self.fn_decl(f, "Fn"),
            ClassMember::Field(f) => {
                self.line(&format!("Field {}: {}", f.name.name, type_to_string(&f.ty)));
            }
            ClassMember::Init(i) => {
                let params = i
                    .params
                    .iter()
                    .map(|p| format!("{}: {}", p.name.name, type_to_string(&p.ty)))
                    .collect::<Vec<_>>()
                    .join(", ");
                self.line(&format!("Init({params})"));
            }
            ClassMember::Deinit(_) => self.line("Deinit"),
        }
    }

    fn element(&mut self, e: &Element) {
        self.enter(&format!("Element {}", e.name.name));
        for m in &e.members {
            match m {
                ElementMember::Property { key, .. } => self.line(&format!("Prop {key}")),
                ElementMember::Child(c) => self.element(c),
                ElementMember::Stmt(_) => {
                    self.line("Stmt");
                }
            }
        }
        self.leave();
    }

    fn fn_decl(&mut self, f: &FnDecl, label: &str) {
        let async_ = if f.is_async { "async " } else { "" };
        let params = f
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name.name, type_to_string(&p.ty)))
            .collect::<Vec<_>>()
            .join(", ");
        let ret = f
            .return_ty
            .as_ref()
            .map(|t| format!(" -> {}", type_to_string(t)))
            .unwrap_or_default();
        self.line(&format!("{label} {async_}{}({params}){ret}", f.name.name));
    }
}

pub fn type_to_string(t: &TypeExpr) -> String {
    match &t.kind {
        TypeKind::Named { path, args } => {
            let p = path
                .iter()
                .map(|i| i.name.as_str())
                .collect::<Vec<_>>()
                .join(".");
            if args.is_empty() {
                p
            } else {
                let a = args
                    .iter()
                    .map(type_to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{p}<{a}>")
            }
        }
        TypeKind::Nullable(inner) => format!("{}?", type_to_string(inner)),
        TypeKind::ErrorUnion(inner) => format!("!{}", type_to_string(inner)),
        TypeKind::Fn { params, ret } => {
            let p = params
                .iter()
                .map(type_to_string)
                .collect::<Vec<_>>()
                .join(", ");
            format!("fn({p}) -> {}", type_to_string(ret))
        }
        TypeKind::SelfType => "Self".to_string(),
    }
}
