//! §12.2's CLI face — `pixie new / add / update / remove`.
//!
//! The machinery (manifest parsing, git fetch + pixie.lock pinning,
//! the version-keyed `.pixie/rpi/` cache) lives in `manifest.rs` and
//! runs implicitly on every build; these verbs are the ergonomic
//! wrappers so nobody has to hand-edit TOML or delete cache files to
//! manage dependencies.
//!
//! Manifest edits are TEXTUAL — a line inserted into / removed from
//! the right section — so user formatting and comments survive. Both
//! entry shapes are handled: inline (`kit = { git = "…" }`) and the
//! standard-table form (`[dependencies.kit]` … up to the next
//! header). `add` syncs immediately (fetch + lock + rpi derivation)
//! and rolls the manifest back if the sync fails, so a bad URL never
//! leaves the project unbuildable.

use std::path::{Path, PathBuf};

use crate::manifest::{self, Lock, Manifest, PkgSource};

/// The manifest governing `cwd`, or a pointer at `pixie new`.
fn manifest_at(cwd: &Path) -> Result<Manifest, String> {
    // find() walks up from the probe's parent; the probe file itself
    // never needs to exist.
    match manifest::find(&cwd.join("__probe.pix"))? {
        Some(m) => Ok(m),
        None => Err(
            "no pixie.toml here (run inside a project, or `pixie new <name>` to start one)"
                .to_string(),
        ),
    }
}

fn manifest_path(m: &Manifest) -> PathBuf {
    m.root.join("pixie.toml")
}

/// Re-run the implicit per-build dependency pipeline: rpi cache for
/// `[crates]`, fetch + lock for `[dependencies]`, dep-side rpi.
/// Mirrors the build path's `prepare_manifest` without the dep-lines
/// product.
fn sync(root: &Path) -> Result<(), String> {
    let m = manifest_at(root)?;
    m.ensure_rpi()?;
    for (name, dep_root) in m.ensure_deps()? {
        let dep_m = manifest::find(&dep_root.join("__probe.pix"))?
            .ok_or_else(|| format!("dependency `{name}` lost its pixie.toml"))?;
        if !dep_m.deps.is_empty() {
            return Err(format!(
                "dependency `{name}` has its own [dependencies] — transitive pixie deps are M2"
            ));
        }
        dep_m.ensure_rpi()?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// pixie new

const MAIN_TEMPLATE: &str = r#"store App {
  state count : Int = 0

  fn bump {
    count += 1
  }
}

view Main {
  Column {
    spacing: 8.0
    padding: 16.0
    Text { text: "Hello from NAME" }
    Text { text: "count: #{App.count}" }
    Button { text: "+1"; onClick: App.bump() }
  }
}
"#;

/// Scaffold `<parent>/<name>` — pixie.toml, src/main.pix, .gitignore.
pub fn cmd_new(parent: &Path, name: &str) -> Result<PathBuf, String> {
    let mut chars = name.chars();
    let head_ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c == '_');
    let tail_ok = chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !head_ok || !tail_ok {
        return Err(format!(
            "`{name}` is not a pixie package name — names become module qualifiers, \
             so use lowercase letters, digits, and underscores (e.g. `my_app`)"
        ));
    }
    let dir = parent.join(name);
    if dir.exists() {
        return Err(format!("`{}` already exists", dir.display()));
    }
    std::fs::create_dir_all(dir.join("src"))
        .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    std::fs::write(
        dir.join("pixie.toml"),
        format!(
            "[package]\n\
             name = \"{name}\"\n\
             version = \"0.1.0\"\n\
             \n\
             # [crates]                    # Rust crates (cargo + derived bindings)\n\
             # serde_json = \"1\"\n\
             \n\
             # [dependencies]              # pixie packages (path / git / registry)\n\
             # kit = {{ git = \"https://…\" }}\n"
        ),
    )
    .map_err(|e| format!("cannot write pixie.toml: {e}"))?;
    std::fs::write(
        dir.join("src").join("main.pix"),
        MAIN_TEMPLATE.replace("NAME", name),
    )
    .map_err(|e| format!("cannot write src/main.pix: {e}"))?;
    // `.pixie/*` is scratch except the rpi cache, which is meant to
    // be committed (collaborators must not need the doc nightly).
    std::fs::write(
        dir.join(".gitignore"),
        "target/\n.pixie/*\n!.pixie/rpi/\n",
    )
    .map_err(|e| format!("cannot write .gitignore: {e}"))?;
    Ok(dir)
}

