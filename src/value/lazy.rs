use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::{fmt, mem};

use rnix::ast;

use crate::scope::Scope;

use super::{AsAttrSet, NixValueWrapped, NixVar};

#[derive(Clone, PartialEq, Eq)]
pub enum LazyNixValue {
    Concrete(NixValueWrapped),
    Pending(Rc<Scope>, ast::Expr),
    Resolving,
}

impl fmt::Debug for LazyNixValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LazyNixValue::Concrete(value) => fmt::Debug::fmt(value.borrow().deref(), f),
            LazyNixValue::Pending(..) => f.write_str("<not-resolved>"),
            LazyNixValue::Resolving => f.write_str("<resolving>"),
        }
    }
}

impl fmt::Display for LazyNixValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LazyNixValue::Concrete(value) => fmt::Display::fmt(value.borrow().deref(), f),
            LazyNixValue::Pending(..) => f.write_str("<not-resolved>"),
            LazyNixValue::Resolving => f.write_str("<resolving>"),
        }
    }
}

impl LazyNixValue {
    pub fn wrap_var(self) -> NixVar {
        NixVar(Rc::new(RefCell::new(self)))
    }

    pub fn as_concrete(&self) -> Option<NixValueWrapped> {
        if let LazyNixValue::Concrete(value) = self {
            Some(value.clone())
        } else {
            None
        }
    }

    pub fn resolve(this: &Rc<RefCell<Self>>) -> NixValueWrapped {
        if let Some(value) = this.borrow().as_concrete() {
            return value;
        }

        let old = mem::replace(this.borrow_mut().deref_mut(), LazyNixValue::Resolving);

        match old {
            LazyNixValue::Concrete(_) => unreachable!(),
            LazyNixValue::Pending(scope, expr) => {
                let value = scope.visit_expr(expr);

                *this.borrow_mut().deref_mut() = LazyNixValue::Concrete(value.clone());

                value
            }
            LazyNixValue::Resolving => {
                unreachable!("Infinite recursion detected. Tried to get a value that is resolving")
            }
        }
    }

    pub fn resolve_set(this: &Rc<RefCell<Self>>, recursive: bool) -> NixValueWrapped {
        let value = Self::resolve(this);

        if value.borrow().is_attr_set() {
            let values = if let Some(set) = value.borrow().as_attr_set() {
                set.values().cloned().collect::<Vec<_>>()
            } else {
                unreachable!()
            };

            for var in values {
                if recursive {
                    var.resolve_set(true);
                } else {
                    var.resolve();
                }
            }

            value
        } else {
            value
        }
    }
}
