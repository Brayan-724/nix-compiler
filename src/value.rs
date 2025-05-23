pub mod attrset;
mod lazy;
mod pretty_print;
mod var;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use rnix::ast;

pub use attrset::{AttrsetBuilder, NixAttrSet, NixAttrSetDynamic};
pub use lazy::LazyNixValue;
pub use var::NixVar;

use crate::builtins::NixBuiltin;
use crate::scope::Scope;
use crate::{NixBacktrace, NixError, NixLabelKind, NixLabelMessage, NixResult};

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

impl NixValue {
    #[nix_macros::profile]
    pub fn try_eq(&self, other: &Self, backtrace: &NixBacktrace) -> NixResult<bool> {
        match (self, other) {
            (Self::AttrSet(NixAttrSet::Dynamic(v1)), Self::AttrSet(NixAttrSet::Dynamic(v2))) => {
                if v1.len() != v2.len() {
                    return Ok(false);
                }

                for (a, b) in v1.iter().zip(v2.iter()) {
                    if a.0 != b.0 || !a.1.try_eq(b.1, backtrace)? {
                        return Ok(false);
                    }
                }

                Ok(true)
            }
            // If both sets are derivation, then compare their outPaths.
            (
                Self::AttrSet(NixAttrSet::Derivation {
                    selected_output: v1_out,
                    derivation: v1_dev,
                }),
                Self::AttrSet(NixAttrSet::Derivation {
                    selected_output: v2_out,
                    derivation: v2_dev,
                }),
            ) => {
                if v1_out != v2_out {
                    return Ok(false);
                }

                Ok(v1_dev.path(&v1_out) == v2_dev.path(&v2_out))
            }
            (Self::AttrSet(_), Self::AttrSet(_)) => Ok(false),
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

    #[nix_macros::profile]
    pub fn wrap(self) -> NixValueWrapped {
        Rc::new(RefCell::new(self))
    }

    #[nix_macros::profile]
    pub fn wrap_var(self) -> NixVar {
        NixVar(Rc::new(RefCell::new(LazyNixValue::Concrete(self.wrap()))))
    }

    #[nix_macros::profile]
    pub fn get(&self, backtrace: &NixBacktrace, attr: &String) -> Result<Option<NixVar>, NixError> {
        let NixValue::AttrSet(set) = self else {
            return Err(backtrace.to_error(
                crate::NixLabelKind::Error,
                crate::NixLabelMessage::Empty,
                "Error handling: Should be AttrSet",
            ));
        };

        Ok(set.get(attr))
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

    pub fn as_string(&self) -> Option<&String> {
        match self {
            NixValue::String(string) => Some(string),
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
        self.cast_to_string().is_some()
    }

    // https://nix.dev/manual/nix/2.24/language/builtins.html?highlight=abort#builtins-toString
    #[nix_macros::profile]
    pub fn cast_to_string(&self) -> Option<String> {
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

    pub fn cast_lambda(&self, backtrace: &NixBacktrace) -> NixResult<NixLambda> {
        if let Some(lambda) = self.as_lambda().cloned() {
            Ok(lambda)
        } else if let Some(functor) = self.as_attr_set().and_then(|set| set.get("__functor")) {
            functor
                .resolve(backtrace)?
                .borrow()
                .as_lambda()
                .cloned()
                .ok_or_else(|| todo!("Error handling: Lambda cast"))
        } else {
            Err(backtrace.to_error(
                NixLabelKind::Error,
                NixLabelMessage::Empty,
                "Cannot cast to lambda",
            ))
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
    pub fn call(&self, backtrace: &NixBacktrace, value: NixVar) -> NixResult<NixVar> {
        match self {
            NixLambda::Apply(scope, param, expr) => {
                let scope = scope.clone().new_child();

                match param {
                    crate::NixLambdaParam::Ident(ident) => {
                        scope
                            .variables
                            .borrow_mut()
                            .insert_var(ident.clone(), value);
                    }
                    crate::NixLambdaParam::Pattern(pattern) => {
                        let argument_var = value.resolve(backtrace)?;

                        nix_macros::profile_start!();

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

                            scope.variables.borrow_mut().insert_var(
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

                            let var = if let Some(var) = argument.get(varname) {
                                var
                            } else if let Some(expr) = entry.default() {
                                LazyNixValue::Pending(
                                    backtrace.clone(),
                                    scope.clone().new_child(),
                                    expr,
                                )
                                .wrap_var()
                            } else {
                                todo!("Error handling: Require {varname}");
                            };

                            scope
                                .variables
                                .borrow_mut()
                                .insert_var(varname.to_owned(), var.clone());
                        }

                        if let Some(unused) = unused {
                            if !unused.is_empty() {
                                todo!("Handle error: Unused keys: {unused:?}")
                            }
                        }

                        nix_macros::profile_end!("before_lambda_call");
                    }
                };

                scope.visit_expr(backtrace, expr.clone())
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
