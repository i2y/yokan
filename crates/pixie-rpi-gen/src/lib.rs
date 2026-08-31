//! rustdoc JSON → `.rpi` (DESIGN §7). The generator is deliberately
//! narrow: it binds exactly the surface the call-site adapters accept
//! today — free functions over {Int, Float, Bool, String} plus
//! fallible returns — and *skips and reports* everything else rather
//! than mis-bind. Generic params are accepted only when every bound is
//! on the string-shaped whitelist (`AsRef<Path|str|OsStr|[u8]>`,
//! `Into<String>`), which is what makes the std::fs family bindable
//! from `&str` call sites.

use std::collections::HashMap;
use std::fmt::Write as _;

use rustdoc_types::{
    Crate, Function, GenericArg, GenericArgs, GenericBound, GenericParamDefKind, Generics, Id,
    Item, ItemEnum, Type, Visibility, WherePredicate,
};

/// One requested binding: a Rust module path mapped to a pixie class.
pub struct BindSpec {
    pub module: String,
    pub class: String,
}

#[derive(Debug)]
pub struct Report {
    pub class: String,
    pub bound: Vec<String>,
    pub skipped: Vec<(String, String)>,
}

#[derive(Clone, PartialEq)]
enum PixTy {
    Int,
    Float,
    Bool,
    Str,
    Unit,
    List(Box<PixTy>),
    /// `Option<T>` in a return position → `T?` (§11.11). Inputs with
    /// `Option` params stay skip-and-report.
    Opt(Box<PixTy>),
    /// `Vec<u8>` returns / `&[u8]` params → the kernel's COW byte
    /// string (§11.10).
    Bytes,
    /// The kernel's own `Map<K, V>` (§12.3 headers and friends) —
    /// passed straight through; K/V limited to the wire-able set
    /// (Str-ish, Int, Float, Bool).
    Map(Box<PixTy>, Box<PixTy>),
    /// A type this run also DECLARES — a C-like enum (§8.76) or a
    /// plain struct (§8.77) — with its Rust counterpart written
    /// alongside. The name is the pixie one; the correspondence lives
    /// in the emitted declaration.
    Declared(String),
}

impl PixTy {
    fn render(&self) -> String {
        match self {
            PixTy::Int => "Int".into(),
            PixTy::Float => "Float".into(),
            PixTy::Bool => "Bool".into(),
            PixTy::Str => "String".into(),
            PixTy::Unit => "Void".into(),
            PixTy::List(t) => format!("List<{}>", t.render()),
            PixTy::Opt(t) => format!("{}?", t.render()),
            PixTy::Bytes => "Bytes".into(),
            PixTy::Map(k, v) => format!("Map<{}, {}>", k.render(), v.render()),
            PixTy::Declared(n) => n.clone(),
        }
    }
}

