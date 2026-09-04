//! `pixie.toml` — §12.2's first slice: the `[crates]` table.
//!
//! A Rust crate named here lands in the generated crate's Cargo.toml
//! verbatim (cargo owns version resolution and the lockfile), and its
//! `.rpi` binding surface is derived once by rpi-gen — from rustdoc
//! JSON produced on the internally pinned nightly — into
//! `<root>/.pixie/rpi/<name>.rpi`, a cache that is meant to be
//! COMMITTED (so collaborators and the tier gate never need the
//! nightly). Regeneration happens only when the cache file is
//! missing; delete it to refresh after a version bump.
//!
//! Schema:
//!
//!     [package]
//!     name = "myapp"          # informational in this slice
//!     version = "0.1.0"
//!
//!     [crates]
//!     serde_json = "1"
//!     mathkit = { path = "vendor/mathkit", bind = "mathkit=MathKit" }
//!     rand = { version = "0.9", features = ["small_rng"] }
//!
//! `bind` defaults to `<crate_underscored>=<PascalCase(crate)>`.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The nightly that produced the committed `std.json` fixtures —
/// rustdoc JSON format 61, matching rpi-gen's pinned rustdoc-types.
const DOC_NIGHTLY: &str = "nightly-2026-08-22";

#[derive(Debug)]
pub struct CrateDep {
    pub name: String,
    pub version: Option<String>,
    /// Absolute (canonicalized against the manifest root).
    pub path: Option<PathBuf>,
    pub features: Vec<String>,
    /// `module=Class` handed to rpi-gen.
    pub bind: String,
}

/// `[window]` — the app's window request, baked into the emitted
/// `run_app` call: `title` (otherwise the exe stem), `width` +
/// `height` in logical pixels (applied as a pair; otherwise the
/// engine default). Headless script runs never open a window, so
/// these never touch a dump.
#[derive(Debug, Default, Clone)]
pub struct WindowSpec {
    pub title: Option<String>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    /// The inset between the window and the app's tree; absent keeps
    /// the engine's own.
    pub padding: Option<f64>,
}

#[derive(Debug)]
pub struct Manifest {
    /// The directory holding pixie.toml.
    pub root: PathBuf,
    pub crates: Vec<CrateDep>,
    /// `[window]` — see `WindowSpec`.
    pub window: WindowSpec,
    /// `[dependencies]` — pixie packages (path / git / registry).
    pub deps: Vec<PkgDep>,
    /// `[registry] index = "…"` — dir or https base holding
    /// `<name>.toml` version indexes.
    pub registry_index: Option<String>,
}

