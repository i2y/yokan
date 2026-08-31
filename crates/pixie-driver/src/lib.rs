//! Minimal pixie driver: the check pipeline with no build system attached.
//!
//! Mirrors the ancestor's `check_file` order — parse, gate, bindings,
//! combine, resolve, check — except that where cute ran desugar pre-passes
//! (store / suite / widget-state), M0 gates those constructs out with a
//! diagnostic instead: HIR has `unreachable!` arms for them, so they must
//! never reach `resolve` undesugared.

pub mod desugar;

use std::path::Path;

use codespan_reporting::diagnostic::{Diagnostic as CrDiag, Label};
use codespan_reporting::files::SimpleFiles;
use codespan_reporting::term::{
    self,
    termcolor::{ColorChoice, StandardStream},
};
use pixie_hir::{ProjectInfo, ResolvedProgram, resolve};
use pixie_syntax::diag::{Diagnostic, Severity};
use pixie_syntax::span::FileId;
use pixie_syntax::{SourceMap, ast};
use pixie_types::{check_linearity, check_program};

#[derive(Debug, thiserror::Error)]
pub enum DriverError {
    #[error("cannot read `{path}`: {message}")]
    Io { path: String, message: String },
    #[error("internal error: {0}")]
    Internal(String),
}

pub struct CheckOutcome {
    pub source_map: SourceMap,
    /// `pub style` declarations of the entry module's direct
    /// imports, as source text — the emitter bakes this into the
    /// binary so the rung-2 reload can resolve cross-module styles.
    pub foreign_styles_src: String,
    /// The views of the entry module's direct imports, as source text
    /// segmented by `//pixie module: <name>` marker lines — the
    /// §8.29 cross-module component leg of the same reload story.
    pub foreign_components_src: String,
    /// Those same imports as (module name, absolute path), sorted.
    /// The running binary rereads them at every reload (§8.72), so a
    /// `pub style` or a component body in another module is live the
    /// way the entry's own view body is.
    pub foreign_paths: Vec<(String, String)>,
    /// The combined module (bindings + user). `None` when parsing failed.
    pub module: Option<ast::Module>,
    pub program: Option<ResolvedProgram>,
    /// What the checker knows and the emitter cannot derive (§8.55).
    /// Hand it to `pixie_codegen::emit_program_with`.
    pub check_info: pixie_codegen::CheckInfo,
    pub diagnostics: Vec<Diagnostic>,
    /// How many leading items of `module` came from bindings — codegen
    /// treats those as type surface (adapters at call sites), not as
    /// classes to emit.
    pub binding_items: usize,
}

impl CheckOutcome {
    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count()
    }
}


/// §12.2: the pixie-package dependencies visible from `entry` —
/// (name, module root `src/`, package root). Parsed from the
/// project's pixie.toml; git deps resolve to the CLI-fetched
/// checkout under `.pixie/deps/` (a missing checkout is reported at
/// use-resolution time with a "run `pixie build`" hint).
fn pixie_dep_roots(entry: &Path) -> Vec<(String, std::path::PathBuf, std::path::PathBuf)> {
    let mut dir = entry.parent().map(std::path::Path::to_path_buf);
    for _ in 0..5 {
        let Some(d) = dir else { break };
        let manifest = d.join("pixie.toml");
        if manifest.is_file() {
            let Ok(text) = std::fs::read_to_string(&manifest) else {
                return Vec::new();
            };
            let Ok(doc) = text.parse::<toml::Value>() else {
                return Vec::new();
            };
            let mut out = Vec::new();
            if let Some(table) = doc.get("dependencies").and_then(|t| t.as_table()) {
                for (name, spec) in table {
                    let pkg_root = match spec.get("path").and_then(|v| v.as_str()) {
                        Some(rel) => match d.join(rel).canonicalize() {
                            Ok(p) => p,
                            Err(_) => continue,
                        },
                        None => d.join(".pixie").join("deps").join(name),
                    };
                    out.push((name.clone(), pkg_root.join("src"), pkg_root));
                }
            }
            return out;
        }
        dir = d.parent().map(std::path::Path::to_path_buf);
    }
    Vec::new()
}

