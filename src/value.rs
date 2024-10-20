use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{self, Write};
use std::ops::Deref;
use std::rc::Rc;

use rnix::ast;

use crate::scope::Scope;

#[derive(Clone)]
pub enum NixLambdaParam {
    Ident(String),
    Pattern(ast::Pattern),
}

#[derive(Clone, Default)]
pub enum NixValue {
    AttrSet(HashMap<String, NixValueWrapped>),
    Lambda(Rc<Scope>, NixLambdaParam, ast::Expr),
    #[default]
    Null,
    String(String),
}

pub type NixValueWrapped = Rc<RefCell<NixValue>>;

impl fmt::Debug for NixValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NixValue::AttrSet(set) => {
                let mut map = f.debug_map();

                for (a, b) in set {
                    let b = b.as_ref().borrow();
                    let b = b.deref();

                    map.entry(a, b);
                }

                map.finish()
            }
            NixValue::Lambda(..) => f.write_str("<lamda>"),
            NixValue::Null => f.write_str("null"),
            NixValue::String(s) => {
                f.write_char('"')?;
                f.write_str(s)?;
                f.write_char('"')
            }
        }
    }
}

impl fmt::Display for NixValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NixValue::AttrSet(set) => {
                let width = f.width().unwrap_or_default();
                let outside_pad = " ".repeat(width);

                let width = width + 2;
                let pad = " ".repeat(width);

                f.write_char('{')?;

                if f.alternate() {
                    f.write_char('\n')?;
                }

                for (key, value) in set {
                    let value = value.as_ref().borrow();
                    let value = value.deref();

                    if f.alternate() {
                        f.write_str(&pad)?;
                    } else {
                        f.write_char(' ')?;
                    }

                    f.write_str(key)?;
                    f.write_str(" = ")?;

                    if f.alternate() {
                        f.write_fmt(format_args!("{value:#width$}"))?;
                    } else {
                        fmt::Display::fmt(value, f)?;
                    }

                    f.write_char(';')?;

                    if f.alternate() {
                        f.write_char('\n')?;
                    }
                }

                if f.alternate() {
                    f.write_str(&outside_pad)?;
                } else {
                    f.write_char(' ')?;
                }

                f.write_char('}')
            }
            NixValue::Lambda(..) => f.write_str("<lamda>"),
            NixValue::Null => f.write_str("null"),
            NixValue::String(s) => {
                f.write_char('"')?;
                f.write_str(s)?;
                f.write_char('"')
            }
        }
    }
}

impl NixValue {
    pub fn wrap(self) -> NixValueWrapped {
        Rc::new(RefCell::new(self))
    }

    pub fn get(&self, attr: &String) -> Result<Option<NixValueWrapped>, ()> {
        let NixValue::AttrSet(set) = self else {
            todo!("Error handling");
            // return Err(());
        };

        Ok(set.get(attr).cloned())
    }

    /// Returns (new_value, old_value)
    pub fn insert(
        &mut self,
        attr: String,
        value: NixValueWrapped,
    ) -> Option<(NixValueWrapped, Option<NixValueWrapped>)> {
        let NixValue::AttrSet(set) = self else {
            todo!("Error handling");
            // return Err(());
        };

        let old = set.insert(attr, value.clone());

        Some((value, old))
    }

    pub fn as_lamda(&self) -> Option<(Rc<Scope>, &NixLambdaParam, &ast::Expr)> {
        if let NixValue::Lambda(scope, param, expr) = self {
            Some((scope.clone(), param, expr))
        } else {
            None
        }
    }

    pub fn is_lamda(&self) -> bool {
        matches!(self, NixValue::Lambda(..))
    }
}

pub trait AsString {
    fn as_string(&self) -> Option<String>;

    #[allow(dead_code)]
    fn is_string(&self) -> bool {
        self.as_string().is_some()
    }
}

impl AsString for NixValue {
    fn as_string(&self) -> Option<String> {
        // TODO: AttrSet to String
        match self {
            NixValue::AttrSet(_) => None,
            NixValue::Lambda(..) => None,
            NixValue::Null => Some(String::from("")),
            NixValue::String(str) => Some(str.clone()),
        }
    }
}

pub trait AsAttrSet {
    fn as_attr_set(&self) -> Option<&HashMap<String, NixValueWrapped>>;
    fn as_attr_set_mut(&mut self) -> Option<&mut HashMap<String, NixValueWrapped>>;

    fn is_attr_set(&self) -> bool {
        self.as_attr_set().is_some()
    }
}

impl AsAttrSet for NixValue {
    fn as_attr_set(&self) -> Option<&HashMap<String, NixValueWrapped>> {
        if let NixValue::AttrSet(set) = self {
            Some(set)
        } else {
            None
        }
    }

    fn as_attr_set_mut(&mut self) -> Option<&mut HashMap<String, NixValueWrapped>> {
        if let NixValue::AttrSet(set) = self {
            Some(set)
        } else {
            None
        }
    }
}
