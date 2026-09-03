//! pixie CLI, M0 surface: `pixie check <file.pix>` and
//! `pixie build <file.pix> [--out-dir DIR] [--run]`.
//!
//! `build` emits a Rust crate next to the entry (default
//! `<entry_dir>/.pixie/<stem>/`) and drives `cargo` on it. The kernel is
//! resolved relative to this binary's own source tree — the dev-tree
//! assumption; `pixie install-runtime` replaces it later.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use pixie_syntax::diag::Diagnostic;

mod manifest;
mod pkg;

fn usage() -> ExitCode {
    eprintln!("usage: pixie check <file.pix>");
    eprintln!("       pixie build [<file.pix>] [--out-dir DIR] [--run] [--release]   (no file: src/main.pix of the pixie.toml project in cwd)");
    eprintln!("       pixie fmt <file.pix> [--check]");
    eprintln!("       pixie test <file.pix>");
    eprintln!("       pixie watch <file.pix>");
    eprintln!("       pixie install-runtime");
    eprintln!("       pixie new <name>                    (scaffold a project)");
    eprintln!("       pixie add <name> [VERSION] [--git URL [--branch|--tag|--rev X]] [--path DIR] [--crate [--features a,b] [--bind mod=Class]]");
    eprintln!("       pixie update [<name>]               (re-resolve pixie deps, refresh pixie.lock)");
    eprintln!("       pixie remove <name>                 (drop a dependency + its caches)");
    ExitCode::from(2)
}

/// Shared cargo target dir for every generated crate: dependencies (the
/// kernel today, the engine tomorrow) compile once per machine instead of
/// once per app.
fn shared_target_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".cache").join("pixie").join("target"))
}

