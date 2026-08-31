//! Per-function type environment used by the bidirectional checker.
//!
//! A `TypeEnv` is a singly-linked scope chain. Each scope holds local
//! bindings (name -> Type). `lookup` walks parent chains. The current
//! class context (the class we are checking a method body of) is carried
//! separately so `@var` and `self` resolve cleanly.

use crate::ty::Type;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct TypeEnv<'parent> {
    bindings: HashMap<String, Type>,
    parent: Option<&'parent TypeEnv<'parent>>,
}

impl<'parent> TypeEnv<'parent> {
    pub fn root() -> Self {
        Self {
            bindings: HashMap::new(),
            parent: None,
        }
    }

    pub fn child<'a>(&'a self) -> TypeEnv<'a> {
        TypeEnv {
            bindings: HashMap::new(),
            parent: Some(self),
        }
    }

    pub fn bind(&mut self, name: impl Into<String>, ty: Type) {
        self.bindings.insert(name.into(), ty);
    }

    pub fn lookup(&self, name: &str) -> Option<&Type> {
        if let Some(t) = self.bindings.get(name) {
            return Some(t);
        }
        self.parent.and_then(|p| p.lookup(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shadowing_works() {
        let mut root = TypeEnv::root();
        root.bind("x", Type::int());
        let mut child = root.child();
        assert_eq!(child.lookup("x"), Some(&Type::int()));
        child.bind("x", Type::string());
        assert_eq!(child.lookup("x"), Some(&Type::string()));
        // Parent still sees the old binding.
        assert_eq!(root.lookup("x"), Some(&Type::int()));
    }

    #[test]
    fn missing_lookup_returns_none() {
        let env = TypeEnv::root();
        assert_eq!(env.lookup("nope"), None);
    }

    #[test]
    fn parent_lookup_walks_chain() {
        let mut root = TypeEnv::root();
        root.bind("a", Type::int());
        let mut mid = root.child();
        mid.bind("b", Type::float());
        let leaf = mid.child();
        assert_eq!(leaf.lookup("a"), Some(&Type::int()));
        assert_eq!(leaf.lookup("b"), Some(&Type::float()));
    }
}