fn snake_to_camel(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut up = false;
    for c in s.chars() {
        if c == '_' {
            up = true;
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// The canonical path of an id, via the crate's paths table.
fn path_of(krate: &Crate, id: &Id) -> Option<Vec<String>> {
    krate.paths.get(id).map(|s| s.path.clone())
}

fn last_seg_of_path(krate: &Crate, p: &rustdoc_types::Path) -> String {
    if let Some(full) = path_of(krate, &p.id) {
        if let Some(last) = full.last() {
            return last.clone();
        }
    }
    p.path.rsplit("::").next().unwrap_or(&p.path).to_string()
}

/// Is this bound on the string-shaped whitelist?
fn bound_is_stringish(krate: &Crate, b: &GenericBound) -> bool {
    let GenericBound::TraitBound {
        trait_,
        generic_params,
        ..
    } = b
    else {
        return false;
    };
    if !generic_params.is_empty() {
        return false;
    }
    let name = last_seg_of_path(krate, trait_);
    let arg = trait_.args.as_deref().and_then(|a| match a {
        GenericArgs::AngleBracketed { args, .. } => args.first(),
        _ => None,
    });
    let Some(GenericArg::Type(t)) = arg else {
        return false;
    };
    match (name.as_str(), t) {
        ("AsRef", Type::Primitive(p)) if p == "str" => true,
        ("AsRef", Type::Slice(inner)) => matches!(&**inner, Type::Primitive(p) if p == "u8"),
        ("AsRef", Type::ResolvedPath(rp)) => {
            let last = last_seg_of_path(krate, rp);
            last == "Path" || last == "OsStr" || last == "PathBuf" || last == "String"
        }
        ("Into", Type::ResolvedPath(rp)) => last_seg_of_path(krate, rp) == "String",
        _ => false,
    }
}

/// Every generic param must be a type param whose bounds are all
/// string-shaped; those params map to `String`. Lifetimes / consts /
/// other bounds reject the fn.
fn whitelisted_generics(
    krate: &Crate,
    generics: &Generics,
) -> Result<std::collections::HashSet<String>, String> {
    let mut ok = std::collections::HashSet::new();
    let mut bounds_of: HashMap<String, Vec<&GenericBound>> = HashMap::new();
    for p in &generics.params {
        match &p.kind {
            GenericParamDefKind::Type { bounds, .. } => {
                bounds_of.entry(p.name.clone()).or_default().extend(bounds);
            }
            GenericParamDefKind::Lifetime { .. } => {
                return Err("explicit lifetimes".into());
            }
            GenericParamDefKind::Const { .. } => {
                return Err("const generics".into());
            }
        }
    }
    for wp in &generics.where_predicates {
        match wp {
            WherePredicate::BoundPredicate { type_, bounds, .. } => {
                let Type::Generic(name) = type_ else {
                    return Err("non-generic where clause".into());
                };
                bounds_of.entry(name.clone()).or_default().extend(bounds);
            }
            _ => return Err("unsupported where clause".into()),
        }
    }
    // Sorted, because the skip REASON names one generic and the
    // generated `.rpi` is a committed file: iterating the map in hash
    // order made a regeneration churn the reason line for a function
    // with two bad parameters.
    let mut ordered: Vec<(String, Vec<&GenericBound>)> = bounds_of.into_iter().collect();
    ordered.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, bounds) in ordered {
        if bounds.is_empty() {
            return Err(format!("unbounded generic `{name}`"));
        }
        if bounds.iter().all(|b| bound_is_stringish(krate, b)) {
            ok.insert(name);
        } else {
            return Err(format!("generic `{name}` has non-string bounds"));
        }
    }
    Ok(ok)
}

fn render_type_short(t: &Type) -> String {
    match t {
        Type::Primitive(p) => p.clone(),
        Type::ResolvedPath(p) => p.path.clone(),
        Type::Generic(g) => g.clone(),
        Type::BorrowedRef {
            is_mutable, type_, ..
        } => format!("&{}{}", if *is_mutable { "mut " } else { "" }, render_type_short(type_)),
        Type::Slice(inner) => format!("[{}]", render_type_short(inner)),
        Type::Tuple(ts) if ts.is_empty() => "()".into(),
        _ => "<complex>".into(),
    }
}


/// `Map<K, V>` with both args in the wire-able set (Str-ish / i64 /
/// f64 / bool) — shared by input and output mapping.
fn map_kv(krate: &Crate, rp: &rustdoc_types::Path) -> Result<PixTy, String> {
    let args: Vec<&Type> = rp
        .args
        .as_deref()
        .and_then(|a| match a {
            GenericArgs::AngleBracketed { args, .. } => Some(
                args.iter()
                    .filter_map(|g| match g {
                        GenericArg::Type(t) => Some(t),
                        _ => None,
                    })
                    .collect(),
            ),
            _ => None,
        })
        .unwrap_or_default();
    let wire = |t: &&Type| -> Option<PixTy> {
        match t {
            Type::ResolvedPath(p) => {
                match last_seg_of_path(krate, p).as_str() {
                    "Str" | "String" => Some(PixTy::Str),
                    _ => None,
                }
            }
            Type::Primitive(p) => match p.as_str() {
                "i64" => Some(PixTy::Int),
                "f64" => Some(PixTy::Float),
                "bool" => Some(PixTy::Bool),
                _ => None,
            },
            _ => None,
        }
    };
    if args.len() == 2 {
        if let (Some(k), Some(v)) = (wire(&args[0]), wire(&args[1])) {
            return Ok(PixTy::Map(Box::new(k), Box::new(v)));
        }
    }
    Err("unsupported Map args (keys/values: String, Int, Float, Bool)".into())
}

fn map_input(
    krate: &Crate,
    t: &Type,
    stringish: &std::collections::HashSet<String>,
    declared: &HashMap<String, String>,
    std_map: Option<&mut bool>,
) -> Result<PixTy, String> {
    match t {
        Type::Primitive(p) => match p.as_str() {
            "i64" => Ok(PixTy::Int),
            "f64" => Ok(PixTy::Float),
            "bool" => Ok(PixTy::Bool),
            other => Err(format!("unsupported type `{other}`")),
        },
        Type::BorrowedRef {
            is_mutable: false,
            type_,
            ..
        } => match &**type_ {
            Type::Primitive(p) if p == "str" => Ok(PixTy::Str),
            Type::ResolvedPath(rp) if last_seg_of_path(krate, rp) == "String" => Ok(PixTy::Str),
            // `&[u8]` → Bytes (§11.10); the adapter passes
            // `.as_slice()`. Other slices stay unsupported.
            Type::Slice(inner) if matches!(&**inner, Type::Primitive(p) if p == "u8") => {
                Ok(PixTy::Bytes)
            }
            other => Err(format!("unsupported type `&{}`", render_type_short(other))),
        },
        Type::BorrowedRef {
            is_mutable: true, ..
        } => Err("`&mut` parameter".into()),
        Type::ResolvedPath(rp) => {
            let last = last_seg_of_path(krate, rp);
            if last == "String" {
                return Ok(PixTy::Str);
            }
            // The kernel's own `Map<K, V>` (battery params) — any
            // wire-able key/value pair.
            if last == "Map" {
                return map_kv(krate, rp);
            }
            // std `HashMap<K, V>` (yokan's crate boundary) — a user crate's map. The
            // call-site adapter converts the kernel Map at the
            // boundary, and the conversion is switched by a marker
            // on the FN (the `stdmap:` path prefix) — so the type
            // crosses whole only: a nested position has no marker.
            if last == "HashMap" {
                return match std_map {
                    Some(flag) => {
                        *flag = true;
                        map_kv(krate, rp)
                    }
                    None => Err("`HashMap` nested inside another type".into()),
                };
            }
            if last == "BTreeMap" {
                return Err(
                    "`BTreeMap` — only `HashMap` crosses; convert at the boundary".into(),
                );
            }
            // `Vec<T>` and `Option<T>` PARAMETERS (§8.76). The
            // call-site adapter has taken both since §8.73; this
            // mapper had not caught up, so a battery of pixie's own
            // came back "unsupported type `Vec`".
            if last == "Vec" || last == "Option" {
                let inner_ty = rp.args.as_deref().and_then(|a| match a {
                    GenericArgs::AngleBracketed { args, .. } => args.iter().find_map(|g| match g {
                        GenericArg::Type(t) => Some(t),
                        _ => None,
                    }),
                    _ => None,
                });
                let Some(inner_ty) = inner_ty else {
                    return Err(format!("unsupported type `{last}<..>`"));
                };
                if last == "Vec" && matches!(inner_ty, Type::Primitive(p) if p == "u8") {
                    return Ok(PixTy::Bytes);
                }
                let inner = map_input(krate, inner_ty, stringish, declared, None)?;
                // The same element set the adapter converts, which is
                // the same one the RETURN side allows — declared types
                // included (§8.77).
                if !matches!(
                    inner,
                    PixTy::Int
                        | PixTy::Float
                        | PixTy::Bool
                        | PixTy::Str
                        | PixTy::Bytes
                        | PixTy::Declared(_)
                ) {
                    return Err(format!("unsupported `{last}` element"));
                }
                return Ok(if last == "Vec" {
                    PixTy::List(Box::new(inner))
                } else {
                    PixTy::Opt(Box::new(inner))
                });
            }
            // A type this run declares (§8.76, §8.77). It crosses in
            // this direction too, by the same correspondence.
            if let Some(pix) = declared.get(&last) {
                return Ok(PixTy::Declared(pix.clone()));
            }
            Err(format!("unsupported type `{}`", rp.path))
        }
        Type::Generic(g) => {
            if stringish.contains(g) {
                Ok(PixTy::Str)
            } else {
                Err(format!("generic `{g}` has non-string bounds"))
            }
        }
        other => Err(format!("unsupported type `{}`", render_type_short(other))),
    }
}

fn map_plain_output(
    krate: &Crate,
    t: &Type,
    declared: &HashMap<String, String>,
    std_map: Option<&mut bool>,
) -> Result<PixTy, String> {
    match t {
        Type::Primitive(p) => match p.as_str() {
            "i64" => Ok(PixTy::Int),
            // The adapter widens every native integer via `as i64`.
            "u64" | "u32" | "usize" | "i32" | "u16" | "i16" | "u8" | "i8" | "isize" => {
                Ok(PixTy::Int)
            }
            "f64" | "f32" => Ok(PixTy::Float),
            "bool" => Ok(PixTy::Bool),
            other => Err(format!("unsupported return `{other}`")),
        },
        Type::ResolvedPath(rp) => {
            let last = last_seg_of_path(krate, rp);
            // Paths come back lossily as String (the adapter's
            // From<PathBuf> for Str).
            if last == "String" || last == "PathBuf" {
                return Ok(PixTy::Str);
            }
            if last == "Vec" {
                let elem_ty = rp.args.as_deref().and_then(|a| match a {
                    GenericArgs::AngleBracketed { args, .. } => {
                        args.iter().find_map(|g| match g {
                            GenericArg::Type(t) => Some(t),
                            _ => None,
                        })
                    }
                    _ => None,
                });
                let Some(elem_ty) = elem_ty else {
                    return Err("unsupported return `Vec<..>`".into());
                };
                // `Vec<u8>` is bytes, not a list of numbers (§11.10):
                // the COW `Bytes` value, before the generic List map.
                if matches!(elem_ty, Type::Primitive(p) if p == "u8") {
                    return Ok(PixTy::Bytes);
                }
                let elem = map_plain_output(krate, elem_ty, declared, None)?;
                // A declared type is an element like any other
                // (§8.77): the call-site adapter converts a list
                // element by the same rule it converts a whole value.
                if !matches!(
                    elem,
                    PixTy::Int
                        | PixTy::Float
                        | PixTy::Bool
                        | PixTy::Str
                        | PixTy::Bytes
                        | PixTy::Declared(_)
                ) {
                    return Err("unsupported List element".into());
                }
                return Ok(PixTy::List(Box::new(elem)));
            }
            if last == "Map" {
                return map_kv(krate, rp);
            }
            // std `HashMap` return (yokan's crate boundary) — same rule as the
            // parameter side: whole only, marked on the fn.
            if last == "HashMap" {
                return match std_map {
                    Some(flag) => {
                        *flag = true;
                        map_kv(krate, rp)
                    }
                    None => Err("`HashMap` nested inside another type".into()),
                };
            }
            if last == "BTreeMap" {
                return Err(
                    "`BTreeMap` — only `HashMap` crosses; convert at the boundary".into(),
                );
            }
            if last == "Option" {
                let inner_ty = rp.args.as_deref().and_then(|a| match a {
                    GenericArgs::AngleBracketed { args, .. } => {
                        args.iter().find_map(|g| match g {
                            GenericArg::Type(t) => Some(t),
                            _ => None,
                        })
                    }
                    _ => None,
                });
                let Some(inner_ty) = inner_ty else {
                    return Err("unsupported return `Option<..>`".into());
                };
                let inner = map_plain_output(krate, inner_ty, declared, None)?;
                // The call-site adapter maps the element conversion
                // through the Option, so the inner set matches
                // `List<T>`'s exactly (no nested Option / List yet).
                if !matches!(
                    inner,
                    PixTy::Int
                        | PixTy::Float
                        | PixTy::Bool
                        | PixTy::Str
                        | PixTy::Bytes
                        | PixTy::Declared(_)
                ) {
                    return Err("unsupported Option inner".into());
                }
                return Ok(PixTy::Opt(Box::new(inner)));
            }
            // A type this run declares — a C-like enum (§8.76) or a
            // struct (§8.77). The correspondence rides the emitted
            // declaration.
            if let Some(pix) = declared.get(&last) {
                return Ok(PixTy::Declared(pix.clone()));
            }
            Err(format!("unsupported return `{}`", rp.path))
        }
        Type::Tuple(ts) if ts.is_empty() => Ok(PixTy::Unit),
        other => Err(format!("unsupported return `{}`", render_type_short(other))),
    }
}

/// (pixie type, fallible?) for the return position. `Result`-shaped
/// paths (std::io::Result included) become `!T`.
fn map_output(
    krate: &Crate,
    t: Option<&Type>,
    declared: &HashMap<String, String>,
    std_map: Option<&mut bool>,
) -> Result<(PixTy, bool), String> {
    let Some(t) = t else {
        return Ok((PixTy::Unit, false));
    };
    if let Type::ResolvedPath(rp) = t {
        if last_seg_of_path(krate, rp) == "Result" {
            let args: Vec<&Type> = rp
                .args
                .as_deref()
                .and_then(|a| match a {
                    GenericArgs::AngleBracketed { args, .. } => Some(
                        args.iter()
                            .filter_map(|g| match g {
                                GenericArg::Type(t) => Some(t),
                                _ => None,
                            })
                            .collect(),
                    ),
                    _ => None,
                })
                .unwrap_or_default();
            let ok_ty = match args.first() {
                Some(t) => map_plain_output(krate, t, declared, std_map)?,
                None => PixTy::Unit,
            };
            return Ok((ok_ty, true));
        }
    }
    Ok((map_plain_output(krate, t, declared, std_map)?, false))
}

/// Walk the module tree collecting `module path -> fn item ids`.
fn modules_of(krate: &Crate) -> HashMap<String, Vec<Id>> {
    let mut out: HashMap<String, Vec<Id>> = HashMap::new();
    fn walk(krate: &Crate, id: &Id, prefix: &str, out: &mut HashMap<String, Vec<Id>>) {
        let Some(item) = krate.index.get(id) else {
            return;
        };
        let ItemEnum::Module(m) = &item.inner else {
            return;
        };
        let name = item.name.as_deref().unwrap_or("");
        let path = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}::{name}")
        };
        for child in &m.items {
            if let Some(ci) = krate.index.get(child) {
                match &ci.inner {
                    ItemEnum::Function(_) => out.entry(path.clone()).or_default().push(*child),
                    ItemEnum::Module(_) => walk(krate, child, &path, out),
                    _ => {}
                }
            }
        }
    }
    walk(krate, &krate.root, "", &mut out);
    out
}