fn cargo_cmd(verb: &str, manifest: &Path) -> Command {
    let mut c = Command::new("cargo");
    // rustup discovers rust-toolchain.toml from the process cwd (not
    // the manifest path), so run cargo from the generated crate's own
    // directory — that's where write_crate copied the dev tree's pin.
    let abs = manifest
        .canonicalize()
        .unwrap_or_else(|_| manifest.to_path_buf());
    c.arg(verb).arg("-q").arg("--manifest-path").arg(&abs);
    if let Some(dir) = abs.parent() {
        c.current_dir(dir);
    }
    // When pixie itself was launched through rustup's cargo shim
    // (`cargo run -p pixie-cli …`), the shim exports RUSTUP_TOOLCHAIN
    // and that inherited var would beat the crate's own pin file.
    c.env_remove("RUSTUP_TOOLCHAIN");
    if let Some(t) = shared_target_dir() {
        c.env("CARGO_TARGET_DIR", t);
    }
    c
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("check") => {
            let [_, file] = args.as_slice() else {
                return usage();
            };
            cmd_check(Path::new(file))
        }
        Some("test") => {
            let Some(file) = args.get(1) else {
                return usage();
            };
            cmd_test(Path::new(file))
        }
        Some("watch") => {
            let Some(file) = args.get(1) else {
                return usage();
            };
            cmd_watch(Path::new(file))
        }
        Some("install-runtime") => cmd_install_runtime(),
        Some("new") => {
            let [_, name] = args.as_slice() else {
                return usage();
            };
            let cwd = std::env::current_dir().unwrap_or_default();
            match pkg::cmd_new(&cwd, name) {
                Ok(dir) => {
                    eprintln!("pixie: created {}", dir.display());
                    eprintln!("pixie: next: cd {name} && pixie build --run");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("pixie: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("add") => {
            let cwd = std::env::current_dir().unwrap_or_default();
            match pkg::cmd_add(&cwd, &args[1..]) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("pixie: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("update") => {
            if args.len() > 2 {
                return usage();
            }
            let cwd = std::env::current_dir().unwrap_or_default();
            match pkg::cmd_update(&cwd, args.get(1).map(String::as_str)) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("pixie: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("remove") => {
            let [_, name] = args.as_slice() else {
                return usage();
            };
            let cwd = std::env::current_dir().unwrap_or_default();
            match pkg::cmd_remove(&cwd, name) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("pixie: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("fmt") => {
            // `--check` is accepted on either side of the path.
            let rest = &args[1..];
            let check_only = rest.iter().any(|a| a == "--check");
            let files: Vec<&String> = rest.iter().filter(|a| a.as_str() != "--check").collect();
            let [file] = files.as_slice() else {
                return usage();
            };
            cmd_fmt(Path::new(file), check_only)
        }
        Some("build") => {
            // §12.2: bare `pixie build` inside a project builds the
            // manifest's conventional entry, `src/main.pix`.
            let manifest_entry: Option<String> =
                if args.get(1).is_none_or(|a| a.starts_with("--")) {
                    let cwd = std::env::current_dir().unwrap_or_default();
                    let probe = cwd.join("pixie.toml");
                    probe
                        .is_file()
                        .then(|| cwd.join("src").join("main.pix").display().to_string())
                } else {
                    None
                };
            let file = match (&manifest_entry, args.get(1)) {
                (Some(m), _) => m,
                (None, Some(f)) if !f.starts_with("--") => f,
                _ => return usage(),
            };
            let mut out_dir: Option<PathBuf> = None;
            let mut run = false;
            let mut release = false;
            // A build nobody will hot-reload — a gate's compiled tier,
            // say — has no use for the interpreter, and leaving it out
            // of the crate graph is most of the link.
            let mut no_interp = false;
            // Flags start right after `build` when the entry came
            // from the manifest, after the path otherwise.
            let mut i = if manifest_entry.is_some() { 1 } else { 2 };
            while i < args.len() {
                match args[i].as_str() {
                    "--out-dir" => {
                        let Some(d) = args.get(i + 1) else {
                            return usage();
                        };
                        out_dir = Some(PathBuf::from(d));
                        i += 2;
                    }
                    "--run" => {
                        run = true;
                        i += 1;
                    }
                    "--release" => {
                        release = true;
                        i += 1;
                    }
                    "--no-interp" => {
                        no_interp = true;
                        i += 1;
                    }
                    _ => return usage(),
                }
            }
            cmd_build(Path::new(file), out_dir, run, release, no_interp)
        }
        _ => usage(),
    }
}

fn cmd_check(file: &Path) -> ExitCode {
    let outcome = match pixie_driver::check_file(file) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("pixie: {e}");
            return ExitCode::FAILURE;
        }
    };
    pixie_driver::render_diagnostics(&outcome.source_map, &outcome.diagnostics);
    if outcome.error_count() > 0 {
        eprintln!("pixie: {} error(s)", outcome.error_count());
        ExitCode::FAILURE
    } else {
        println!("ok");
        ExitCode::SUCCESS
    }
}

fn cmd_test(file: &Path) -> ExitCode {
    let outcome = match pixie_driver::check_file(file) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("pixie: {e}");
            return ExitCode::FAILURE;
        }
    };
    pixie_driver::render_diagnostics(&outcome.source_map, &outcome.diagnostics);
    if outcome.error_count() > 0 {
        eprintln!("pixie: {} error(s)", outcome.error_count());
        return ExitCode::FAILURE;
    }
    let module = outcome.module.as_ref().expect("checked module");
    let code = match pixie_codegen::emit_test_program(module, outcome.binding_items) {
        Ok(c) => c,
        Err(e) => {
            let d = Diagnostic::error(e.span, e.message);
            pixie_driver::render_diagnostics(&outcome.source_map, &[d]);
            eprintln!("pixie: emit failed");
            return ExitCode::FAILURE;
        }
    };
    let stem = file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("app")
        .replace('-', "_");
    let entry_dir = file.parent().unwrap_or(Path::new("."));
    let out = entry_dir.join(".pixie").join(format!("{stem}_test"));
    let kernel_path = match kernel_dir() {
        Some(k) => k,
        None => {
            eprintln!("pixie: cannot locate pixie-kernel (dev tree expected)");
            return ExitCode::FAILURE;
        }
    };
    let crate_name = format!("{stem}_test");
    let test_extra = match prepare_manifest(file) {
        Ok(m) => m.map(|(_, lines)| lines).unwrap_or_default(),
        Err(e) => {
            eprintln!("pixie: {e}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(e) = write_crate(&out, &crate_name, &kernel_path, None, None, &test_extra, &code) {
        eprintln!("pixie: cannot write generated crate: {e}");
        return ExitCode::FAILURE;
    }
    let status = cargo_cmd("run", &out.join("Cargo.toml")).status();
    match status {
        Ok(s) if s.success() => ExitCode::SUCCESS,
        Ok(_) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("pixie: cannot run cargo: {e}");
            ExitCode::FAILURE
        }
    }
}

fn cmd_fmt(file: &Path, check_only: bool) -> ExitCode {
    let src = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("pixie: cannot read {}: {e}", file.display());
            return ExitCode::FAILURE;
        }
    };
    match pixie_syntax::format_source(pixie_syntax::FileId(0), &src) {
        Ok(formatted) => {
            if formatted == src {
                ExitCode::SUCCESS
            } else if check_only {
                eprintln!("pixie: {} needs formatting", file.display());
                ExitCode::FAILURE
            } else if let Err(e) = std::fs::write(file, formatted) {
                eprintln!("pixie: cannot write {}: {e}", file.display());
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(e) => {
            eprintln!("pixie: fmt failed: {e:?}");
            ExitCode::FAILURE
        }
    }
}

/// The rung-2 wiring for a build: the entry's canonical path plus the
/// fingerprint of its raw parse (the same parse the in-app reload
/// does). `None` when the source can't be parsed — the check already
/// failed in that case.
/// What the running binary needs to reload: where its source lives,
/// which modules it imports, and a fingerprint.
///
/// `fingerprint` is `pixie_interp::program_fingerprint` — the entry
/// plus its imports. The build bakes it, `pixie watch` compares it,
/// and the running binary re-derives it; one function for all three,
/// because three expressions of the same idea drifted apart the
/// first time (§8.72).
fn reload_info_with(
    file: &Path,
    foreign_paths: Vec<(String, String)>,
) -> Option<pixie_codegen::ReloadInfo> {
    let text = std::fs::read_to_string(file).ok()?;
    let fingerprint = pixie_interp::program_fingerprint_of(&text, &foreign_paths).ok()?;
    let abs = file.canonicalize().ok()?;
    Some(pixie_codegen::ReloadInfo {
        source_path: abs.display().to_string(),
        fingerprint,
        foreign_paths,
    })
}

/// Check + emit + cargo-build one program. Returns the binary path on
/// success; diagnostics were already rendered on failure.
/// §12.2: locate pixie.toml above the entry, fetch pixie package
/// deps to their locked revisions, and derive every `[crates]`
/// binding (the project's own AND each dep's) into the caches BEFORE
/// the check runs. Returns the manifest plus the combined
/// `[dependencies]` lines for the generated crate.
fn prepare_manifest(file: &Path) -> Result<Option<(manifest::Manifest, String)>, String> {
    let Some(m) = manifest::find(file)? else {
        return Ok(None);
    };
    m.ensure_rpi()?;
    let mut dep_lines = m.dep_lines();
    for (name, root) in m.ensure_deps()? {
        let dep_manifest_path = root.join("pixie.toml");
        let dep_m = manifest::find(&dep_manifest_path.join("x"))?
            .ok_or_else(|| format!("dependency `{name}` lost its pixie.toml"))?;
        if !dep_m.deps.is_empty() {
            return Err(format!(
                "dependency `{name}` has its own [dependencies] — transitive pixie deps are M2"
            ));
        }
        dep_m.ensure_rpi()?;
        dep_lines.push_str(&dep_m.dep_lines());
    }
    Ok(Some((m, dep_lines)))
}

fn build_file(
    file: &Path,
    out_dir: Option<PathBuf>,
    release: bool,
    no_interp: bool,
) -> Option<PathBuf> {
    let mf = match prepare_manifest(file) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("pixie: {e}");
            return None;
        }
    };
    let outcome = match pixie_driver::check_file(file) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("pixie: {e}");
            return None;
        }
    };
    pixie_driver::render_diagnostics(&outcome.source_map, &outcome.diagnostics);
    if outcome.error_count() > 0 {
        eprintln!("pixie: {} error(s)", outcome.error_count());
        return None;
    }
    let module = outcome.module.as_ref().expect("checked module");

    // §11.9: `--release` is AOT-only — no reload support, no embedded
    // source path, no interpreter in the crate graph. `--no-interp`
    // asks for the same crate graph while keeping the dev profile: a
    // gate's compiled tier is never reloaded, and the interpreter is
    // most of what it would link.
    let reload = if release || no_interp {
        None
    } else {
        reload_info_with(file, outcome.foreign_paths.clone())
    };
    let win = mf
        .as_ref()
        .map(|(m, _)| pixie_codegen::WindowOpts {
            title: m.window.title.clone(),
            width: m.window.width,
            height: m.window.height,
        })
        .unwrap_or_default();
    let code = match pixie_codegen::emit_program_with_window(
        module,
        outcome.binding_items,
        reload.as_ref(),
        &outcome.check_info,
        &win,
    ) {
        Ok(c) => c,
        Err(e) => {
            let d = Diagnostic::error(e.span, e.message);
            pixie_driver::render_diagnostics(&outcome.source_map, &[d]);
            eprintln!("pixie: emit failed");
            return None;
        }
    };

    let stem = file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("app")
        .replace('-', "_");
    let entry_dir = file.parent().unwrap_or(Path::new("."));
    let out = out_dir.unwrap_or_else(|| entry_dir.join(".pixie").join(&stem));

    let kernel_path = kernel_dir()?;
    let extra = mf.as_ref().map(|(_, lines)| lines.clone()).unwrap_or_default();
    let interp = if release || no_interp { None } else { interp_dir() };
    if let Err(e) = write_crate(
        &out,
        &stem,
        &kernel_path,
        engine_dir().as_deref(),
        interp.as_deref(),
        &extra,
        &code,
    ) {
        eprintln!("pixie: cannot write generated crate: {e}");
        return None;
    }
    let manifest = out.join("Cargo.toml");
    let mut cargo = cargo_cmd("build", &manifest);
    if release {
        cargo.arg("--release");
    }
    match cargo.status() {
        Ok(s) if s.success() => {
            let base = shared_target_dir().unwrap_or_else(|| out.join("target"));
            let profile = if release { "release" } else { "debug" };
            Some(base.join(profile).join(&stem))
        }
        Ok(s) => {
            eprintln!("pixie: cargo exited with {s}");
            None
        }
        Err(e) => {
            eprintln!("pixie: cannot run cargo: {e}");
            None
        }
    }
}

