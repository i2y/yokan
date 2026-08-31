//! Source code formatter for `.pixie`.
//!
//! Top-level approach: re-emit each AST item in canonical form,
//! falling back to the original source text for sub-trees we don't
//! yet print explicitly. Comments are preserved by associating each
//! `#` line in the source with the next AST item / class member /
//! statement that follows it — header comments above a `class`,
//! per-property comments above a `prop`, per-stmt comments above a
//! statement all flow through unchanged. Mid-expression and
//! same-line trailing comments are not yet handled (rare in current
//! demos).
//!
//! The formatter is **idempotent on its own output**: running fmt
//! twice produces identical text. Any deviation from that is a bug.

use crate::ast::*;
use crate::parse::{ParseError, parse};
use crate::span::{FileId, Span};

#[derive(Debug)]
pub enum FormatError {
    Parse(ParseError),
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormatError::Parse(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for FormatError {}

/// Parse `source` and return its canonical formatted form. The input
/// must be a complete `.cute` file (the parser's `Module` entry).
/// Comments associated with item / member / stmt boundaries flow
/// through; mid-expression comments are dropped.
pub fn format_source(file: FileId, source: &str) -> Result<String, FormatError> {
    let module = parse(file, source).map_err(FormatError::Parse)?;
    let mut p = Printer::new(source);
    p.print_module(&module);
    Ok(p.into_output())
}

struct Printer<'a> {
    out: String,
    indent: usize,
    source: &'a str,
    /// Byte offset of the source up to which we've already accounted
    /// for comments. Each pre-item / pre-member comment scan picks up
    /// from here so we don't repeat ourselves.
    cursor: u32,
}

impl<'a> Printer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            out: String::new(),
            indent: 0,
            source,
            cursor: 0,
        }
    }

    fn into_output(mut self) -> String {
        // Trailing newlines / whitespace at end of file: collapse to
        // exactly one. Most editors expect a single final newline.
        while self.out.ends_with("\n\n") {
            self.out.pop();
        }
        if !self.out.ends_with('\n') {
            self.out.push('\n');
        }
        self.out
    }

    fn write_indent(&mut self) {
        for _ in 0..self.indent {
            self.out.push_str("  ");
        }
    }

    fn writeln(&mut self, s: &str) {
        self.write_indent();
        self.out.push_str(s);
        self.out.push('\n');
    }

    fn write(&mut self, s: &str) {
        self.out.push_str(s);
    }

    fn newline(&mut self) {
        self.out.push('\n');
    }

    /// Walk the source from `self.cursor` up to `up_to` (exclusive)
    /// and emit any comment-only lines plus blank-line groupings we
    /// find, indented at the current level. Used to preserve header
    /// comments above an item / member / statement.
    ///
    /// Blank-line policy: we count the number of newlines in the
    /// gap. The "expected" count between consecutive content lines
    /// is 1 (a single line break); anything beyond that is a blank
    /// line the user inserted intentionally and we emit one back —
    /// capped at one regardless of how many they had, to keep the
    /// output stable under `fmt . fmt`.
    fn flush_trivia_until(&mut self, up_to: u32) {
        let start = self.cursor as usize;
        let end = (up_to as usize).min(self.source.len());
        if start >= end {
            self.cursor = up_to;
            return;
        }
        let slice = &self.source[start..end];

        // Walk the slice line by line, classifying each newline-
        // terminated chunk as a comment line or a blank line. The
        // trailing partial chunk (leading whitespace of the upcoming
        // content line) is skipped — it isn't trivia.
        //
        // Rule for blank-line preservation: emit at most one blank
        // line between non-blank emissions. The first newline in
        // the slice is the line-terminator of the preceding output
        // (when there is one) and never becomes a blank line.
        enum Trivia {
            Comment(String),
            Blank,
        }
        let mut chunks: Vec<Trivia> = Vec::new();
        let mut chunk_idx = 0usize;
        let initial_skip = if self.out.is_empty() { 0 } else { 1 };
        for line in slice.split_inclusive('\n') {
            if !line.ends_with('\n') {
                continue;
            }
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') && !trimmed.starts_with("#{") {
                chunks.push(Trivia::Comment(trimmed.trim_end_matches('\n').to_string()));
            } else if trimmed.trim_end().is_empty() {
                if chunk_idx >= initial_skip {
                    chunks.push(Trivia::Blank);
                }
            } else {
                break;
            }
            chunk_idx += 1;
        }

        // Emit, collapsing runs of blanks. Suppress a blank that
        // would land right after another blank or at the very start
        // of the output buffer.
        let mut last_was_blank = self.out.ends_with("\n\n") || self.out.is_empty();
        for c in chunks {
            match c {
                Trivia::Comment(text) => {
                    self.write_indent();
                    self.write(&text);
                    self.newline();
                    last_was_blank = false;
                }
                Trivia::Blank => {
                    if !last_was_blank {
                        self.newline();
                        last_was_blank = true;
                    }
                }
            }
        }
        self.cursor = up_to;
    }

    /// Source-text fallback: emit the substring `[span.start, span.end)`
    /// verbatim. Used by node kinds the formatter doesn't rewrite
    /// explicitly (view / widget / style item bodies, error decls,
    /// fn bodies). Lines after the first keep their original
    /// indentation so multi-line structures (nested elements,
    /// statement bodies) preserve their nesting.
    fn write_span_verbatim(&mut self, span: Span) {
        let start = span.start as usize;
        let end = (span.end as usize).min(self.source.len());
        if start >= end {
            return;
        }
        self.out.push_str(&self.source[start..end]);
        if span.end > self.cursor {
            self.cursor = span.end;
        }
    }

    fn print_module(&mut self, m: &Module) {
        let mut prev_kind: Option<&'static str> = None;
        for (i, item) in m.items.iter().enumerate() {
            let span = item_span(item);
            self.flush_trivia_until(span.start);
            let kind = item_kind_label(item);
            // Group consecutive `use` / `use qml` lines together;
            // separate other items with a blank line.
            if i > 0 {
                let separate = match (prev_kind, kind) {
                    (Some("use"), "use")
                    | (Some("usqml"), "usqml")
                    | (Some("usqml"), "use")
                    | (Some("use"), "usqml") => false,
                    _ => true,
                };
                if separate && !self.out.ends_with("\n\n") {
                    self.newline();
                }
            }
            self.print_item(item);
            prev_kind = Some(kind);
            self.cursor = span.end;
        }
        // Trailing comments past the last item.
        self.flush_trivia_until(self.source.len() as u32);
    }

    fn print_item(&mut self, item: &Item) {
        match item {
            Item::Use(u) => self.print_use(u),
            Item::UseQml(u) => self.print_use_qml(u),
            Item::Class(c) => self.print_class(c),
            Item::Struct(_)
            | Item::View(_)
            | Item::Widget(_)
            | Item::Style(_)
            | Item::Trait(_)
            | Item::Impl(_)
            | Item::Let(_)
            | Item::Enum(_)
            | Item::Flags(_)
            | Item::Store(_)
            | Item::Suite(_) => {
                // Less-common item kinds: source-verbatim until the
                // formatter has explicit print logic for them.
                //
                // The AST stores the item span starting at the kind
                // keyword (`style`, `view`, `let`, …), not at the
                // optional `pub` prefix — caller-side parsers consume
                // `pub` then call into the per-kind decl helper. So a
                // raw `write_span_verbatim(span)` would silently drop
                // the `pub` modifier. Re-emit it here so visibility
                // survives a fmt round-trip.
                let span = item_span(item);
                self.write_indent();
                if item_is_pub(item) {
                    self.write("pub ");
                }
                self.write_span_verbatim(span);
                self.newline();
            }
            Item::Fn(f) => self.print_fn(f, /*pub_prefix=*/ false),
        }
    }

    fn print_use(&mut self, u: &UseItem) {
        self.write_indent();
        self.write("use ");
        let dotted = u
            .path
            .iter()
            .map(|i| i.name.as_str())
            .collect::<Vec<_>>()
            .join(".");
        self.write(&dotted);
        match &u.kind {
            UseKind::Module(Some(alias)) => {
                self.write(" as ");
                self.write(&alias.name);
            }
            UseKind::Names(names) => {
                self.write(".{");
                for (i, n) in names.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write(&n.name.name);
                    if let Some(alias) = &n.alias {
                        self.write(" as ");
                        self.write(&alias.name);
                    }
                }
                self.write("}");
            }
            UseKind::Module(None) => {}
        }
        self.newline();
    }

    fn print_use_qml(&mut self, u: &UseQmlItem) {
        self.write_indent();
        self.write("use qml ");
        self.write(&format!("\"{}\"", u.module_uri));
        if let Some(alias) = &u.alias {
            self.write(" as ");
            self.write(&alias.name);
        }
        self.newline();
    }

    fn print_class(&mut self, c: &ClassDecl) {
        self.write_indent();
        if c.is_pub {
            self.write("pub ");
        }
        if c.is_arc {
            self.write("arc ");
        } else if c.is_extern_value {
            self.write("extern value ");
        } else {
            self.write("class ");
        }
        self.write(&c.name.name);
        if !c.generics.is_empty() {
            self.write("<");
            self.write(&format_generics(&c.generics));
            self.write(">");
        }
        if let Some(super_) = &c.super_class {
            self.write(" < ");
            self.write_type(super_);
        }
        // `arc Foo: ~Copyable { ... }` — linear / non-copyable opt-in.
        // Default is copyable; only emit when the user asked for the
        // non-copyable form. Class with super and `: ~Copyable` is
        // grammatically excluded (arc has no super) so the two never
        // collide on the same line.
        if !c.is_copyable {
            self.write(": ~Copyable");
        }
        self.write(" {");
        self.newline();
        self.indent += 1;
        // Skip the source past the opening `{` so trivia scans for
        // class members start inside the body. The AST doesn't
        // store the brace position, so locate it textually.
        if let Some(rel) = self.source[c.name.span.end as usize..].find('{') {
            self.cursor = c.name.span.end + rel as u32 + 1;
        } else {
            self.cursor = c.name.span.end;
        }
        // Walk members but coalesce consecutive Property / Field
        // items that share a `Some(block_id)` back into a `prop ( ...
        // )` / `let ( ... )` / `var ( ... )` block. A group of size
        // one falls through to the per-member printer (block-of-one
        // is unusual to author and unhelpful to preserve).
        let mut i = 0;
        while i < c.members.len() {
            let m = &c.members[i];
            let span = class_member_span(m);
            self.flush_trivia_until(span.start);
            // Detect a block group starting at i: consecutive members
            // of the same kind sharing the same `Some(block_id)`.
            let group_end = group_block_run_end(&c.members, i);
            if group_end - i >= 2 {
                self.print_class_member_block(&c.members[i..group_end]);
                let last_span = class_member_span(&c.members[group_end - 1]);
                self.cursor = last_span.end;
                i = group_end;
            } else {
                self.print_class_member(m);
                self.cursor = span.end;
                i += 1;
            }
        }
        // Trivia after the last member, before the closing `}`.
        // The `}` is at c.span.end - 1 (parser includes it in span).
        let body_end = c.span.end.saturating_sub(1);
        self.flush_trivia_until(body_end);
        self.indent -= 1;
        self.writeln("}");
    }

    /// Render a run of consecutive class members that all carry the
    /// same `Some(block_id)` back into the original `kw ( ... )`
    /// block sugar. The run is uniform by construction (parser only
    /// assigns the same id to items from one block, all of which
    /// share kind / pub / weak / unowned / is_mut), so reading the
    /// header values off the first member is safe.
    fn print_class_member_block(&mut self, group: &[ClassMember]) {
        let first = &group[0];
        let (kw, is_pub, weak, unowned) = match first {
            ClassMember::Property(p) => ("prop", p.is_pub, false, false),
            ClassMember::Field(f) => {
                let kw = if f.is_mut { "var" } else { "let" };
                (kw, f.is_pub, f.weak, f.unowned)
            }
            // Other ClassMember variants don't carry a block_id and
            // will never appear in a group; the caller only invokes
            // this with Property / Field runs.
            _ => unreachable!("non-Property/Field in block run"),
        };
        self.write_indent();
        if is_pub {
            self.write("pub ");
        }
        if weak {
            self.write("weak ");
        } else if unowned {
            self.write("unowned ");
        }
        self.write(kw);
        self.write(" (");
        self.newline();
        self.indent += 1;
        for m in group {
            // Emit each item without its own pub / weak / unowned
            // prefix — the block header already carries them. The
            // per-item printers below honour `inside_block=true`.
            match m {
                ClassMember::Property(p) => self.print_property_inside_block(p),
                ClassMember::Field(f) => self.print_field_inside_block(f),
                _ => unreachable!(),
            }
        }
        self.indent -= 1;
        self.write_indent();
        self.write(")");
        self.newline();
    }

    fn print_class_member(&mut self, m: &ClassMember) {
        match m {
            ClassMember::Property(p) => self.print_property(p),
            ClassMember::Signal(s) => self.print_signal(s),
            ClassMember::Fn(f) => self.print_fn(f, /*pub_prefix=*/ false),
            ClassMember::Slot(f) => {
                self.write_indent();
                if f.is_pub {
                    self.write("pub ");
                }
                self.write("slot ");
                self.write(&f.name.name);
                self.write_fn_params_and_body(f);
            }
            ClassMember::Field(f) => self.print_field(f),
            ClassMember::Init(i) => self.print_init(i),
            ClassMember::Deinit(d) => self.print_deinit(d),
        }
    }

    fn print_init(&mut self, i: &InitDecl) {
        self.write_indent();
        self.write("init");
        if !i.params.is_empty() {
            self.write("(");
            for (idx, p) in i.params.iter().enumerate() {
                if idx > 0 {
                    self.write(", ");
                }
                self.write_param(p);
            }
            self.write(")");
        }
        let body_text = body_text_from_source(self.source, i.span);
        self.write(" ");
        self.write(body_text);
        self.newline();
        if i.span.end > self.cursor {
            self.cursor = i.span.end;
        }
    }

    fn print_deinit(&mut self, d: &DeinitDecl) {
        self.write_indent();
        self.write("deinit");
        let body_text = body_text_from_source(self.source, d.span);
        self.write(" ");
        self.write(body_text);
        self.newline();
        if d.span.end > self.cursor {
            self.cursor = d.span.end;
        }
    }

    /// Inside-block variant: skip `pub ` (block header carries it)
    /// and the leading `prop` keyword. Used when re-emitting items
    /// of a `prop ( ... )` block.
    fn print_property_inside_block(&mut self, p: &PropertyDecl) {
        self.write_indent();
        self.write(&p.name.name);
        self.write(" : ");
        self.write_property_type_and_attrs(p);
    }

    /// Inside-block variant for a `let ( ... )` / `var ( ... )`
    /// item. Skips the block-header-carried `pub` / `weak` /
    /// `unowned` and the `let` / `var` keyword.
    fn print_field_inside_block(&mut self, f: &Field) {
        self.write_indent();
        self.write(&f.name.name);
        self.write(" : ");
        self.write_type(&f.ty);
        if let Some(expr) = &f.default {
            self.write(" = ");
            self.write_span_verbatim(expr.span);
        }
        self.newline();
    }

    fn print_property(&mut self, p: &PropertyDecl) {
        self.write_indent();
        if p.is_pub {
            self.write("pub ");
        }
        self.write("prop ");
        self.write(&p.name.name);
        self.write(" : ");
        self.write_property_type_and_attrs(p);
    }

    /// Shared property tail: type (re-wrapping ModelList<T> if the
    /// parser lifted it to List<T> + model:true), then `, notify:` /
    /// `, bindable` / `, bind { }` / `, fresh { }` / `, default:`
    /// attribute fragments. Used by both the standalone print path
    /// and the inside-block path.
    fn write_property_type_and_attrs(&mut self, p: &PropertyDecl) {
        if p.model {
            self.write_type(&model_list_surface_from_storage(&p.ty));
        } else {
            self.write_type(&p.ty);
        }
        if let Some(notify) = &p.notify {
            self.write(", notify: :");
            self.write(&notify.name);
        }
        if p.bindable {
            self.write(", bindable");
        }
        if let Some(expr) = &p.binding {
            self.write(", bind { ");
            self.write_span_verbatim(expr.span);
            self.write(" }");
        }
        if let Some(expr) = &p.fresh {
            self.write(", fresh { ");
            self.write_span_verbatim(expr.span);
            self.write(" }");
        }
        if let Some(expr) = &p.default {
            self.write(", default: ");
            self.write_span_verbatim(expr.span);
        }
        self.newline();
    }

    fn print_field(&mut self, f: &Field) {
        self.write_indent();
        if f.is_pub {
            self.write("pub ");
        }
        if f.weak {
            self.write("weak ");
        } else if f.unowned {
            self.write("unowned ");
        }
        self.write(if f.is_mut { "var " } else { "let " });
        self.write(&f.name.name);
        self.write(" : ");
        self.write_type(&f.ty);
        if let Some(expr) = &f.default {
            self.write(" = ");
            self.write_span_verbatim(expr.span);
        }
        self.newline();
    }

    fn print_signal(&mut self, s: &SignalDecl) {
        self.write_indent();
        if s.is_pub {
            self.write("pub ");
        }
        self.write("signal ");
        self.write(&s.name.name);
        if !s.params.is_empty() {
            self.write("(");
            for (i, p) in s.params.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.write_param(p);
            }
            self.write(")");
        }
        self.newline();
    }

    fn print_fn(&mut self, f: &FnDecl, _pub_prefix: bool) {
        self.write_indent();
        if f.is_pub {
            self.write("pub ");
        }
        if f.is_test {
            self.write("test ");
        }
        if f.is_async {
            self.write("async ");
        }
        self.write("fn ");
        self.write(&f.name.name);
        if !f.generics.is_empty() {
            self.write("<");
            self.write(&format_generics(&f.generics));
            self.write(">");
        }
        self.write_fn_params_and_body(f);
    }

    fn write_fn_params_and_body(&mut self, f: &FnDecl) {
        // Empty parameter list: omit the `()` (same as Cute source
        // ergonomics — `fn run { ... }` rather than `fn run() { ... }`).
        if !f.params.is_empty() {
            self.write("(");
            for (i, p) in f.params.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.write_param(p);
            }
            self.write(")");
        }
        if let Some(ret) = &f.return_ty {
            self.write(" ");
            self.write_type(ret);
        }
        for attr in &f.attributes {
            self.write(" @");
            self.write(&attr.name.name);
            if !attr.args.is_empty() {
                self.write("(");
                for (i, arg) in attr.args.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write(arg);
                }
                self.write(")");
            }
        }
        match &f.body {
            Some(_body) => {
                // Body re-emission is verbatim from the source so
                // user formatting and (importantly) all comments
                // inside the body flow through. AST-level body
                // re-emission is a follow-up.
                let body_text = body_text_from_source(self.source, f.span);
                self.write(" ");
                self.write(body_text);
                self.newline();
                if f.span.end > self.cursor {
                    self.cursor = f.span.end;
                }
            }
            None => {
                // Binding declaration: no body.
                self.newline();
            }
        }
    }

    /// Write one fn / init / slot parameter including its modifiers.
    /// Order: `escaping` (closure capture), `consuming` (linear-move
    /// arg), `name`, `: <type>`, optional `= <default-expr>`.
    /// `escaping` and `consuming` are bare keywords — the `@` sigil
    /// is no longer accepted on closure / linear-move parameters.
    fn write_param(&mut self, p: &Param) {
        if p.is_escaping {
            self.write("escaping ");
        }
        if p.is_consuming {
            self.write("consuming ");
        }
        self.write(&p.name.name);
        self.write(": ");
        self.write_type(&p.ty);
        if let Some(default) = &p.default {
            self.write(" = ");
            self.write_span_verbatim(default.span);
        }
    }

    fn write_type(&mut self, t: &TypeExpr) {
        use TypeKind as TK;
        match &t.kind {
            TK::Named { path, args } => {
                let dotted = path
                    .iter()
                    .map(|i| i.name.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                self.write(&dotted);
                if !args.is_empty() {
                    self.write("<");
                    for (i, a) in args.iter().enumerate() {
                        if i > 0 {
                            self.write(", ");
                        }
                        self.write_type(a);
                    }
                    self.write(">");
                }
            }
            TK::Nullable(inner) => {
                self.write_type(inner);
                self.write("?");
            }
            TK::ErrorUnion(inner) => {
                self.write("!");
                self.write_type(inner);
            }
            TK::Fn { params, ret } => {
                self.write("fn(");
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write_type(p);
                }
                self.write(") -> ");
                self.write_type(ret);
            }
            TK::SelfType => self.write("Self"),
        }
    }
}