/// The public C-LIKE enums a module declares: every variant nullary,
/// every name a pixie identifier (§8.76). A payload-bearing enum is
/// skipped — the correspondence §8.74 generates is a match over unit
/// variants, and a payload would need its fields related too.
fn enums_of(krate: &Crate, module_path: &str) -> Vec<(String, Vec<String>, String)> {
    let mut out = Vec::new();
    fn walk(
        krate: &Crate,
        id: &Id,
        prefix: &str,
        want: &str,
        out: &mut Vec<(String, Vec<String>, String)>,
    ) {
        let Some(item) = krate.index.get(id) else {
            return;
        };
        let ItemEnum::Module(m) = &item.inner else {
            return;
        };
        let name = item.name.as_deref().unwrap_or("");
        let path = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}::{name}")
        };
        for child in &m.items {
            let Some(ci) = krate.index.get(child) else {
                continue;
            };
            match &ci.inner {
                ItemEnum::Module(_) => walk(krate, child, &path, want, out),
                ItemEnum::Enum(e) if path == want => {
                    let Some(ename) = ci.name.clone() else { continue };
                    if !matches!(ci.visibility, Visibility::Public) {
                        continue;
                    }
                    if !e.generics.params.is_empty() {
                        continue;
                    }
                    let mut variants = Vec::new();
                    let mut ok = true;
                    for vid in &e.variants {
                        let Some(vi) = krate.index.get(vid) else {
                            ok = false;
                            break;
                        };
                        let ItemEnum::Variant(v) = &vi.inner else {
                            ok = false;
                            break;
                        };
                        if !matches!(v.kind, rustdoc_types::VariantKind::Plain) {
                            ok = false;
                            break;
                        }
                        match vi.name.clone() {
                            Some(n) if is_pixie_ident(&n) => variants.push(n),
                            _ => {
                                ok = false;
                                break;
                            }
                        }
                    }
                    if ok && !variants.is_empty() && is_pixie_ident(&ename) {
                        out.push((ename.clone(), variants, format!("{path}::{ename}")));
                    }
                }
                _ => {}
            }
        }
    }
    walk(krate, &krate.root, "", module_path, &mut out);
    out
}

