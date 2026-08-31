//! `.rpi` (pixie Rust Package Interface) binding-file loader.
//!
//! Bindings describe the surface of a foreign Rust crate to pixie's type
//! checker - method signatures, property types, signal parameters - so
//! calls resolve against a real definition instead of falling through to
//! `Type::External` soft-pass.
//!
//! The format is a strict subset of pixie syntax: `class Name { ... }`
//! with property / signal / fn members, no fn bodies. The pixie parser
//! handles it directly. Codegen never sees these classes - the driver
//! merges them into the HIR/type-check view but emits Rust only for
//! the user module.
//!
//! Unlike the Qt ancestor (which baked ~80 .qpi files into the binary),
//! pixie ships no binding stdlib yet: the prelude is the kernel's own
//! surface, and crate bindings arrive with the rpi-gen pipeline
//! (DESIGN.md section 7).

use pixie_syntax::{Module, ParseError, SourceMap, parse_binding};

#[derive(Debug, thiserror::Error)]
pub enum BindingError {
    #[error("failed to parse binding `{name}`: {message}")]
    Parse { name: String, message: String },
}

/// Parse one `.rpi` source string as a pixie module. `name` lands in the
/// SourceMap entry so spans get a real, distinct `FileId` — the visibility
/// check keys items by declaring file id, so bindings must never alias the
/// entry file's id.
pub fn parse_rpi(
    source_map: &mut SourceMap,
    name: &str,
    src: &str,
) -> Result<Module, BindingError> {
    let owned = src.to_string();
    let file_id = source_map.add(format!("<binding:{name}>"), owned);
    let stored = source_map.source(file_id);
    parse_binding(file_id, stored).map_err(|e: ParseError| BindingError::Parse {
        name: name.to_string(),
        message: format!("{e:?}"),
    })
}

/// Built-in bindings loaded before user source. Empty for now — pixie's
/// checker builtins (String / List / Map / primitives) live in
/// pixie-types, and everything else comes from explicit `.rpi` files.
/// The signature stays so the driver's load order has one place to grow.
pub fn load_stdlib(_source_map: &mut SourceMap) -> Result<Vec<Module>, BindingError> {
    Ok(Vec::new())
}
