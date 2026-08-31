//! pixie Language Server.
//!
//! Each `textDocument/didOpen` and `didChange` runs a fresh parse +
//! resolve + type-check pass on the buffer's text and publishes the
//! resulting diagnostics back to the editor. There is no binding stdlib
//! yet (entry-adjacent `.rpi` files load per project), so the server
//! launches from any cwd.
//!
//! Implemented LSP requests: `textDocument/publishDiagnostics`,
//! `textDocument/hover`, `textDocument/definition`. Hover and definition
//! cover top-level item self-hover (the name in `class X { ... }` /
//! `fn f(...)`) and identifier references inside fn bodies / view
//! element trees that resolve to a top-level item. Class member hover
//! and completion are out of scope for this iteration.

use pixie_hir::{ItemKind, ProjectInfo, ResolvedProgram};
use pixie_syntax::{
    Module, ParseError, SourceMap, Span,
    ast::{
        Block, ClassMember, Element, ElementMember, Expr, ExprKind, FnDecl, Item, Stmt, StrPart,
        TypeExpr, TypeKind, type_expr_render,
    },
    diag::{Diagnostic as CuteDiag, Severity as CuteSev},
    parse,
    span::FileId,
};
use pixie_types::{ProgramTable, Type};
use dashmap::DashMap;
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tower_lsp::jsonrpc::Result as RpcResult;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionOptions, CompletionParams, CompletionResponse,
    Diagnostic as LspDiag, DiagnosticRelatedInformation, DiagnosticSeverity,
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverContents, HoverParams,
    HoverProviderCapability, InitializeParams, InitializeResult, InitializedParams, Location,
    MarkupContent, MarkupKind, MessageType, OneOf, Position, Range, ServerCapabilities, ServerInfo,
    TextDocumentSyncCapability, TextDocumentSyncKind, Url,
};
use tower_lsp::{Client, LanguageServer};

pub struct Backend {
    pub client: Client,
    docs: DashMap<Url, Document>,
    /// Single-slot project-analysis memo. Each request hashes
    /// (entry uri, entry text, sorted open-buffer contents) and
    /// reuses the prior `Analysis` when the hash matches. Back-to-
    /// back hover + completion + definition without any buffer
    /// change all share one analysis pass.
    cache: Mutex<Option<CachedAnalysis>>,
}

struct CachedAnalysis {
    key: u64,
    analysis: Arc<Analysis>,
}

struct Document {
    text: String,
    version: i32,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            docs: DashMap::new(),
            cache: Mutex::new(None),
        }
    }

    /// Memoized project analysis. The key includes the requesting
    /// buffer's URI + text and every other open buffer's path +
    /// content. Disk files aren't hashed; they're trusted to be
    /// stable while the editor is in control.
    fn cached_analysis(
        &self,
        uri: &Url,
        text: &str,
        buffers: &HashMap<PathBuf, String>,
    ) -> Option<Arc<Analysis>> {
        let key = compute_cache_key(uri, text, buffers);
        if let Ok(g) = self.cache.lock() {
            if let Some(c) = g.as_ref() {
                if c.key == key {
                    return Some(c.analysis.clone());
                }
            }
        }
        let analysis = analyze_full(uri, text, buffers)?;
        let arc = Arc::new(analysis);
        if let Ok(mut g) = self.cache.lock() {
            *g = Some(CachedAnalysis {
                key,
                analysis: arc.clone(),
            });
        }
        Some(arc)
    }

    /// Snapshot of every open editor buffer keyed by absolute path.
    /// Multi-file analysis prefers these over on-disk content for any
    /// path that's currently being edited.
    fn buffer_snapshot(&self) -> HashMap<PathBuf, String> {
        let mut map = HashMap::new();
        for entry in self.docs.iter() {
            if let Ok(p) = entry.key().to_file_path() {
                map.insert(p, entry.value().text.clone());
            }
        }
        map
    }

    /// Re-run analysis for `uri` against the cached buffer text and push
    /// the resulting diagnostics. No-op if the document isn't tracked.
    async fn refresh(&self, uri: Url) {
        let Some((text, version)) = self.docs.get(&uri).map(|d| (d.text.clone(), d.version)) else {
            return;
        };
        let buffers = self.buffer_snapshot();
        let diagnostics = match self.cached_analysis(&uri, &text, &buffers) {
            Some(a) => a.diagnostics.clone(),
            None => Vec::new(),
        };
        self.client
            .publish_diagnostics(uri, diagnostics, Some(version))
            .await;
    }

    /// Refresh diagnostics for `changed_uri` and every other open
    /// buffer that shares its project root. Cross-file edits often
    /// change the surface other files type-check against, so the
    /// stale ones get a fresh pass too.
    async fn refresh_project(&self, changed_uri: Url) {
        let companions = self.same_project_open_uris(&changed_uri);
        self.refresh(changed_uri).await;
        for uri in companions {
            self.refresh(uri).await;
        }
    }

    /// URIs of every other open buffer that lives under the same
    /// `cute.toml`-rooted project as `changed_uri` (or, in the
    /// absence of a manifest, the same parent directory).
    fn same_project_open_uris(&self, changed_uri: &Url) -> Vec<Url> {
        let Some(changed_path) = changed_uri.to_file_path().ok() else {
            return Vec::new();
        };
        let target_root = locate_project_root(&changed_path);
        let mut out = Vec::new();
        for entry in self.docs.iter() {
            let other = entry.key();
            if other == changed_uri {
                continue;
            }
            let Ok(other_path) = other.to_file_path() else {
                continue;
            };
            if locate_project_root(&other_path) == target_root {
                out.push(other.clone());
            }
        }
        out
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _params: InitializeParams) -> RpcResult<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions {
                    // `.` triggers member completion. Other chars
                    // (alphanumerics, `_`) drive the standard
                    // identifier-prefix completion that the editor
                    // requests on its own; no extra trigger needed.
                    trigger_characters: Some(vec![".".into()]),
                    ..Default::default()
                }),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "pixie-lsp".into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "pixie-lsp ready")
            .await;
    }

    async fn shutdown(&self) -> RpcResult<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let doc = params.text_document;
        let uri = doc.uri.clone();
        self.docs.insert(
            uri.clone(),
            Document {
                text: doc.text,
                version: doc.version,
            },
        );
        // Opening a file might unblock a cross-file reference in
        // another buffer that was previously falling through to
        // External soft-pass — refresh project siblings too.
        self.refresh_project(uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        // FULL sync model: each notification carries the entire buffer
        // in a single content change. Ignore everything but the last
        // change just in case the client batches.
        if let Some(change) = params.content_changes.into_iter().last() {
            self.docs.insert(
                uri.clone(),
                Document {
                    text: change.text,
                    version: params.text_document.version,
                },
            );
        }
        // Editing a file may break or fix cross-file references in
        // sibling buffers — re-publish their diagnostics so they
        // don't stay stale until the user touches them.
        self.refresh_project(uri).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.docs.remove(&uri);
        // LSP convention: empty diagnostics array clears the file's
        // markers in the editor.
        self.client.publish_diagnostics(uri, vec![], None).await;
    }

    async fn hover(&self, params: HoverParams) -> RpcResult<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let Some(text) = self.docs.get(&uri).map(|d| d.text.clone()) else {
            return Ok(None);
        };
        let buffers = self.buffer_snapshot();
        let Some(snap) = self.cached_analysis(&uri, &text, &buffers) else {
            return Ok(None);
        };
        let Some(target) = hover_from_snap(&snap, &text, pos) else {
            return Ok(None);
        };
        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                // The fence language is what the editor renders the hover
                    // block with, and what a user SEES. It said `cute`,
                    // the fork's ancestor (§8.52).
                    value: format!("```pixie\n{}\n```", target.label),
            }),
            range: Some(target.range),
        }))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> RpcResult<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let Some(text) = self.docs.get(&uri).map(|d| d.text.clone()) else {
            return Ok(None);
        };
        let buffers = self.buffer_snapshot();
        let Some(snap) = self.cached_analysis(&uri, &text, &buffers) else {
            return Ok(None);
        };
        let Some(target) = hover_from_snap(&snap, &text, pos) else {
            return Ok(None);
        };
        let Some(def) = target.def_location else {
            return Ok(None);
        };
        Ok(Some(GotoDefinitionResponse::Scalar(def)))
    }

    async fn completion(&self, params: CompletionParams) -> RpcResult<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let Some(text) = self.docs.get(&uri).map(|d| d.text.clone()) else {
            return Ok(None);
        };
        let buffers = self.buffer_snapshot();
        // Member-context completion may inject a placeholder into
        // the buffer; that modifies the cache key and forces a
        // fresh analysis. Hover / definition / diagnostics on the
        // un-modified buffer still hit cache.
        let offset = position_to_byte(&text, pos) as usize;
        let ctx = completion_context(&text, offset);
        let buffer_for_parse = match &ctx {
            CompletionContext::Member { partial, .. } if partial.is_empty() => {
                let mut s = String::with_capacity(text.len() + 5);
                s.push_str(&text[..offset]);
                s.push_str("__lsp");
                s.push_str(&text[offset..]);
                std::borrow::Cow::Owned(s)
            }
            _ => std::borrow::Cow::Borrowed(text.as_str()),
        };
        let Some(snap) = self.cached_analysis(&uri, &buffer_for_parse, &buffers) else {
            return Ok(Some(CompletionResponse::Array(Vec::new())));
        };
        let items = completion_from_snap_ctx(&snap, ctx, offset as u32);
        Ok(Some(CompletionResponse::Array(items)))
    }
}