/// One field of a struct this run declares: the pixie name, the Rust
/// name when the two differ, and the Rust type still to be mapped.
struct RpiField {
    pix: String,
    rust: Option<String>,
    ty: Type,
}

/// A struct this run may declare (§8.77) — the candidate stage, before
/// its fields are known to cross.
struct RpiStruct {
    name: String,
    rust_path: String,
    fields: Vec<RpiField>,
}

/// The public structs a module declares with every field public and
/// named. Generic ones are skipped (the correspondence is per type,
/// and pixie's side would have to be generic too), and so is a tuple
/// struct — a pixie field needs a name to map.
fn structs_of(krate: &Crate, module_path: &str) -> Vec<RpiStruct> {
    let mut out = Vec::new();
    fn walk(krate: &Crate, id: &Id, prefix: &str, want: &str, out: &mut Vec<RpiStruct>) {
        let Some(item) = krate.index.get(id) else {
            return;
        };
        let ItemEnum::Module(m) = &item.inner else {
            return;
        };
        let name = item.name.as_deref().unwrap_or("");
        let path = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}::{name}")
        };
        for child in &m.items {
            let Some(ci) = krate.index.get(child) else {
                continue;
            };
            match &ci.inner {
                ItemEnum::Module(_) => walk(krate, child, &path, want, out),
                ItemEnum::Struct(st) if path == want => {
                    let Some(sname) = ci.name.clone() else { continue };
                    if !matches!(ci.visibility, Visibility::Public) {
                        continue;
                    }
                    if !st.generics.params.is_empty() || !is_pixie_ident(&sname) {
                        continue;
                    }
                    // A TUPLE struct's fields are positional
                    // (§8.78): Rust reaches them as `.0`, and so does
                    // the correspondence — a `.rpi` field named `0`
                    // reads `v.0` and writes `T { 0: .. }`, both
                    // valid Rust. The pixie side needs a name, so a
                    // newtype's single field is `value` and a wider
                    // one numbers its fields.
                    let (ids, tuple) = match &st.kind {
                        rustdoc_types::StructKind::Plain {
                            fields,
                            has_stripped_fields,
                        } => {
                            // A non-public field is stripped from the
                            // JSON, and a struct pixie cannot fill
                            // completely is one pixie cannot build.
                            if *has_stripped_fields {
                                continue;
                            }
                            (fields.iter().cloned().map(Some).collect::<Vec<_>>(), false)
                        }
                        rustdoc_types::StructKind::Tuple(fields) => (fields.clone(), true),
                        rustdoc_types::StructKind::Unit => continue,
                    };
                    let arity = ids.len();
                    let mut mapped = Vec::new();
                    let mut ok = true;
                    for (pos, fid) in ids.iter().enumerate() {
                        // A tuple struct's private field arrives as
                        // `None`, because position matters.
                        let Some(fid) = fid else {
                            ok = false;
                            break;
                        };
                        let Some(fi) = krate.index.get(fid) else {
                            ok = false;
                            break;
                        };
                        let ItemEnum::StructField(fty) = &fi.inner else {
                            ok = false;
                            break;
                        };
                        if !matches!(fi.visibility, Visibility::Public) {
                            ok = false;
                            break;
                        }
                        let (pix, rust) = if tuple {
                            let pix = if arity == 1 {
                                "value".to_string()
                            } else {
                                format!("field{pos}")
                            };
                            (pix, Some(pos.to_string()))
                        } else {
                            let Some(fname) = fi.name.clone() else {
                                ok = false;
                                break;
                            };
                            let pix = snake_to_camel(&fname);
                            let rust = (pix != fname).then(|| fname.clone());
                            (pix, rust)
                        };
                        if !is_pixie_ident(&pix) {
                            ok = false;
                            break;
                        }
                        mapped.push(RpiField {
                            pix,
                            rust,
                            ty: fty.clone(),
                        });
                    }
                    if ok && !mapped.is_empty() {
                        out.push(RpiStruct {
                            name: sname.clone(),
                            rust_path: format!("{path}::{sname}"),
                            fields: mapped,
                        });
                    }
                }
                _ => {}
            }
        }
    }
    walk(krate, &krate.root, "", module_path, &mut out);
    out
}