/// Re-wrap the storage type `List<T>` as the surface form
/// `ModelList<T>` for `model: true` props. Inverse of
/// `parse::unwrap_model_list_surface` so `cute fmt` is idempotent on
/// `ModelList<T>` declarations.
fn model_list_surface_from_storage(storage: &TypeExpr) -> TypeExpr {
    use crate::ast::Ident;
    let TypeKind::Named { path, args } = &storage.kind else {
        return storage.clone();
    };
    if path.len() != 1 || path[0].name != "List" || args.len() != 1 {
        return storage.clone();
    }
    let span = storage.span;
    TypeExpr {
        kind: TypeKind::Named {
            path: vec![Ident {
                name: "ModelList".to_string(),
                span: path[0].span,
            }],
            args: args.clone(),
        },
        span,
    }
}

fn format_generics(generics: &[crate::ast::GenericParam]) -> String {
    generics
        .iter()
        .map(|g| {
            if g.bounds.is_empty() {
                g.name.name.clone()
            } else {
                let bounds = g
                    .bounds
                    .iter()
                    .map(|b| b.name.clone())
                    .collect::<Vec<_>>()
                    .join(" + ");
                format!("{}: {}", g.name.name, bounds)
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn body_text_from_source(source: &str, fn_span: Span) -> &str {
    // The fn's body lives between the first `{` after the params and
    // the closing `}` at fn_span.end. Find the first `{` past the
    // start of the span (the parser guarantees one, when body is Some).
    let s = fn_span.start as usize;
    let e = (fn_span.end as usize).min(source.len());
    let slice = &source[s..e];
    if let Some(open) = slice.find('{') {
        &slice[open..]
    } else {
        ""
    }
}

fn item_span(item: &Item) -> Span {
    match item {
        Item::Use(u) => u.span,
        Item::UseQml(u) => u.span,
        Item::Class(c) => c.span,
        Item::Struct(s) => s.span,
        Item::Fn(f) => f.span,
        Item::View(v) => v.span,
        Item::Widget(w) => w.span,
        Item::Style(s) => s.span,
        Item::Trait(t) => t.span,
        Item::Impl(i) => i.span,
        Item::Let(l) => l.span,
        Item::Enum(e) => e.span,
        Item::Flags(f) => f.span,
        Item::Store(s) => s.span,
        Item::Suite(s) => s.span,
    }
}

/// Visibility flag for items that fall through to source-text fallback
/// in `print_item`. Item spans don't include the `pub` prefix, so the
/// fallback emits it from this flag instead.
fn item_is_pub(item: &Item) -> bool {
    match item {
        Item::Use(u) => u.is_pub,
        Item::UseQml(_) => false,
        Item::Class(c) => c.is_pub,
        Item::Struct(s) => s.is_pub,
        Item::Fn(f) => f.is_pub,
        Item::View(v) => v.is_pub,
        Item::Widget(w) => w.is_pub,
        Item::Style(s) => s.is_pub,
        Item::Trait(t) => t.is_pub,
        Item::Impl(_) => false,
        Item::Let(l) => l.is_pub,
        Item::Enum(e) => e.is_pub,
        Item::Flags(f) => f.is_pub,
        Item::Store(s) => s.is_pub,
        // `suite` and the test fns inside it are runner-internal —
        // visibility is meaningless. Always report `false`.
        Item::Suite(_) => false,
    }
}

fn item_kind_label(item: &Item) -> &'static str {
    match item {
        Item::Use(_) => "use",
        Item::UseQml(_) => "usqml",
        Item::Class(_) => "class",
        Item::Struct(_) => "struct",
        Item::Fn(_) => "fn",
        Item::View(_) => "view",
        Item::Widget(_) => "widget",
        Item::Style(_) => "style",
        Item::Trait(_) => "trait",
        Item::Impl(_) => "impl",
        Item::Let(_) => "let",
        Item::Enum(_) => "enum",
        Item::Flags(_) => "flags",
        Item::Store(_) => "store",
        Item::Suite(_) => "suite",
    }
}

fn class_member_span(m: &ClassMember) -> Span {
    match m {
        ClassMember::Property(p) => p.span,
        ClassMember::Signal(s) => s.span,
        ClassMember::Fn(f) | ClassMember::Slot(f) => f.span,
        ClassMember::Field(f) => f.span,
        ClassMember::Init(i) => i.span,
        ClassMember::Deinit(d) => d.span,
    }
}

/// Read the `block_id` off a class member, or `None` for kinds that
/// don't carry one (Signal / Fn / Slot / Init / Deinit). Used to
/// detect runs of consecutive members that came from the same
/// `prop ( ... )` / `let ( ... )` / `var ( ... )` block.
fn class_member_block_id(m: &ClassMember) -> Option<u32> {
    match m {
        ClassMember::Property(p) => p.block_id,
        ClassMember::Field(f) => f.block_id,
        _ => None,
    }
}

/// Walk forward from `start` over class members and return the
/// exclusive end index of a run that all share the same
/// `Some(block_id)` AND the same kind (all Property or all Field).
/// Returns `start + 1` for a singleton (no run, or different
/// neighbours).
fn group_block_run_end(members: &[ClassMember], start: usize) -> usize {
    let id = match class_member_block_id(&members[start]) {
        Some(id) => id,
        None => return start + 1,
    };
    let kind = std::mem::discriminant(&members[start]);
    let mut end = start + 1;
    while end < members.len() {
        let m = &members[end];
        if std::mem::discriminant(m) != kind {
            break;
        }
        if class_member_block_id(m) != Some(id) {
            break;
        }
        end += 1;
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::FileId;

    fn fmt(src: &str) -> String {
        format_source(FileId(0), src).expect("parse")
    }

    #[test]
    fn idempotent_on_clean_input() {
        let src = "use qml \"QtQuick\"\n\nclass Counter {\n  prop Count : Int, default: 0\n}\n";
        let once = fmt(src);
        let twice = fmt(&once);
        assert_eq!(once, twice, "fmt must be idempotent");
    }

    #[test]
    fn preserves_header_comment_above_class() {
        let src = "# A reactive counter.\nclass Counter {\n  prop Count : Int, default: 0\n}\n";
        let out = fmt(src);
        assert!(out.contains("# A reactive counter."), "comment lost: {out}");
        assert!(out.contains("class Counter"), "class header lost: {out}");
    }

    #[test]
    fn normalizes_property_spacing() {
        // Input has weird internal spacing; output should be canonical.
        let src = "class X {\n  prop  Count   :   Int   ,   default:   0\n}\n";
        let out = fmt(src);
        assert!(
            out.contains("prop Count : Int, default: 0"),
            "expected canonical spacing, got:\n{out}"
        );
    }

    #[test]
    fn use_qml_round_trips() {
        let src = "use qml \"QtQuick.Controls.Material\"\n";
        let out = fmt(src);
        assert!(
            out.contains("use qml \"QtQuick.Controls.Material\""),
            "got: {out}"
        );
    }

    #[test]
    fn ends_with_single_newline() {
        let src = "use qml \"QtQuick\"\n\n\n\n";
        let out = fmt(src);
        // Output must end with exactly one newline.
        assert!(out.ends_with('\n'));
        assert!(!out.ends_with("\n\n"));
    }

    /// `prop xs : ModelList<T>` round-trips through fmt as
    /// `ModelList<T>` (the parser lifts the surface to `List<T>` +
    /// `model: true` for downstream; fmt re-wraps on print). Pin so a
    /// future regression can't reintroduce the retired `, model` flag
    /// in the formatter output — that would make `cute fmt`
    /// destructive on every `ModelList<T>` source file.
    #[test]
    fn model_list_prop_round_trips_as_model_list() {
        let src = "class Store < Base {\n  prop Items : ModelList<Book>, default: []\n}\n";
        let out = fmt(src);
        assert!(
            out.contains("ModelList<Book>"),
            "fmt must keep the wrapper type on print, got:\n{out}"
        );
        assert!(
            !out.contains(", model"),
            "fmt must not emit the retired `, model` flag, got:\n{out}"
        );
        let twice = fmt(&out);
        assert_eq!(out, twice, "ModelList<T> fmt must be idempotent");
    }

    /// `prop ( ... )` block form is preserved through fmt: the parser
    /// tags each expanded item with the originating block id and the
    /// formatter regroups consecutive same-id items back into a
    /// `pub prop ( ... )` block. Users who hand-write block form
    /// (visual grouping for many sibling props) get to keep it.
    #[test]
    fn prop_block_round_trips_through_fmt() {
        let src = "pub class C < Base {\n  pub prop (\n    count : Int, notify: :countChanged, default: 0\n    label : String, notify: :labelChanged, default: \"\"\n  )\n}\n";
        let out = fmt(src);
        assert!(
            out.contains("pub prop ("),
            "prop block header should survive fmt, got:\n{out}",
        );
        assert!(
            out.contains("count : Int, notify: :countChanged, default: 0"),
            "first prop body should land inside the block:\n{out}",
        );
        assert!(
            out.contains("label : String, notify: :labelChanged, default: \"\""),
            "second prop body should land inside the block:\n{out}",
        );
        // No standalone `pub prop count` (would mean expansion).
        assert!(
            !out.contains("pub prop count :"),
            "items should not be re-emitted as standalone props:\n{out}",
        );
        let twice = fmt(&out);
        assert_eq!(out, twice, "fmt must be idempotent on block form");
    }

    /// `let ( ... )` block also round-trips; `pub` lives on the
    /// block header (not per-item), and the parser already rejects
    /// per-item `pub` inside a block — so re-emitting just one
    /// header is the only correct shape.
    #[test]
    fn let_block_round_trips_through_fmt() {
        let src =
            "class C < Base {\n  pub let (\n    salt : Int = 0\n    seed : Int = 1\n  )\n}\n";
        let out = fmt(src);
        assert!(
            out.contains("pub let ("),
            "let block header should survive fmt: {out}"
        );
        assert!(
            out.contains("salt : Int = 0") && out.contains("seed : Int = 1"),
            "block items should be present: {out}"
        );
        assert!(
            !out.contains("pub let salt"),
            "items should not be re-emitted as standalone:\n{out}"
        );
        let twice = fmt(&out);
        assert_eq!(out, twice, "fmt must be idempotent on let-block form");
    }

    /// Standalone (non-block) decls stay standalone — fmt should not
    /// invent a block for adjacent same-kind items the user wrote
    /// individually. Only true source-level blocks survive as blocks.
    #[test]
    fn adjacent_standalone_props_stay_standalone() {
        let src = "class C < Base {\n  pub prop count : Int, default: 0\n  pub prop label : String, default: \"\"\n}\n";
        let out = fmt(src);
        assert!(
            !out.contains("prop ("),
            "fmt must not synthesise a block from adjacent standalone props:\n{out}"
        );
        assert!(out.contains("pub prop count : Int, default: 0"));
        assert!(out.contains("pub prop label : String, default: \"\""));
    }

    /// Regression: `bind { (a / b) * c }` was emitting `bind { a / b)
    /// * c }` because the parser strips paren wrappers (no Paren
    /// ExprKind), leaving the inner expression's span starting at the
    /// inner first token. The formatter's source-text fallback then
    /// emitted that truncated substring. Fix lives at
    /// parse.rs LParen branch — extend `e.span` to cover the parens.
    #[test]
    fn bind_block_preserves_inner_parens() {
        let src = "class P {\n  pub prop a : Float, notify: :aChanged, bindable, default: 1.0\n  pub prop b : Float, bind { (a / 2.0) * 100.0 }\n  pub prop c : Float, bind { 1.0 + (a / 2.0) }\n}\n";
        let out = fmt(src);
        assert!(
            out.contains("bind { (a / 2.0) * 100.0 }"),
            "leading paren in bind expr was dropped, got:\n{out}"
        );
        assert!(
            out.contains("bind { 1.0 + (a / 2.0) }"),
            "trailing paren in bind expr was dropped, got:\n{out}"
        );
        let twice = fmt(&out);
        assert_eq!(out, twice, "fmt must be idempotent on bind exprs");
    }

    /// Same fix, exercised through `default:` (which also uses
    /// `write_span_verbatim` on a parsed expression).
    #[test]
    fn default_expr_preserves_inner_parens() {
        let src = "class P {\n  prop x : Float, default: (1.0 + 2.0)\n}\n";
        let out = fmt(src);
        assert!(
            out.contains("default: (1.0 + 2.0)"),
            "parens in default expr were dropped, got:\n{out}"
        );
    }

    /// Regression: `pub style Foo { ... }` was emitting `style Foo { ... }`
    /// because the verbatim-fallback span starts at `style` (the AST
    /// stores the kw-anchored span). Same trap on view/widget/trait/let.
    #[test]
    fn pub_style_round_trips_through_fmt() {
        let src = "pub style Heading {\n  font.bold: true\n}\n";
        let out = fmt(src);
        assert!(
            out.contains("pub style Heading"),
            "pub modifier dropped on style, got:\n{out}"
        );
    }



}
