use std::cell::RefCell;
use std::fmt;
use std::ops::Deref;
use std::rc::Rc;

use crate::NixResult;

use super::{LazyNixValue, NixValue, NixValueWrapped};

#[derive(Clone)]
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

impl PartialEq for NixVar {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_ptr() == other.0.as_ptr()
    }
}

impl Eq for NixVar {}

impl NixVar {
    pub fn try_eq(&self, rhs: &Self) -> NixResult<bool> {
        LazyNixValue::try_eq(&self.0, &rhs.0)
    }

    pub fn as_concrete(&self) -> Option<NixValueWrapped> {
        self.0.borrow().as_concrete()
    }

    pub fn resolve(&self) -> NixResult {
        if let Some(value) = self.0.borrow().as_concrete() {
            return Ok(value);
        }

        LazyNixValue::resolve(&self.0)
    }

    pub fn resolve_set(&self, recursive: bool) -> NixResult {
        if let Some(value) = self.0.borrow().as_concrete() {
            return Ok(value);
        }

        LazyNixValue::resolve_set(&self.0, recursive)
    }

    pub fn resolve_map<T>(&self, f: impl FnOnce(&NixValue) -> T) -> NixResult<T> {
        Ok(f(self.resolve()?.borrow().deref()))
    }
}