/// What a field's declaration must say about its Rust TYPE (§8.78).
/// `Ok(None)` — the default writing rule already produces that type.
/// `Ok(Some(t))` — the declaration has to name `t`, because reading
/// widens on its own (`as i64` absorbs any width, `Str::from`
/// absorbs a `PathBuf`) while writing back has to hit it exactly.
/// `Err(())` — pixie cannot write that type at all.
fn field_rust_ty(
    krate: &Crate,
    t: &Type,
    declared: &HashMap<String, String>,
) -> Result<Option<String>, ()> {
    match t {
        Type::Primitive(p) => match p.as_str() {
            "i64" | "f64" | "bool" => Ok(None),
            "u64" | "u32" | "usize" | "i32" | "u16" | "i16" | "u8" | "i8" | "isize" | "f32" => {
                Ok(Some(p.clone()))
            }
            _ => Err(()),
        },
        Type::ResolvedPath(rp) => {
            let last = last_seg_of_path(krate, rp);
            if last == "String" {
                return Ok(None);
            }
            if last == "PathBuf" {
                return Ok(Some(
                    path_of(krate, &rp.id)
                        .map(|p| p.join("::"))
                        .unwrap_or_else(|| "std::path::PathBuf".to_string()),
                ));
            }
            if last == "Vec" || last == "Option" {
                let Some(inner) = rp.args.as_deref().and_then(|a| match a {
                    GenericArgs::AngleBracketed { args, .. } => args.iter().find_map(|g| match g {
                        GenericArg::Type(t) => Some(t),
                        _ => None,
                    }),
                    _ => None,
                }) else {
                    return Err(());
                };
                if last == "Vec" && matches!(inner, Type::Primitive(p) if p == "u8") {
                    return Ok(None);
                }
                // The annotation names ONE type, so an element that
                // needs its own cannot be expressed.
                return match field_rust_ty(krate, inner, declared)? {
                    None => Ok(None),
                    Some(_) => Err(()),
                };
            }
            declared.contains_key(&last).then_some(None).ok_or(())
        }
        _ => Err(()),
    }
}

