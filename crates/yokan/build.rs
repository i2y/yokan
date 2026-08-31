fn main() {
    // macOS Python extension modules leave CPython symbols undefined
    // and resolve them from the host process at import time. These
    // args apply to the cdylib only, so test binaries are unaffected.
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("apple") {
        println!("cargo:rustc-cdylib-link-arg=-undefined");
        println!("cargo:rustc-cdylib-link-arg=dynamic_lookup");
    }
}
