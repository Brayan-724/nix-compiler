mod lazy;
mod var;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt::{self, Write};
use std::ops::Deref;
use std::path::PathBuf;
use std::rc::Rc;

pub use lazy::LazyNixValue;
pub use var::NixVar;

use rnix::ast;

use crate::builtins::NixBuiltin;
use crate::scope::Scope;
use crate::{NixBacktrace, NixError, NixResult};

#[derive(Clone, PartialEq, Eq)]
pub enum NixLambdaParam {
    Ident(String),
    Pattern(ast::Pattern),
}

#[derive(Clone)]
pub enum NixLambda {
    Apply(Rc<Scope>, NixLambdaParam, ast::Expr),
    /// https://nix.dev/manual/nix/2.24/language/builtins
    Builtin(Rc<Box<dyn NixBuiltin>>),
}

#[derive(Clone, PartialEq, Eq)]
pub struct NixList(pub Rc<Vec<NixVar>>);

pub type NixAttrSet = BTreeMap<String, NixVar>;

/// https://nix.dev/manual/nix/2.24/language/types
#[derive(Default)]
pub enum NixValue {
    AttrSet(NixAttrSet),
    Bool(bool),
    Float(f64),
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
            NixValue::Float(val) => f.write_str(&val.to_string()),
            NixValue::Int(val) => f.write_str(&val.to_string()),
            NixValue::Lambda(NixLambda::Apply(..)) => f.write_str("<lamda>"),
            NixValue::Lambda(NixLambda::Builtin(builtin)) => fmt::Debug::fmt(builtin, f),
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
            NixValue::Float(val) => f.write_str(&val.to_string()),
            NixValue::Int(val) => f.write_str(&val.to_string()),
            NixValue::Lambda(NixLambda::Apply(..)) => f.write_str("<lamda>"),
            NixValue::Lambda(NixLambda::Builtin(builtin)) => fmt::Display::fmt(builtin, f),
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
    pub fn try_eq(&self, other: &Self, backtrace: Rc<NixBacktrace>) -> NixResult<bool> {
        match (self, other) {
            (Self::AttrSet(v1), Self::AttrSet(v2)) => {
                // TODO: If both sets denote a derivation (type = "derivation"),
                // then compare their outPaths.
                // https://github.com/NixOS/nix/blob/da7e3be8fc4338e9cd7bb49eac3cbcf5f0560850/src/libexpr/eval.cc#L2758-L2765

                if v1.len() != v2.len() {
                    return Ok(false);
                }

                for (a, b) in v1.iter().zip(v2) {
                    if a.0 != b.0 || !a.1.try_eq(b.1, backtrace.clone())? {
                        return Ok(false);
                    }
                }

                Ok(true)
            }
            (Self::Bool(v1), Self::Bool(v2)) => Ok(v1 == v2),
            (Self::Float(v1), Self::Float(v2)) => Ok(v1 == v2),
            (Self::Float(v1), Self::Int(v2)) => Ok(*v1 == *v2 as f64),
            (Self::Int(v1), Self::Int(v2)) => Ok(v1 == v2),
            (Self::Int(v1), Self::Float(v2)) => Ok(*v1 as f64 == *v2),
            // Functions are incomparable.
            (Self::Lambda(..), Self::Lambda(..)) => Ok(false),
            (Self::List(v1), Self::List(v2)) => Ok(v1 == v2),
            (Self::Null, Self::Null) => Ok(true),
            (Self::Path(v1), Self::Path(v2)) => Ok(v1 == v2),
            (Self::String(v1), Self::String(v2)) => Ok(v1 == v2),

            // Value types are not comparable
            (_, _) => Ok(false),
        }
    }

    pub fn wrap(self) -> NixValueWrapped {
        Rc::new(RefCell::new(self))
    }

    pub fn wrap_var(self) -> NixVar {
        NixVar(Rc::new(RefCell::new(LazyNixValue::Concrete(self.wrap()))))
    }

