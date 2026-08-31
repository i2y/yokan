//! Local type-variable + substitution + unification for generic
//! instantiation at call sites.
//!
//! Each call to a generic fn / method gets its own freshly-allocated
//! `VarId`s (one per generic parameter). The signature stored in the
//! `ProgramTable` already uses declaration-time `VarId`s; we substitute
//! them with fresh ones via `instantiate`, then unify each argument's
//! synthesized type against the (substituted) expected param type. The
//! resulting bindings live in a per-call `Substitution`, applied to the
//! return type before it propagates outward.
//!
//! "Unify or subtype": fully concrete types still go through the
//! existing nominal subtype relation in `crate::ty::is_subtype`; only
//! `Var` slots and parametric `Generic { base, args }` recursions
//! actually unify. Function types unify component-wise (treating params
//! invariantly for simplicity in this minimum implementation).

use crate::table::FnTy;
use crate::ty::{Type, VarId, is_subtype};
use pixie_hir::ResolvedProgram;
use std::collections::HashMap;

/// Monotonic allocator for fresh `VarId`s. Owned by the `Checker`.
#[derive(Default, Debug)]
pub struct VarSource {
    next: u32,
}

impl VarSource {
    pub fn fresh(&mut self) -> VarId {
        let v = VarId(self.next);
        self.next += 1;
        v
    }
}

#[derive(Default, Debug, Clone)]
pub struct Substitution {
    map: HashMap<VarId, Type>,
}

impl Substitution {
    /// Walk `t` replacing `Var(v)` with the bound type (transitively),
    /// then recursing into the structural slots. Cycles are not possible
    /// here because we never bind a var to itself - if we tried to, it
    /// would already be transitively-resolvable to the same var.
    pub fn apply(&self, t: &Type) -> Type {
        match t {
            Type::Var(v) => match self.map.get(v) {
                Some(t2) => self.apply(t2),
                None => Type::Var(*v),
            },
            Type::Generic { base, args } => Type::Generic {
                base: base.clone(),
                args: args.iter().map(|a| self.apply(a)).collect(),
            },
            Type::Nullable(inner) => Type::Nullable(Box::new(self.apply(inner))),
            Type::ErrorUnion { ok, err } => Type::ErrorUnion {
                ok: Box::new(self.apply(ok)),
                err: err.clone(),
            },
            Type::Fn { params, ret } => Type::Fn {
                params: params.iter().map(|p| self.apply(p)).collect(),
                ret: Box::new(self.apply(ret)),
            },
            other => other.clone(),
        }
    }

    pub fn bind(&mut self, v: VarId, t: Type) {
        self.map.insert(v, t);
    }
}

#[derive(Debug)]
pub struct Mismatch {
    pub expected: Type,
    pub actual: Type,
}

/// Unify-or-subtype the actual type against the expected type, recording
/// any var bindings into `subst`. Returns `Err(Mismatch)` if the types
/// are structurally incompatible after substitution.
pub fn unify_or_subtype(
    actual: &Type,
    expected: &Type,
    subst: &mut Substitution,
    program: &ResolvedProgram,
) -> Result<(), Mismatch> {
    let a = subst.apply(actual);
    let e = subst.apply(expected);
    match (&a, &e) {
        (Type::Var(va), Type::Var(vb)) if va == vb => Ok(()),
        (Type::Var(v), other) | (other, Type::Var(v)) => {
            subst.bind(*v, other.clone());
            Ok(())
        }
        (Type::Generic { base: b1, args: a1 }, Type::Generic { base: b2, args: a2 })
            if b1 == b2 && a1.len() == a2.len() =>
        {
            for (x, y) in a1.iter().zip(a2.iter()) {
                unify_or_subtype(x, y, subst, program)?;
            }
            Ok(())
        }
        (
            Type::Fn {
                params: p1,
                ret: r1,
            },
            Type::Fn {
                params: p2,
                ret: r2,
            },
        ) if p1.len() == p2.len() => {
            for (x, y) in p1.iter().zip(p2.iter()) {
                unify_or_subtype(x, y, subst, program)?;
            }
            unify_or_subtype(r1, r2, subst, program)
        }
        (Type::Nullable(x), Type::Nullable(y)) => unify_or_subtype(x, y, subst, program),
        _ => {
            if is_subtype(&a, &e, program) {
                Ok(())
            } else {
                Err(Mismatch {
                    actual: a,
                    expected: e,
                })
            }
        }
    }
}

