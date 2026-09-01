# gpui_macos (vendored)

This crate is a copy of `crates/gpui_macos` from the Zed repository
(<https://github.com/zed-industries/zed>) at revision
`d9ad6aff67e47de43abb270d22de75dd950f1b48`, copyright Zed
Industries, Inc., licensed under the Apache License, Version 2.0
(see [LICENSE-APACHE](LICENSE-APACHE)). It is vendored and patched
here; it is not a Zed Industries release.

Local modifications, per Apache-2.0 §4(b):

- `Cargo.toml` — sibling-crate dependencies concretized as git
  references pinned to the same revision, so the crate builds
  outside the Zed workspace.
- `src/window.rs` — panic containment around the input-method
  callback in `with_input_handler`: a panic raised inside an IME
  callback is caught and logged instead of aborting the process.

Every other file is byte-identical to the upstream revision.