/// A name pixie can use as written./// A name pixie can use as written. Rust allows raw identifiers and
/// pixie does not, so anything else is left to the skip report.
fn is_pixie_ident(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with(|c: char| c.is_ascii_digit())
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn bind_fn(
    krate: &Crate,
    item: &Item,
    f: &Function,
    module: &str,
    declared: &HashMap<String, String>,
) -> Result<String, String> {
    if !matches!(item.visibility, Visibility::Public | Visibility::Default) {
        return Err("not public".into());
    }
    if f.header.is_unsafe {
        return Err("unsafe fn".into());
    }
    if f.header.is_async {
        return Err("async fn".into());
    }
    if item.deprecation.is_some() {
        return Err("deprecated".into());
    }
    let stringish = whitelisted_generics(krate, &f.generics)?;
    let name = item.name.as_deref().ok_or("unnamed fn")?;
    let mut params = Vec::new();
    // Set when a param or the return is a std `HashMap` (yokan's crate boundary):
    // the emitted @rust path carries the `stdmap:` prefix, which is
    // what tells the call-site adapter to convert at the boundary.
    let mut std_map = false;
    for (pname, pty) in &f.sig.inputs {
        let mapped = map_input(krate, pty, &stringish, declared, Some(&mut std_map))
            .map_err(|e| format!("param `{pname}`: {e}"))?;
        if mapped == PixTy::Unit {
            return Err(format!("param `{pname}`: unsupported type `()`"));
        }
        params.push((snake_to_camel(pname), mapped));
    }
    let (ret, fallible) = map_output(krate, f.sig.output.as_ref(), declared, Some(&mut std_map))?;
    let mut line = format!("  static fn {}(", snake_to_camel(name));
    for (i, (pn, pt)) in params.iter().enumerate() {
        if i > 0 {
            line.push_str(", ");
        }
        let _ = write!(line, "{pn}: {}", pt.render());
    }
    line.push(')');
    match (ret, fallible) {
        (PixTy::Unit, false) => {}
        (PixTy::Unit, true) => line.push_str(" !Void"),
        (t, false) => {
            let _ = write!(line, " {}", t.render());
        }
        (t, true) => {
            let _ = write!(line, " !{}", t.render());
        }
    }
    let marker = if std_map { "stdmap:" } else { "" };
    let _ = write!(line, " @rust(\"{marker}{module}::{name}\")");
    Ok(line)
}

/// Generate one `.rpi` covering every requested module. Returns the
/// file text and a per-class report.
pub fn generate(krate: &Crate, specs: &[BindSpec]) -> Result<(String, Vec<Report>), String> {
    let modules = modules_of(krate);
    let mut out = String::new();
    out.push_str("# Generated by rpi-gen — do not edit. Regenerate from rustdoc JSON.\n");
    let mut reports = Vec::new();
    // The C-like enums the bound modules declare, emitted once each
    // ahead of the classes so the functions below can name them
    // (§8.76). Keyed by the Rust type's last segment, which is what
    // a signature's path resolves to.
    let mut enum_names: HashMap<String, String> = HashMap::new();
    let mut enum_decls: Vec<(String, Vec<String>, String)> = Vec::new();
    for spec in specs {
        for (name, variants, rust_path) in enums_of(krate, &spec.module) {
            if enum_names.contains_key(&name) {
                continue;
            }
            enum_names.insert(name.clone(), name.clone());
            enum_decls.push((name, variants, rust_path));
        }
    }
    enum_decls.sort();
    for (name, variants, rust_path) in &enum_decls {
        // Every variant keeps its Rust name, so the correspondence
        // needs one attribute rather than one per variant (§8.74).
        let _ = write!(out, "\nenum {name} @rust(\"{rust_path}\") {{\n");
        for v in variants {
            let _ = writeln!(out, "  {v}");
        }
        out.push_str("}\n");
    }

    // Then the STRUCTS (§8.77). A struct crosses when every field
    // does, and a field may itself be a declared struct, so the set
    // shrinks to a fixpoint: drop whoever has a field that will not
    // map, and try again for the ones that named it.
    let mut declared = enum_names.clone();
    let mut cands: Vec<RpiStruct> = Vec::new();
    for spec in specs {
        for st in structs_of(krate, &spec.module) {
            if declared.contains_key(&st.name) || cands.iter().any(|c| c.name == st.name) {
                continue;
            }
            cands.push(st);
        }
    }
    for c in &cands {
        declared.insert(c.name.clone(), c.name.clone());
    }
    type RpiFieldDecl = (String, Option<String>, Option<String>, PixTy);
    let mut struct_decls: Vec<(String, String, Vec<RpiFieldDecl>)> = Vec::new();
    loop {
        let mut kept = Vec::new();
        let mut dropped = false;
        struct_decls.clear();
        for c in &cands {
            let mut fields = Vec::new();
            let mut ok = true;
            for f in &c.fields {
                let Ok(rust_ty) = field_rust_ty(krate, &f.ty, &declared) else {
                    ok = false;
                    break;
                };
                match map_plain_output(krate, &f.ty, &declared, None) {
                    Ok(t) if t != PixTy::Unit => {
                        fields.push((f.pix.clone(), f.rust.clone(), rust_ty, t))
                    }
                    _ => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok {
                struct_decls.push((c.name.clone(), c.rust_path.clone(), fields));
                kept.push(c.name.clone());
            } else {
                dropped = true;
            }
        }
        if !dropped {
            break;
        }
        declared.retain(|k, _| enum_names.contains_key(k) || kept.contains(k));
        cands.retain(|c| kept.contains(&c.name));
    }
    struct_decls.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, rust_path, fields) in &struct_decls {
        let _ = write!(out, "\nstruct {name} @rust(\"{rust_path}\") {{\n");
        for (pix, rust, rust_ty, ty) in fields {
            // A field carries an `@rust` only when something differs:
            // the naming conventions (`read_only` against `readOnly`,
            // or a tuple position), the Rust TYPE (§8.78), or both.
            let attr = match (rust, rust_ty) {
                (None, None) => String::new(),
                (Some(r), None) => format!(" @rust(\"{r}\")"),
                (r, Some(t)) => {
                    let name = r.clone().unwrap_or_else(|| pix.clone());
                    format!(" @rust(\"{name}: {t}\")")
                }
            };
            let _ = writeln!(out, "  var {pix} : {}{attr}", ty.render());
        }
        out.push_str("}\n");
    }

    for spec in specs {
        let Some(fn_ids) = modules.get(&spec.module) else {
            return Err(format!(
                "module `{}` not found (have: {})",
                spec.module,
                modules.keys().cloned().collect::<Vec<_>>().join(", ")
            ));
        };
        let mut report = Report {
            class: spec.class.clone(),
            bound: Vec::new(),
            skipped: Vec::new(),
        };
        let mut lines = Vec::new();
        let mut items: Vec<&Item> = fn_ids.iter().filter_map(|id| krate.index.get(id)).collect();
        items.sort_by_key(|i| i.name.clone());
        for item in items {
            let ItemEnum::Function(f) = &item.inner else {
                continue;
            };
            let name = item.name.clone().unwrap_or_default();
            match bind_fn(krate, item, f, &spec.module, &declared) {
                Ok(line) => {
                    lines.push(line);
                    report.bound.push(name);
                }
                Err(reason) => report.skipped.push((name, reason)),
            }
        }
        let _ = write!(out, "\nclass {} {{\n", spec.class);
        for l in &lines {
            out.push_str(l);
            out.push('\n');
        }
        out.push_str("}\n");
        if !report.skipped.is_empty() {
            out.push_str("# skipped (unbindable in v0):\n");
            for (n, r) in &report.skipped {
                let _ = writeln!(out, "#   {n} — {r}");
            }
        }
        reports.push(report);
    }
    Ok((out, reports))
}

pub fn parse_crate(json: &str) -> Result<Crate, String> {
    serde_json::from_str(json).map_err(|e| format!("rustdoc JSON parse error: {e}"))
}