/// Build a fresh-VarId substitution covering the fn's generic params,
/// then return the substituted (params, ret) pair plus the substitution
/// (so the call-site can apply additional unifications and read back the
/// return type at the end).
pub fn instantiate(fn_ty: &FnTy, source: &mut VarSource) -> (Substitution, Vec<Type>, Type) {
    let mut subst = Substitution::default();
    for v in &fn_ty.generics {
        let fresh = source.fresh();
        subst.bind(*v, Type::Var(fresh));
    }
    let params = fn_ty.params.iter().map(|p| subst.apply(p)).collect();
    let ret = subst.apply(&fn_ty.ret);
    (subst, params, ret)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_prog() -> ResolvedProgram {
        ResolvedProgram::default()
    }

    #[test]
    fn substitution_applies_through_generics() {
        let mut subst = Substitution::default();
        subst.bind(VarId(0), Type::int());
        let t = Type::Generic {
            base: "List".into(),
            args: vec![Type::Var(VarId(0))],
        };
        let result = subst.apply(&t);
        assert_eq!(
            result,
            Type::Generic {
                base: "List".into(),
                args: vec![Type::int()]
            }
        );
    }

    #[test]
    fn unify_binds_var_to_concrete() {
        let mut subst = Substitution::default();
        let prog = dummy_prog();
        unify_or_subtype(&Type::Var(VarId(0)), &Type::int(), &mut subst, &prog).unwrap();
        assert_eq!(subst.apply(&Type::Var(VarId(0))), Type::int());
    }

    #[test]
    fn unify_binds_var_inside_generic() {
        let mut subst = Substitution::default();
        let prog = dummy_prog();
        let actual = Type::Generic {
            base: "List".into(),
            args: vec![Type::Var(VarId(0))],
        };
        let expected = Type::Generic {
            base: "List".into(),
            args: vec![Type::string()],
        };
        unify_or_subtype(&actual, &expected, &mut subst, &prog).unwrap();
        assert_eq!(subst.apply(&Type::Var(VarId(0))), Type::string());
    }

    #[test]
    fn unify_rejects_mismatched_bases() {
        let mut subst = Substitution::default();
        let prog = dummy_prog();
        let actual = Type::Generic {
            base: "List".into(),
            args: vec![Type::int()],
        };
        let expected = Type::Generic {
            base: "Set".into(),
            args: vec![Type::int()],
        };
        assert!(unify_or_subtype(&actual, &expected, &mut subst, &prog).is_err());
    }

    #[test]
    fn unify_chains_through_nested_vars() {
        let mut subst = Substitution::default();
        let prog = dummy_prog();
        let actual = Type::Generic {
            base: "Map".into(),
            args: vec![
                Type::Var(VarId(0)),
                Type::Generic {
                    base: "List".into(),
                    args: vec![Type::Var(VarId(1))],
                },
            ],
        };
        let expected = Type::Generic {
            base: "Map".into(),
            args: vec![
                Type::string(),
                Type::Generic {
                    base: "List".into(),
                    args: vec![Type::int()],
                },
            ],
        };
        unify_or_subtype(&actual, &expected, &mut subst, &prog).unwrap();
        assert_eq!(subst.apply(&Type::Var(VarId(0))), Type::string());
        assert_eq!(subst.apply(&Type::Var(VarId(1))), Type::int());
    }

    #[test]
    fn instantiate_yields_fresh_vars_and_substituted_signature() {
        let mut source = VarSource::default();
        let fn_ty = FnTy {
            generics: vec![VarId(100), VarId(101)],
            generic_bounds: vec![Vec::new(), Vec::new()],
            params: vec![
                Type::Generic {
                    base: "List".into(),
                    args: vec![Type::Var(VarId(100))],
                },
                Type::Var(VarId(101)),
            ],
            ret: Type::Var(VarId(100)),
            is_static: false,
        };
        let (_subst, params, ret) = instantiate(&fn_ty, &mut source);
        // The fresh vars should be Var(0) and Var(1) (allocated in order).
        match &params[0] {
            Type::Generic { base, args } => {
                assert_eq!(base, "List");
                assert!(matches!(args[0], Type::Var(_)));
            }
            _ => panic!("expected Generic"),
        }
        match (&ret, &params[0]) {
            (Type::Var(rv), Type::Generic { args, .. }) => {
                if let Type::Var(av) = args[0] {
                    assert_eq!(
                        *rv, av,
                        "ret and params[0] should share the same fresh var for T"
                    );
                } else {
                    panic!("expected Var inside Generic");
                }
            }
            _ => panic!(),
        }
    }
}