pub fn check_file(input: &Path) -> Result<CheckOutcome, DriverError> {
    let text = std::fs::read_to_string(input).map_err(|e| DriverError::Io {
        path: input.display().to_string(),
        message: e.to_string(),
    })?;

    let mut sm = SourceMap::default();
    // Bindings first so they own FileIds distinct from the user file —
    // the visibility check keys items by declaring file id.
    let mut bindings = pixie_binding::load_stdlib(&mut sm)
        .map_err(|e| DriverError::Internal(format!("stdlib binding: {e}")))?;

    // Entry-adjacent `.rpi` files load before the user source (sorted
    // for determinism), followed by the project's `[crates]` binding
    // cache (`<manifest root>/.pixie/rpi/` — §12.2): the CLI derives
    // those before calling us; here they are just more binding files.
    let dep_roots = pixie_dep_roots(input);
    let mut rpi_dirs: Vec<std::path::PathBuf> = Vec::new();
    if let Some(dir) = input.parent() {
        rpi_dirs.push(dir.to_path_buf());
        for (_, _, pkg_root) in &dep_roots {
            let cache = pkg_root.join(".pixie").join("rpi");
            if cache.is_dir() {
                rpi_dirs.push(cache);
            }
        }
        let mut probe = Some(dir.to_path_buf());
        for _ in 0..5 {
            let Some(d) = probe else { break };
            if d.join("pixie.toml").is_file() {
                let cache = d.join(".pixie").join("rpi");
                if cache.is_dir() {
                    rpi_dirs.push(cache);
                }
                break;
            }
            probe = d.parent().map(std::path::Path::to_path_buf);
        }
    }
    for dir in &rpi_dirs {
        let mut rpis: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "rpi"))
            .collect();
        rpis.sort();
        for p in rpis {
            let rpi_text = std::fs::read_to_string(&p).map_err(|e| DriverError::Io {
                path: p.display().to_string(),
                message: e.to_string(),
            })?;
            let name = p
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("binding.rpi")
                .to_string();
            let m = pixie_binding::parse_rpi(&mut sm, &name, &rpi_text)
                .map_err(|e| DriverError::Internal(e.to_string()))?;
            bindings.push(m);
        }
    }
    let binding_items: usize = bindings.iter().map(|b| b.items.len()).sum();

    // Load the entry plus every `use`d sibling module (BFS). Module name
    // = file stem; `use foo` resolves to `<entry_dir>/foo.pix`. Nested
    // paths, aliases, and selective imports are M2.
    let root = input
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let entry_module = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("main")
        .to_string();

    let parse_module = |sm: &mut SourceMap,
                            display: String,
                            text: String|
     -> Result<ast::Module, Diagnostic> {
        let fid = sm.add(display, text);
        let src = sm.source(fid).to_string();
        match pixie_syntax::parse(fid, &src) {
            Ok(m) => {
                let m = desugar::desugar_suite(m);
                let m = desugar::desugar_store(m);
                Ok(desugar::desugar_view_state(m))
            }
            Err(e) => Err(Diagnostic::error(e.span, e.message)),
        }
    };

    let entry = match parse_module(&mut sm, input.display().to_string(), text) {
        Ok(m) => m,
        Err(d) => {
            return Ok(CheckOutcome {
                check_info: Default::default(),
                source_map: sm,
                module: None,
                program: None,
                diagnostics: vec![d],
                binding_items,
                foreign_styles_src: String::new(),
                foreign_components_src: String::new(),
                foreign_paths: Vec::new(),
            });
        }
    };

    let mut info = ProjectInfo::default();
    // Where each module was read from — the reload rereads them.
    let mut module_path: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut loaded: Vec<(String, ast::Module)> = Vec::new();
    // Per-module loading context: (module root dir, canonical-name
    // prefix). Entry-side modules: (entry dir, ""); modules of a
    // pixie dependency `kit`: (kit/src, "kit.") — their internal
    // sibling imports resolve inside the package and namespace under
    // it (§12.2; a dep's own [dependencies] is M2).
    let mut module_ctx: std::collections::HashMap<String, (std::path::PathBuf, String)> =
        std::collections::HashMap::new();
    module_ctx.insert(entry_module.clone(), (root.clone(), String::new()));
    let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
    visited.insert(entry_module.clone());
    // fid of a loaded module = the span file of its first item; simpler:
    // record at load time.
    {
        let fid = FileId(sm.file_count() as u32 - 1);
        info.module_for_file.insert(fid, entry_module.clone());
    }
    loaded.push((entry_module.clone(), entry));

    let mut cursor = 0;
    while cursor < loaded.len() {
        let current_name = loaded[cursor].0.clone();
        let uses: Vec<ast::UseItem> = loaded[cursor]
            .1
            .items
            .iter()
            .filter_map(|i| match i {
                ast::Item::Use(u) => Some(u.clone()),
                _ => None,
            })
            .collect();
        cursor += 1;
        let (cur_root, cur_prefix) = module_ctx
            .get(&current_name)
            .cloned()
            .unwrap_or((root.clone(), String::new()));
        for u in uses {
            // §12.1: paths mirror directories — `use ui.buttons`
            // loads `<root>/ui/buttons.pix`; the canonical module
            // name is the dotted path. §12.2: from an entry-side
            // module, a first segment naming a pixie dependency
            // switches the walk root to that package's `src/`
            // (`use kit` = its `src/lib.pix` face).
            let segs: Vec<String> = u.path.iter().map(|i| i.name.clone()).collect();
            let dep_hit = if cur_prefix.is_empty() {
                dep_roots.iter().find(|(n, _, _)| *n == segs[0])
            } else {
                None
            };
            let (target, load_root, load_rel, load_prefix): (
                String,
                std::path::PathBuf,
                std::path::PathBuf,
                String,
            ) = if let Some((dep_name, dep_src, _)) = dep_hit {
                let target = segs.join(".");
                let mut rel = std::path::PathBuf::new();
                if segs.len() == 1 {
                    rel.push("lib");
                } else {
                    for s in &segs[1..] {
                        rel.push(s);
                    }
                }
                rel.set_extension("pix");
                (target, dep_src.clone(), rel, format!("{dep_name}."))
            } else {
                let target = format!("{cur_prefix}{}", segs.join("."));
                let mut rel = std::path::PathBuf::new();
                for s in &segs {
                    rel.push(s);
                }
                rel.set_extension("pix");
                (target, cur_root.clone(), rel, cur_prefix.clone())
            };
            match &u.kind {
                ast::UseKind::Module(alias) => {
                    info.imports_for_module
                        .entry(current_name.clone())
                        .or_default()
                        .insert(target.clone());
                    // The qualifier: the alias, else the leaf segment
                    // (whole-module imports reach items bare AND
                    // qualified — UseKind's contract).
                    let qual = alias
                        .as_ref()
                        .map(|a| a.name.clone())
                        .unwrap_or_else(|| segs.last().expect("nonempty use path").clone());
                    info.module_aliases
                        .entry(current_name.clone())
                        .or_default()
                        .insert(qual, target.clone());
                }
                ast::UseKind::Names(names) => {
                    for n in names {
                        // `X as A` binds the local name A to foo's X
                        // — resolved (and re-pointed at the final
                        // name) by the §12.1 pass below.
                        let local = n.alias.as_ref().unwrap_or(&n.name).name.clone();
                        info.selective_imports
                            .entry(current_name.clone())
                            .or_default()
                            .insert(local, (target.clone(), n.name.name.clone()));
                    }
                }
            }
            if visited.contains(&target) {
                continue;
            }
            let path = load_root.join(&load_rel);
            let text = match std::fs::read_to_string(&path) {
                Ok(t) => t,
                Err(_) => {
                    return Ok(CheckOutcome {
                        check_info: Default::default(),
                        source_map: sm,
                        module: None,
                        program: None,
                        diagnostics: vec![Diagnostic::error(
                            u.span,
                            format!(
                                "cannot find module `{target}` (looked at {}){}",
                                path.display(),
                                if path.starts_with(root.join(".pixie").join("deps")) {
                                    " — run `pixie build` to fetch pixie dependencies"
                                } else {
                                    ""
                                }
                            ),
                        )],
                        binding_items,
                        foreign_styles_src: String::new(),
                        foreign_components_src: String::new(),
                        foreign_paths: Vec::new(),
                    });
                }
            };
            let m = match parse_module(&mut sm, path.display().to_string(), text) {
                Ok(m) => m,
                Err(d) => {
                    return Ok(CheckOutcome {
                        check_info: Default::default(),
                        source_map: sm,
                        module: None,
                        program: None,
                        diagnostics: vec![d],
                        binding_items,
                        foreign_styles_src: String::new(),
                        foreign_components_src: String::new(),
                        foreign_paths: Vec::new(),
                    });
                }
            };
            let fid = FileId(sm.file_count() as u32 - 1);
            info.module_for_file.insert(fid, target.clone());
            module_ctx.insert(target.clone(), (load_root.clone(), load_prefix.clone()));
            visited.insert(target.clone());
            module_path.insert(
                target.clone(),
                path.canonicalize()
                    .unwrap_or_else(|_| path.clone())
                    .display()
                    .to_string(),
            );
            loaded.push((target, m));
        }
    }

    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    for (_, m) in &loaded {
        diagnostics.extend(m0_gate(m));
    }
    if diagnostics.iter().any(|d| d.severity == Severity::Error) {
        return Ok(CheckOutcome {
            check_info: Default::default(),
            source_map: sm,
            module: None,
            program: None,
            diagnostics,
            binding_items,
            foreign_styles_src: String::new(),
            foreign_components_src: String::new(),
            foreign_paths: Vec::new(),
        });
    }

    // §12.1 — cross-module name resolution (§8.22). The emitter's
    // namespace is flat, so a surface name declared in more than one
    // module ("contested") mangles per declaring module, and every
    // reference — bare, alias-qualified, selective (renames
    // included), or `pub use`-re-exported — rewrites to the final
    // unique name before the merge. Shadowing is never guessed: a
    // rewrite key that is also a binder is a hard error.
    {
        use std::collections::{HashMap, HashSet};
        let fail = |sm: SourceMap, span, msg: String, binding_items| CheckOutcome {
            check_info: Default::default(),
            source_map: sm,
            module: None,
            program: None,
            diagnostics: vec![Diagnostic::error(span, msg)],
            binding_items,
            foreign_styles_src: String::new(),
            foreign_components_src: String::new(),
            foreign_paths: Vec::new(),
        };
        // Item declarations per module; contested = declared in >1.
        let mut decls: HashMap<String, Vec<(String, bool)>> = HashMap::new();
        for (name, m) in &loaded {
            decls.insert(name.clone(), desugar::item_decls(m));
        }
        // DISTINCT declaring modules per name (a store desugars into
        // a same-named class + let INSIDE one module — not a contest).
        let mut owners: HashMap<&str, HashSet<&str>> = HashMap::new();
        for (mod_name, items) in &decls {
            for (n, _) in items {
                owners.entry(n.as_str()).or_default().insert(mod_name.as_str());
            }
        }
        let contested: HashSet<String> = owners
            .iter()
            .filter(|(_, ms)| ms.len() > 1)
            .map(|(n, _)| n.to_string())
            .collect();
        // Guard: a mangled name colliding with a real declaration
        // would silently alias — refuse.
        for c in &contested {
            for (mod_name, _items) in &decls {
                let mangled = desugar::mangle(c, mod_name);
                if decls.values().flatten().any(|(n, _)| *n == mangled) {
                    return Ok(fail(
                        sm,
                        loaded[0].1.span,
                        format!(
                            "`{mangled}` is declared explicitly and also the mangled name of `{c}` in `{mod_name}` — rename one"
                        ),
                        binding_items,
                    ));
                }
            }
        }
        // `pub use foo.{X}` re-exports + the exporter's own use of X.
        // (Whole-module `pub use foo` is M2.)
        for (mod_name, m) in &loaded {
            for item in &m.items {
                let ast::Item::Use(u) = item else { continue };
                if !u.is_pub {
                    continue;
                }
                let target = u
                    .path
                    .iter()
                    .map(|i| i.name.clone())
                    .collect::<Vec<_>>()
                    .join(".");
                match &u.kind {
                    ast::UseKind::Names(names) => {
                        for n in names {
                            let exported =
                                n.alias.as_ref().unwrap_or(&n.name).name.clone();
                            info.re_exports
                                .entry(mod_name.clone())
                                .or_default()
                                .insert(exported.clone(), (target.clone(), n.name.name.clone()));
                            info.selective_imports
                                .entry(mod_name.clone())
                                .or_default()
                                .insert(exported, (target.clone(), n.name.name.clone()));
                        }
                    }
                    ast::UseKind::Module(_) => {
                        return Ok(fail(
                            sm,
                            u.span,
                            "whole-module `pub use foo` is M2 — re-export names selectively (`pub use foo.{X}`)".into(),
                            binding_items,
                        ));
                    }
                }
            }
        }
        // Resolve (module, member) to its FINAL declaring module and
        // bare name, following re-export chains (≤ 8 hops).
        let resolve_full = |module: &str, member: &str| -> Option<(String, String)> {
            let mut m = module.to_string();
            let mut n = member.to_string();
            for _ in 0..8 {
                if decls
                    .get(&m)
                    .is_some_and(|is| is.iter().any(|(dn, _)| *dn == n))
                {
                    let f = if contested.contains(&n) {
                        desugar::mangle(&n, &m)
                    } else {
                        n.clone()
                    };
                    return Some((m, f));
                }
                match info
                    .re_exports
                    .get(&m)
                    .and_then(|t| t.get(&n))
                    .cloned()
                {
                    Some((src, orig)) => {
                        m = src;
                        n = orig;
                    }
                    None => return None,
                }
            }
            None
        };
        // §8.29: pub views per module, for the selective-import
        // exemption below (computed up front — the loop holds a
        // mutable borrow of `loaded`).
        let pub_views: HashMap<String, std::collections::HashSet<String>> = loaded
            .iter()
            .map(|(n, lm)| {
                let vs = lm
                    .items
                    .iter()
                    .filter_map(|i| match i {
                        ast::Item::View(v) if v.is_pub => Some(v.name.name.clone()),
                        _ => None,
                    })
                    .collect();
                (n.clone(), vs)
            })
            .collect();
        // Per module: erasure (qualified refs), then the bare-name
        // rename map (own contested decls, selective imports,
        // whole-import contested resolution).
        for (mod_name, m) in &mut loaded {
            let binders = desugar::binder_names(m);
            let quals: HashMap<String, String> = info
                .module_aliases
                .get(mod_name)
                .cloned()
                .unwrap_or_default();
            if let Some(clash) = quals.keys().find(|q| binders.contains(*q)) {
                return Ok(fail(
                    sm,
                    m.span,
                    format!(
                        "module qualifier `{clash}` collides with a declared name in `{mod_name}` — alias the import (`use … as Other`) or rename the local"
                    ),
                    binding_items,
                ));
            }
            let resolver = |md: &str, member: &str| resolve_full(md, member).map(|(_, f)| f);
            let mut ecx = desugar::EraseCtx {
                quals: &quals,
                resolve: &resolver,
                error: None,
            };
            desugar::erase_module_qualifiers(m, &mut ecx);
            if let Some((span, msg)) = ecx.error {
                return Ok(fail(sm, span, msg, binding_items));
            }

            let mut rename: HashMap<String, String> = HashMap::new();
            // Own contested declarations.
            if let Some(items) = decls.get(mod_name) {
                for (n, _) in items {
                    if contested.contains(n) {
                        rename.insert(n.clone(), desugar::mangle(n, mod_name));
                    }
                }
            }
            // Selective imports (renames included).
            let selectives: Vec<(String, (String, String))> = info
                .selective_imports
                .get(mod_name)
                .map(|t| t.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default();
            for (local, (src, orig)) in &selectives {
                let Some((decl_mod, f)) = resolve_full(src, orig) else {
                    // §8.29: a selectively imported `pub view` is a
                    // component — the splice resolves it from the
                    // module's own `use` items, no rename needed here.
                    let is_view = pub_views
                        .get(src.as_str())
                        .is_some_and(|vs| vs.contains(orig.as_str()));
                    if is_view {
                        continue;
                    }
                    return Ok(fail(
                        sm,
                        m.span,
                        format!("module `{src}` has no item `{orig}` (selective import in `{mod_name}`)"),
                        binding_items,
                    ));
                };
                if *local != f {
                    rename.insert(local.clone(), f.clone());
                }
                let tbl = info.selective_imports.entry(mod_name.clone()).or_default();
                tbl.remove(local);
                tbl.insert(f.clone(), (decl_mod, f));
            }
            // Contested names reachable bare through whole imports.
            let own: HashSet<&str> = decls
                .get(mod_name)
                .map(|is| is.iter().map(|(n, _)| n.as_str()).collect())
                .unwrap_or_default();
            let imports: Vec<String> = info
                .imports_for_module
                .get(mod_name)
                .map(|s| {
                    let mut v: Vec<String> = s.iter().cloned().collect();
                    v.sort();
                    v
                })
                .unwrap_or_default();
            for c in &contested {
                if own.contains(c.as_str()) || rename.contains_key(c) {
                    continue;
                }
                let mut candidates: Vec<(String, String)> = Vec::new();
                for imp in &imports {
                    if let Some(hit) = resolve_full(imp, c) {
                        if !candidates.contains(&hit) {
                            candidates.push(hit);
                        }
                    }
                }
                match candidates.len() {
                    0 => {}
                    1 => {
                        rename.insert(c.clone(), candidates[0].1.clone());
                    }
                    _ => {
                        if desugar::mentions_ident(m, c) {
                            let mods: Vec<&str> =
                                candidates.iter().map(|(md, _)| md.as_str()).collect();
                            return Ok(fail(
                                sm,
                                m.span,
                                format!(
                                    "`{c}` is ambiguous in `{mod_name}` (declared in {}) — qualify it or import selectively",
                                    mods.join(" and ")
                                ),
                                binding_items,
                            ));
                        }
                    }
                }
            }
            if let Some(clash) = rename.keys().find(|k| binders.contains(*k)) {
                return Ok(fail(
                    sm,
                    m.span,
                    format!(
                        "`{clash}` names both a cross-module item and a local binder in `{mod_name}` — rename the local"
                    ),
                    binding_items,
                ));
            }
            desugar::rename_in_module(m, &rename);
        }
    }

    // Style splice, whole-project env: each module resolves its own
    // styles plus the `pub` styles of its DIRECT imports (visibility
    // discipline — no transitive reach). Runs after every module is
    // loaded so a `use theme`-provided style works, and before the
    // merge so codegen sees only ordinary properties. The entry's
    // foreign styles are also collected as source text for the
    // emitter (the rung-2 reload path re-resolves from that snippet).
    let mut foreign_styles_src = String::new();
    let mut foreign_components_src = String::new();
    {
        // §8.29: every module's views, cloned once — the component
        // splice resolves imported components from here, and the
        // entry's direct imports' views are collected as source text
        // for the rung-2 reload (same story as foreign styles; the
        // slices are pre-rename source, so contested global names
        // inside a foreign component body stay a rebuild-only zone).
        let pub_styles_of = |name: &str| -> Vec<&ast::StyleDecl> {
            loaded
                .iter()
                .filter(|(n, _)| n == name)
                .flat_map(|(_, m)| m.items.iter())
                .filter_map(|i| match i {
                    ast::Item::Style(s) if s.is_pub => Some(s),
                    _ => None,
                })
                .collect()
        };
        // PHASE 1 (§8.75): every module's styles resolve in the module
        // that WROTE them, before any body moves. `views_of` is built
        // from the result, so an exported component travels with its
        // styles already spliced in — the importer never has to know
        // the exporter's private ones.
        let mut styled: Vec<(String, ast::Module)> = Vec::new();
        for (name, m) in &loaded {
            let mut own_foreign: Vec<&ast::StyleDecl> = Vec::new();
            if let Some(imports) = info.imports_for_module.get(name) {
                let mut sorted: Vec<&String> = imports.iter().collect();
                sorted.sort();
                for imp in sorted {
                    own_foreign.extend(pub_styles_of(imp));
                }
            }
            let m = pixie_syntax::iflet::desugar_module(m);
            match pixie_syntax::style::desugar_module_with(&m, &own_foreign) {
                Ok(sm1) => styled.push((name.clone(), sm1)),
                Err(e) => {
                    return Ok(CheckOutcome {
                        check_info: Default::default(),
                        source_map: sm,
                        module: None,
                        program: None,
                        diagnostics: vec![Diagnostic::error(e.span, e.message)],
                        binding_items,
                        foreign_styles_src: String::new(),
                        foreign_components_src: String::new(),
                        foreign_paths: Vec::new(),
                    });
                }
            }
        }
        let views_of: std::collections::HashMap<String, Vec<ast::ViewDecl>> = styled
            .iter()
            .map(|(name, m)| {
                let vs: Vec<ast::ViewDecl> = m
                    .items
                    .iter()
                    .filter_map(|i| match i {
                        ast::Item::View(v) => Some(v.clone()),
                        _ => None,
                    })
                    .collect();
                (name.clone(), vs)
            })
            .collect();

        let mut spliced: Vec<(String, ast::Module)> = Vec::new();
        for (name, m) in &styled {
            let mut foreign: Vec<&ast::StyleDecl> = Vec::new();
            if let Some(imports) = info.imports_for_module.get(name) {
                let mut sorted: Vec<&String> = imports.iter().collect();
                sorted.sort();
                for imp in sorted {
                    foreign.extend(pub_styles_of(imp));
                }
            }
            let mut foreign_views: Vec<(String, Vec<ast::ViewDecl>)> = Vec::new();
            if let Some(imports) = info.imports_for_module.get(name) {
                let mut sorted: Vec<&String> = imports.iter().collect();
                sorted.sort();
                for imp in sorted {
                    if let Some(vs) = views_of.get(imp.as_str()) {
                        if !vs.is_empty() {
                            foreign_views.push((imp.clone(), vs.clone()));
                        }
                    }
                }
            }
            if name == &entry_module {
                for s in &foreign {
                    let src = sm.source(s.span.file);
                    foreign_styles_src
                        .push_str(&src[s.span.start as usize..s.span.end as usize]);
                    foreign_styles_src.push('\n');
                }
                for (imp, vs) in &foreign_views {
                    foreign_components_src.push_str(&format!("//pixie module: {imp}\n"));
                    for v in vs {
                        let src = sm.source(v.span.file);
                        // The decl span starts at `view` — the `pub`
                        // keyword sits before it, so restate it.
                        if v.is_pub {
                            foreign_components_src.push_str("pub ");
                        }
                        foreign_components_src
                            .push_str(&src[v.span.start as usize..v.span.end as usize]);
                        foreign_components_src.push('\n');
                    }
                }
            }
            // PHASE 2: components. Every body arrives style-resolved
            // from phase 1, so moving one between modules carries its
            // meaning with it (§8.75).
            let cm = match pixie_syntax::component::splice_module_with(
                m,
                &foreign_views,
                name == &entry_module,
            ) {
                Ok(cm) => cm,
                Err(e) => {
                    return Ok(CheckOutcome {
                        check_info: Default::default(),
                        source_map: sm,
                        module: None,
                        program: None,
                        diagnostics: vec![Diagnostic::error(e.span, e.message)],
                        binding_items,
                        foreign_styles_src: String::new(),
                        foreign_components_src: String::new(),
                        foreign_paths: Vec::new(),
                    });
                }
            };
            // Phase 1 already resolved what this module wrote, and the
            // spliced bodies came in resolved — so this pass has
            // nothing left to find. It stays as the backstop for a
            // `style:` the splice itself introduces.
            match pixie_syntax::style::desugar_module_with(&cm, &foreign) {
                Ok(sm2) => spliced.push((name.clone(), sm2)),
                Err(e) => {
                    return Ok(CheckOutcome {
                        check_info: Default::default(),
                        source_map: sm,
                        module: None,
                        program: None,
                        diagnostics: vec![Diagnostic::error(e.span, e.message)],
                        binding_items,
                        foreign_styles_src: String::new(),
                        foreign_components_src: String::new(),
                        foreign_paths: Vec::new(),
                    });
                }
            }
        }
        loaded = spliced;
    }

    let mut items: Vec<ast::Item> = bindings
        .iter()
        .flat_map(|b| b.items.iter().cloned())
        .collect();
    let entry_span = loaded[0].1.span;
    for (_, m) in &loaded {
        items.extend(m.items.iter().cloned());
    }
    let combined = ast::Module {
        items,
        span: entry_span,
    };

    let rr = resolve(&combined, &info);
    diagnostics.extend(rr.diagnostics);
    let check = check_program(&combined, &rr.program);
    diagnostics.extend(check.diagnostics);
    diagnostics.extend(check_linearity(&combined, &rr.program));

    // The entry's direct imports, by name and canonical path (§8.72).
    let mut foreign_paths: Vec<(String, String)> = info
        .imports_for_module
        .get(&entry_module)
        .into_iter()
        .flatten()
        .filter_map(|imp| module_path.get(imp).map(|p| (imp.clone(), p.clone())))
        .collect();
    foreign_paths.sort();
    Ok(CheckOutcome {
        foreign_paths,
        source_map: sm,
        module: Some(combined),
        program: Some(rr.program),
        check_info: pixie_codegen::CheckInfo {
            int_to_float: check.int_to_float,
        },
        diagnostics,
        binding_items,
        foreign_styles_src,
        foreign_components_src,
    })
}