fn pascal_case(name: &str) -> String {
    name.split(['-', '_'])
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut c = s.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// Walk up from the entry file looking for `pixie.toml` (at most five
/// levels — projects are shallow; an unrelated manifest far above
/// should not capture a scratch build).
pub fn find(entry: &Path) -> Result<Option<Manifest>, String> {
    let mut dir = entry.parent().map(Path::to_path_buf);
    for _ in 0..5 {
        let Some(d) = dir else { break };
        let candidate = d.join("pixie.toml");
        if candidate.is_file() {
            return parse(&candidate).map(Some);
        }
        dir = d.parent().map(Path::to_path_buf);
    }
    Ok(None)
}

fn parse(path: &Path) -> Result<Manifest, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let doc: toml::Value = text
        .parse()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let root = path.parent().expect("manifest has a parent").to_path_buf();
    let mut crates = Vec::new();
    if let Some(table) = doc.get("crates").and_then(|c| c.as_table()) {
        for (name, spec) in table {
            let dep = match spec {
                toml::Value::String(v) => CrateDep {
                    name: name.clone(),
                    version: Some(v.clone()),
                    path: None,
                    features: Vec::new(),
                    bind: default_bind(name),
                },
                toml::Value::Table(t) => {
                    let version = t.get("version").and_then(|v| v.as_str()).map(String::from);
                    let path = match t.get("path").and_then(|v| v.as_str()) {
                        Some(rel) => Some(root.join(rel).canonicalize().map_err(|e| {
                            format!("[crates] {name}: path `{rel}` does not resolve: {e}")
                        })?),
                        None => None,
                    };
                    if version.is_none() && path.is_none() {
                        return Err(format!(
                            "[crates] {name}: needs `version` or `path`"
                        ));
                    }
                    let features = t
                        .get("features")
                        .and_then(|v| v.as_array())
                        .map(|a| {
                            a.iter()
                                .filter_map(|x| x.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default();
                    let bind = t
                        .get("bind")
                        .and_then(|v| v.as_str())
                        .map(String::from)
                        .unwrap_or_else(|| default_bind(name));
                    CrateDep {
                        name: name.clone(),
                        version,
                        path,
                        features,
                        bind,
                    }
                }
                _ => return Err(format!("[crates] {name}: expected a version string or a table")),
            };
            crates.push(dep);
        }
    }
    // Deterministic dependency order (toml tables preserve source
    // order only sometimes; sort by name so the generated manifest
    // and rpi loading never reorder between builds).
    crates.sort_by(|a, b| a.name.cmp(&b.name));

    let mut deps: Vec<PkgDep> = Vec::new();
    if let Some(table) = doc.get("dependencies").and_then(|c| c.as_table()) {
        for (name, spec) in table {
            if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                || !name.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            {
                return Err(format!(
                    "[dependencies] {name}: package names must be pixie identifiers (they become module qualifiers)"
                ));
            }
            let source = match spec {
                toml::Value::String(req) => PkgSource::Version(req.clone()),
                toml::Value::Table(t) => {
                    if let Some(rel) = t.get("path").and_then(|v| v.as_str()) {
                        let p = root.join(rel).canonicalize().map_err(|e| {
                            format!("[dependencies] {name}: path `{rel}` does not resolve: {e}")
                        })?;
                        PkgSource::Path(p)
                    } else if let Some(url) = t.get("git").and_then(|v| v.as_str()) {
                        let refspec = ["tag", "branch", "rev"]
                            .iter()
                            .find_map(|k| t.get(*k).and_then(|v| v.as_str()))
                            .map(String::from);
                        PkgSource::Git {
                            url: url.to_string(),
                            refspec,
                        }
                    } else if let Some(req) = t.get("version").and_then(|v| v.as_str()) {
                        PkgSource::Version(req.to_string())
                    } else {
                        return Err(format!(
                            "[dependencies] {name}: needs `path`, `git`, or `version`"
                        ));
                    }
                }
                _ => {
                    return Err(format!(
                        "[dependencies] {name}: expected a version string or a table"
                    ))
                }
            };
            deps.push(PkgDep {
                name: name.clone(),
                source,
            });
        }
    }
    deps.sort_by(|a, b| a.name.cmp(&b.name));
    let registry_index = doc
        .get("registry")
        .and_then(|r| r.get("index"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let mut window = WindowSpec::default();
    if let Some(t) = doc.get("window").and_then(|w| w.as_table()) {
        window.title = t.get("title").and_then(|v| v.as_str()).map(String::from);
        let num = |key: &str| -> Result<Option<f64>, String> {
            match t.get(key) {
                None => Ok(None),
                Some(v) => v
                    .as_float()
                    .or_else(|| v.as_integer().map(|n| n as f64))
                    .map(Some)
                    .ok_or_else(|| format!("[window] {key}: expected a number")),
            }
        };
        window.width = num("width")?;
        window.height = num("height")?;
        window.padding = num("padding")?;
        if window.width.is_some() != window.height.is_some() {
            return Err("[window] width and height come as a pair".to_string());
        }
    }

    Ok(Manifest {
        root,
        crates,
        window,
        deps,
        registry_index,
    })
}

fn default_bind(name: &str) -> String {
    format!("{}={}", name.replace('-', "_"), pascal_case(name))
}

impl Manifest {
    pub fn rpi_dir(&self) -> PathBuf {
        self.root.join(".pixie").join("rpi")
    }

    /// The `[dependencies]` lines the generated crate needs.
    pub fn dep_lines(&self) -> String {
        let mut out = String::new();
        for c in &self.crates {
            let mut parts: Vec<String> = Vec::new();
            if let Some(v) = &c.version {
                parts.push(format!("version = \"{v}\""));
            }
            if let Some(p) = &c.path {
                parts.push(format!("path = \"{}\"", p.display()));
            }
            if !c.features.is_empty() {
                let fs: Vec<String> = c.features.iter().map(|f| format!("\"{f}\"")).collect();
                parts.push(format!("features = [{}]", fs.join(", ")));
            }
            out.push_str(&format!("{} = {{ {} }}\n", c.name, parts.join(", ")));
        }
        out
    }

    /// The invalidation key a cache entry was derived under: the
    /// version requirement (or, for path deps, the crate's OWN
    /// version from its Cargo.toml — bump it to refresh), the sorted
    /// feature set, and the rustdoc-JSON format the pinned nightly
    /// emits. Stored as the `.rpi`'s first line; a mismatch
    /// regenerates in place.
    fn cache_key(&self, c: &CrateDep) -> String {
        let version = match (&c.version, &c.path) {
            (Some(v), _) => v.clone(),
            (None, Some(p)) => crate_version_of(p).unwrap_or_else(|| "path".into()),
            (None, None) => "?".into(),
        };
        let mut feats = c.features.clone();
        feats.sort();
        format!(
            "# pixie-cache-key: version={version}; features={}; format=61",
            feats.join(",")
        )
    }

    /// Make sure every `[crates]` entry has a CURRENT `.rpi` in the
    /// cache — missing or stale-keyed entries re-derive through the
    /// real rpi-gen pipeline. Returns the number generated.
    pub fn ensure_rpi(&self) -> Result<usize, String> {
        let mut generated = 0;
        for c in &self.crates {
            let cache = self.rpi_dir().join(format!("{}.rpi", c.name));
            let key = self.cache_key(c);
            if let Ok(existing) = std::fs::read_to_string(&cache) {
                if existing.lines().next() == Some(key.as_str()) {
                    continue;
                }
                eprintln!(
                    "pixie: bindings for `{}` are stale (key changed) — re-deriving…",
                    c.name
                );
            } else {
                eprintln!("pixie: deriving bindings for `{}` (first use)…", c.name);
            }
            let rpi = derive_rpi(self, c)?;
            std::fs::create_dir_all(self.rpi_dir())
                .map_err(|e| format!("cannot create {}: {e}", self.rpi_dir().display()))?;
            std::fs::write(&cache, format!("{key}\n{rpi}"))
                .map_err(|e| format!("cannot write {}: {e}", cache.display()))?;
            eprintln!("pixie: wrote {}", cache.display());
            generated += 1;
        }
        Ok(generated)
    }
}

/// `package.version` of the crate at `path` (path-dep cache keying).
fn crate_version_of(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path.join("Cargo.toml")).ok()?;
    let doc: toml::Value = text.parse().ok()?;
    doc.get("package")?
        .get("version")?
        .as_str()
        .map(String::from)
}

/// One crate's `.rpi`, the §7 pipeline end to end: scratch crate →
/// pinned-nightly rustdoc JSON → rpi-gen.
fn derive_rpi(m: &Manifest, c: &CrateDep) -> Result<String, String> {
    let scratch = m.root.join(".pixie").join("rpi-scratch").join(&c.name);
    std::fs::create_dir_all(scratch.join("src"))
        .map_err(|e| format!("cannot create scratch: {e}"))?;
    let mut spec: Vec<String> = Vec::new();
    if let Some(v) = &c.version {
        spec.push(format!("version = \"{v}\""));
    }
    if let Some(p) = &c.path {
        spec.push(format!("path = \"{}\"", p.display()));
    }
    if !c.features.is_empty() {
        let fs: Vec<String> = c.features.iter().map(|f| format!("\"{f}\"")).collect();
        spec.push(format!("features = [{}]", fs.join(", ")));
    }
    std::fs::write(
        scratch.join("Cargo.toml"),
        format!(
            "# Generated by pixie — rustdoc-JSON scratch for `{}`.\n\
             [package]\n\
             name = \"rpi-scratch\"\n\
             version = \"0.0.0\"\n\
             edition = \"2024\"\n\n\
             [dependencies]\n\
             {} = {{ {} }}\n\n\
             [workspace]\n",
            c.name,
            c.name,
            spec.join(", ")
        ),
    )
    .map_err(|e| format!("cannot write scratch manifest: {e}"))?;
    std::fs::write(scratch.join("src").join("lib.rs"), "")
        .map_err(|e| format!("cannot write scratch lib: {e}"))?;

    let out = Command::new("cargo")
        .arg(format!("+{DOC_NIGHTLY}"))
        .args(["doc", "-q", "--no-deps", "-p", &c.name])
        .env(
            "RUSTDOCFLAGS",
            "-Z unstable-options --output-format json",
        )
        .current_dir(&scratch)
        .output()
        .map_err(|e| format!("cargo doc failed to launch: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "rustdoc JSON for `{}` failed (is the `{DOC_NIGHTLY}` toolchain installed?):\n{}",
            c.name,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let json_path = scratch
        .join("target")
        .join("doc")
        .join(format!("{}.json", c.name.replace('-', "_")));
    let json = std::fs::read_to_string(&json_path)
        .map_err(|e| format!("cannot read {}: {e}", json_path.display()))?;
    let krate = pixie_rpi_gen::parse_crate(&json)?;
    let (module, class) = c
        .bind
        .split_once('=')
        .ok_or_else(|| format!("[crates] {}: bind must be `module=Class`", c.name))?;
    let (text, reports) = pixie_rpi_gen::generate(
        &krate,
        &[pixie_rpi_gen::BindSpec {
            module: module.to_string(),
            class: class.to_string(),
        }],
    )?;
    for r in &reports {
        for (name, why) in &r.skipped {
            eprintln!("pixie:   skipped {name} — {why}");
        }
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_both_dep_forms_and_defaults_bind() {
        let dir = std::env::temp_dir().join("pixie-manifest-parse");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("vendor/kit")).unwrap();
        std::fs::write(
            dir.join("pixie.toml"),
            "[package]\nname = \"x\"\nversion = \"0.1.0\"\n\n[crates]\nserde_json = \"1\"\nmy-kit = { path = \"vendor/kit\", bind = \"my_kit=Kit\", features = [\"extra\"] }\n",
        )
        .unwrap();
        std::fs::write(dir.join("src-entry.pix"), "").unwrap();
        let m = find(&dir.join("src-entry.pix")).expect("parses").expect("found");
        assert_eq!(m.crates.len(), 2);
        // Sorted by name: my-kit, serde_json.
        assert_eq!(m.crates[0].name, "my-kit");
        assert_eq!(m.crates[0].bind, "my_kit=Kit");
        assert_eq!(m.crates[0].features, ["extra"]);
        assert!(m.crates[0].path.as_ref().unwrap().is_absolute());
        assert_eq!(m.crates[1].name, "serde_json");
        assert_eq!(m.crates[1].version.as_deref(), Some("1"));
        assert_eq!(m.crates[1].bind, "serde_json=SerdeJson");
        let deps = m.dep_lines();
        assert!(deps.contains("serde_json = { version = \"1\" }"), "{deps}");
        assert!(deps.contains("my-kit = { path = "), "{deps}");
        assert!(deps.contains("features = [\"extra\"]"), "{deps}");
    }

    #[test]
    fn missing_version_and_path_is_an_error() {
        let dir = std::env::temp_dir().join("pixie-manifest-bad");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("pixie.toml"), "[crates]\nx = { bind = \"x=X\" }\n").unwrap();
        std::fs::write(dir.join("e.pix"), "").unwrap();
        let err = find(&dir.join("e.pix")).unwrap_err();
        assert!(err.contains("needs `version` or `path`"), "{err}");
    }
}

// ---------------------------------------------------------------------------
// §12.2 — pixie package dependencies + pixie.lock + registry index.
//
// A pixie package is a source-level artifact: `src/**.pix`, `.rpi`
// files, a pixie.toml. Dependencies come from a path, a git URL, or
// a version resolved through a registry INDEX (a directory or static
// site with one TOML per package — the mechanism without the
// governance; publishing tooling comes with a server story later).
// Git checkouts land in `<root>/.pixie/deps/<name>/` and the
// resolved revision is pinned in `pixie.lock`; a checkout matching
// the lock skips the network entirely.

#[derive(Debug, Clone)]
pub enum PkgSource {
    Path(PathBuf),
    Git {
        url: String,
        /// tag / branch / rev requested (None = default branch head).
        refspec: Option<String>,
    },
    /// Version requirement, resolved through `[registry] index`.
    Version(String),
}

#[derive(Debug)]
pub struct PkgDep {
    pub name: String,
    pub source: PkgSource,
}

impl Manifest {
    pub fn deps_dir(&self) -> PathBuf {
        self.root.join(".pixie").join("deps")
    }

    pub fn lock_path(&self) -> PathBuf {
        self.root.join("pixie.lock")
    }

    fn dep_checkout_dir(&self, name: &str) -> PathBuf {
        self.deps_dir().join(name)
    }

    /// Fetch every git/registry dependency to the revision pinned in
    /// pixie.lock (resolving + writing the pin on first fetch). Path
    /// deps just get validated. Returns dep name → package root dir.
    pub fn ensure_deps(&self) -> Result<Vec<(String, PathBuf)>, String> {
        let mut lock = Lock::read(&self.lock_path())?;
        let mut out = Vec::new();
        for d in &self.deps {
            let root = match &d.source {
                PkgSource::Path(p) => {
                    if !p.join("pixie.toml").is_file() {
                        return Err(format!(
                            "[dependencies] {}: `{}` has no pixie.toml",
                            d.name,
                            p.display()
                        ));
                    }
                    p.clone()
                }
                PkgSource::Git { url, refspec } => {
                    self.fetch_git(&d.name, url.as_str(), refspec.as_deref(), &mut lock)?
                }
                PkgSource::Version(req) => {
                    let (url, rev) = self.resolve_via_index(&d.name, req, &lock)?;
                    self.fetch_git(&d.name, &url, Some(&rev), &mut lock)?
                }
            };
            out.push((d.name.clone(), root));
        }
        lock.write(&self.lock_path())?;
        Ok(out)
    }

    /// One git dependency at its pinned (or newly resolved) revision.
    fn fetch_git(
        &self,
        name: &str,
        url: &str,
        refspec: Option<&str>,
        lock: &mut Lock,
    ) -> Result<PathBuf, String> {
        let dir = self.dep_checkout_dir(name);
        let marker = dir.join(".pixie-rev");
        if let Some(pinned) = lock.rev_of(name) {
            if std::fs::read_to_string(&marker).is_ok_and(|m| m.trim() == pinned) {
                return Ok(dir);
            }
        }
        eprintln!("pixie: fetching `{name}` from {url}…");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(self.deps_dir())
            .map_err(|e| format!("cannot create deps dir: {e}"))?;
        run_git(&["clone", "--quiet", url, &dir.display().to_string()], None)?;
        // The lock wins over the manifest refspec (repeatable builds);
        // first fetch resolves the refspec and pins it.
        let want = lock.rev_of(name).map(String::from).or_else(|| refspec.map(String::from));
        if let Some(r) = &want {
            run_git(&["checkout", "--quiet", r], Some(&dir))?;
        }
        let rev = run_git(&["rev-parse", "HEAD"], Some(&dir))?.trim().to_string();
        if !dir.join("pixie.toml").is_file() {
            return Err(format!("[dependencies] {name}: {url} has no pixie.toml"));
        }
        std::fs::write(&marker, &rev).map_err(|e| format!("cannot write rev marker: {e}"))?;
        lock.pin(name, url, &rev);
        eprintln!("pixie: `{name}` pinned at {}", &rev[..12.min(rev.len())]);
        Ok(dir)
    }

    /// Resolve `name@req` through the registry index: the index holds
    /// `<name>.toml` with a `[versions]` table mapping exact versions
    /// to `{ git, rev }`. Highest version matching the requirement
    /// wins (prefix match: "1" ⊇ "1.4.2", "1.4" ⊇ "1.4.x").
    fn resolve_via_index(
        &self,
        name: &str,
        req: &str,
        lock: &Lock,
    ) -> Result<(String, String), String> {
        // The lock short-circuits resolution entirely.
        if let (Some(url), Some(rev)) = (lock.url_of(name), lock.rev_of(name)) {
            return Ok((url.to_string(), rev.to_string()));
        }
        let Some(index) = &self.registry_index else {
            return Err(format!(
                "[dependencies] {name} = \"{req}\" needs a `[registry] index` (or use git/path)"
            ));
        };
        let text = if index.starts_with("http://") || index.starts_with("https://") {
            let out = Command::new("curl")
                .args(["-fsSL", &format!("{index}/{name}.toml")])
                .output()
                .map_err(|e| format!("curl failed to launch: {e}"))?;
            if !out.status.success() {
                return Err(format!(
                    "registry has no entry for `{name}` at {index}: {}",
                    String::from_utf8_lossy(&out.stderr)
                ));
            }
            String::from_utf8_lossy(&out.stdout).into_owned()
        } else {
            let p = self.root.join(index).join(format!("{name}.toml"));
            std::fs::read_to_string(&p)
                .map_err(|e| format!("registry has no entry for `{name}` ({}): {e}", p.display()))?
        };
        let doc: toml::Value = text
            .parse()
            .map_err(|e| format!("registry entry for `{name}` is not TOML: {e}"))?;
        let versions = doc
            .get("versions")
            .and_then(|v| v.as_table())
            .ok_or_else(|| format!("registry entry for `{name}` has no [versions]"))?;
        let matches_req = |v: &str| -> bool {
            v == req
                || (v.starts_with(req)
                    && v.as_bytes().get(req.len()) == Some(&b'.'))
        };
        let mut best: Option<(Vec<u64>, String, &toml::Value)> = None;
        for (v, spec) in versions {
            if !matches_req(v) {
                continue;
            }
            let key: Vec<u64> = v.split('.').map(|s| s.parse().unwrap_or(0)).collect();
            if best.as_ref().is_none_or(|(bk, _, _)| key > *bk) {
                best = Some((key, v.clone(), spec));
            }
        }
        let Some((_, version, spec)) = best else {
            let have: Vec<&str> = versions.keys().map(String::as_str).collect();
            return Err(format!(
                "no version of `{name}` matches \"{req}\" (index has: {})",
                have.join(", ")
            ));
        };
        let url = spec
            .get("git")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("registry entry `{name}` {version} has no `git`"))?;
        let rev = spec
            .get("rev")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("registry entry `{name}` {version} has no `rev`"))?;
        eprintln!("pixie: registry resolved `{name}` \"{req}\" → {version}");
        Ok((url.to_string(), rev.to_string()))
    }
}

fn run_git(args: &[&str], cwd: Option<&Path>) -> Result<String, String> {
    let mut c = Command::new("git");
    c.args(args);
    if let Some(d) = cwd {
        c.current_dir(d);
    }
    let out = c.output().map_err(|e| format!("git failed to launch: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {} failed:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// pixie.lock — `[[package]] name / git / rev` entries, sorted by
/// name. The lock is authoritative: a locked dep never touches the
/// network again until its entry (or checkout) is deleted.
#[derive(Debug, Default)]
pub struct Lock {
    entries: Vec<(String, String, String)>,
}

impl Lock {
    pub(crate) fn read(path: &Path) -> Result<Lock, String> {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Ok(Lock::default());
        };
        let doc: toml::Value = text
            .parse()
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let mut entries = Vec::new();
        if let Some(arr) = doc.get("package").and_then(|p| p.as_array()) {
            for e in arr {
                let g = |k: &str| e.get(k).and_then(|v| v.as_str()).map(String::from);
                if let (Some(n), Some(u), Some(r)) = (g("name"), g("git"), g("rev")) {
                    entries.push((n, u, r));
                }
            }
        }
        Ok(Lock { entries })
    }

    pub(crate) fn write(&self, path: &Path) -> Result<(), String> {
        if self.entries.is_empty() {
            return Ok(());
        }
        let mut sorted = self.entries.clone();
        sorted.sort();
        let mut out = String::from("# Generated by pixie — do not edit.\n");
        for (n, u, r) in &sorted {
            out.push_str(&format!(
                "\n[[package]]\nname = \"{n}\"\ngit = \"{u}\"\nrev = \"{r}\"\n"
            ));
        }
        std::fs::write(path, out).map_err(|e| format!("cannot write {}: {e}", path.display()))
    }

    pub(crate) fn rev_of(&self, name: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(n, _, _)| n == name)
            .map(|(_, _, r)| r.as_str())
    }

    fn url_of(&self, name: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(n, _, _)| n == name)
            .map(|(_, u, _)| u.as_str())
    }

    /// Drop one pin (the `pixie update` / `pixie remove` verbs).
    pub(crate) fn unpin(&mut self, name: &str) {
        self.entries.retain(|(n, _, _)| n != name);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn pin(&mut self, name: &str, url: &str, rev: &str) {
        if let Some(e) = self.entries.iter_mut().find(|(n, _, _)| n == name) {
            e.1 = url.to_string();
            e.2 = rev.to_string();
        } else {
            self.entries
                .push((name.to_string(), url.to_string(), rev.to_string()));
        }
    }
}

#[cfg(test)]
mod pkg_tests {
    use super::*;

    fn sh(dir: &Path, cmd: &str, args: &[&str]) {
        let out = Command::new(cmd).args(args).current_dir(dir).output().unwrap();
        assert!(out.status.success(), "{cmd} {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }

    /// A local git package + a file registry index: version resolves
    /// through the index, the checkout pins into pixie.lock, and a
    /// second ensure with the network path renamed away proves the
    /// lock short-circuits everything.
    #[test]
    fn git_registry_and_lock_round_trip() {
        let dir = std::env::temp_dir().join("pixie-pkg-registry");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("pkgsrc/src")).unwrap();
        std::fs::write(
            dir.join("pkgsrc/pixie.toml"),
            "[package]\nname = \"kit\"\nversion = \"1.2.0\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("pkgsrc/src/lib.pix"),
            "pub fn ping String {\n  \"pong\"\n}\n",
        )
        .unwrap();
        let repo = dir.join("pkgsrc");
        sh(&repo, "git", &["init", "-q"]);
        sh(&repo, "git", &["add", "pixie.toml", "src/lib.pix"]);
        sh(&repo, "git", &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-qm", "v1"]);
        let rev = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&repo)
            .output()
            .unwrap();
        let rev = String::from_utf8_lossy(&rev.stdout).trim().to_string();

        // File-based registry index: index/kit.toml.
        std::fs::create_dir_all(dir.join("index")).unwrap();
        std::fs::write(
            dir.join("index/kit.toml"),
            format!(
                "[versions]\n\"1.2.0\" = {{ git = \"{}\", rev = \"{rev}\" }}\n\"0.9.0\" = {{ git = \"{}\", rev = \"{rev}\" }}\n",
                repo.display(),
                repo.display()
            ),
        )
        .unwrap();

        // The consuming project asks for "1".
        std::fs::create_dir_all(dir.join("app/src")).unwrap();
        std::fs::write(
            dir.join("app/pixie.toml"),
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[registry]\nindex = \"../index\"\n\n[dependencies]\nkit = \"1\"\n",
        )
        .unwrap();
        std::fs::write(dir.join("app/src/main.pix"), "").unwrap();
        let m = find(&dir.join("app/src/main.pix")).unwrap().unwrap();
        let roots = m.ensure_deps().unwrap();
        assert_eq!(roots.len(), 1);
        assert!(roots[0].1.join("src/lib.pix").is_file());

        // The lock pinned the resolved rev.
        let lock = std::fs::read_to_string(m.lock_path()).unwrap();
        assert!(lock.contains(&rev), "lock: {lock}");
        assert!(lock.contains("name = \"kit\""), "lock: {lock}");

        // Locked: rename the index AND the source repo away; ensure
        // still succeeds from the existing checkout.
        std::fs::rename(dir.join("index"), dir.join("index-gone")).unwrap();
        std::fs::rename(&repo, dir.join("pkgsrc-gone")).unwrap();
        let m2 = find(&dir.join("app/src/main.pix")).unwrap().unwrap();
        let roots2 = m2.ensure_deps().unwrap();
        assert_eq!(roots2.len(), 1, "locked dep must resolve offline");
    }

    /// Version requirements pick the highest matching entry.
    #[test]
    fn registry_prefix_matching_picks_highest() {
        let dir = std::env::temp_dir().join("pixie-pkg-semver");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("index")).unwrap();
        std::fs::write(
            dir.join("index/kit.toml"),
            "[versions]\n\"1.2.0\" = { git = \"g\", rev = \"r120\" }\n\"1.10.0\" = { git = \"g\", rev = \"r1100\" }\n\"2.0.0\" = { git = \"g\", rev = \"r200\" }\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.join("app")).unwrap();
        std::fs::write(
            dir.join("app/pixie.toml"),
            "[registry]\nindex = \"../index\"\n\n[dependencies]\nkit = \"1\"\n",
        )
        .unwrap();
        std::fs::write(dir.join("app/e.pix"), "").unwrap();
        let m = find(&dir.join("app/e.pix")).unwrap().unwrap();
        let lock = Lock::default();
        let (_, rev) = m.resolve_via_index("kit", "1", &lock).unwrap();
        assert_eq!(rev, "r1100", "numeric compare, not lexicographic");
        let (_, rev) = m.resolve_via_index("kit", "1.2", &lock).unwrap();
        assert_eq!(rev, "r120");
        assert!(m.resolve_via_index("kit", "3", &lock).is_err());
    }
}