// ---------------------------------------------------------------------------
// pixie add

#[derive(Default)]
struct AddSpec {
    name: String,
    /// `--crate`: target `[crates]` instead of `[dependencies]`.
    crate_table: bool,
    version: Option<String>,
    git: Option<String>,
    /// (`branch` | `tag` | `rev`, value)
    refspec: Option<(String, String)>,
    path: Option<String>,
    features: Vec<String>,
    bind: Option<String>,
}

fn parse_add_args(args: &[String]) -> Result<AddSpec, String> {
    let mut spec = AddSpec::default();
    let mut i = 0;
    let take = |args: &[String], i: usize, flag: &str| -> Result<String, String> {
        args.get(i + 1)
            .cloned()
            .ok_or_else(|| format!("{flag} needs a value"))
    };
    while i < args.len() {
        match args[i].as_str() {
            "--crate" => {
                spec.crate_table = true;
                i += 1;
            }
            "--git" => {
                spec.git = Some(take(args, i, "--git")?);
                i += 2;
            }
            "--path" => {
                spec.path = Some(take(args, i, "--path")?);
                i += 2;
            }
            f @ ("--branch" | "--tag" | "--rev") => {
                spec.refspec = Some((f[2..].to_string(), take(args, i, f)?));
                i += 2;
            }
            "--features" => {
                spec.features = take(args, i, "--features")?
                    .split(',')
                    .map(str::to_string)
                    .filter(|s| !s.is_empty())
                    .collect();
                i += 2;
            }
            "--bind" => {
                spec.bind = Some(take(args, i, "--bind")?);
                i += 2;
            }
            flag if flag.starts_with("--") => {
                return Err(format!("unknown flag `{flag}`"));
            }
            positional => {
                if spec.name.is_empty() {
                    spec.name = positional.to_string();
                } else if spec.version.is_none() {
                    spec.version = Some(positional.to_string());
                } else {
                    return Err(format!("unexpected argument `{positional}`"));
                }
                i += 1;
            }
        }
    }
    if spec.name.is_empty() {
        return Err("usage: pixie add <name> [VERSION] [--git URL | --path DIR] [--crate]".into());
    }
    if spec.git.is_some() && (spec.path.is_some() || spec.version.is_some()) {
        return Err("--git cannot be combined with --path or a version".into());
    }
    if spec.refspec.is_some() && spec.git.is_none() {
        return Err("--branch/--tag/--rev need --git".into());
    }
    if spec.crate_table {
        if spec.git.is_some() {
            return Err("[crates] entries come from cargo — use a version or --path, not --git".into());
        }
        if spec.version.is_none() && spec.path.is_none() {
            return Err(format!(
                "a Rust crate needs a version (or --path): pixie add {} --crate 1",
                spec.name
            ));
        }
    } else {
        if !spec.features.is_empty() || spec.bind.is_some() {
            return Err("--features/--bind apply to Rust crates — add --crate".into());
        }
        if spec.git.is_none() && spec.path.is_none() && spec.version.is_none() {
            return Err(format!(
                "pixie dependencies need a source: pixie add {} --git URL, --path DIR, \
                 or a registry version",
                spec.name
            ));
        }
    }
    Ok(spec)
}