    pub fn get(&self, backtrace: NixBacktrace, attr: &String) -> Result<Option<NixVar>, NixError> {
        let NixValue::AttrSet(set) = self else {
            return Err(NixError::from_backtrace(
                backtrace,
                crate::NixLabelKind::Error,
                crate::NixLabelMessage::Empty,
                "Error handling: Should be AttrSet",
            ));
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

    pub fn as_int(&self) -> Option<i64> {
        if let NixValue::Int(value) = self {
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

    pub fn as_type(&self) -> &'static str {
        match self {
            NixValue::AttrSet(_) => "set",
            NixValue::Bool(_) => "bool",
            NixValue::Float(_) => "float",
            NixValue::Int(_) => "int",
            NixValue::Lambda(_) => "lambda",
            NixValue::List(_) => "list",
            NixValue::Null => "null",
            NixValue::Path(_) => "path",
            NixValue::String(_) => "string",
        }
    }

    pub fn is_attr_set(&self) -> bool {
        matches!(self, NixValue::AttrSet(_))
    }

    pub fn is_function(&self) -> bool {
        matches!(self, NixValue::Lambda(_))
    }

    pub fn is_float(&self) -> bool {
        matches!(self, NixValue::Float(_))
    }

    pub fn is_int(&self) -> bool {
        matches!(self, NixValue::Int(_))
    }

    pub fn is_list(&self) -> bool {
        matches!(self, NixValue::List(_))
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

    pub fn can_cast_string(&self) -> bool {
        self.as_string().is_some()
    }

    // https://nix.dev/manual/nix/2.24/language/builtins.html?highlight=abort#builtins-toString
    pub fn as_string(&self) -> Option<String> {
        // TODO: AttrSet to String
        match self {
            NixValue::AttrSet(_) => todo!(),
            NixValue::Bool(false) => Some(String::from("")),
            NixValue::Bool(true) => Some(String::from("1")),
            NixValue::Float(n) => Some(n.to_string()),
            NixValue::Int(n) => Some(n.to_string()),
            NixValue::Null => Some(String::from("")),
            NixValue::Path(path) => Some(path.display().to_string()),
            NixValue::String(str) => Some(str.clone()),
            _ => None,
        }
    }

    pub fn as_attr_set(&self) -> Option<&NixAttrSet> {
        if let NixValue::AttrSet(set) = self {
            Some(set)
        } else {
            None
        }
    }

    pub fn as_attr_set_mut(&mut self) -> Option<&mut NixAttrSet> {
        if let NixValue::AttrSet(set) = self {
            Some(set)
        } else {
            None
        }
    }
}

impl PartialEq for NixLambda {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (NixLambda::Apply(_, _, v1), NixLambda::Apply(_, _, v2)) => v1 == v2,
            (NixLambda::Builtin(v1), NixLambda::Builtin(v2)) => v1 == v2,
            _ => false,
        }
    }
}

impl NixLambda {
    pub fn call(&self, backtrace: Rc<NixBacktrace>, value: NixVar) -> NixResult<NixVar> {
        match self {
            NixLambda::Apply(scope, param, expr) => {
                let scope = scope.clone().new_child();

                match param {
                    crate::NixLambdaParam::Ident(ident) => {
                        scope.set_variable(ident.clone(), value);
                    }
                    crate::NixLambdaParam::Pattern(pattern) => {
                        let argument_var = value.resolve(backtrace.clone())?;
                        let argument = argument_var.borrow();
                        let Some(argument) = argument.as_attr_set() else {
                            todo!("Error handling")
                        };

                        if let Some(pat_bind) = pattern.pat_bind() {
                            let varname = pat_bind
                                .ident()
                                .unwrap()
                                .ident_token()
                                .unwrap()
                                .text()
                                .to_owned();

                            scope.set_variable(
                                varname,
                                LazyNixValue::Concrete(argument_var.clone()).wrap_var(),
                            );
                        }

                        let has_ellipsis = pattern.ellipsis_token().is_some();

                        let mut unused =
                            (!has_ellipsis).then(|| argument.keys().collect::<Vec<_>>());

                        for entry in pattern.pat_entries() {
                            let varname = entry.ident().unwrap().ident_token().unwrap();
                            let varname = varname.text();

                            if let Some(unused) = unused.as_mut() {
                                if let Some(idx) = unused.iter().position(|&key| key == varname) {
                                    unused.swap_remove(idx);
                                }
                            }

                            let var = if let Some(var) = argument.get(varname).cloned() {
                                var
                            } else if let Some(expr) = entry.default() {
                                LazyNixValue::Pending(backtrace.clone(), scope.clone(), expr)
                                    .wrap_var()
                            } else {
                                todo!("Error handling: Require {varname}");
                            };

                            scope.set_variable(varname.to_owned(), var.clone());
                        }

                        if let Some(unused) = unused {
                            if !unused.is_empty() {
                                todo!("Handle error: Unused keys: {unused:?}")
                            }
                        }
                    }
                };

                scope.visit_expr(backtrace.clone(), expr.clone())
            }
            NixLambda::Builtin(builtin) => builtin
                .run(backtrace, value)
                .map(LazyNixValue::Concrete)
                .map(LazyNixValue::wrap_var),
        }
    }
}

impl From<NixValue> for NixVar {
    fn from(value: NixValue) -> Self {
        value.wrap_var()
    }
}

impl From<NixValueWrapped> for NixVar {
    fn from(value: NixValueWrapped) -> Self {
        Self(Rc::new(RefCell::new(LazyNixValue::Concrete(value))))
    }
}
