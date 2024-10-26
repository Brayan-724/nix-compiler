use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::{fmt, mem};

use rnix::ast;

use crate::scope::Scope;
use crate::NixResult;

use super::{AsAttrSet, NixValueWrapped, NixVar};

#[derive(Clone)]
pub enum LazyNixValue {
    Concrete(NixValueWrapped),
    Pending(Rc<Scope>, ast::Expr),
    Eval(
        Rc<Scope>,
        Rc<RefCell<Option<Box<dyn FnOnce(Rc<Scope>) -> NixResult>>>>,
    ),
    Resolving,
}

impl fmt::Debug for LazyNixValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LazyNixValue::Concrete(value) => fmt::Debug::fmt(value.borrow().deref(), f),
            LazyNixValue::Pending(..) => f.write_str("<not-resolved>"),
            LazyNixValue::Eval(..) => f.write_str("<not-resolved>"),
            LazyNixValue::Resolving => f.write_str("<resolving>"),
        }
    }
}

impl fmt::Display for LazyNixValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LazyNixValue::Concrete(value) => fmt::Display::fmt(value.borrow().deref(), f),
            LazyNixValue::Pending(..) => f.write_str("<not-resolved>"),
            LazyNixValue::Eval(..) => f.write_str("<not-resolved>"),
            LazyNixValue::Resolving => f.write_str("<resolving>"),
        }
    }
}

impl LazyNixValue {
    pub fn try_eq(lhs: &Rc<RefCell<Self>>, rhs: &Rc<RefCell<Self>>) -> NixResult<bool> {
        let lhs = LazyNixValue::resolve(lhs)?;
        let rhs = LazyNixValue::resolve(rhs)?;

        let lhs = lhs.borrow();
        let rhs = rhs.borrow();

        Ok(*lhs == *rhs)
    }
}

impl LazyNixValue {
    pub fn new_eval(scope: Rc<Scope>, fun: Box<dyn FnOnce(Rc<Scope>) -> NixResult>) -> Self {
        LazyNixValue::Eval(scope, Rc::new(RefCell::new(Option::Some(fun))))
    }

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

    pub fn resolve(this: &Rc<RefCell<Self>>) -> NixResult {
        if let Some(value) = this.borrow().as_concrete() {
            return Ok(value);
        }

        let old = mem::replace(this.borrow_mut().deref_mut(), LazyNixValue::Resolving);

        match old {
            LazyNixValue::Concrete(_) => unreachable!(),
            LazyNixValue::Pending(scope, expr) => {
                let value = scope.visit_expr(expr)?;

                *this.borrow_mut().deref_mut() = LazyNixValue::Concrete(value.clone());

                Ok(value)
            }
            LazyNixValue::Eval(scope, eval) => {
                let value =
                    eval.borrow_mut()
                        .take()
                        .expect("Eval cannot be called twice")(scope.clone())?;

                *this.borrow_mut().deref_mut() = LazyNixValue::Concrete(value.clone());

                Ok(value)
            }
            LazyNixValue::Resolving => {
                unreachable!("Infinite recursion detected. Tried to get a value that is resolving")
            }
        }
    }

    pub fn resolve_set(this: &Rc<RefCell<Self>>, recursive: bool) -> NixResult {
        let value = Self::resolve(this)?;

        if value.borrow().is_attr_set() {
            let values = if let Some(set) = value.borrow().as_attr_set() {
                set.values().cloned().collect::<Vec<_>>()
            } else {
                unreachable!()
            };

            for var in values {
                if recursive {
                    var.resolve_set(true)?;
                } else {
                    var.resolve()?;
                }
            }

            Ok(value)
        } else {
            Ok(value)
        }
    }
}
