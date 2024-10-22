mod lazy;
mod var;

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{self, Write};
use std::ops::Deref;
use std::path::PathBuf;
use std::rc::Rc;

pub use lazy::LazyNixValue;
pub use var::NixVar;

use rnix::ast;

use crate::scope::Scope;

#[derive(Clone)]
pub enum NixLambdaParam {
    Ident(String),
    Pattern(ast::Pattern),
}

#[derive(Clone)]
pub enum NixValueBuiltin {
    Import,
}

#[derive(Clone, Default)]
pub enum NixValue {
    AttrSet(HashMap<String, NixVar>),
    Builtin(NixValueBuiltin),
    Lambda(Rc<Scope>, NixLambdaParam, ast::Expr),
    List(Vec<NixVar>),
    #[default]
    Null,
    Path(PathBuf),
    String(String),
}

pub type NixValueWrapped = Rc<RefCell<NixValue>>;

impl fmt::Debug for NixValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NixValue::AttrSet(set) => {
                let mut map = f.debug_map();

                for (key, value) in set {
                    map.entry(key, value);
                }

                map.finish()
            }
            NixValue::Builtin(NixValueBuiltin::Import) => f.write_str("import"),
            NixValue::Lambda(..) => f.write_str("<lamda>"),
            NixValue::List(list) => {
                let mut debug_list = f.debug_list();

                for item in list {
                    debug_list.entry(item);
                }

                debug_list.finish()
            }
            NixValue::Null => f.write_str("null"),
            NixValue::Path(path) => fmt::Debug::fmt(path, f),
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
                    let value = value.resolve();
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
            NixValue::Builtin(NixValueBuiltin::Import) => f.write_str("import"),
            NixValue::Lambda(..) => f.write_str("<lamda>"),
            NixValue::List(list) => {
                let width = f.width().unwrap_or_default();
                let outside_pad = " ".repeat(width);

                let width = width + 2;
                let pad = " ".repeat(width);

                f.write_char('[')?;

                if f.alternate() {
                    f.write_char('\n')?;
                }

                for value in list {
                    let value = value.resolve();
                    let value = value.as_ref().borrow();
                    let value = value.deref();

                    if f.alternate() {
                        f.write_str(&pad)?;
                    } else {
                        f.write_char(' ')?;
                    }

                    if f.alternate() {
                        f.write_fmt(format_args!("{value:#width$}"))?;
                    } else {
                        fmt::Display::fmt(value, f)?;
                    }

                    if f.alternate() {
                        f.write_char('\n')?;
                    }
                }

                if f.alternate() {
                    f.write_str(&outside_pad)?;
                } else {
                    f.write_char(' ')?;
                }

                f.write_char(']')
            }
            NixValue::Null => f.write_str("null"),
            NixValue::Path(path) => f.write_fmt(format_args!("{}", path.display())),
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

    pub fn wrap_var(self) -> NixVar {
        NixVar(Rc::new(RefCell::new(LazyNixValue::Concrete(self.wrap()))))
    }

    pub fn get(&self, attr: &String) -> Result<Option<NixVar>, ()> {
        let NixValue::AttrSet(set) = self else {
            todo!("Error handling");
        };

        Ok(set.get(attr).cloned())
    }

    /// Returns (new_value, old_value)
    pub fn insert(&mut self, attr: String, value: NixVar) -> Option<(NixVar, Option<NixVar>)> {
        let NixValue::AttrSet(set) = self else {
            todo!("Error handling");
            // return Err(());
        };

        let old = set.insert(attr, value.clone());

        Some((value, old))
    }

    pub fn as_lambda(&self) -> Option<(Rc<Scope>, &NixLambdaParam, &ast::Expr)> {
        if let NixValue::Lambda(scope, param, expr) = self {
            Some((scope.clone(), param, expr))
        } else {
            None
        }
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
            NixValue::Null => Some(String::from("")),
            NixValue::Path(path) => Some(path.display().to_string()),
            NixValue::String(str) => Some(str.clone()),
            _ => None,
        }
    }
}

pub trait AsAttrSet {
    fn as_attr_set(&self) -> Option<&HashMap<String, NixVar>>;
    fn as_attr_set_mut(&mut self) -> Option<&mut HashMap<String, NixVar>>;

    fn is_attr_set(&self) -> bool {
        self.as_attr_set().is_some()
    }
}

impl AsAttrSet for NixValue {
    fn as_attr_set(&self) -> Option<&HashMap<String, NixVar>> {
        if let NixValue::AttrSet(set) = self {
            Some(set)
        } else {
            None
        }
    }

    fn as_attr_set_mut(&mut self) -> Option<&mut HashMap<String, NixVar>> {
        if let NixValue::AttrSet(set) = self {
            Some(set)
        } else {
            None
        }
    }
}