/// Render the manifest line for the spec — string form when only a
/// version is present, inline table otherwise.
fn render_entry(spec: &AddSpec) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(g) = &spec.git {
        parts.push(format!("git = \"{g}\""));
        if let Some((kind, v)) = &spec.refspec {
            parts.push(format!("{kind} = \"{v}\""));
        }
    }
    if let Some(p) = &spec.path {
        parts.push(format!("path = \"{p}\""));
    }
    if parts.is_empty() && spec.features.is_empty() && spec.bind.is_none() {
        let v = spec.version.as_deref().expect("validated");
        return format!("{} = \"{v}\"", spec.name);
    }
    if let Some(v) = &spec.version {
        parts.insert(0, format!("version = \"{v}\""));
    }
    if !spec.features.is_empty() {
        let fs: Vec<String> = spec.features.iter().map(|f| format!("\"{f}\"")).collect();
        parts.push(format!("features = [{}]", fs.join(", ")));
    }
    if let Some(b) = &spec.bind {
        parts.push(format!("bind = \"{b}\""));
    }
    format!("{} = {{ {} }}", spec.name, parts.join(", "))
}

pub fn cmd_add(cwd: &Path, args: &[String]) -> Result<(), String> {
    let mut spec = parse_add_args(args)?;
    let m = manifest_at(cwd)?;
    // A relative --path is what the user typed at THEIR cwd; the
    // manifest resolves paths against its own root. Re-anchor (and
    // fall back to the absolute form when the target lives outside
    // the project).
    if let Some(rel) = &spec.path {
        let given = Path::new(rel);
        if given.is_relative() {
            let abs = cwd.join(given).canonicalize().map_err(|e| {
                format!("--path `{rel}` does not resolve from here: {e}")
            })?;
            let root = m.root.canonicalize().unwrap_or_else(|_| m.root.clone());
            spec.path = Some(match abs.strip_prefix(&root) {
                Ok(inside) => inside.display().to_string(),
                Err(_) => abs.display().to_string(),
            });
        }
    }
    let already = if spec.crate_table {
        m.crates.iter().any(|c| c.name == spec.name)
    } else {
        m.deps.iter().any(|d| d.name == spec.name)
    };
    let section = if spec.crate_table { "crates" } else { "dependencies" };
    if already {
        return Err(format!(
            "`{}` is already in [{section}] — `pixie update {}` re-resolves it, \
             `pixie remove {}` drops it",
            spec.name, spec.name, spec.name
        ));
    }
    let path = manifest_path(&m);
    let before = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let lock_before = std::fs::read_to_string(m.lock_path()).ok();
    let after = insert_into_section(&before, section, &render_entry(&spec));
    std::fs::write(&path, &after).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    eprintln!("pixie: added `{}` to [{section}]", spec.name);
    if let Err(e) = sync(&m.root) {
        // Roll the project back to its pre-add state — manifest text,
        // lock file, and the new dep's checkout — so a typo'd URL
        // never leaves it unbuildable.
        let _ = std::fs::write(&path, &before);
        match &lock_before {
            Some(t) => {
                let _ = std::fs::write(m.lock_path(), t);
            }
            None => {
                let _ = std::fs::remove_file(m.lock_path());
            }
        }
        let _ = std::fs::remove_dir_all(m.deps_dir().join(&spec.name));
        return Err(format!("{e}\npixie: `{}` rolled back", spec.name));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// pixie update

pub fn cmd_update(cwd: &Path, name: Option<&str>) -> Result<(), String> {
    let m = manifest_at(cwd)?;
    let targets: Vec<&manifest::PkgDep> = match name {
        Some(n) => {
            let Some(d) = m.deps.iter().find(|d| d.name == n) else {
                if m.crates.iter().any(|c| c.name == n) {
                    return Err(format!(
                        "`{n}` is a [crates] entry — bump its version in pixie.toml; \
                         the binding cache re-derives from the new key on the next build"
                    ));
                }
                return Err(format!("`{n}` is not in [dependencies]"));
            };
            vec![d]
        }
        None => m.deps.iter().collect(),
    };
    if targets.is_empty() {
        eprintln!("pixie: no [dependencies] to update");
        return Ok(());
    }
    let mut lock = Lock::read(&m.lock_path())?;
    let lock_before = std::fs::read_to_string(m.lock_path()).ok();
    let mut old_revs: Vec<(String, Option<String>)> = Vec::new();
    let mut unpinned = 0;
    for d in &targets {
        if matches!(d.source, PkgSource::Path(_)) {
            if name.is_some() {
                eprintln!("pixie: `{}` is a path dependency — nothing to update", d.name);
            }
            continue;
        }
        old_revs.push((d.name.clone(), lock.rev_of(&d.name).map(String::from)));
        lock.unpin(&d.name);
        let _ = std::fs::remove_dir_all(m.deps_dir().join(&d.name));
        unpinned += 1;
    }
    if unpinned == 0 {
        return Ok(());
    }
    if lock.is_empty() {
        // Lock::write no-ops on empty — the stale file must go, or
        // the old pins would win the re-resolve.
        let _ = std::fs::remove_file(m.lock_path());
    } else {
        lock.write(&m.lock_path())?;
    }
    if let Err(e) = sync(&m.root) {
        if let Some(t) = &lock_before {
            let _ = std::fs::write(m.lock_path(), t);
        }
        return Err(format!("{e}\npixie: pixie.lock restored"));
    }
    let fresh = Lock::read(&m.lock_path())?;
    for (n, old) in &old_revs {
        let new = fresh.rev_of(n);
        let short = |r: &str| r[..12.min(r.len())].to_string();
        match (old.as_deref(), new) {
            (Some(o), Some(nw)) if o == nw => {
                eprintln!("pixie: `{n}` unchanged ({})", short(o));
            }
            (Some(o), Some(nw)) => {
                eprintln!("pixie: `{n}` {} → {}", short(o), short(nw));
            }
            (None, Some(nw)) => eprintln!("pixie: `{n}` pinned at {}", short(nw)),
            (_, None) => {}
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// pixie remove

pub fn cmd_remove(cwd: &Path, name: &str) -> Result<(), String> {
    let m = manifest_at(cwd)?;
    let in_deps = m.deps.iter().any(|d| d.name == name);
    let in_crates = m.crates.iter().any(|c| c.name == name);
    if !in_deps && !in_crates {
        let mut have: Vec<&str> = m
            .deps
            .iter()
            .map(|d| d.name.as_str())
            .chain(m.crates.iter().map(|c| c.name.as_str()))
            .collect();
        have.sort_unstable();
        return Err(if have.is_empty() {
            format!("`{name}` is not a dependency (the manifest has none)")
        } else {
            format!("`{name}` is not a dependency (have: {})", have.join(", "))
        });
    }
    let path = manifest_path(&m);
    let mut text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    if in_deps {
        text = remove_from_section(&text, "dependencies", name)
            .ok_or_else(|| format!("cannot find `{name}` in [dependencies] textually — edit pixie.toml by hand"))?;
        let mut lock = Lock::read(&m.lock_path())?;
        lock.unpin(name);
        if lock.is_empty() {
            let _ = std::fs::remove_file(m.lock_path());
        } else {
            lock.write(&m.lock_path())?;
        }
        let _ = std::fs::remove_dir_all(m.deps_dir().join(name));
        eprintln!("pixie: removed `{name}` from [dependencies] (lock + checkout cleaned)");
    }
    if in_crates {
        text = remove_from_section(&text, "crates", name)
            .ok_or_else(|| format!("cannot find `{name}` in [crates] textually — edit pixie.toml by hand"))?;
        let _ = std::fs::remove_file(m.rpi_dir().join(format!("{name}.rpi")));
        let _ = std::fs::remove_dir_all(m.root.join(".pixie").join("rpi-scratch").join(name));
        eprintln!("pixie: removed `{name}` from [crates] (binding cache cleaned)");
    }
    std::fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

// ---------------------------------------------------------------------------
// Textual TOML surgery. Line-based: sections run from their `[header]`
// line to the next line whose first non-blank char is `[`.

fn is_header(line: &str, section: &str) -> bool {
    let t = line.trim();
    t == format!("[{section}]")
}

/// A `[section.name]` / `[section."name"]` sub-table header.
fn is_subtable_header(line: &str, section: &str, name: &str) -> bool {
    let t = line.trim();
    t == format!("[{section}.{name}]") || t == format!("[{section}.\"{name}\"]")
}

fn is_entry(line: &str, name: &str) -> bool {
    let t = line.trim_start();
    for prefix in [
        format!("{name}="),
        format!("{name} ="),
        format!("\"{name}\" ="),
        format!("\"{name}\"="),
    ] {
        if t.starts_with(&prefix) {
            return true;
        }
    }
    false
}

/// Insert `entry` at the end of `[section]` (before its trailing blank
/// lines), creating the section at EOF when absent.
fn insert_into_section(text: &str, section: &str, entry: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let Some(h) = lines.iter().position(|l| is_header(l, section)) else {
        let mut out = text.to_string();
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        if !out.is_empty() && !out.ends_with("\n\n") {
            out.push('\n');
        }
        out.push_str(&format!("[{section}]\n{entry}\n"));
        return out;
    };
    let mut end = lines.len();
    for (i, l) in lines.iter().enumerate().skip(h + 1) {
        if l.trim_start().starts_with('[') {
            end = i;
            break;
        }
    }
    // Back over the section's trailing blank lines.
    let mut at = end;
    while at > h + 1 && lines[at - 1].trim().is_empty() {
        at -= 1;
    }
    let mut out: Vec<String> = lines[..at].iter().map(|s| s.to_string()).collect();
    out.push(entry.to_string());
    out.extend(lines[at..].iter().map(|s| s.to_string()));
    out.join("\n") + "\n"
}

/// Remove `name`'s entry from `[section]` — its inline line, or its
/// whole `[section.name]` sub-table. `None` when nothing matched.
fn remove_from_section(text: &str, section: &str, name: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    // Sub-table form first: drop header through the next header.
    if let Some(h) = lines
        .iter()
        .position(|l| is_subtable_header(l, section, name))
    {
        let mut end = lines.len();
        for (i, l) in lines.iter().enumerate().skip(h + 1) {
            if l.trim_start().starts_with('[') {
                end = i;
                break;
            }
        }
        let mut out: Vec<&str> = lines[..h].to_vec();
        out.extend(&lines[end..]);
        return Some(out.join("\n") + "\n");
    }
    let h = lines.iter().position(|l| is_header(l, section))?;
    let mut end = lines.len();
    for (i, l) in lines.iter().enumerate().skip(h + 1) {
        if l.trim_start().starts_with('[') {
            end = i;
            break;
        }
    }
    let hit = (h + 1..end).find(|&i| is_entry(lines[i], name))?;
    let mut out: Vec<&str> = lines.to_vec();
    out.remove(hit);
    Some(out.join("\n") + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn sh(dir: &Path, cmd: &str, args: &[&str]) {
        let out = Command::new(cmd)
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{cmd} {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// A committed local git repo holding a minimal pixie package.
    fn kit_repo(dir: &Path) -> String {
        let repo = dir.join("kitsrc");
        std::fs::create_dir_all(repo.join("src")).unwrap();
        std::fs::write(
            repo.join("pixie.toml"),
            "[package]\nname = \"kit\"\nversion = \"1.0.0\"\n",
        )
        .unwrap();
        std::fs::write(
            repo.join("src/lib.pix"),
            "pub fn ping String {\n  \"pong\"\n}\n",
        )
        .unwrap();
        sh(&repo, "git", &["init", "-q"]);
        sh(&repo, "git", &["add", "pixie.toml", "src/lib.pix"]);
        sh(
            &repo,
            "git",
            &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-qm", "v1"],
        );
        repo.display().to_string()
    }

    fn fresh(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The scaffold parses, checks clean, and is fmt-stable.
    #[test]
    fn new_scaffolds_a_checkable_project() {
        let dir = fresh("pixie-cmd-new");
        let proj = cmd_new(&dir, "hello_app").expect("scaffolds");
        assert!(proj.join("pixie.toml").is_file());
        assert!(proj.join(".gitignore").is_file());
        let main = proj.join("src/main.pix");
        let outcome = pixie_driver::check_file(&main).expect("driver runs");
        assert_eq!(
            outcome.error_count(),
            0,
            "template must check clean: {:?}",
            outcome.diagnostics
        );
        let src = std::fs::read_to_string(&main).unwrap();
        let formatted = pixie_syntax::format_source(pixie_syntax::FileId(0), &src).unwrap();
        assert_eq!(formatted, src, "template must be fmt-stable");

        // Bad names are refused with the module-qualifier reason.
        let err = cmd_new(&dir, "My-App").unwrap_err();
        assert!(err.contains("module qualifiers"), "{err}");
        // Existing directory is refused.
        assert!(cmd_new(&dir, "hello_app").is_err());
    }

    /// add → manifest line + lock pin + checkout; remove → all gone.
    #[test]
    fn add_then_remove_round_trip() {
        let dir = fresh("pixie-cmd-addrm");
        let repo = kit_repo(&dir);
        let app = cmd_new(&dir, "app").unwrap();

        cmd_add(&app, &["kit".into(), "--git".into(), repo.clone()]).expect("add");
        let toml = std::fs::read_to_string(app.join("pixie.toml")).unwrap();
        assert!(toml.contains(&format!("kit = {{ git = \"{repo}\" }}")), "{toml}");
        assert!(app.join("pixie.lock").is_file(), "lock written");
        assert!(app.join(".pixie/deps/kit/src/lib.pix").is_file(), "checkout");

        // Duplicate add is refused before touching anything.
        let err = cmd_add(&app, &["kit".into(), "--git".into(), repo.clone()]).unwrap_err();
        assert!(err.contains("already"), "{err}");

        cmd_remove(&app, "kit").expect("remove");
        let toml = std::fs::read_to_string(app.join("pixie.toml")).unwrap();
        assert!(
            !toml.contains(&format!("kit = {{ git = \"{repo}\" }}")),
            "{toml}"
        );
        assert!(!app.join("pixie.lock").exists(), "lock deleted when empty");
        assert!(!app.join(".pixie/deps/kit").exists(), "checkout cleaned");

        // Removing what isn't there names the actual inventory.
        let err = cmd_remove(&app, "kit").unwrap_err();
        assert!(err.contains("not a dependency"), "{err}");
    }

    /// A failed add (bogus URL) rolls the manifest and lock back.
    #[test]
    fn failed_add_rolls_back() {
        let dir = fresh("pixie-cmd-rollback");
        let repo = kit_repo(&dir);
        let app = cmd_new(&dir, "app").unwrap();
        cmd_add(&app, &["kit".into(), "--git".into(), repo]).expect("good add");
        let toml_before = std::fs::read_to_string(app.join("pixie.toml")).unwrap();
        let lock_before = std::fs::read_to_string(app.join("pixie.lock")).unwrap();

        let bogus = dir.join("no-such-repo").display().to_string();
        let err = cmd_add(&app, &["broken".into(), "--git".into(), bogus]).unwrap_err();
        assert!(err.contains("rolled back"), "{err}");
        assert_eq!(
            std::fs::read_to_string(app.join("pixie.toml")).unwrap(),
            toml_before,
            "manifest restored"
        );
        assert_eq!(
            std::fs::read_to_string(app.join("pixie.lock")).unwrap(),
            lock_before,
            "lock restored"
        );
        assert!(!app.join(".pixie/deps/broken").exists());
    }

    /// update unpins, re-resolves, and reports old → new.
    #[test]
    fn update_moves_the_pin_to_the_new_head() {
        let dir = fresh("pixie-cmd-update");
        let repo = kit_repo(&dir);
        let app = cmd_new(&dir, "app").unwrap();
        cmd_add(&app, &["kit".into(), "--git".into(), repo.clone()]).expect("add");
        let lock1 = std::fs::read_to_string(app.join("pixie.lock")).unwrap();

        // The upstream moves.
        let repo_dir = dir.join("kitsrc");
        std::fs::write(
            repo_dir.join("src/lib.pix"),
            "pub fn ping String {\n  \"pong2\"\n}\n",
        )
        .unwrap();
        sh(&repo_dir, "git", &["add", "src/lib.pix"]);
        sh(
            &repo_dir,
            "git",
            &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-qm", "v2"],
        );

        // A build-path sync alone must NOT move the pin (lock wins)…
        sync(&app).expect("locked sync");
        assert_eq!(
            std::fs::read_to_string(app.join("pixie.lock")).unwrap(),
            lock1,
            "lock must hold without an explicit update"
        );

        // …the update verb does.
        cmd_update(&app, Some("kit")).expect("update");
        let lock2 = std::fs::read_to_string(app.join("pixie.lock")).unwrap();
        assert_ne!(lock1, lock2, "pin must move to the new head");
        let fetched =
            std::fs::read_to_string(app.join(".pixie/deps/kit/src/lib.pix")).unwrap();
        assert!(fetched.contains("pong2"), "checkout at the new rev");

        // Unknown names are named errors.
        assert!(cmd_update(&app, Some("nope")).is_err());
    }

    /// Textual surgery handles both entry shapes and preserves the
    /// rest of the file byte for byte.
    #[test]
    fn toml_surgery_shapes() {
        let text = "[package]\nname = \"x\"\n\n[dependencies]\na = \"1\"\n\n[crates]\nserde = \"1\"\n";
        let added = insert_into_section(text, "dependencies", "b = { git = \"g\" }");
        assert!(added.contains("a = \"1\"\nb = { git = \"g\" }\n"), "{added}");
        let removed = remove_from_section(&added, "dependencies", "b").unwrap();
        assert_eq!(removed, text, "add then remove is identity");

        // Creating a missing section appends it.
        let no_sec = "[package]\nname = \"x\"\n";
        let made = insert_into_section(no_sec, "dependencies", "a = \"1\"");
        assert!(made.ends_with("[dependencies]\na = \"1\"\n"), "{made}");

        // Sub-table form: the whole block goes.
        let sub = "[dependencies.kit]\ngit = \"g\"\n\n[crates]\nserde = \"1\"\n";
        let removed = remove_from_section(sub, "dependencies", "kit").unwrap();
        assert!(!removed.contains("kit"), "{removed}");
        assert!(removed.contains("[crates]"), "{removed}");

        // A [crates] remove leaves [dependencies] alone.
        let both = "[dependencies]\nserde = \"9\"\n\n[crates]\nserde = \"1\"\n";
        let removed = remove_from_section(both, "crates", "serde").unwrap();
        assert!(removed.contains("serde = \"9\""), "{removed}");
        assert!(!removed.contains("serde = \"1\""), "{removed}");
    }

    /// The add grammar's guard rails.
    #[test]
    fn add_flag_validation() {
        let s = |args: &[&str]| parse_add_args(&args.iter().map(|a| a.to_string()).collect::<Vec<_>>());
        assert!(s(&["kit", "--git", "u", "--path", "p"]).is_err());
        assert!(s(&["kit", "--branch", "b"]).is_err(), "refspec needs --git");
        assert!(s(&["kit"]).is_err(), "a pixie dep needs a source");
        assert!(s(&["serde", "--crate"]).is_err(), "a crate needs a version");
        assert!(s(&["serde", "--features", "x"]).is_err(), "--features needs --crate");
        let ok = s(&["serde", "1", "--crate", "--features", "derive,rc"]).unwrap();
        assert_eq!(render_entry(&ok), "serde = { version = \"1\", features = [\"derive\", \"rc\"] }");
        let ok = s(&["kit", "--git", "u", "--tag", "v1"]).unwrap();
        assert_eq!(render_entry(&ok), "kit = { git = \"u\", tag = \"v1\" }");
        let ok = s(&["kit", "1.2"]).unwrap();
        assert_eq!(render_entry(&ok), "kit = \"1.2\"");
    }
}