/// The M0 surface gate. Constructs that cute desugared before HIR (or that
/// pixie drops outright) get a diagnostic here instead of reaching
/// `resolve`, which would hit `unreachable!` arms for some of them.
fn m0_gate(module: &ast::Module) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for item in &module.items {
        match item {
            ast::Item::UseQml(u) => out.push(Diagnostic::error(
                u.span,
                "`use qml` is not part of pixie (no QML target)",
            )),
            ast::Item::Widget(w) => out.push(Diagnostic::error(
                w.span,
                "`widget` was merged into `view` in pixie; declare `view` instead",
            )),
            _ => {}
        }
    }
    out
}

/// Render diagnostics to stderr with codespan. Files are added in FileId
/// order so codespan's opaque ids coincide with `FileId(n)`.
pub fn render_diagnostics(source_map: &SourceMap, diags: &[Diagnostic]) {
    let mut files: SimpleFiles<String, String> = SimpleFiles::new();
    for i in 0..source_map.file_count() {
        let id = FileId(i as u32);
        files.add(
            source_map.name(id).to_string(),
            source_map.source(id).to_string(),
        );
    }
    let writer = StandardStream::stderr(ColorChoice::Auto);
    let config = term::Config::default();
    for d in diags {
        let mut cr = match d.severity {
            Severity::Error => CrDiag::error(),
            Severity::Warning => CrDiag::warning(),
            Severity::Note => CrDiag::note(),
        }
        .with_message(&d.message)
        .with_labels(vec![Label::primary(
            d.primary.file.0 as usize,
            d.primary.start as usize..d.primary.end as usize,
        )]);
        for (span, note) in &d.notes {
            cr = cr.with_labels(vec![
                Label::secondary(span.file.0 as usize, span.start as usize..span.end as usize)
                    .with_message(note),
            ]);
        }
        let _ = term::emit(&mut writer.lock(), &config, &files, &cr);
    }
}