fn cmd_build(
    file: &Path,
    out_dir: Option<PathBuf>,
    run: bool,
    release: bool,
    no_interp: bool,
) -> ExitCode {
    let Some(bin) = build_file(file, out_dir, release, no_interp) else {
        return ExitCode::FAILURE;
    };
    if run {
        match Command::new(&bin).status() {
            Ok(s) if s.success() => ExitCode::SUCCESS,
            Ok(_) => ExitCode::FAILURE,
            Err(e) => {
                eprintln!("pixie: cannot run {}: {e}", bin.display());
                ExitCode::FAILURE
            }
        }
    } else {
        println!("built: {}", bin.display());
        ExitCode::SUCCESS
    }
}

fn scan_sources(dir: &Path) -> Vec<(PathBuf, std::time::SystemTime)> {
    let mut out: Vec<(PathBuf, std::time::SystemTime)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|x| x == "pix" || x == "rpi")
        })
        .filter_map(|p| {
            let m = std::fs::metadata(&p).ok()?.modified().ok()?;
            Some((p, m))
        })
        .collect();
    out.sort();
    out
}

fn changed_paths(
    old: &[(PathBuf, std::time::SystemTime)],
    new: &[(PathBuf, std::time::SystemTime)],
) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for (p, t) in new {
        if old.iter().find(|(q, _)| q == p).map(|(_, u)| u) != Some(t) {
            out.push(p.clone());
        }
    }
    for (p, _) in old {
        if !new.iter().any(|(q, _)| q == p) {
            out.push(p.clone());
        }
    }
    out
}