/// Run the compiler frontend on `text` and convert every Cute diagnostic
/// that points at the user buffer into an LSP diagnostic. Stdlib bindings
/// are loaded into the same `SourceMap`; their diagnostics (which would
/// normally be impossible — bindings are vetted) are filtered out so the
/// editor only sees errors in the file the user is editing.
pub fn analyze(uri: &Url, text: &str) -> Vec<LspDiag> {
    analyze_with_buffers(uri, text, &HashMap::new())
}

pub fn analyze_with_buffers(
    uri: &Url,
    text: &str,
    open_buffers: &HashMap<PathBuf, String>,
) -> Vec<LspDiag> {
    let Some(snap) = analyze_full(uri, text, open_buffers) else {
        return Vec::new();
    };
    snap.diagnostics
}

/// Hover/definition target at an LSP position. Re-runs the full
/// analysis pipeline (cheap: stdlib bindings are include_str!-baked,
/// parse + resolve + check is single-file scale) and walks the AST
/// to locate the smallest interesting node containing the cursor.
pub fn hover_at(uri: &Url, text: &str, pos: Position) -> Option<HoverResult> {
    hover_at_with_buffers(uri, text, pos, &HashMap::new())
}

pub fn hover_at_with_buffers(
    uri: &Url,
    text: &str,
    pos: Position,
    open_buffers: &HashMap<PathBuf, String>,
) -> Option<HoverResult> {
    let snap = analyze_full(uri, text, open_buffers)?;
    hover_from_snap(&snap, text, pos)
}

fn hover_from_snap(snap: &Analysis, text: &str, pos: Position) -> Option<HoverResult> {
    let user_file = snap.user_file_id?;
    let offset = position_to_byte(text, pos);
    // Walk only the entry file's AST to find what the cursor points
    // at — definitions found in other user files are looked up via
    // `combined_module` so cross-file references resolve.
    let entry_module = snap.user_module_for(user_file)?;
    let target = target_at(entry_module, &snap.resolved, &snap.combined_module, offset)?;
    let def_location = target.def_span.and_then(|s| span_to_location(s, snap));
    Some(HoverResult {
        label: target.label,
        range: span_to_range(target.span, &snap.source_map),
        def_location,
    })
}

pub struct HoverResult {
    /// Human-readable signature, e.g. "fn compute(a: Int, b: Int) -> Int".
    pub label: String,
    /// Range covering the hovered identifier (for editor highlighting).
    pub range: Range,
    /// Definition's URI + range, when known. Drives go-to-definition;
    /// the URI may differ from the hover's URI in multi-file projects.
    pub def_location: Option<Location>,
}

/// Single-file analysis snapshot: everything hover/definition/diagnostics
/// share. Recomputed from scratch on each request — fine at single-file
/// scale, would need incremental reuse for multi-file projects.
struct Analysis {
    source_map: SourceMap,
    /// Each user `.pix` file in the analysis (entry + transitive
    /// `use` reachables), keyed by `FileId`. The entry file's id is
    /// in `user_file_id`.
    user_files: HashMap<FileId, UserFile>,
    /// FileId of the buffer the request originated from, when it
    /// successfully parsed. None on parse failure (the diagnostic
    /// list still reflects the failure).
    user_file_id: Option<FileId>,
    /// The combined `Module` (every user file + bindings).
    combined_module: Module,
    resolved: ResolvedProgram,
    table: ProgramTable,
    diagnostics: Vec<LspDiag>,
}

struct UserFile {
    path: PathBuf,
    module: Module,
}

impl Analysis {
    fn user_module_for(&self, file_id: FileId) -> Option<&Module> {
        self.user_files.get(&file_id).map(|f| &f.module)
    }
}

fn analyze_full(
    uri: &Url,
    text: &str,
    open_buffers: &HashMap<PathBuf, String>,
) -> Option<Analysis> {
    let entry_path = uri.to_file_path().ok();

    let mut sm = SourceMap::default();
    let mut user_files: HashMap<FileId, UserFile> = HashMap::new();
    let mut module_for_file: HashMap<FileId, String> = HashMap::new();
    let mut imports_for_module: HashMap<String, HashSet<String>> = HashMap::new();

    // Parse the entry buffer first so its FileId is stable and we
    // can attribute diagnostics back to it.
    let entry_label = entry_path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| uri.to_string());
    let entry_file = sm.add(entry_label, text.to_string());
    let entry_module = match parse(entry_file, text) {
        Ok(m) => m,
        Err(e) => {
            let diag = parse_error_to_lsp(&e, &sm);
            let empty = empty_module(entry_file);
            user_files.insert(
                entry_file,
                UserFile {
                    path: entry_path.clone().unwrap_or_default(),
                    module: empty.clone(),
                },
            );
            return Some(Analysis {
                source_map: sm,
                user_files,
                user_file_id: None,
                combined_module: empty,
                resolved: ResolvedProgram::default(),
                table: ProgramTable::default(),
                diagnostics: vec![diag],
            });
        }
    };
    let entry_module_name = module_name_from_path(entry_path.as_deref());
    module_for_file.insert(entry_file, entry_module_name.clone());
    record_imports(&entry_module, &entry_module_name, &mut imports_for_module);
    user_files.insert(
        entry_file,
        UserFile {
            path: entry_path.clone().unwrap_or_default(),
            module: entry_module,
        },
    );

    // Walk transitive `use foo` imports starting from the entry
    // file's directory (or the project root if a `cute.toml` lives
    // higher up). Each resolved sibling `.pix` is parsed once and
    // added to the source map; in-memory editor buffers override
    // on-disk content.
    if let Some(entry) = entry_path.as_ref() {
        let project_root = locate_project_root(entry);
        let mut queue: VecDeque<PathBuf> = VecDeque::new();
        let mut seen: HashSet<PathBuf> = HashSet::new();
        seen.insert(canonical_or_self(entry));
        // Seed the queue with the entry's `use` paths so we begin
        // walking siblings immediately.
        for path in resolve_uses(
            user_files.get(&entry_file).unwrap().module.items.iter(),
            &project_root,
        ) {
            queue.push_back(path);
        }

        while let Some(path) = queue.pop_front() {
            let canon = canonical_or_self(&path);
            if !seen.insert(canon.clone()) {
                continue;
            }
            let content = match open_buffers.get(&canon) {
                Some(s) => s.clone(),
                None => match std::fs::read_to_string(&path) {
                    Ok(s) => s,
                    Err(_) => continue,
                },
            };
            let label = path.display().to_string();
            let fid = sm.add(label, content.clone());
            let module = match parse(fid, sm.source(fid)) {
                Ok(m) => m,
                Err(_) => continue, // ignore broken siblings; LSP keeps editing
            };
            let name = module_name_from_path(Some(path.as_path()));
            module_for_file.insert(fid, name.clone());
            record_imports(&module, &name, &mut imports_for_module);
            for child in resolve_uses(module.items.iter(), &project_root) {
                queue.push_back(child);
            }
            user_files.insert(fid, UserFile { path, module });
        }
    }

    let bindings = pixie_binding::load_stdlib(&mut sm).unwrap_or_default();

    // Compose ProjectInfo: user files get their own module names
    // (file stem), bindings remain implicit prelude. Cross-file
    // visibility relies on `imports_for_module` matching what
    // each file's `use foo` declared.
    let mut info = ProjectInfo::default();
    info.module_for_file = module_for_file;
    info.imports_for_module = imports_for_module;
    let mut prelude: HashSet<String> = HashSet::new();
    for m in &bindings {
        for it in &m.items {
            if let Some(n) = item_name(it) {
                prelude.insert(n);
            }
        }
    }
    info.prelude_items = prelude;

    let user_module = merge_user_modules(user_files.values().map(|f| &f.module));
    // Mirror the driver's pre-pass order so HIR + the type checker see
    // the desugared shapes they expect (their `Item::Store` /
    // `Item::Suite` arms are `unreachable!()`).
    let user_module = pixie_driver::desugar::desugar_suite(user_module);
    let user_module = pixie_driver::desugar::desugar_store(user_module);
    let user_module = pixie_driver::desugar::desugar_view_state(user_module);
    let combined = combine_modules(&user_module, &bindings);
    let resolved = pixie_hir::resolve(&combined, &info);
    let typed = pixie_types::check_program(&combined, &resolved.program);

    let mut var_source = pixie_types::VarSource::default();
    let table = pixie_types::build_table(&combined, &resolved.program, &mut var_source);

    let mut all: Vec<CuteDiag> =
        Vec::with_capacity(resolved.diagnostics.len() + typed.diagnostics.len());
    all.extend(resolved.diagnostics);
    all.extend(typed.diagnostics);

    let diagnostics = all
        .into_iter()
        .filter(|d| d.primary.file == entry_file)
        .map(|d| cute_diag_to_lsp(d, &sm, uri))
        .collect();

    Some(Analysis {
        source_map: sm,
        user_files,
        user_file_id: Some(entry_file),
        combined_module: combined,
        resolved: resolved.program,
        table,
        diagnostics,
    })
}

