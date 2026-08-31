//! The [crates] demo target: a plain Rust crate pixie binds through
//! the manifest pipeline (pixie.toml → rustdoc JSON → rpi-gen cache).

pub fn triple(x: i64) -> i64 {
    x * 3
}

pub fn shout(s: &str) -> String {
    s.to_uppercase()
}

pub fn half(x: f64) -> f64 {
    x / 2.0
}