/// Rebuild + relaunch on saves (rung 1) — except view-slice edits to
/// the entry, which the running app absorbs in-process (rung 2): the
/// child is left alone and hot-reloads itself. Poll-based (300 ms);
/// Ctrl-C to stop.
fn cmd_watch(file: &Path) -> ExitCode {
    let dir = file
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    println!(
        "pixie watch: view-slice edits hot-reload in-process; anything else rebuilds (Ctrl-C to stop)"
    );
    let mut child: Option<std::process::Child> = None;
    let mut last = scan_sources(&dir);
    loop {
        if let Some(mut c) = child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        // Which modules the entry imports. The classifier rereads them
        // (§8.72), so a change in one is a question the fingerprint
        // answers rather than an automatic rebuild.
        let imports = pixie_driver::check_file(file)
            .ok()
            .map(|o| o.foreign_paths)
            .unwrap_or_default();
        let baseline = reload_info_with(file, imports.clone()).map(|r| r.fingerprint);
        match build_file(file, None, false, false) {
            Some(bin) => match Command::new(&bin).spawn() {
                Ok(c) => child = Some(c),
                Err(e) => eprintln!("pixie: cannot launch {}: {e}", bin.display()),
            },
            None => eprintln!("pixie: build failed — waiting for changes"),
        }
        loop {
            std::thread::sleep(std::time::Duration::from_millis(300));
            let now = scan_sources(&dir);
            if now == last {
                continue;
            }
            let changed = changed_paths(&last, &now);
            last = now;
            // A change anywhere in the program's own sources — the
            // entry or one of its imports — asks the same question.
            // Anything else (a `.rpi`, a module nobody imports) is
            // outside the reload's reach.
            let ours: std::collections::HashSet<std::path::PathBuf> =
                std::iter::once(file.canonicalize().unwrap_or_else(|_| file.to_path_buf()))
                    .chain(imports.iter().map(|(_, p)| std::path::PathBuf::from(p)))
                    .collect();
            let reloadable = !changed.is_empty()
                && changed.iter().all(|c| {
                    ours.contains(&c.canonicalize().unwrap_or_else(|_| c.clone()))
                });
            if reloadable && child.is_some() {
                match reload_info_with(file, imports.clone()).map(|r| r.fingerprint) {
                    Some(fp) if Some(fp) == baseline => {
                        println!("pixie watch: view-slice edit — the app reloads in-process");
                        continue;
                    }
                    None => {
                        println!(
                            "pixie watch: does not parse — the app keeps its last good view"
                        );
                        continue;
                    }
                    Some(_) => {}
                }
            }
            println!("pixie watch: rebuilding");
            break;
        }
    }
}