/// Aggregate hash of the inputs that determine an `Analysis`'s
/// content: the entry buffer's URI and text, plus every open
/// buffer's path and content (sorted for determinism). Disk-only
/// files aren't hashed; if they change underneath, the cache may
/// briefly serve stale data — acceptable for editor scenarios
/// where the user is the sole editor.
fn compute_cache_key(uri: &Url, text: &str, buffers: &HashMap<PathBuf, String>) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    uri.as_str().hash(&mut h);
    text.hash(&mut h);
    let mut sorted: Vec<(&PathBuf, &String)> = buffers.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));
    for (p, t) in sorted {
        p.hash(&mut h);
        t.hash(&mut h);
    }
    h.finish()
}

fn merge_user_modules<'a>(modules: impl IntoIterator<Item = &'a Module>) -> Module {
    let mut items: Vec<Item> = Vec::new();
    let mut span: Option<Span> = None;
    for m in modules {
        items.extend(m.items.iter().cloned());
        if span.is_none() {
            span = Some(m.span);
        }
    }
    Module {
        items,
        span: span.unwrap_or_else(|| Span::new(FileId(0), 0, 0)),
    }
}

fn module_name_from_path(path: Option<&Path>) -> String {
    path.and_then(|p| p.file_stem().and_then(|s| s.to_str()))
        .map(|s| s.to_string())
        .unwrap_or_else(|| "main".into())
}

fn canonical_or_self(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// Walk up from `entry` looking for the nearest `cute.toml`. Falls
/// back to the entry file's parent dir when none is found.
fn locate_project_root(entry: &Path) -> PathBuf {
    let start = entry.parent().unwrap_or_else(|| Path::new("."));
    let mut current: Option<&Path> = Some(start);
    while let Some(dir) = current {
        if dir.join("pixie.toml").exists() {
            return dir.to_path_buf();
        }
        current = dir.parent();
    }
    start.to_path_buf()
}

/// Resolve every `use foo` / `use foo.bar` declaration in `items`
/// to a sibling `.pix` path under `project_root`. Mirrors the
/// driver's resolution rule (`use foo` → `<root>/foo.pix`,
/// `use foo.bar` → `<root>/foo/bar.pix`). `qt.*` imports are
/// reserved for stdlib and skipped.
fn resolve_uses<'a>(items: impl Iterator<Item = &'a Item>, project_root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for item in items {
        if let Item::Use(u) = item {
            if u.path.first().map(|i| i.name.as_str()) == Some("qt") {
                continue;
            }
            let mut p = project_root.to_path_buf();
            let n = u.path.len();
            for (i, seg) in u.path.iter().enumerate() {
                if i + 1 == n {
                    p.push(format!("{}.pix", seg.name));
                } else {
                    p.push(&seg.name);
                }
            }
            if p.exists() {
                out.push(p);
            }
        }
    }
    out
}

fn record_imports(
    module: &Module,
    self_name: &str,
    imports: &mut HashMap<String, HashSet<String>>,
) {
    let entry = imports.entry(self_name.to_string()).or_default();
    for it in &module.items {
        if let Item::Use(u) = it {
            if u.path.first().map(|i| i.name.as_str()) == Some("qt") {
                continue;
            }
            if let Some(last) = u.path.last() {
                entry.insert(last.name.clone());
            }
        }
    }
}

/// Convert a span to an LSP `Location` (file URI + range), looking
/// up the path from the analysis snapshot's `user_files` map. Returns
/// `None` for spans that point at a file we never loaded
/// (binding-only definitions, etc.).
fn span_to_location(span: Span, snap: &Analysis) -> Option<Location> {
    let path = snap.user_files.get(&span.file).map(|f| f.path.clone())?;
    let uri = Url::from_file_path(&path).ok()?;
    Some(Location {
        uri,
        range: span_to_range(span, &snap.source_map),
    })
}

fn empty_module(file: FileId) -> Module {
    Module {
        items: Vec::new(),
        span: Span::new(file, 0, 0),
    }
}

fn parse_error_to_lsp(e: &ParseError, sm: &SourceMap) -> LspDiag {
    LspDiag {
        range: span_to_range(e.span, sm),
        severity: Some(DiagnosticSeverity::ERROR),
        message: e.message.clone(),
        source: Some("pixie".into()),
        ..Default::default()
    }
}

fn cute_diag_to_lsp(d: CuteDiag, sm: &SourceMap, uri: &Url) -> LspDiag {
    let range = span_to_range(d.primary, sm);
    let severity = Some(match d.severity {
        CuteSev::Error => DiagnosticSeverity::ERROR,
        CuteSev::Warning => DiagnosticSeverity::WARNING,
        CuteSev::Note => DiagnosticSeverity::INFORMATION,
    });
    let related = if d.notes.is_empty() {
        None
    } else {
        Some(
            d.notes
                .into_iter()
                .map(|(span, msg)| DiagnosticRelatedInformation {
                    location: Location {
                        uri: uri.clone(),
                        range: span_to_range(span, sm),
                    },
                    message: msg,
                })
                .collect(),
        )
    };
    LspDiag {
        range,
        severity,
        message: d.message,
        related_information: related,
        source: Some("pixie".into()),
        ..Default::default()
    }
}

fn span_to_range(span: Span, sm: &SourceMap) -> Range {
    let (sl, sc) = sm.line_col(span);
    let end_span = Span {
        file: span.file,
        start: span.end,
        end: span.end,
    };
    let (el, ec) = sm.line_col(end_span);
    Range {
        start: Position {
            line: sl.saturating_sub(1) as u32,
            character: sc.saturating_sub(1) as u32,
        },
        end: Position {
            line: el.saturating_sub(1) as u32,
            character: ec.saturating_sub(1) as u32,
        },
    }
}

/// LSP `Position` (line, UTF-16 character) -> byte offset into `text`.
/// Approximates UTF-16 code units as bytes within the line, which is
/// exact for ASCII (the bulk of `.pix` source) and acceptable error
/// for the occasional multi-byte identifier in comments.
fn position_to_byte(text: &str, pos: Position) -> u32 {
    let mut byte = 0usize;
    let mut line = 0u32;
    for current in text.split_inclusive('\n') {
        if line == pos.line {
            // Walk pos.character chars (best-effort UTF-16) into the
            // line, summing UTF-8 byte widths.
            let mut col_chars_left = pos.character as usize;
            for c in current.chars() {
                if col_chars_left == 0 {
                    break;
                }
                if c == '\n' {
                    break;
                }
                byte += c.len_utf8();
                col_chars_left -= 1;
            }
            return byte as u32;
        }
        byte += current.len();
        line += 1;
    }
    text.len() as u32
}

fn combine_modules(user: &Module, bindings: &[Module]) -> Module {
    let mut items: Vec<Item> = bindings
        .iter()
        .flat_map(|m| m.items.iter().cloned())
        .collect();
    items.extend(user.items.iter().cloned());
    Module {
        items,
        span: user.span,
    }
}

