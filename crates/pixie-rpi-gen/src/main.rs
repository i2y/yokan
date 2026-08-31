//! CLI: `rpi-gen <rustdoc.json> --bind <module=Class>... [--out <file.rpi>]`
//!
//! Producing the input on the pinned toolchain:
//!   RUSTC_BOOTSTRAP=1 RUSTDOCFLAGS="-Z unstable-options --output-format json" \
//!     cargo doc --no-deps
//! (or `cargo +nightly rustdoc` / the `rust-docs-json` component for std).

use std::process::ExitCode;

fn usage() -> ExitCode {
    eprintln!("usage: rpi-gen <rustdoc.json> --bind <rust::module=ClassName>... [--out <file.rpi>]");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut json_path: Option<String> = None;
    let mut specs: Vec<pixie_rpi_gen::BindSpec> = Vec::new();
    let mut out_path: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--bind" => {
                let Some(v) = args.get(i + 1) else {
                    return usage();
                };
                let Some((module, class)) = v.split_once('=') else {
                    eprintln!("rpi-gen: --bind wants `rust::module=ClassName`, got `{v}`");
                    return ExitCode::from(2);
                };
                specs.push(pixie_rpi_gen::BindSpec {
                    module: module.to_string(),
                    class: class.to_string(),
                });
                i += 2;
            }
            "--out" => {
                let Some(v) = args.get(i + 1) else {
                    return usage();
                };
                out_path = Some(v.clone());
                i += 2;
            }
            other if json_path.is_none() && !other.starts_with('-') => {
                json_path = Some(other.to_string());
                i += 1;
            }
            _ => return usage(),
        }
    }
    let (Some(json_path), false) = (json_path, specs.is_empty()) else {
        return usage();
    };
    let json = match std::fs::read_to_string(&json_path) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("rpi-gen: cannot read {json_path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let krate = match pixie_rpi_gen::parse_crate(&json) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("rpi-gen: {e}");
            eprintln!(
                "rpi-gen: (the tool is pinned to rustdoc JSON format {}; regenerate the JSON with the matching toolchain)",
                rustdoc_types::FORMAT_VERSION
            );
            return ExitCode::FAILURE;
        }
    };
    match pixie_rpi_gen::generate(&krate, &specs) {
        Ok((text, reports)) => {
            for r in &reports {
                eprintln!(
                    "rpi-gen: class {} — bound {} fn(s), skipped {}",
                    r.class,
                    r.bound.len(),
                    r.skipped.len()
                );
                for (n, why) in &r.skipped {
                    eprintln!("rpi-gen:   skipped {n} — {why}");
                }
            }
            match out_path {
                Some(p) => {
                    if let Err(e) = std::fs::write(&p, text) {
                        eprintln!("rpi-gen: cannot write {p}: {e}");
                        return ExitCode::FAILURE;
                    }
                    eprintln!("rpi-gen: wrote {p}");
                }
                None => print!("{text}"),
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("rpi-gen: {e}");
            ExitCode::FAILURE
        }
    }
}