/// Prebuild the runtime into the shared target dir so first app builds
/// don't pay the dependency compile.
fn cmd_install_runtime() -> ExitCode {
    let Some(base) = shared_target_dir() else {
        eprintln!("pixie: no HOME — cannot place the shared target dir");
        return ExitCode::FAILURE;
    };
    let Some(kernel) = kernel_dir() else {
        eprintln!("pixie: cannot locate pixie-kernel (dev tree expected)");
        return ExitCode::FAILURE;
    };
    let warm = base.parent().unwrap_or(&base).join("warmup");
    let code = "use pixie_engine_gpui as _;\nuse pixie_interp as _;\nfn main() { let _ = pixie_kernel::World::new(); }\n";
    if let Err(e) = write_crate(
        &warm,
        "pixie_warmup",
        &kernel,
        engine_dir().as_deref(),
        interp_dir().as_deref(),
        "",
        code,
    ) {
        eprintln!("pixie: cannot write warmup crate: {e}");
        return ExitCode::FAILURE;
    }
    match cargo_cmd("build", &warm.join("Cargo.toml")).status() {
        Ok(s) if s.success() => {
            println!("runtime ready under {}", base.display());
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("pixie: runtime build failed");
            ExitCode::FAILURE
        }
    }
}

/// The kernel crate, resolved for the dev tree: this file lives in
/// crates/pixie-cli, the kernel in crates/pixie-kernel.
fn kernel_dir() -> Option<PathBuf> {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let k = here.parent()?.join("pixie-kernel");
    k.canonicalize().ok()
}

