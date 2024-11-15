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

use crate::builtins::NixBuiltin;
use crate::scope::Scope;
use crate::{NixBacktrace, NixResult, NixSpan};

#[derive(Clone, PartialEq, Eq)]
pub enum NixLambdaParam {
    Ident(String),
    Pattern(ast::Pattern),
}

#[derive(Clone, PartialEq, Eq)]
pub struct NixLambda(pub Rc<Scope>, pub NixLambdaParam, pub ast::Expr);

#[derive(Clone, PartialEq, Eq)]
pub struct NixList(pub Rc<Vec<NixVar>>);

pub type NixAttrSet = HashMap<String, NixVar>;

/// https://nix.dev/manual/nix/2.24/language/types
#[derive(Default, PartialEq, Eq)]
pub enum NixValue {
    AttrSet(NixAttrSet),
    Bool(bool),
    /// https://nix.dev/manual/nix/2.24/language/builtins
    Builtin(Rc<Box<dyn NixBuiltin>>),
    Int(i64),
    Lambda(NixLambda),
    List(NixList),
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
            NixValue::Bool(true) => f.write_str("true"),
            NixValue::Bool(false) => f.write_str("false"),
            NixValue::Builtin(builtin) => fmt::Debug::fmt(builtin, f),
            NixValue::Int(val) => f.write_str(&val.to_string()),
            NixValue::Lambda(..) => f.write_str("<lamda>"),
            NixValue::List(list) => {
                let mut debug_list = f.debug_list();

                for item in &*list.0 {
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
                    let value = value.as_concrete().unwrap_or_else(|| {
                        eprintln!("Can't display something unresolved, run `.resolve_set()` before display it");
                        std::process::exit(1)
                    });

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
            NixValue::Bool(true) => f.write_str("true"),
            NixValue::Bool(false) => f.write_str("false"),
            NixValue::Builtin(builtin) => fmt::Display::fmt(&builtin, f),
            NixValue::Int(val) => f.write_str(&val.to_string()),
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

                for value in &*list.0 {
                    let value = value.as_concrete().unwrap_or_else(|| {
                        eprintln!("Can't display something unresolved, run `.resolve_set()` before display it");
                        std::process::exit(1)
                    });
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

    pub fn as_bool(&self) -> Option<bool> {
        if let NixValue::Bool(value) = self {
            Some(*value)
        } else {
            None
        }
    }

    pub fn as_lambda(&self) -> Option<&NixLambda> {
        if let NixValue::Lambda(lambda) = self {
            Some(lambda)
        } else {
            None
        }
    }

    pub fn as_list(&self) -> Option<NixList> {
        if let NixValue::List(list) = self {
            Some(list.clone())
        } else {
            None
        }
    }

    pub fn as_path(&self) -> Option<PathBuf> {
        match self {
            NixValue::Path(path) => Some(path.to_path_buf()),
            NixValue::String(string) => Some(PathBuf::from(string)),
            _ => None,
        }
    }

    pub fn is_attr_set(&self) -> bool {
        matches!(self, NixValue::AttrSet(_))
    }

    pub fn is_function(&self) -> bool {
        matches!(self, NixValue::Lambda(_))
    }

    pub fn is_int(&self) -> bool {
        matches!(self, NixValue::Int(_))
    }

    pub fn is_null(&self) -> bool {
        matches!(self, NixValue::Null)
    }

    pub fn is_path(&self) -> bool {
        matches!(self, NixValue::Path(_))
    }

    pub fn is_string(&self) -> bool {
        matches!(self, NixValue::String(_))
    }
}

impl NixLambda {
    pub fn call(&self, backtrace: Rc<NixBacktrace>, value: NixVar) -> NixResult {
        let NixLambda(scope, param, expr) = self;
        let span = Rc::new(NixSpan::from_ast_node(&scope.file, expr));

        match param {
            crate::NixLambdaParam::Ident(ident) => {
                scope.set_variable(ident.clone(), value);
            }
            crate::NixLambdaParam::Pattern(_) => {
                return Err(crate::NixError::todo(
                    span,
                    "Pattern lambda param",
                    Some(backtrace.clone()),
                ))
            }
        };

        scope.visit_expr(backtrace, expr.clone())
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
    // https://nix.dev/manual/nix/2.24/language/builtins.html?highlight=abort#builtins-toString
    fn as_string(&self) -> Option<String> {
        // TODO: AttrSet to String
        match self {
            NixValue::AttrSet(_) => None,
            NixValue::Bool(false) => Some(String::from("")),
            NixValue::Bool(true) => Some(String::from("1")),
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