fn item_name(it: &Item) -> Option<String> {
    match it {
        Item::Class(c) => Some(c.name.name.clone()),
        Item::Struct(s) => Some(s.name.name.clone()),
        Item::Fn(f) => Some(f.name.name.clone()),
        Item::View(v) => Some(v.name.name.clone()),
        Item::Widget(w) => Some(w.name.name.clone()),
        Item::Style(s) => Some(s.name.name.clone()),
        Item::Trait(t) => Some(t.name.name.clone()),
        Item::Let(l) => Some(l.name.name.clone()),
        Item::Enum(e) => Some(e.name.name.clone()),
        Item::Flags(f) => Some(f.name.name.clone()),
        Item::Store(s) => Some(s.name.name.clone()),
        // Suite labels are runner-internal — they don't introduce a
        // value-namespace name worth surfacing in symbol lists.
        Item::Suite(_) => None,
        // Impl blocks have no introducible name of their own; the
        // (trait, target) pair is the identity. LSP-side this means
        // they don't show up in symbol lists, which is fine — users
        // navigate via the trait or target type instead.
        Item::Impl(_) => None,
        Item::Use(_) | Item::UseQml(_) => None,
    }
}

// ---- completion -----------------------------------------------------------

/// Build a completion list for the cursor at `pos` in `text`.
/// Two contexts are recognized:
///
/// - **Member access**: the previous non-identifier byte is `.`. We
///   look back to find the receiver token, resolve it to a class
///   (via fn parameters or top-level item), and return the class's
///   methods / properties / signals as completion items.
/// - **Bare identifier**: anywhere else. We return every top-level
///   item plus binding-prelude items, filtered by the partial
///   prefix the user has typed so far.
///
/// Local `let` / `var` bindings inside the surrounding fn are not yet
/// resolved as receivers — the most useful 80% (`fn run(c: Counter)`
/// → `c.<TAB>`) is covered by the param path.
pub fn completion_at(uri: &Url, text: &str, pos: Position) -> Vec<CompletionItem> {
    completion_at_with_buffers(uri, text, pos, &HashMap::new())
}

pub fn completion_at_with_buffers(
    uri: &Url,
    text: &str,
    pos: Position,
    open_buffers: &HashMap<PathBuf, String>,
) -> Vec<CompletionItem> {
    let offset = position_to_byte(text, pos) as usize;
    let ctx = completion_context(text, offset);

    // Member context with empty partial means the buffer ends with
    // `c.` (or similar) — a parse error. Inject a placeholder
    // identifier at the cursor so the parser can build the full
    // Module and we can resolve receiver types via fn parameters.
    // The user-visible partial stays empty, so the placeholder
    // doesn't bleed into the filter.
    let buffer = match &ctx {
        CompletionContext::Member { partial, .. } if partial.is_empty() => {
            let mut s = String::with_capacity(text.len() + 5);
            s.push_str(&text[..offset]);
            s.push_str("__lsp");
            s.push_str(&text[offset..]);
            std::borrow::Cow::Owned(s)
        }
        _ => std::borrow::Cow::Borrowed(text),
    };

    let snap = match analyze_full(uri, &buffer, open_buffers) {
        Some(s) => s,
        None => return Vec::new(),
    };
    completion_from_snap_ctx(&snap, ctx, offset as u32)
}

fn completion_from_snap_ctx(
    snap: &Analysis,
    ctx: CompletionContext,
    cursor_offset: u32,
) -> Vec<CompletionItem> {
    match ctx {
        CompletionContext::Member { receiver, partial } => {
            member_completions(snap, &receiver, &partial, cursor_offset)
        }
        CompletionContext::Bare { partial } => bare_completions(snap, &partial),
    }
}

enum CompletionContext {
    Bare { partial: String },
    Member { receiver: String, partial: String },
}

fn completion_context(text: &str, offset: usize) -> CompletionContext {
    let bytes = text.as_bytes();
    let cap = offset.min(bytes.len());

    // Walk back over the partial identifier the user is currently
    // typing. This is what the completion list is filtering by.
    let mut start = cap;
    while start > 0 && is_ident_byte(bytes[start - 1]) {
        start -= 1;
    }
    let partial = text[start..cap].to_string();

    // Member context: the byte directly preceding the partial is `.`.
    if start > 0 && bytes[start - 1] == b'.' {
        let r_end = start - 1;
        let mut r_start = r_end;
        while r_start > 0 && is_ident_byte(bytes[r_start - 1]) {
            r_start -= 1;
        }
        let receiver = text[r_start..r_end].to_string();
        if !receiver.is_empty() {
            return CompletionContext::Member { receiver, partial };
        }
    }
    CompletionContext::Bare { partial }
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn bare_completions(snap: &Analysis, partial: &str) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    for (name, kind) in &snap.resolved.items {
        if !name.starts_with(partial) {
            continue;
        }
        let (lsp_kind, detail) = match kind {
            ItemKind::Class { .. } => (CompletionItemKind::CLASS, format!("class {name}")),
            ItemKind::Struct { .. } => (CompletionItemKind::STRUCT, format!("struct {name}")),
            ItemKind::Trait { .. } => (CompletionItemKind::INTERFACE, format!("trait {name}")),
            ItemKind::Fn { .. } => (
                CompletionItemKind::FUNCTION,
                snap.combined_module
                    .items
                    .iter()
                    .find_map(|it| match it {
                        Item::Fn(f) if &f.name.name == name => Some(format_fn_signature(f)),
                        _ => None,
                    })
                    .unwrap_or_else(|| format!("fn {name}")),
            ),
            ItemKind::Let { .. } => (CompletionItemKind::CONSTANT, format!("let {name}")),
            ItemKind::Enum { is_error: true, .. } => {
                (CompletionItemKind::CLASS, format!("error {name}"))
            }
            ItemKind::Enum { .. } => (CompletionItemKind::ENUM, format!("enum {name}")),
            ItemKind::Flags { .. } => (CompletionItemKind::ENUM, format!("flags {name}")),
        };
        items.push(CompletionItem {
            label: name.clone(),
            kind: Some(lsp_kind),
            detail: Some(detail),
            ..Default::default()
        });
    }
    items
}

fn member_completions(
    snap: &Analysis,
    receiver: &str,
    partial: &str,
    cursor: u32,
) -> Vec<CompletionItem> {
    // Resolve the receiver name to a class. Priority order:
    // 1. local let/var binding in the surrounding fn (cursor-relative)
    // 2. fn parameter
    // 3. receiver IS a top-level class name (e.g. `Counter.new()` form)
    let class_name = resolve_local_binding_class(snap, receiver, cursor)
        .or_else(|| resolve_receiver_class(snap, receiver))
        .or_else(|| {
            snap.resolved
                .items
                .get(receiver)
                .and_then(|kind| match kind {
                    ItemKind::Class { .. } => Some(receiver.to_string()),
                    // A `store` is a top-level `let` holding a class
                    // singleton, so `Store.` is the single most common
                    // member completion there is — and it answered
                    // nothing, because only a bare class name was
                    // recognised here (§8.52).
                    ItemKind::Let { ty, .. } => named_type(ty),
                    _ => None,
                })
        });
    let Some(class_name) = class_name else {
        return Vec::new();
    };
    let Some(entry) = snap.table.classes.get(&class_name) else {
        return Vec::new();
    };
    let mut items = Vec::new();
    for (name, ty) in &entry.properties {
        if !name.starts_with(partial) {
            continue;
        }
        items.push(CompletionItem {
            label: name.clone(),
            kind: Some(CompletionItemKind::PROPERTY),
            detail: Some(format!("prop {name} : {}", ty.render())),
            ..Default::default()
        });
    }
    for (name, overloads) in &entry.methods {
        if !name.starts_with(partial) {
            continue;
        }
        // Emit one completion entry per overload so the LSP shows
        // each signature distinctly.
        for fnty in overloads {
            let params = fnty
                .params
                .iter()
                .map(|p| p.render())
                .collect::<Vec<_>>()
                .join(", ");
            items.push(CompletionItem {
                label: name.clone(),
                kind: Some(CompletionItemKind::METHOD),
                detail: Some(format!("fn {name}({params}) {}", fnty.ret.render())),
                ..Default::default()
            });
        }
    }
    for (name, _params) in &entry.signals {
        if !name.starts_with(partial) {
            continue;
        }
        items.push(CompletionItem {
            label: name.clone(),
            kind: Some(CompletionItemKind::EVENT),
            detail: Some(format!("signal {name}")),
            ..Default::default()
        });
    }
    items
}

/// Walk the fn body that contains `cursor` looking for the most
/// recent `let` / `var` declaration of `name` before that position.
/// Returns the class the binding's value resolves to, when the
/// initializer is one of the patterns we recognize:
///
/// - `Counter()` — bare-callable constructor form
/// - `Counter.new(...)` — explicit form-(b)
/// - identifier referencing a fn parameter or another local
///
/// More elaborate patterns (method-call return types, generic
/// instantiation, control-flow merging) need a real type pass —
/// out of scope for this iteration.
fn resolve_local_binding_class(snap: &Analysis, name: &str, cursor: u32) -> Option<String> {
    let body = surrounding_fn_body(snap, cursor)?;
    locate_binding_in_block(body, name, cursor, snap)
}