fn engine_dir() -> Option<PathBuf> {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let k = here.parent()?.join("pixie-engine-gpui");
    k.canonicalize().ok()
}

fn interp_dir() -> Option<PathBuf> {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let k = here.parent()?.join("pixie-interp");
    k.canonicalize().ok()
}

/// The vendored, patched gpui_macos (DESIGN §5, P1). Generated crates
/// are their own workspaces, so each must carry the `[patch]` table
/// itself for the carried patches to take effect.
fn vendored_gpui_macos_dir() -> Option<PathBuf> {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let v = here.parent()?.parent()?.join("vendor").join("gpui_macos");
    v.canonicalize().ok()
}

fn write_crate(
    out: &Path,
    name: &str,
    kernel: &Path,
    engine: Option<&Path>,
    interp: Option<&Path>,
    extra_deps: &str,
    code: &str,
) -> std::io::Result<()> {
    std::fs::create_dir_all(out.join("src"))?;
    // Carry the dev tree's rustc pin into the generated crate:
    // rust-toolchain.toml is directory-scoped, so an out-of-tree
    // project would otherwise build gpui with whatever rustup
    // defaults to (measured: E0658 on `cold_path`).
    if let Some(k) = kernel_dir() {
        if let Some(repo) = k.parent().and_then(Path::parent) {
            let pin = repo.join("rust-toolchain.toml");
            if pin.is_file() {
                let _ = std::fs::copy(&pin, out.join("rust-toolchain.toml"));
            }
            // ...and the dev tree's resolution, once, as a seed. A
            // generated crate is its own workspace, so without a lock
            // it resolves the whole graph afresh at every new app and
            // picks up whatever the registry published this morning —
            // which is how tinyvec 1.13.0 (published 2026-09-03, does
            // not compile) started breaking new apps while every
            // already-resolved demo stayed green. Generated code is
            // the compiler's responsibility, and so is what it builds
            // against: a new app now starts from the versions this
            // tree is tested with. Seeded, never overwritten — cargo
            // keeps adjusting it from there, and an app that has
            // resolved (or that someone ran `cargo update` in) is
            // left alone.
            let lock = repo.join("Cargo.lock");
            if lock.is_file() && !out.join("Cargo.lock").exists() {
                let _ = std::fs::copy(&lock, out.join("Cargo.lock"));
            }
        }
    }
    let engine_dep = match engine {
        Some(e) => format!("pixie-engine-gpui = {{ path = \"{}\" }}\n", e.display()),
        None => String::new(),
    };
    let interp_dep = match interp {
        Some(i) => format!("pixie-interp = {{ path = \"{}\" }}\n", i.display()),
        None => String::new(),
    };
    let patch = match (engine, vendored_gpui_macos_dir()) {
        (Some(_), Some(v)) => format!(
            "\n# Vendored lower half: pixie's patched gpui_macos (P1).\n\
             [patch.\"https://github.com/zed-industries/zed\"]\n\
             gpui_macos = {{ path = \"{}\" }}\n",
            v.display()
        ),
        _ => String::new(),
    };
    let manifest = format!(
        "# Generated by pixie — do not edit.\n\
         [package]\n\
         name = \"{name}\"\n\
         version = \"0.1.0\"\n\
         edition = \"2024\"\n\n\
         [dependencies]\n\
         pixie-kernel = {{ path = \"{}\" }}\n\
         {engine_dep}{interp_dep}{extra_deps}\n\
         [profile.release]\n\
         overflow-checks = true\n\n\
         # The dev build is for running an app, not for debugging the\n\
         # generated Rust: full debuginfo made a 64 MB binary whose\n\
         # LINK was most of every build (measured: 110s, 19% CPU).\n\
         [profile.dev]\n\
         debug = 0\n\
         strip = \"debuginfo\"\n\n\
         [workspace]\n{patch}",
        kernel.display()
    );
    std::fs::write(out.join("Cargo.toml"), manifest)?;
    std::fs::write(out.join("src").join("main.rs"), code)?;
    Ok(())
}
