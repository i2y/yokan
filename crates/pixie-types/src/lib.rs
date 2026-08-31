//! pixie type system.
//!
//! Approach: bidirectional type checking with local inference and nominal
//! subtyping. See `check.rs` for the synth/check engine, `ty.rs` for the
//! `Type` representation and the subtype relation, and `env.rs` for the
//! per-fn scope chain.
//!
//! Soft-fail policy: anything we don't have a binding for (foreign Qt
//! classes, unresolved names) is treated as `Type::External` and accepts
//! arbitrary access. Once `.qpi` binding files land we tighten this.

pub mod check;
pub mod env;
pub mod flow;
pub mod infer;
pub mod qss;
pub mod table;
pub mod ty;

pub use check::{CheckResult, check_program};
pub use env::TypeEnv;
pub use flow::check_linearity;
pub use infer::{Mismatch, Substitution, VarSource, instantiate, unify_or_subtype};
pub use table::{ClassEntry, ErrorEntry, FnTy, ProgramTable, build as build_table};
pub use ty::{Prim, Type, VarId, is_subtype, lower_type};