fn surrounding_fn_body<'a>(snap: &'a Analysis, cursor: u32) -> Option<&'a Block> {
    for file in snap.user_files.values() {
        for it in &file.module.items {
            match it {
                Item::Fn(f) => {
                    if let Some(body) = &f.body {
                        if span_contains(body.span, cursor) {
                            return Some(body);
                        }
                    }
                }
                Item::Class(c) => {
                    for m in &c.members {
                        if let ClassMember::Fn(f) | ClassMember::Slot(f) = m {
                            if let Some(body) = &f.body {
                                if span_contains(body.span, cursor) {
                                    return Some(body);
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    None
}

fn locate_binding_in_block(
    block: &Block,
    name: &str,
    cursor: u32,
    snap: &Analysis,
) -> Option<String> {
    // Iterate stmts in source order. Track the latest matching
    // binding seen before `cursor`; later bindings shadow earlier
    // ones at the same name.
    let mut hit: Option<String> = None;
    for s in &block.stmts {
        let s_span = stmt_span_lsp(s);
        if s_span.start >= cursor {
            break;
        }
        match s {
            Stmt::Let { name: n, value, .. } | Stmt::Var { name: n, value, .. } => {
                if n.name == name {
                    hit = synth_class_of_expr(value, snap);
                }
            }
            // Nested blocks (if/case bodies, for loop bodies)
            // could declare bindings that shadow at the cursor —
            // not handled in v0.
            _ => {}
        }
    }
    hit
}

fn stmt_span_lsp(s: &Stmt) -> Span {
    match s {
        Stmt::Let { span, .. }
        | Stmt::Var { span, .. }
        | Stmt::Return { span, .. }
        | Stmt::Emit { span, .. }
        | Stmt::Assign { span, .. }
        | Stmt::For { span, .. }
        | Stmt::While { span, .. }
        | Stmt::Break { span, .. }
        | Stmt::Continue { span, .. }
        | Stmt::Batch { span, .. } => *span,
        Stmt::Expr(e) => e.span,
    }
}

fn synth_class_of_expr(e: &Expr, snap: &Analysis) -> Option<String> {
    use ExprKind as K;
    match &e.kind {
        K::Call { callee, .. } => {
            // `Counter(...)` form. The callee is an Ident referring
            // to a class.
            if let K::Ident(n) = &callee.kind {
                if let Some(ItemKind::Class { .. }) = snap.resolved.items.get(n) {
                    return Some(n.clone());
                }
                // Top-level fn call — look up its declared return
                // type (rare for `let c = make()` to bind a class
                // instance, but worth handling).
                if let Some(class) = fn_return_class(&snap.combined_module, n) {
                    return Some(class);
                }
            }
        }
        K::MethodCall {
            receiver, method, ..
        } => {
            // `Counter.new(...)` constructor form.
            if method.name == "new" {
                if let K::Ident(n) = &receiver.kind {
                    if let Some(ItemKind::Class { .. }) = snap.resolved.items.get(n) {
                        return Some(n.clone());
                    }
                }
            }
        }
        K::Ident(n) => {
            // Aliasing: `let alias = other_local`. Recurse.
            return resolve_receiver_class(snap, n);
        }
        _ => {}
    }
    None
}

fn fn_return_class(module: &Module, fn_name: &str) -> Option<String> {
    for it in &module.items {
        if let Item::Fn(f) = it {
            if f.name.name == fn_name {
                return f.return_ty.as_ref().and_then(type_class_name);
            }
        }
    }
    None
}

fn type_class_name(t: &pixie_syntax::ast::TypeExpr) -> Option<String> {
    use pixie_syntax::ast::TypeKind as TK;
    match &t.kind {
        TK::Named { path, .. } => path.last().map(|i| i.name.clone()),
        TK::Nullable(inner) | TK::ErrorUnion(inner) => type_class_name(inner),
        _ => None,
    }
}

/// Look up `name` in every user file's fn parameter lists. Returns
/// the class that the param's static type resolves to (if any).
/// The single name a type expression denotes, if it denotes one.
/// `Session` from `let Session : Session`.
fn named_type(ty: &TypeExpr) -> Option<String> {
    match &ty.kind {
        TypeKind::Named { path, args } if path.len() == 1 && args.is_empty() => {
            Some(path[0].name.clone())
        }
        _ => None,
    }
}

fn resolve_receiver_class(snap: &Analysis, name: &str) -> Option<String> {
    for file in snap.user_files.values() {
        for it in &file.module.items {
            match it {
                Item::Fn(f) => {
                    if let Some(c) = receiver_in_fn(f, name, &snap.resolved) {
                        return Some(c);
                    }
                }
                Item::Class(c) => {
                    for m in &c.members {
                        if let ClassMember::Fn(f) | ClassMember::Slot(f) = m {
                            if let Some(c) = receiver_in_fn(f, name, &snap.resolved) {
                                return Some(c);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    None
}

fn receiver_in_fn(f: &FnDecl, name: &str, prog: &ResolvedProgram) -> Option<String> {
    for p in &f.params {
        if p.name.name == name {
            // Param type is a TypeExpr; lower it and pull a class
            // name out of the leaf if possible.
            let ty = pixie_types::lower_type(&p.ty, prog);
            return class_name_of(&ty);
        }
    }
    None
}

fn class_name_of(ty: &Type) -> Option<String> {
    match ty {
        Type::Class(name) | Type::External(name) => Some(name.clone()),
        Type::Nullable(inner) => class_name_of(inner),
        Type::Generic { base, .. } => Some(base.clone()),
        // `obj.signal` hover navigates to the declaring class (the one
        // returned by `lookup_signal`'s super-class chain walk).
        Type::SignalRef { class, .. } => Some(class.clone()),
        _ => None,
    }
}

// ---- hover / go-to-def AST walker -----------------------------------------

struct Target {
    /// Span of the hovered token (Ident, Element name, AtIdent body...).
    span: Span,
    /// Single-line summary.
    label: String,
    /// Definition site, when known.
    def_span: Option<Span>,
}

fn span_contains(s: Span, offset: u32) -> bool {
    s.start <= offset && offset < s.end
}

/// Walk the user module to find what the cursor at `offset` is pointing
/// at. Returns the smallest interesting node it overlaps. Cross-file
/// definitions (referenced via `use`) are looked up via
/// `combined_module` which contains every user file's items merged.
fn target_at(
    module: &Module,
    prog: &ResolvedProgram,
    combined: &Module,
    offset: u32,
) -> Option<Target> {
    // First pass: top-level item self-hover (the name token in
    // `class X { ... }` / `fn f(...)` / etc.).
    for it in &module.items {
        if let Some(t) = item_self_hover(it, offset) {
            return Some(t);
        }
    }
    // Second pass: descend into bodies looking for Ident references.
    for it in &module.items {
        if let Some(t) = walk_item_body(it, offset, prog, combined) {
            return Some(t);
        }
    }
    None
}

fn item_self_hover(it: &Item, offset: u32) -> Option<Target> {
    let (name_span, label) = match it {
        Item::Class(c) => (c.name.span, format!("class {}", c.name.name)),
        Item::Struct(s) => (s.name.span, format!("struct {}", s.name.name)),
        Item::Fn(f) => (f.name.span, format_fn_signature(f)),
        Item::View(v) => (v.name.span, format!("view {}", v.name.name)),
        Item::Widget(w) => (w.name.span, format!("widget {}", w.name.name)),
        Item::Style(s) => (s.name.span, format!("style {}", s.name.name)),
        Item::Trait(t) => (t.name.span, format!("trait {}", t.name.name)),
        Item::Let(l) => (
            l.name.span,
            format!("let {} : {}", l.name.name, type_expr_render(&l.ty)),
        ),
        Item::Enum(e) => {
            let prefix = if e.is_error {
                "error"
            } else if e.is_extern {
                "extern enum"
            } else {
                "enum"
            };
            (e.name.span, format!("{prefix} {}", e.name.name))
        }
        Item::Flags(f) => {
            let prefix = if f.is_extern { "extern flags" } else { "flags" };
            (
                f.name.span,
                format!("{prefix} {} of {}", f.name.name, f.of.name),
            )
        }
        Item::Store(s) => (s.name.span, format!("store {}", s.name.name)),
        Item::Suite(s) => (s.name_span, format!("suite \"{}\"", s.name)),
        // Hovering over the trait name in `impl Foo for Bar` falls
        // through to the bare-ident reference path below; the impl
        // itself doesn't introduce a name.
        Item::Impl(_) => return None,
        Item::Use(_) | Item::UseQml(_) => return None,
    };
    if span_contains(name_span, offset) {
        Some(Target {
            span: name_span,
            label,
            def_span: Some(name_span),
        })
    } else {
        None
    }
}

fn walk_item_body(
    it: &Item,
    offset: u32,
    prog: &ResolvedProgram,
    module: &Module,
) -> Option<Target> {
    match it {
        Item::Fn(f) => f
            .body
            .as_ref()
            .and_then(|b| walk_block(b, offset, prog, module)),
        Item::Class(c) => {
            for m in &c.members {
                if let ClassMember::Fn(f) | ClassMember::Slot(f) = m {
                    if let Some(b) = &f.body {
                        if let Some(t) = walk_block(b, offset, prog, module) {
                            return Some(t);
                        }
                    }
                }
            }
            None
        }
        Item::View(v) => {
            // Element tree inside a view: hover on Element.name
            // resolves to the corresponding class / struct / error decl.
            walk_element(&v.root, offset, prog, module)
        }
        Item::Widget(w) => walk_element(&w.root, offset, prog, module),
        _ => None,
    }
}

fn walk_block(b: &Block, offset: u32, prog: &ResolvedProgram, module: &Module) -> Option<Target> {
    if !span_contains(b.span, offset) {
        return None;
    }
    for s in &b.stmts {
        if let Some(t) = walk_stmt(s, offset, prog, module) {
            return Some(t);
        }
    }
    if let Some(t) = &b.trailing {
        return walk_expr(t, offset, prog, module);
    }
    None
}

fn walk_stmt(s: &Stmt, offset: u32, prog: &ResolvedProgram, module: &Module) -> Option<Target> {
    use Stmt as S;
    match s {
        S::Let { value, .. } | S::Var { value, .. } => walk_expr(value, offset, prog, module),
        S::Expr(e) => walk_expr(e, offset, prog, module),
        S::Return { value, .. } => value
            .as_ref()
            .and_then(|v| walk_expr(v, offset, prog, module)),
        S::Emit { args, .. } => args.iter().find_map(|a| walk_expr(a, offset, prog, module)),
        S::Assign { target, value, .. } => walk_expr(target, offset, prog, module)
            .or_else(|| walk_expr(value, offset, prog, module)),
        S::For { iter, body, .. } => {
            walk_expr(iter, offset, prog, module).or_else(|| walk_block(body, offset, prog, module))
        }
        S::While { cond, body, .. } => {
            walk_expr(cond, offset, prog, module).or_else(|| walk_block(body, offset, prog, module))
        }
        S::Break { .. } | S::Continue { .. } => None,
        S::Batch { body, .. } => walk_block(body, offset, prog, module),
    }
}

fn walk_expr(e: &Expr, offset: u32, prog: &ResolvedProgram, module: &Module) -> Option<Target> {
    if !span_contains(e.span, offset) {
        return None;
    }
    use ExprKind as K;
    let nested = match &e.kind {
        K::Ident(name) => {
            // Bare identifier reference. Resolve against top-level items.
            return ident_reference_target(name, e.span, prog, module);
        }
        K::Call {
            callee,
            args,
            block,
            ..
        } => walk_expr(callee, offset, prog, module)
            .or_else(|| args.iter().find_map(|a| walk_expr(a, offset, prog, module)))
            .or_else(|| {
                block
                    .as_deref()
                    .and_then(|b| walk_expr(b, offset, prog, module))
            }),
        // `x.method(..)`. The METHOD name was dropped here the same
        // way the member name was below (§8.52) — which meant hover
        // and go-to-definition missed every call site in the file.
        K::MethodCall {
            receiver,
            method,
            args,
            block,
            ..
        } => {
            if span_contains(method.span, offset) {
                if let Some(t) =
                    member_target(receiver, &method.name, method.span, prog, module)
                {
                    return Some(t);
                }
            }
            walk_expr(receiver, offset, prog, module)
                .or_else(|| args.iter().find_map(|a| walk_expr(a, offset, prog, module)))
                .or_else(|| {
                    block
                        .as_deref()
                        .and_then(|b| walk_expr(b, offset, prog, module))
                })
        }
        // `x.member`. The receiver was walked and the member NAME was
        // dropped on the floor, so hovering the half of the
        // expression that carries the information answered nothing
        // (§8.52). Try the member first: it is the inner span, and
        // the receiver's own span encloses it.
        K::Member { receiver, name } => {
            if span_contains(name.span, offset) {
                if let Some(t) = member_target(receiver, &name.name, name.span, prog, module) {
                    return Some(t);
                }
            }
            walk_expr(receiver, offset, prog, module)
        }
        K::Index { receiver, index } => walk_expr(receiver, offset, prog, module)
            .or_else(|| walk_expr(index, offset, prog, module)),
        K::Unary { expr, .. } => walk_expr(expr, offset, prog, module),
        K::Binary { lhs, rhs, .. } => {
            walk_expr(lhs, offset, prog, module).or_else(|| walk_expr(rhs, offset, prog, module))
        }
        K::Try(inner) | K::Await(inner) => walk_expr(inner, offset, prog, module),
        K::Block(b) => walk_block(b, offset, prog, module),
        K::Lambda { body, .. } => walk_block(body, offset, prog, module),
        K::If {
            cond,
            then_b,
            else_b,
            ..
        } => walk_expr(cond, offset, prog, module)
            .or_else(|| walk_block(then_b, offset, prog, module))
            .or_else(|| {
                else_b
                    .as_ref()
                    .and_then(|b| walk_block(b, offset, prog, module))
            }),
        K::Case { scrutinee, arms } => walk_expr(scrutinee, offset, prog, module).or_else(|| {
            arms.iter()
                .find_map(|arm| walk_block(&arm.body, offset, prog, module))
        }),
        K::Str(parts) => parts.iter().find_map(|p| match p {
            StrPart::Interp(inner) | StrPart::InterpFmt { expr: inner, .. } => {
                walk_expr(inner, offset, prog, module)
            }
            StrPart::Text(_) => None,
        }),
        _ => None,
    };
    nested
}

fn walk_element(
    e: &Element,
    offset: u32,
    prog: &ResolvedProgram,
    module: &Module,
) -> Option<Target> {
    if !span_contains(e.span, offset) {
        return None;
    }
    // Hover on the element's leaf name (`Calculator { ... }`'s `Calculator`)
    // resolves to the matching top-level class / struct / error decl.
    if span_contains(e.name.span, offset) && e.module_path.is_empty() {
        if let Some(t) = ident_reference_target(&e.name.name, e.name.span, prog, module) {
            return Some(t);
        }
    }
    for m in &e.members {
        if let Some(t) = walk_element_member(m, offset, prog, module) {
            return Some(t);
        }
    }
    None
}

fn walk_element_member(
    m: &ElementMember,
    offset: u32,
    prog: &ResolvedProgram,
    module: &Module,
) -> Option<Target> {
    use ElementMember as M;
    match m {
        M::Property { value, .. } => walk_expr(value, offset, prog, module),
        M::Child(child) => walk_element(child, offset, prog, module),
        M::Stmt(s) => walk_stmt(s, offset, prog, module),
    }
}

/// Hover for `receiver.name`: find the class the receiver denotes,
/// then the property, method or signal called `name` on it. Only the
/// shapes the completion path already resolves — a top-level class,
/// or a `let` singleton, which is what a `store` desugars into.
fn member_target(
    receiver: &Expr,
    name: &str,
    span: Span,
    prog: &ResolvedProgram,
    module: &Module,
) -> Option<Target> {
    let ExprKind::Ident(r) = &receiver.kind else {
        return None;
    };
    let class = match prog.items.get(r)? {
        ItemKind::Class { .. } => r.clone(),
        ItemKind::Let { ty, .. } => named_type(ty)?,
        _ => return None,
    };
    // The declaration itself is the best label AND the definition
    // site, so one walk answers hover and go-to-definition together.
    for it in &module.items {
        let Item::Class(c) = it else { continue };
        if c.name.name != class {
            continue;
        }
        for m in &c.members {
            match m {
                ClassMember::Property(p) if p.name.name == name => {
                    return Some(Target {
                        span,
                        label: format!("prop {}.{} : {}", class, name, type_expr_render(&p.ty)),
                        def_span: Some(p.name.span),
                    });
                }
                ClassMember::Field(f) if f.name.name == name => {
                    return Some(Target {
                        span,
                        label: format!("let {}.{} : {}", class, name, type_expr_render(&f.ty)),
                        def_span: Some(f.name.span),
                    });
                }
                ClassMember::Fn(f) | ClassMember::Slot(f) if f.name.name == name => {
                    return Some(Target {
                        span,
                        label: format!("{}.{}", class, format_fn_signature(f)),
                        def_span: Some(f.name.span),
                    });
                }
                ClassMember::Signal(sg) if sg.name.name == name => {
                    return Some(Target {
                        span,
                        label: format!("signal {}.{}", class, name),
                        def_span: Some(sg.name.span),
                    });
                }
                _ => {}
            }
        }
    }
    None
}

fn ident_reference_target(
    name: &str,
    use_span: Span,
    prog: &ResolvedProgram,
    module: &Module,
) -> Option<Target> {
    let kind = prog.items.get(name)?;
    let label = match kind {
        ItemKind::Class {
            is_world_class,
            is_extern_value,
            ..
        } => {
            if *is_extern_value {
                format!("extern value {name} — C++ value type")
            } else if *is_world_class {
                format!("class {name} — World-backed handle type")
            } else {
                format!("arc {name} — legacy arc form (removed in pixie)")
            }
        }
        ItemKind::Struct { .. } => format!("struct {name} — value type, copy semantics"),
        ItemKind::Trait { .. } => format!("trait {name}"),
        ItemKind::Fn { .. } => module
            .items
            .iter()
            .find_map(|it| match it {
                Item::Fn(f) if f.name.name == name => Some(format_fn_signature(f)),
                _ => None,
            })
            .unwrap_or_else(|| format!("fn {name}")),
        ItemKind::Let { ty, .. } => format!(
            "let {name} : {} — top-level immutable",
            type_expr_render(ty)
        ),
        ItemKind::Enum {
            is_extern,
            is_error,
            variants,
            ..
        } => {
            let prefix = if *is_error {
                "error"
            } else if *is_extern {
                "extern enum"
            } else {
                "enum"
            };
            format!("{prefix} {name} — {} variant(s)", variants.len())
        }
        ItemKind::Flags { is_extern, of, .. } => {
            let prefix = if *is_extern { "extern flags" } else { "flags" };
            format!("{prefix} {name} of {of}")
        }
    };
    let def_span = find_item_name_span(module, name);
    Some(Target {
        span: use_span,
        label,
        def_span,
    })
}

fn find_item_name_span(module: &Module, name: &str) -> Option<Span> {
    for it in &module.items {
        let span = match it {
            Item::Class(c) if c.name.name == name => c.name.span,
            Item::Struct(s) if s.name.name == name => s.name.span,
            Item::Enum(e) if e.name.name == name => e.name.span,
            Item::Fn(f) if f.name.name == name => f.name.span,
            Item::View(v) if v.name.name == name => v.name.span,
            Item::Widget(w) if w.name.name == name => w.name.span,
            Item::Style(s) if s.name.name == name => s.name.span,
            _ => continue,
        };
        return Some(span);
    }
    None
}

fn format_fn_signature(f: &pixie_syntax::ast::FnDecl) -> String {
    let params: Vec<String> = f
        .params
        .iter()
        .map(|p| format!("{}: {}", p.name.name, render_type(&p.ty)))
        .collect();
    let ret = match &f.return_ty {
        Some(t) => format!(" {}", render_type(t)),
        None => String::new(),
    };
    let async_ = if f.is_async { "async " } else { "" };
    format!("{async_}fn {}({}){}", f.name.name, params.join(", "), ret)
}

fn render_type(t: &pixie_syntax::ast::TypeExpr) -> String {
    use pixie_syntax::ast::TypeKind as TK;
    match &t.kind {
        TK::Named { path, args } => {
            let dotted = path
                .iter()
                .map(|i| i.name.as_str())
                .collect::<Vec<_>>()
                .join(".");
            if args.is_empty() {
                dotted
            } else {
                let inner = args.iter().map(render_type).collect::<Vec<_>>().join(", ");
                format!("{dotted}<{inner}>")
            }
        }
        TK::Nullable(inner) => format!("{}?", render_type(inner)),
        TK::ErrorUnion(inner) => format!("!{}", render_type(inner)),
        TK::Fn { params, ret } => {
            let p = params
                .iter()
                .map(render_type)
                .collect::<Vec<_>>()
                .join(", ");
            format!("fn({p}) -> {}", render_type(ret))
        }
        TK::SelfType => "Self".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uri() -> Url {
        Url::parse("file:///tmp/test.pix").unwrap()
    }

    #[test]
    fn empty_buffer_has_no_diagnostics() {
        let diags = analyze(&uri(), "");
        assert!(diags.is_empty(), "got: {diags:?}");
    }

    #[test]
    fn syntax_error_surfaces_one_diagnostic() {
        let diags = analyze(&uri(), "fn main { ");
        assert!(!diags.is_empty(), "expected at least one diagnostic");
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::ERROR));
    }

    #[test]
    fn analyze_does_not_panic_on_store_decl() {
        // Reproducer for the missing-desugar regression: HIR has
        // `Item::Store(_) => unreachable!()` because the driver
        // pipeline runs `desugar_store` before HIR. LSP's analyze
        // path calls HIR directly, so without an equivalent desugar
        // wired in here the editor would panic on every keystroke
        // inside a store-bearing file.
        let src = r#"
store Application {
  state user : String = ""
  fn login(u: String) { user = u }
}
fn main { cli_app { Application.login("alice") } }
"#;
        let diags = analyze(&uri(), src);
        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    }

    /// Hover on the `store` name returns the `store Foo` label, not
    /// the desugared `class Foo` shape. The pre-pass replaces the
    /// `Item::Store` with `Item::Class + Item::Let` for the type
    /// checker's benefit, but the editor surface should report what
    /// the user wrote.
    #[test]
    fn hover_on_store_name_returns_store_label() {
        let src = "store Application { state user : String = \"\" }\n";
        // Cursor inside "Application" — column 8 hits 'p'.
        let pos = Position {
            line: 0,
            character: 8,
        };
        let h = hover_at(&uri(), src, pos).expect("hover");
        assert!(
            h.label.starts_with("store Application"),
            "expected `store Application` label, got `{}`",
            h.label,
        );
    }

    #[test]
    fn analyze_does_not_panic_on_suite_decl() {
        let src = r#"
suite "compute" {
  test "adds positive numbers" {
    assert_eq(2 + 2, 4)
  }
}
"#;
        let diags = analyze(&uri(), src);
        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    }

    #[test]
    fn well_formed_program_is_clean() {
        let src = r#"
            class Counter {
              prop count : Int, notify: :countChanged
              signal countChanged
              fn increment {
                count = count + 1
                emit countChanged
              }
            }
        "#;
        let diags = analyze(&uri(), src);
        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    }

    #[test]
    fn hover_on_top_level_class_name_returns_signature() {
        // `class Counter` starts at line 1 col 0. Cursor inside the
        // name "Counter" -> hover.
        let src = "class Counter {}\n";
        let pos = Position {
            line: 0,
            character: 8,
        };
        let h = hover_at(&uri(), src, pos).expect("hover");
        assert!(h.label.starts_with("class Counter"), "label: {}", h.label);
        assert!(h.def_location.is_some());
    }

    #[test]
    fn hover_on_top_level_fn_returns_full_signature() {
        let src = "fn add(a: Int, b: Int) Int { a + b }\n";
        let pos = Position {
            line: 0,
            character: 4,
        };
        let h = hover_at(&uri(), src, pos).expect("hover");
        assert_eq!(h.label, "fn add(a: Int, b: Int) Int");
    }

    #[test]
    fn hover_on_call_site_resolves_to_definition() {
        // Cursor on the `add` reference inside main's body. Should
        // surface the same signature as if hovering on the def.
        let src = "fn add(a: Int, b: Int) Int { a + b }\nfn main { let r = add(1, 2) }\n";
        // Locate the second occurrence of "add" (the call site).
        let call_byte = src.find("add(1, 2)").unwrap();
        let line = src[..call_byte].matches('\n').count() as u32;
        let line_start = src[..call_byte].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let character = (call_byte - line_start) as u32 + 1;
        let h = hover_at(&uri(), src, Position { line, character }).expect("hover");
        assert_eq!(h.label, "fn add(a: Int, b: Int) Int");
        assert!(h.def_location.is_some());
    }

    #[test]
    fn completion_after_dot_on_param_returns_class_members() {
        let src = "\
class Counter {
  prop count : Int, default: 0
  signal step
  fn run(c: Counter) {
    c.
  }
}
";
        // Cursor right after the `c.`, line 4 (0-indexed) col 6.
        let pos = Position {
            line: 4,
            character: 6,
        };
        let items = completion_at(&uri(), src, pos);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"count"), "missing `count`: {labels:?}");
        assert!(labels.contains(&"step"), "missing `step`: {labels:?}");
        assert!(labels.contains(&"run"), "missing `run`: {labels:?}");
    }

    #[test]
    fn completion_with_partial_filters_member_list() {
        let src = "\
class Counter {
  prop count : Int, default: 0
  signal step
  fn run(c: Counter) {
    c.co
  }
}
";
        let pos = Position {
            line: 4,
            character: 8,
        };
        let items = completion_at(&uri(), src, pos);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"count"), "expected `count` in: {labels:?}");
        assert!(
            !labels.contains(&"step"),
            "expected `step` filtered out: {labels:?}"
        );
    }

    #[test]
    fn completion_bare_returns_top_level_items() {
        let src = "\
class Counter {
  prop count : Int, default: 0
}

fn add(a: Int, b: Int) Int { a + b }

fn main {
  Co
}
";
        let pos = Position {
            line: 7,
            character: 4,
        };
        let items = completion_at(&uri(), src, pos);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(
            labels.contains(&"Counter"),
            "expected `Counter`: {labels:?}"
        );
    }

    #[test]
    fn completion_resolves_let_binding_to_class() {
        let src = "\
class Counter {
  prop count : Int, default: 0
  signal step
}

fn run {
  let c = Counter()
  c.
}
";
        // Cursor at line 7 ('  c.' — 0-indexed line 7), col 4 (after the `.`).
        let pos = Position {
            line: 7,
            character: 4,
        };
        let items = completion_at(&uri(), src, pos);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"count"), "missing count: {labels:?}");
        assert!(labels.contains(&"step"), "missing step: {labels:?}");
    }

    #[test]
    fn completion_resolves_let_binding_via_dot_new() {
        let src = "\
class Counter {
  prop count : Int, default: 0
}

fn run {
  let c = Counter.new()
  c.
}
";
        let pos = Position {
            line: 6,
            character: 4,
        };
        let items = completion_at(&uri(), src, pos);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"count"), "missing count: {labels:?}");
    }

    #[test]
    fn multi_file_hover_resolves_across_use() {
        // Project with main.pix that `use model`s a sibling
        // declaring `class Counter`. Hovering on `Counter` inside
        // main's body should resolve to model.pix's class decl.
        let dir = tempfile::tempdir().expect("tempdir");
        let model_path = dir.path().join("model.pix");
        let main_path = dir.path().join("main.pix");
        std::fs::write(
            &model_path,
            "class Counter {\n  prop Count : Int, default: 0\n}\n",
        )
        .unwrap();
        let main_src = "use model\n\nfn run {\n  let c = Counter()\n}\n";
        std::fs::write(&main_path, main_src).unwrap();

        let main_uri = Url::from_file_path(&main_path).unwrap();
        // Cursor on `Counter` in `let c = Counter()` (line 3, char 12).
        let pos = Position {
            line: 3,
            character: 12,
        };
        let h = hover_at_with_buffers(&main_uri, main_src, pos, &HashMap::new()).expect("hover");
        assert!(h.label.starts_with("class Counter"), "label: {}", h.label);
        let def = h.def_location.expect("definition known");
        assert!(
            def.uri.path().ends_with("model.pix"),
            "expected definition in model.pix, got {}",
            def.uri
        );
    }

    #[test]
    fn multi_file_completion_sees_use_imported_class() {
        let dir = tempfile::tempdir().expect("tempdir");
        let model_path = dir.path().join("model.pix");
        let main_path = dir.path().join("main.pix");
        std::fs::write(
            &model_path,
            "class Counter {\n  prop Count : Int, default: 0\n  fn Increment {}\n}\n",
        )
        .unwrap();
        let main_src = "use model\n\nfn run(c: Counter) {\n  c.\n}\n";
        std::fs::write(&main_path, main_src).unwrap();

        let main_uri = Url::from_file_path(&main_path).unwrap();
        let pos = Position {
            line: 3,
            character: 4,
        };
        let items = completion_at_with_buffers(&main_uri, main_src, pos, &HashMap::new());
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(
            labels.contains(&"Count"),
            "missing `Count` from cross-file class: {labels:?}"
        );
        assert!(
            labels.contains(&"Increment"),
            "missing `Increment`: {labels:?}"
        );
    }

    #[test]
    fn cache_key_is_deterministic_and_sensitive_to_inputs() {
        let uri1 = Url::parse("file:///a.pix").unwrap();
        let uri2 = Url::parse("file:///b.pix").unwrap();
        let mut buffers = HashMap::new();
        buffers.insert(PathBuf::from("/x.pix"), "x".to_string());
        buffers.insert(PathBuf::from("/y.pix"), "y".to_string());

        // Same inputs → same key.
        let k1 = compute_cache_key(&uri1, "hello", &buffers);
        let k2 = compute_cache_key(&uri1, "hello", &buffers);
        assert_eq!(k1, k2);

        // Different uri → different key.
        let k3 = compute_cache_key(&uri2, "hello", &buffers);
        assert_ne!(k1, k3);

        // Different text → different key.
        let k4 = compute_cache_key(&uri1, "world", &buffers);
        assert_ne!(k1, k4);

        // Different buffer content → different key.
        let mut buffers2 = buffers.clone();
        buffers2.insert(PathBuf::from("/x.pix"), "X".to_string());
        let k5 = compute_cache_key(&uri1, "hello", &buffers2);
        assert_ne!(k1, k5);

        // Buffer iteration order shouldn't matter (HashMap order
        // is non-deterministic; key sort happens inside).
        let mut buffers3 = HashMap::new();
        buffers3.insert(PathBuf::from("/y.pix"), "y".to_string());
        buffers3.insert(PathBuf::from("/x.pix"), "x".to_string());
        let k6 = compute_cache_key(&uri1, "hello", &buffers3);
        assert_eq!(k1, k6);
    }

    #[test]
    fn hover_outside_any_item_returns_none() {
        let src = "fn add(a: Int, b: Int) Int { a + b }\n";
        // Position past end of buffer.
        let pos = Position {
            line: 5,
            character: 0,
        };
        assert!(hover_at(&uri(), src, pos).is_none());
    }

    /// §8.52: hover, definition and completion on a MEMBER — the
    /// half of an expression that carries the information. The walker
    /// descended into receivers and dropped the member and method
    /// names, so every `Store.thing` in a file answered nothing.
    #[test]
    fn member_hover_definition_and_completion() {
        let src = "store Session {\n  state name : String = \"\"\n\n  fn greet(t: String) {\n    name = t\n  }\n}\n\nview Main {\n  Column {\n    Text { text: Session.name }\n    Button { text: \"go\"; onClick: Session.greet(\"x\") }\n  }\n}\n";
        let uri = Url::parse("file:///tmp/lsp-member.pix").unwrap();

        // Hovering `name` in `Session.name` describes the property
        // and points at its declaration, not at `Session`.
        let h = hover_at(&uri, src, Position { line: 10, character: 26 })
            .expect("hover on a member");
        assert!(
            h.label.contains("prop Session.name : String"),
            "hover said `{}`",
            h.label
        );
        assert!(h.def_location.is_some(), "a member hover knows its definition");

        // The same for a method name in a call.
        let m = hover_at(&uri, src, Position { line: 11, character: 42 })
            .expect("hover on a method name");
        assert!(m.label.contains("greet"), "hover said `{}`", m.label);
        assert_eq!(
            m.def_location.expect("definition").range.start.line,
            3,
            "go-to-definition lands on `fn greet`"
        );

        // And completion after the dot. A `store` is a top-level
        // `let` holding a class, which is the receiver shape that
        // used to resolve to nothing.
        let items = completion_at(&uri, src, Position { line: 10, character: 25 });
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"name"), "completion offered {labels:?}");
        assert!(labels.contains(&"greet"), "completion offered {labels:?}");
    }

    /// The hover fence is what the editor renders with, and it named
    /// the language pixie was forked FROM.
    #[test]
    fn the_hover_fence_says_pixie() {
        let src = "class C {\n  pub prop v : Int, default: 0\n}\n\nstore S {\n  state n : Int = 0\n}\n\nview Main {\n  Column { Text { text: \"#{S.n}\" } }\n}\n";
        let uri = Url::parse("file:///tmp/lsp-fence.pix").unwrap();
        let diags = analyze(&uri, src);
        assert!(
            diags.iter().all(|d| d.source.as_deref() == Some("pixie")),
            "diagnostics are sourced `pixie`"
        );
    }
}
