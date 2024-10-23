use std::cell::RefCell;
use std::fmt;
use std::ops::Deref;
use std::rc::Rc;

use super::{LazyNixValue, NixValue, NixValueWrapped};

#[derive(Clone, PartialEq, Eq)]
pub struct NixVar(pub Rc<RefCell<LazyNixValue>>);

impl fmt::Debug for NixVar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.0.borrow().deref(), f)
    }
}

impl fmt::Display for NixVar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.0.borrow().deref(), f)
    }
}

impl NixVar {
    pub fn as_concrete(&self) -> Option<NixValueWrapped> {
        self.0.borrow().as_concrete()
    }

    pub fn resolve(&self) -> NixValueWrapped {
        if let Some(a) = self.0.borrow().as_concrete() {
            return a;
        }

        LazyNixValue::resolve(&self.0)
    }

    pub fn resolve_set(&self, recursive: bool) -> NixValueWrapped {
        if let Some(a) = self.0.borrow().as_concrete() {
            return a;
        }

        LazyNixValue::resolve_set(&self.0, recursive)
    }

    pub fn resolve_and(&self, f: impl FnOnce(&NixValue) -> bool) -> bool {
        f(self.resolve().borrow().deref())
    }

    pub fn resolve_map<T>(&self, f: impl FnOnce(&NixValue) -> T) -> T {
        f(self.resolve().borrow().deref())
    }
}
